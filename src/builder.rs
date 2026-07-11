use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::assets;
use crate::config::Config;
use crate::ledger;
use crate::templates::Templates;

pub fn run(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let site_dir = PathBuf::from("_site");

    if site_dir.exists() {
        fs::remove_dir_all(&site_dir)?;
    }
    fs::create_dir_all(&site_dir)?;

    eprintln!("Copying snapshots ...");
    copy_snapshots(&site_dir)?;

    eprintln!("Rewriting links + injecting overlay ...");
    rewrite_and_inject(&site_dir)?;
    rewrite_css_urls(&site_dir)?;

    eprintln!("Rendering index page ...");
    let templates = Templates::new()?;
    let ledger = ledger::read()?;

    let index_html = templates
        .render_index(
            &ledger,
            &config.site.title,
            &config.site.description,
            &config.site.base_url,
        )
        .map_err(|e| {
            eprintln!("DEBUG render_index error: {:?}", e);
            format!("Failed to render index.html: {}", e)
        })?;
    fs::write(site_dir.join("index.html"), index_html)?;

    let not_found_html = templates
        .render_404(&config.site.title, &config.site.description)
        .map_err(|e| format!("Failed to render 404.html: {}", e))?;
    fs::write(site_dir.join("404.html"), not_found_html)?;

    eprintln!("Writing static assets ...");
    assets::write_assets(&site_dir)?;

    eprintln!("Build complete -> _site/");
    Ok(())
}

