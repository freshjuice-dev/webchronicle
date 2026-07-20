use std::path::{Path, PathBuf};

use regex::Regex;

/// Rewrite HTML content at runtime: localize links, srcs, srcsets, meta tags.
/// This is the runtime equivalent of the old builder::rewrite_and_inject.
pub fn rewrite_html(
    html: &str,
    snapshot_base: &Path,
    domain: &str,
    keep_srcset: bool,
    rel_prefix: &str,
) -> String {
    let live_base = format!("https://{}", domain);

    let href_re =
        Regex::new(r#"href\s*=\s*(?P<q>["']?)(?P<val>/[^\s"'<>]*)["']?"#).unwrap();
    let full_href_re =
        Regex::new(r#"href\s*=\s*(?P<q>["'])(?P<val>https?://[^\s"'<>]+)["']"#).unwrap();
    let asset_re =
        Regex::new(r#"(?P<attr>src|data-src|poster)\s*=\s*(?P<q>["']?)(?P<val>/[^\s"'<>]+)["']?"#)
            .unwrap();
    let full_src_re = Regex::new(
        r#"(?P<attr>src|data-src|poster)\s*=\s*(?P<q>["'])(?P<val>https?://[^\s"'<>]+)["']"#,
    )
    .unwrap();
    let srcset_re = Regex::new(r#"(?:data-)?srcset\s*=\s*["']([^"']+)["']"#).unwrap();
    let content_re =
        Regex::new(r#"content\s*=\s*(?P<q>["']?)(?P<val>/[^\s"'<>]+)["']?"#).unwrap();
    let full_content_re =
        Regex::new(r#"content\s*=\s*(?P<q>["'])(?P<val>https?://[^\s"'<>]+)["']"#).unwrap();

    let mut modified = html.to_string();

    // 1. Rewrite href="/..." → relative local path
    modified = href_re
        .replace_all(&modified, |caps: &regex::Captures| {
            let val = caps.name("val").unwrap().as_str();
            if val.starts_with("//") {
                return caps.get(0).unwrap().as_str().to_string();
            }
            let path_trimmed = val.trim_end_matches('/');
            let ext = Path::new(path_trimmed).extension().and_then(|e| e.to_str());
            let is_asset = is_asset_ext(ext);
            if is_asset {
                let local_path = resolve_local_asset(snapshot_base, val);
                if local_path.exists() {
                    let local_rel = local_path.strip_prefix(snapshot_base).unwrap();
                    return format!("href=\"{}{}\"", rel_prefix, local_rel.to_string_lossy());
                }
                return format!("href=\"{}{}\"", live_base, val);
            }
            let local_path = resolve_local_path(snapshot_base, val);
            if local_path.exists() {
                let local_rel = local_path.strip_prefix(snapshot_base).unwrap();
                format!("href=\"{}{}\"", rel_prefix, local_rel.to_string_lossy())
            } else {
                format!("href=\"{}{}\"", live_base, val)
            }
        })
        .to_string();

    // 2. Rewrite src=/path → live domain
    modified = asset_re
        .replace_all(&modified, |caps: &regex::Captures| {
            let attr = caps.name("attr").unwrap().as_str();
            let val = caps.name("val").unwrap().as_str();
            if attr == "src" && val == "/js/overlay.js" {
                return caps.get(0).unwrap().as_str().to_string();
            }
            if val.starts_with("//") {
                return caps.get(0).unwrap().as_str().to_string();
            }
            format!("{}=\"{}{}\"", attr, live_base, val)
        })
        .to_string();

    // 3. Rewrite srcset
    if keep_srcset {
        modified = srcset_re
            .replace_all(&modified, |caps: &regex::Captures| {
                let val = caps.get(1).unwrap().as_str();
                let parsed = parse_srcset_entries(val);
                let new_entries: Vec<String> = parsed
                    .iter()
                    .map(|(url, desc)| {
                        let descriptor = if desc.is_empty() {
                            String::new()
                        } else {
                            format!(" {}", desc)
                        };
                        if url.starts_with('/') {
                            format!("{}{}{}", live_base, url, descriptor)
                        } else if url.starts_with("https://") || url.starts_with("http://") {
                            if url.starts_with(&live_base) {
                                let path_part = &url[live_base.len()..];
                                let local_path = resolve_local_asset(snapshot_base, path_part);
                                if local_path.exists() {
                                    let local_rel =
                                        local_path.strip_prefix(snapshot_base).unwrap();
                                    format!(
                                        "{}{}{}",
                                        rel_prefix,
                                        local_rel.to_string_lossy(),
                                        descriptor
                                    )
                                } else {
                                    format!("{}{}", url, descriptor)
                                }
                            } else if let Some(local_path) =
                                resolve_cdn_asset(snapshot_base, url, domain)
                            {
                                if local_path.exists() {
                                    let local_rel =
                                        local_path.strip_prefix(snapshot_base).unwrap();
                                    format!(
                                        "{}{}{}",
                                        rel_prefix,
                                        local_rel.to_string_lossy(),
                                        descriptor
                                    )
                                } else {
                                    format!("{}{}", url, descriptor)
                                }
                            } else {
                                format!("{}{}", url, descriptor)
                            }
                        } else {
                            format!("{}{}", url, descriptor)
                        }
                    })
                    .collect();
                let attr_name = if caps.get(0).unwrap().as_str().starts_with("data-srcset") {
                    "data-srcset"
                } else {
                    "srcset"
                };
                format!("{}=\"{}\"", attr_name, new_entries.join(", "))
            })
            .to_string();
    } else {
        modified = srcset_re.replace_all(&modified, "").to_string();
    }

    // 3b. Lazy-load placeholders
    let lazy_src_re =
        Regex::new(r#"src="data:image/svg\+xml[^"]*"[^>]*?data-src="([^"]+)""#).unwrap();
    modified = lazy_src_re
        .replace_all(&modified, |caps: &regex::Captures| {
            let data_src = caps.get(1).unwrap().as_str();
            let full_match = caps.get(0).unwrap().as_str();
            full_match.replacen(
                "src=\"data:image/svg+xml",
                &format!("src=\"{}", data_src),
                1,
            )
        })
        .to_string();

    // 4. Rewrite content=/path → live domain
    modified = content_re
        .replace_all(&modified, |caps: &regex::Captures| {
            let val = caps.name("val").unwrap().as_str();
            if val.starts_with("//") {
                return caps.get(0).unwrap().as_str().to_string();
            }
            format!("content=\"{}{}\"", live_base, val)
        })
        .to_string();

    // 5. Rewrite full-URL internal links
    modified = full_href_re
        .replace_all(&modified, |caps: &regex::Captures| {
            let val = caps.name("val").unwrap().as_str();
            if !val.starts_with(&live_base) {
                return caps.get(0).unwrap().as_str().to_string();
            }
            let path_part = &val[live_base.len()..];
            let path_trimmed = path_part.trim_end_matches('/');
            let ext = Path::new(path_trimmed).extension().and_then(|e| e.to_str());
            let is_asset = is_asset_ext(ext);
            if is_asset {
                let local_path = resolve_local_asset(snapshot_base, path_part);
                if local_path.exists() {
                    let local_rel = local_path.strip_prefix(snapshot_base).unwrap();
                    return format!("\"{}{}\"", rel_prefix, local_rel.to_string_lossy());
                }
                return caps.get(0).unwrap().as_str().to_string();
            }
            let local_path = resolve_local_path(snapshot_base, path_part);
            if local_path.exists() {
                let local_rel = local_path.strip_prefix(snapshot_base).unwrap();
                format!("href=\"{}{}\"", rel_prefix, local_rel.to_string_lossy())
            } else {
                caps.get(0).unwrap().as_str().to_string()
            }
        })
        .to_string();

    // 6. Rewrite full-URL assets
    modified = full_src_re
        .replace_all(&modified, |caps: &regex::Captures| {
            let attr = caps.name("attr").unwrap().as_str();
            let val = caps.name("val").unwrap().as_str();
            if attr == "src" && val == "/js/overlay.js" {
                return caps.get(0).unwrap().as_str().to_string();
            }
            if val.starts_with(&live_base) {
                let path_part = &val[live_base.len()..];
                let local_path = resolve_local_asset(snapshot_base, path_part);
                if local_path.exists() {
                    let local_rel = local_path.strip_prefix(snapshot_base).unwrap();
                    return format!("{}=\"{}{}\"", attr, rel_prefix, local_rel.to_string_lossy());
                }
                return caps.get(0).unwrap().as_str().to_string();
            }
            if let Some(local_path) = resolve_cdn_asset(snapshot_base, val, domain) {
                if local_path.exists() {
                    let local_rel = local_path.strip_prefix(snapshot_base).unwrap();
                    return format!("{}=\"{}{}\"", attr, rel_prefix, local_rel.to_string_lossy());
                }
            }
            caps.get(0).unwrap().as_str().to_string()
        })
        .to_string();

    // 7. Rewrite full-URL content meta tags
    modified = full_content_re
        .replace_all(&modified, |caps: &regex::Captures| {
            let val = caps.name("val").unwrap().as_str();
            if !val.starts_with(&live_base) {
                return caps.get(0).unwrap().as_str().to_string();
            }
            let path_part = &val[live_base.len()..];
            let local_path = resolve_local_path(snapshot_base, path_part);
            if local_path.exists() {
                let local_rel = local_path.strip_prefix(snapshot_base).unwrap();
                format!("content=\"{}{}\"", rel_prefix, local_rel.to_string_lossy())
            } else {
                caps.get(0).unwrap().as_str().to_string()
            }
        })
        .to_string();

    modified
}

/// Rewrite CSS url() references at runtime.
pub fn rewrite_css(css: &str, snapshot_base: &Path, domain: &str, css_rel_path: &str) -> String {
    let live_base = format!("https://{}", domain);
    let url_re = Regex::new(r#"(?i)url\(\s*["']?(/[^)"'\s]+)["']?\s*\)"#).unwrap();
    let full_url_re = Regex::new(r#"(?i)url\(\s*["']?(https?://[^)"'\s]+)["']?\s*\)"#).unwrap();

    let css_depth = css_rel_path.matches('/').count();
    let up = if css_depth == 0 {
        "./".to_string()
    } else {
        "../".repeat(css_depth)
    };

    let mut modified = css.to_string();

    modified = url_re
        .replace_all(&modified, |caps: &regex::Captures| {
            let val = caps.get(1).unwrap().as_str();
            let local_path = snapshot_base.join(val.trim_start_matches('/'));
            if local_path.exists() {
                format!("url({}{})", up, val.trim_start_matches('/'))
            } else {
                caps.get(0).unwrap().as_str().to_string()
            }
        })
        .to_string();

    modified = full_url_re
        .replace_all(&modified, |caps: &regex::Captures| {
            let val = caps.get(1).unwrap().as_str();
            if let Some(path_part) = val.strip_prefix(&live_base) {
                let local_path = snapshot_base.join(path_part.trim_start_matches('/'));
                if local_path.exists() {
                    return format!("url({}{})", up, path_part.trim_start_matches('/'));
                }
            }
            if let Some(local_path) = resolve_cdn_asset(snapshot_base, val, domain) {
                if local_path.exists() {
                    let local_rel = local_path.strip_prefix(snapshot_base).unwrap();
                    return format!("url({}{})", up, local_rel.to_string_lossy());
                }
            }
            caps.get(0).unwrap().as_str().to_string()
        })
        .to_string();

    modified
}

/// Compute the relative prefix for a file inside a snapshot domain dir.
/// e.g. file_rel = "about/index.html" → rel_prefix = "../"
pub fn get_relative_prefix(file_rel: &str) -> String {
    let depth = file_rel.matches('/').count();
    if depth == 0 {
        "./".to_string()
    } else {
        "../".repeat(depth)
    }
}

fn is_asset_ext(ext: Option<&str>) -> bool {
    matches!(
        ext,
        Some("css" | "js" | "png" | "jpg" | "jpeg" | "gif" | "svg" | "ico" | "woff" | "woff2" | "ttf" | "webp" | "avif" | "pdf")
    )
}

fn parse_srcset_entries(srcset: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let mut current = String::new();
    let tokens: Vec<&str> = srcset.split_whitespace().collect();

    for token in &tokens {
        if token.starts_with("http://") || token.starts_with("https://") || token.starts_with("/")
        {
            if !current.is_empty() {
                let (url, desc) = split_url_descriptor(&current);
                entries.push((url, desc));
                current.clear();
            }
            current = token.to_string();
        } else if token.starts_with("data:") {
            continue;
        } else if !current.is_empty() {
            if token.ends_with('w') || token.ends_with('x') || token.contains('x') {
                current.push(' ');
                current.push_str(token);
                let (url, desc) = split_url_descriptor(&current);
                entries.push((url, desc));
                current.clear();
            } else {
                current.push_str(token);
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
    if let Some(idx) = entry.rfind(' ') {
        let url = entry[..idx].to_string();
        let desc = entry[idx + 1..].to_string();
        if desc.ends_with('w') || desc.ends_with('x') || desc.contains('x') {
            return (url, desc);
        }
    }
    (entry.to_string(), String::new())
}

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

fn resolve_local_asset(snapshot_base: &Path, url_path: &str) -> PathBuf {
    let trimmed = url_path.trim_start_matches('/');
    let trimmed = trimmed.split('?').next().unwrap_or(trimmed);
    snapshot_base.join(trimmed)
}

fn resolve_cdn_asset(snapshot_base: &Path, url: &str, domain: &str) -> Option<PathBuf> {
    let domain_marker = format!("/{}/", domain);
    let idx = url.find(&domain_marker)?;
    let after_domain = &url[idx + 1..];
    let after_domain = after_domain.split('?').next().unwrap_or(after_domain);
    Some(snapshot_base.join(after_domain))
}