fn copy_snapshots(site_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let source = Path::new("scraped-websites");
    if !source.exists() {
        eprintln!("  (no scraped-websites/ directory, skipping snapshots)");
        return Ok(());
    }

    let dest = site_dir.join("snapshots");
    fs::create_dir_all(&dest)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }
        copy_dir_recursive(&path, &dest.join(&name))?;
    }

    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let meta = fs::symlink_metadata(&from)?;
        if meta.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else if meta.is_file() {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Get the relative prefix for a file inside snapshots.
/// e.g. _site/snapshots/2024-01-01/example.com/about/index.html
///   → snapshot_base = "/snapshots/2024-01-01/example.com"
///   → file_rel = "about/index.html"
///   → rel_prefix = "../../" (to get back to snapshot_base)
fn get_relative_prefix(file_rel: &str) -> String {
    let depth = file_rel.matches('/').count();
    if depth == 0 {
        "./".to_string()
    } else {
        "../".repeat(depth)
    }
}

fn rewrite_and_inject(site_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let snapshots_dir = site_dir.join("snapshots");
    if !snapshots_dir.exists() {
        return Ok(());
    }

    let mut html_files = Vec::new();
    find_html_files(&snapshots_dir, &mut html_files)?;

    // Match href=/path, href="/path", href='/path'
    let href_re = Regex::new(
        r#"href\s*=\s*(?P<q>["']?)(?P<val>/[^\s"'<>]+)["']?"#
    ).unwrap();

    // Match href="https://domain/path" — full URL internal links
    let full_href_re = Regex::new(
        r#"href\s*=\s*(?P<q>["'])(?P<val>https?://[^\s"'<>]+)["']"#
    ).unwrap();

    // Match src=/path, src="/path", data-src=/path, poster=/path
    let asset_re =
        Regex::new(r#"(?P<attr>src|data-src|poster)\s*=\s*(?P<q>["']?)(?P<val>/[^\s"'<>]+)["']?"#)
            .unwrap();

    // Match src="https://domain/path" — full URL assets (same domain only)
    let full_asset_re = Regex::new(
        r#"(?P<attr>src|data-src|poster)\s*=\s*(?P<q>["'])(?P<val>https?://[^\s"'<>]+)["']"#,
    )
    .unwrap();

    // Match external CDN src="https://cdn.example.com/.../domain.com/path"
    // We rewrite to local if the file exists locally
    let full_src_re = Regex::new(
        r#"(?P<attr>src|data-src|poster)\s*=\s*(?P<q>["'])(?P<val>https?://[^\s"'<>]+)["']"#,
    )
    .unwrap();
    // Match srcset="..." or srcset='...' or data-srcset="..."
    let srcset_re = Regex::new(r#"(?:data-)?srcset\s*=\s*["']([^"']+)["']"#).unwrap();

    // Match content=/path (meta tags)
    let content_re = Regex::new(
        r#"content\s*=\s*(?P<q>["']?)(?P<val>/[^\s"'<>]+)["']?"#
    ).unwrap();

    // Match content="https://domain/path" (meta tags, og:url etc)
    let full_content_re = Regex::new(
        r#"content\s*=\s*(?P<q>["'])(?P<val>https?://[^\s"'<>]+)["']"#
    ).unwrap();

    let mut count = 0;

    for file in &html_files {
        let rel = file.strip_prefix(&snapshots_dir)?;
        let components: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        if components.len() < 2 {
            continue;
        }
        let timestamp = &components[0];
        let domain = &components[1];
        let live_base = format!("https://{}", domain);

        // File relative path within the snapshot domain dir
        let file_rel = components[2..].join("/");
        let rel_prefix = get_relative_prefix(&file_rel);

        let html = fs::read_to_string(file)?;
        let mut modified = html;

        let snapshot_base = snapshots_dir.join(timestamp).join(domain);

        // 1. Rewrite href="/..." → relative local path
        //    /about/ → ../../about/index.html (relative to current file depth)
        //    BUT: if the local file doesn't exist, fall back to live domain
        modified = href_re.replace_all(&modified, |caps: &regex::Captures| {
            let val = caps.name("val").unwrap().as_str();

            // Skip non-page links
            if val.starts_with("//") {
                return caps.get(0).unwrap().as_str().to_string();
            }

            // Check if this is an asset (has file extension that's not .html)
            let path_trimmed = val.trim_end_matches('/');
            let ext = Path::new(path_trimmed).extension().and_then(|e| e.to_str());
            let is_asset = match ext {
                Some("css") | Some("js") | Some("png") | Some("jpg") | Some("jpeg")
                | Some("gif") | Some("svg") | Some("ico") | Some("woff") | Some("woff2")
                | Some("ttf") | Some("webp") | Some("avif") | Some("pdf") => true,
                _ => false,
            };

            if is_asset {
                // Assets: point to live domain
                return format!("href=\"{}{}\"", live_base, val);
            }

            // Page link: try to find local file
            // /about/ → look for about/index.html
            // /about → look for about/index.html
            // /page.html → look for page.html
            let local_path = resolve_local_path(&snapshot_base, val);

            if local_path.exists() {
                // Local file exists — rewrite to relative path
                let local_rel = local_path.strip_prefix(&snapshot_base).unwrap();
                let local_str = local_rel.to_string_lossy().to_string();
                format!("href=\"{}{}\"", rel_prefix, local_str)
            } else {
                // No local copy — point to live domain
                format!("href=\"{}{}\"", live_base, val)
            }
        }).to_string();

        // 2. Rewrite src=/path, data-src=/path, poster=/path → live domain
        modified = asset_re.replace_all(&modified, |caps: &regex::Captures| {
            let attr = caps.name("attr").unwrap().as_str();
            let val = caps.name("val").unwrap().as_str();

            if attr == "src" && val == "/js/overlay.js" {
                return caps.get(0).unwrap().as_str().to_string();
            }
            if val.starts_with("//") {
                return caps.get(0).unwrap().as_str().to_string();
            }
            format!("{}=\"{}{}\"", attr, live_base, val)
        }).to_string();

        // 3. Rewrite srcset + data-srcset — handle /path, https://domain/path, CDN URLs with spaces
        modified = srcset_re.replace_all(&modified, |caps: &regex::Captures| {
            let val = caps.get(1).unwrap().as_str();
            // Use smart parser that handles spaces in CDN URLs
            let parsed = parse_srcset_entries(val);
            let new_entries: Vec<String> = parsed.iter().map(|(url, desc)| {
                let descriptor = if desc.is_empty() { String::new() } else { format!(" {}", desc) };
                
                // Absolute path /foo → live domain
                if url.starts_with('/') {
                    format!("{}{}{}", live_base, url, descriptor)
                } else if url.starts_with("https://") || url.starts_with("http://") {
                    // Same domain
                    if url.starts_with(&live_base) {
                        let path_part = &url[live_base.len()..];
                        let local_path = resolve_local_asset(&snapshot_base, path_part);
                        if local_path.exists() {
                            let local_rel = local_path.strip_prefix(&snapshot_base).unwrap();
                            format!("{}{}{}", rel_prefix, local_rel.to_string_lossy(), descriptor)
                        } else {
                            format!("{}{}", url, descriptor)
                        }
                    } else if let Some(local_path) = resolve_cdn_asset(&snapshot_base, url, domain) {
                        // External CDN
                        if local_path.exists() {
                            let local_rel = local_path.strip_prefix(&snapshot_base).unwrap();
                            format!("{}{}{}", rel_prefix, local_rel.to_string_lossy(), descriptor)
                        } else {
                            format!("{}{}", url, descriptor)
                        }
                    } else {
                        format!("{}{}", url, descriptor)
                    }
                } else {
                    format!("{}{}", url, descriptor)
                }
            }).collect();
            let attr_name = if caps.get(0).unwrap().as_str().starts_with("data-srcset") {
                "data-srcset"
            } else {
                "srcset"
            };
            format!("{}=\"{}\"", attr_name, new_entries.join(", "))
        }).to_string();

        // 3b. Replace lazy-load placeholder src="data:image/svg+xml..." with local path from data-src
        let lazy_src_re = Regex::new(
            r#"src="data:image/svg\+xml[^"]*"[^>]*?data-src="([^"]+)""#
        ).unwrap();
        modified = lazy_src_re.replace_all(&modified, |caps: &regex::Captures| {
            let data_src = caps.get(1).unwrap().as_str();
            // data-src is already rewritten to local path in step 2
            // Replace the whole match but change src to the local path
            let full_match = caps.get(0).unwrap().as_str();
            // Just replace src="data:..." with src="local-path"
            full_match.replacen(
                &format!("src=\"data:image/svg+xml"),
                &format!("src=\"{}", data_src),
                1,
            )
        }).to_string();

        // 4. Rewrite content=/path (meta tags) → live domain
        modified = content_re.replace_all(&modified, |caps: &regex::Captures| {
            let val = caps.name("val").unwrap().as_str();
            if val.starts_with("//") {
                return caps.get(0).unwrap().as_str().to_string();
            }
            format!("content=\"{}{}\"", live_base, val)
        }).to_string();

        // 5. Rewrite full-URL internal links: href="https://domain/path" → local or live
        modified = full_href_re.replace_all(&modified, |caps: &regex::Captures| {
            let val = caps.name("val").unwrap().as_str();

            // Only rewrite links to our own domain
            if !val.starts_with(&live_base) {
                return caps.get(0).unwrap().as_str().to_string();
            }

            // Extract path from full URL
            let path_part = &val[live_base.len()..];

            // Check if asset
            let path_trimmed = path_part.trim_end_matches('/');
            let ext = Path::new(path_trimmed).extension().and_then(|e| e.to_str());
            let is_asset = match ext {
                Some("css") | Some("js") | Some("png") | Some("jpg") | Some("jpeg")
                | Some("gif") | Some("svg") | Some("ico") | Some("woff") | Some("woff2")
                | Some("ttf") | Some("webp") | Some("avif") | Some("pdf") => true,
                _ => false,
            };

            if is_asset {
                return caps.get(0).unwrap().as_str().to_string();
            }

            // Page link: try local
            let local_path = resolve_local_path(&snapshot_base, path_part);
            if local_path.exists() {
                let local_rel = local_path.strip_prefix(&snapshot_base).unwrap();
                let local_str = local_rel.to_string_lossy().to_string();
                format!("href=\"{}{}\"", rel_prefix, local_str)
            } else {
                caps.get(0).unwrap().as_str().to_string()
            }
        }).to_string();

        // 6. Rewrite full-URL assets: src="https://domain/path" → local if exists, else keep
        modified = full_src_re.replace_all(&modified, |caps: &regex::Captures| {
            let attr = caps.name("attr").unwrap().as_str();
            let val = caps.name("val").unwrap().as_str();

            // Skip our overlay
            if attr == "src" && val == "/js/overlay.js" {
                return caps.get(0).unwrap().as_str().to_string();
            }

            // Same domain asset
            if val.starts_with(&live_base) {
                let path_part = &val[live_base.len()..];
                let local_path = resolve_local_asset(&snapshot_base, path_part);
                if local_path.exists() {
                    let local_rel = local_path.strip_prefix(&snapshot_base).unwrap();
                    let local_str = local_rel.to_string_lossy().to_string();
                    return format!("{}=\"{}{}\"", attr, rel_prefix, local_str);
                }
                return caps.get(0).unwrap().as_str().to_string();
            }

            // External CDN — try to extract original path
            if let Some(local_path) = resolve_cdn_asset(&snapshot_base, val, domain) {
                if local_path.exists() {
                    let local_rel = local_path.strip_prefix(&snapshot_base).unwrap();
                    let local_str = local_rel.to_string_lossy().to_string();
                    return format!("{}=\"{}{}\"", attr, rel_prefix, local_str);
                }
            }

            // Keep as-is (live URL)
            caps.get(0).unwrap().as_str().to_string()
        }).to_string();

        // 7. Rewrite full-URL content meta tags
        modified = full_content_re.replace_all(&modified, |caps: &regex::Captures| {
            let val = caps.name("val").unwrap().as_str();
            if !val.starts_with(&live_base) {
                return caps.get(0).unwrap().as_str().to_string();
            }
            let path_part = &val[live_base.len()..];
            let local_path = resolve_local_path(&snapshot_base, path_part);
            if local_path.exists() {
                let local_rel = local_path.strip_prefix(&snapshot_base).unwrap();
                let local_str = local_rel.to_string_lossy().to_string();
                format!("content=\"{}{}\"", rel_prefix, local_str)
            } else {
                caps.get(0).unwrap().as_str().to_string()
            }
        }).to_string();

        // Note: overlay is injected on-the-fly by the server, not baked into snapshots.
        // This keeps snapshot files clean — users can copy the folder without our code.

        fs::write(file, modified)?;
        count += 1;
    }

    eprintln!("  Rewrote {} HTML file(s)", count);
    Ok(())
}

/// Parse srcset into (url, descriptor) pairs.
/// Handles CDN URLs with commas/spaces like:
///   "https://spcdn.shortpixel.ai/spio/ret_img, q_cdnize, to_auto, s_webp:avif/covalent.com/...webp 310w"
/// Strategy: find URL start (http://, https://, /), descriptor is the token after the last space
/// that looks like a size descriptor (ends with w, x, or contains x).
fn parse_srcset_entries(srcset: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    
    // Split by comma-space patterns that separate entries
    // But CDN URLs contain commas — so we look for ", https://" or ", /" patterns
    // Actually: entries are separated by ", " followed by a URL start
    let mut current = String::new();
    let tokens: Vec<&str> = srcset.split_whitespace().collect();
    
    for token in &tokens {
        if token.starts_with("http://") || token.starts_with("https://") || token.starts_with("/") {
            // New URL starts
            if !current.is_empty() {
                // Push previous entry
                let (url, desc) = split_url_descriptor(&current);
                entries.push((url, desc));
                current.clear();
            }
            current = token.to_string();
        } else if token.starts_with("data:") {
            continue;
        } else {
            // Descriptor or part of CDN URL
            if !current.is_empty() {
                // Check if this looks like a descriptor (ends with 'w' or 'x' or contains 'x')
                if token.ends_with('w') || token.ends_with('x') || token.contains('x') {
                    // It's a descriptor — URL is complete
                    current.push(' ');
                    current.push_str(token);
                    let (url, desc) = split_url_descriptor(&current);
                    entries.push((url, desc));
                    current.clear();
                } else {
                    // Part of CDN URL (comma-separated params)
                    current.push_str(token);
                }
            }
        }
    }
    if !current.is_empty() {
        let (url, desc) = split_url_descriptor(&current);
        entries.push((url, desc));
    }
    
    entries
}

fn split_url_descriptor(entry: &str) -> (String, String) {
    // Find the last space — everything after is the descriptor
    if let Some(idx) = entry.rfind(' ') {
        let url = entry[..idx].to_string();
        let desc = entry[idx + 1..].to_string();
        // Check if desc looks like a descriptor
        if desc.ends_with('w') || desc.ends_with('x') || desc.contains('x') {
            return (url, desc);
        }
    }
    (entry.to_string(), String::new())
}

/// Resolve a URL path to a local file path.
/// /about/ → about/index.html
/// /about → about/index.html
/// /page.html → page.html
fn resolve_local_path(snapshot_base: &Path, url_path: &str) -> PathBuf {
    let trimmed = url_path.trim_start_matches('/').trim_end_matches('/');
    if trimmed.is_empty() {
        return snapshot_base.join("index.html");
    }
    if trimmed.ends_with(".html") {
        return snapshot_base.join(trimmed);
    }
    snapshot_base.join(trimmed).join("index.html")
}

/// Resolve an asset URL path to local file path (no index.html logic)
fn resolve_local_asset(snapshot_base: &Path, url_path: &str) -> PathBuf {
    let trimmed = url_path.trim_start_matches('/');
    // Strip query string
    let trimmed = trimmed.split('?').next().unwrap_or(trimmed);
    snapshot_base.join(trimmed)
}

/// Extract original path from a CDN URL.
/// CDN format: https://spcdn.shortpixel.ai/spio/ret_img,q_cdnize,to_auto,s_webp:avif/covalent.com/wp-content/uploads/...
/// We look for /domain.com/ in the path and extract everything after it.
fn resolve_cdn_asset(snapshot_base: &Path, url: &str, domain: &str) -> Option<PathBuf> {
    let domain_marker = format!("/{}/", domain);
    let idx = url.find(&domain_marker)?;
    let after_domain = &url[idx + 1..]; // skip leading /
    // Strip query string
    let after_domain = after_domain.split('?').next().unwrap_or(after_domain);
    Some(snapshot_base.join(after_domain))
}

fn find_html_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        if meta.is_dir() {
            find_html_files(&path, files)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("html") {
            files.push(path);
        }
    }
    Ok(())
}

/// Rewrite url(...) in CSS files to relative local paths.
/// CSS at _site/snapshots/<timestamp>/<domain>/path/to.css
/// url(/fonts/x.woff2) → url(../fonts/x.woff2) relative to CSS file location
fn rewrite_css_urls(site_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let snapshots_dir = site_dir.join("snapshots");
    if !snapshots_dir.exists() {
        return Ok(());
    }

    let mut css_files = Vec::new();
    find_css_files(&snapshots_dir, &mut css_files)?;

    // Match url(/path), url("/path"), url('/path')
    let url_re = Regex::new(r#"url\(\s*["']?(/[^)"'\s]+)["']?\s*\)"#).unwrap();

    let mut count = 0;
    for file in &css_files {
        let rel = file.strip_prefix(&snapshots_dir)?;
        let components: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        if components.len() < 2 {
            continue;
        }

        // snapshot_base is the domain directory: _site/snapshots/<timestamp>/<domain>/
        let snapshot_base: PathBuf = snapshots_dir.join(&components[0]).join(&components[1]);

        // CSS file depth relative to snapshot_base
        let css_rel = file.strip_prefix(&snapshot_base).unwrap_or(file);
        let css_depth = css_rel.components().count();
        let up = if css_depth <= 1 { "./".to_string() } else { "../".repeat(css_depth - 1) };

        let css_content = fs::read_to_string(file)?;
        let mut modified = css_content.clone();

        // Rewrite url(/path) → url(../path) relative to CSS file
        modified = url_re
            .replace_all(&modified, |caps: &regex::Captures| {
                let val = caps.get(1).unwrap().as_str();
                let local_path = snapshot_base.join(val.trim_start_matches('/'));
                if local_path.exists() {
                    format!("url({}{})", up, val.trim_start_matches('/'))
                } else {
                    // Keep original if file doesn't exist locally
                    caps.get(0).unwrap().as_str().to_string()
                }
            })
            .to_string();

        if modified != css_content {
            fs::write(file, modified)?;
            count += 1;
        }
    }

    eprintln!("  Rewrote {} CSS file(s)", count);
    Ok(())
}

fn find_css_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        if meta.is_dir() {
            find_css_files(&path, files)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("css") {
            files.push(path);
        }
    }
    Ok(())
}