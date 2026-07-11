use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::blocking::{Client, ClientBuilder};
use reqwest::header::CONTENT_TYPE;
use scraper::{Html, Selector};
use url::Url;

use crate::config::Config;
use crate::ledger;

pub fn run(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    if config.scraper.urls.is_empty() {
        return Err("No URLs configured. Edit webchronicle.toml.".into());
    }

    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    let base_dir = format!("scraped-websites/{}", timestamp);
    fs::create_dir_all(&base_dir)?;

    let client = Arc::new(ClientBuilder::new()
        .timeout(Duration::from_secs(30))
        .user_agent("webChronicle/2.0")
        .pool_max_idle_per_host(20)
        .build()?);

    let seen: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    for seed_url in &config.scraper.urls {
        let base_url = Url::parse(seed_url)?;
        let domain = base_url.host_str().unwrap_or("unknown").to_string();
        let dest = PathBuf::from(&base_dir).join(&domain);
        fs::create_dir_all(&dest)?;

        eprintln!("-> {} ", domain);

        // If the seed URL itself is a sitemap (.xml), parse it directly
        let sitemap_urls = if seed_url.ends_with(".xml") {
            eprintln!("  Sitemap URL provided directly");
            let mut urls = Vec::new();
            fetch_sitemap_recursive(&client, seed_url, &mut urls)?;
            urls
        } else {
            discover_sitemap_urls(&client, seed_url)?
        };

        if sitemap_urls.is_empty() {
            eprintln!("  No sitemap found, falling back to link crawling");
            scrape_page(&client, seed_url, &base_url, &dest, config, 0, &seen)?;
        } else {
            eprintln!("  Sitemap: {} URLs found", sitemap_urls.len());

            let dest = Arc::new(dest);
            let base_url = Arc::new(base_url);

            let mut handles = Vec::new();
            let urls: Vec<String> = sitemap_urls
                .into_iter()
                .filter(|u| config.url_allowed(u))
                .collect();
            let total = urls.len();

            // Process in batches of 10 concurrent threads
            for (i, url) in urls.into_iter().enumerate() {
                let client = Arc::clone(&client);
                let base_url = Arc::clone(&base_url);
                let dest = Arc::clone(&dest);
                let seen = Arc::clone(&seen);

                let handle = std::thread::spawn(move || {
                    if let Err(e) = scrape_page_sitemap(&client, &url, &base_url, &dest, &seen, i + 1, total) {
                        eprintln!("  [ERROR] {}: {}", url, e);
                    }
                });
                handles.push(handle);

                if handles.len() >= 10 {
                    for h in handles.drain(..) {
                        let _ = h.join();
                    }
                }
            }
            for h in handles {
                let _ = h.join();
            }
        }
    }

    eprintln!("Updating ledger ...");
    ledger::update()?;
    eprintln!("Done. Snapshot: {}/", timestamp);
    Ok(())
}

/// Discover all page URLs from sitemap(s).
/// Tries common sitemap locations, follows sitemapindex recursively.
fn discover_sitemap_urls(
    client: &Client,
    seed_url: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let base = Url::parse(seed_url)?;
    let host = format!("{}://{}", base.scheme(), base.host_str().unwrap_or(""));

    // Try common sitemap locations
    let candidates = vec![
        format!("{}/sitemap.xml", host),
        format!("{}/sitemap_index.xml", host),
        format!("{}/sitemaps.xml", host),
        format!("{}/wp-sitemap.xml", host),
        format!("{}/sitemap-index.xml", host),
        format!("{}/sitemap.php", host),
    ];

    let mut sitemap_url = None;
    for candidate in &candidates {
        let resp = client.get(candidate).send();
        if let Ok(resp) = resp {
            let ct = resp
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            let status = resp.status().as_u16();
            if (status == 200 || status == 301 || status == 302) && (ct.contains("xml") || ct.is_empty()) {
                sitemap_url = Some(resp.url().to_string());
                break;
            }
        }
    }

    let sitemap_url = match sitemap_url {
        Some(u) => u,
        None => return Ok(vec![]),
    };

    eprintln!("  Sitemap found: {}", sitemap_url);
    let mut all_urls = Vec::new();
    fetch_sitemap_recursive(client, &sitemap_url, &mut all_urls)?;
    Ok(all_urls)
}

/// Recursively fetch sitemap, handling sitemapindex (nested sitemaps) and urlset (actual URLs)
fn fetch_sitemap_recursive(
    client: &Client,
    url: &str,
    urls: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let resp = client.get(url).send()?;
    let body = resp.text()?;

    // Parse XML: check if it's sitemapindex (contains <sitemap>) or urlset (contains <url>)
    if body.contains("<sitemapindex") || body.contains("<sitemap>") {
        // It's a sitemap index — recurse into each sub-sitemap
        let doc = Html::parse_document(&body);
        let sel = Selector::parse("sitemap > loc").unwrap();
        for el in doc.select(&sel) {
            let sub_url = el.text().collect::<String>();
            let trimmed = sub_url.trim();
            if !trimmed.is_empty() {
                fetch_sitemap_recursive(client, trimmed, urls)?;
            }
        }
    } else {
        // It's a urlset — extract <loc> URLs
        let doc = Html::parse_document(&body);
        let sel = Selector::parse("url > loc").unwrap();
        for el in doc.select(&sel) {
            let loc = el.text().collect::<String>();
            let trimmed = loc.trim();
            if !trimmed.is_empty() && trimmed.starts_with("http") {
                urls.push(trimmed.to_string());
            }
        }
    }

    Ok(())
}

/// Scrape a single page from sitemap (no recursion — just download HTML + assets)
fn scrape_page_sitemap(
    client: &Client,
    url: &str,
    base_url: &Url,
    dest: &Path,
    seen: &Mutex<HashSet<String>>,
    idx: usize,
    total: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let normalized = normalize_url(url);
    {
        let mut seen_guard = seen.lock().unwrap();
        if seen_guard.contains(&normalized) {
            return Ok(());
        }
        seen_guard.insert(normalized.clone());
    }

    let resp = client.get(url).send()?;
    let content_type = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if !content_type.contains("text/html") {
        return Ok(());
    }

    let body = resp.bytes()?;
    let parsed = Url::parse(url)?;
    let path = parsed.path();

    let local_path = if path == "/" || path.is_empty() {
        dest.join("index.html")
    } else if path.ends_with('/') {
        // /about/ → about/index.html
        let trimmed = path.trim_start_matches('/').trim_end_matches('/');
        let dir = dest.join(trimmed);
        fs::create_dir_all(&dir)?;
        dir.join("index.html")
    } else if path.ends_with(".html") {
        let rel = path.trim_start_matches('/');
        let full = dest.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        full
    } else {
        // /about → about/index.html (no trailing slash)
        let trimmed = path.trim_start_matches('/').trim_end_matches('/');
        let dir = dest.join(trimmed);
        fs::create_dir_all(&dir)?;
        dir.join("index.html")
    };

    let html_str = String::from_utf8_lossy(&body);
    let document = Html::parse_document(&html_str);
    let asset_sel = Selector::parse("link[href], script[src], img[src], source[src], video[src]").unwrap();

    for element in document.select(&asset_sel) {
        if let Some(href) = element.value().attr("href") {
            if let Some(abs) = base_url.join(href).ok() {
                let _ = download_asset_any(client, abs.as_str(), &base_url, dest, seen);
            }
        }
        if let Some(src) = element.value().attr("src") {
            if let Some(abs) = base_url.join(src).ok() {
                let _ = download_asset_any(client, abs.as_str(), &base_url, dest, seen);
            }
        }
        // Lazy loading: data-src
        if let Some(data_src) = element.value().attr("data-src") {
            if !data_src.starts_with("data:") {
                if let Some(abs) = base_url.join(data_src).ok() {
                    let _ = download_asset_any(client, abs.as_str(), &base_url, dest, seen);
                }
            }
        }
    }

    // srcset + data-srcset (lazy loading)
    let img_sel = Selector::parse("img[srcset], source[srcset], img[data-srcset], source[data-srcset]").unwrap();
    for img in document.select(&img_sel) {
        // Regular srcset
        if let Some(srcset) = img.value().attr("srcset") {
            for url in parse_srcset_urls(srcset) {
                if let Some(abs) = base_url.join(&url).ok() {
                    let _ = download_asset_any(client, abs.as_str(), &base_url, dest, seen);
                }
            }
        }
        // data-srcset (lazy loading)
        if let Some(srcset) = img.value().attr("data-srcset") {
            for url in parse_srcset_urls(srcset) {
                if let Some(abs) = base_url.join(&url).ok() {
                    let _ = download_asset_any(client, abs.as_str(), &base_url, dest, seen);
                }
            }
        }
    }

    fs::write(&local_path, &body)?;
    eprintln!("  [{}/{}] {}", idx, total, url);
    Ok(())
}

/// Fallback: scrape page by following links (used when no sitemap)
fn scrape_page(
    client: &Client,
    url: &str,
    base_url: &Url,
    dest: &Path,
    config: &Config,
    depth: u32,
    seen: &Mutex<HashSet<String>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let normalized = normalize_url(url);
    {
        let mut seen_guard = seen.lock().unwrap();
        if seen_guard.contains(&normalized) {
            return Ok(());
        }
        seen_guard.insert(normalized.clone());
    }

    let resp = client.get(url).send()?;
    let content_type = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let body = resp.bytes()?;
    let is_html = content_type.contains("text/html");

    let parsed = Url::parse(url)?;
    let path = parsed.path();
    let local_path = if path == "/" || path.is_empty() {
        dest.join("index.html")
    } else if path.ends_with('/') {
        let trimmed = path.trim_start_matches('/').trim_end_matches('/');
        let dir = dest.join(trimmed);
        fs::create_dir_all(&dir)?;
        dir.join("index.html")
    } else if is_html && !path.ends_with(".html") {
        let trimmed = path.trim_start_matches('/').trim_end_matches('/');
        let dir = dest.join(trimmed);
        fs::create_dir_all(&dir)?;
        dir.join("index.html")
    } else {
        let rel = path.trim_start_matches('/');
        let full = dest.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        full
    };

    fs::write(&local_path, &body)?;

    if is_html && config.scraper.recursive && depth < config.scraper.max_depth {
        let html_str = String::from_utf8_lossy(&body);
        let document = Html::parse_document(&html_str);
        let link_sel = Selector::parse("a[href]").unwrap();
        let asset_sel = Selector::parse("link[href], script[src], img[src]").unwrap();

        for element in document.select(&asset_sel) {
            if let Some(href) = element.value().attr("href") {
                if let Some(abs) = base_url.join(href).ok() {
                    if abs.host_str() == base_url.host_str() {
                        let _ = download_asset(client, abs.as_str(), dest, seen);
                    }
                }
            }
            if let Some(src) = element.value().attr("src") {
                if let Some(abs) = base_url.join(src).ok() {
                    if abs.host_str() == base_url.host_str() {
                        let _ = download_asset(client, abs.as_str(), dest, seen);
                    }
                }
            }
        }

        for element in document.select(&link_sel) {
            if let Some(href) = element.value().attr("href") {
                if href.starts_with('#')
                    || href.starts_with("javascript:")
                    || href.starts_with("mailto:")
                    || href.starts_with("tel:")
                {
                    continue;
                }
                if let Some(abs) = base_url.join(href).ok() {
                    let abs_str = abs.as_str();
                    let already_seen = seen.lock().unwrap().contains(&normalize_url(abs_str));
                    if config.url_allowed(abs_str)
                        && abs.host_str() == base_url.host_str()
                        && !already_seen
                    {
                        let _ = scrape_page(client, abs_str, base_url, dest, config, depth + 1, seen);
                    }
                }
            }
        }
    }

    Ok(())
}

fn download_asset(
    client: &Client,
    url: &str,
    dest: &Path,
    seen: &Mutex<HashSet<String>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let normalized = normalize_url(url);
    {
        let mut seen_guard = seen.lock().unwrap();
        if seen_guard.contains(&normalized) {
            return Ok(());
        }
        seen_guard.insert(normalized);
    }

    let resp = client.get(url).send()?;
    let body = resp.bytes()?;

    let parsed = Url::parse(url)?;
    let path = parsed.path();
    let rel = path.trim_start_matches('/');
    let local_path = dest.join(rel);
    if let Some(parent) = local_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&local_path, &body)?;
    Ok(())
}

/// Download any asset (same domain or external CDN).
/// For CDN URLs like shortpixel.ai, extract the original path and save locally.
fn download_asset_any(
    client: &Client,
    url: &str,
    base_url: &Url,
    dest: &Path,
    seen: &Mutex<HashSet<String>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let normalized = normalize_url(url);
    {
        let mut seen_guard = seen.lock().unwrap();
        if seen_guard.contains(&normalized) {
            return Ok(());
        }
        seen_guard.insert(normalized);
    }

    // Skip non-asset URLs (data:, javascript:, mailto:, etc.)
    if url.starts_with("data:") || url.starts_with("javascript:") || url.starts_with("mailto:") || url.starts_with("tel:") {
        return Ok(());
    }

    let parsed = Url::parse(url)?;

    // Determine local path:
    // - Same domain: use path directly
    // - External CDN (e.g. spcdn.shortpixel.ai): extract original path from URL
    let local_rel = if parsed.host_str() == base_url.host_str() {
        // Same domain — use path as-is
        parsed.path().trim_start_matches('/').to_string()
    } else {
        // External CDN — try to extract original domain/path from URL
        // shortpixel.ai format: /spio/ret_img,q_cdnize,to_auto,s_webp:avif/covalent.com/wp-content/uploads/...
        // The original path is after the last comma-separated params + /
        let path = parsed.path();
        // Look for pattern: /something/domain.com/original-path
        // Find the domain reference after the CDN params
        if let Some(idx) = path.find(&format!("/{}/", base_url.host_str().unwrap_or(""))) {
            // Extract everything after the domain reference
            let after_domain = &path[idx + 1..]; // skip leading /
            after_domain.to_string()
        } else {
            // No domain reference — use full path from CDN
            path.trim_start_matches('/').to_string()
        }
    };

    if local_rel.is_empty() {
        return Ok(());
    }

    let local_path = dest.join(&local_rel);
    if let Some(parent) = local_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let resp = client.get(url).send()?;
    let body = resp.bytes()?;
    fs::write(&local_path, &body)?;
    Ok(())
}

/// Parse srcset attribute and extract URLs.
/// Handles spaces in URLs (e.g. "ret_img, q_cdnize, to_auto" from shortpixel CDN).
/// Format: "url descriptor, url descriptor" — but URLs themselves can contain commas+spaces.
/// Strategy: split by " <number>" pattern — descriptors always start with a number.
fn parse_srcset_urls(srcset: &str) -> Vec<String> {
    let mut urls = Vec::new();
    // Split on comma followed by space+URL pattern
    // But CDN URLs contain commas (ret_img,q_cdnize,to_auto)
    // Descriptor format: "310w" or "300x192" or "2x" — always ends with w/x
    // So we split on patterns like ", https://" or ", /" or ", data:"
    
    // Better approach: find all URLs by looking for http:// or https:// or / followed by non-space
    let parts: Vec<&str> = srcset.split_whitespace().collect();
    let mut current_url = String::new();
    
    for part in parts {
        if part.starts_with("http://") || part.starts_with("https://") || part.starts_with("/") {
            if !current_url.is_empty() {
                urls.push(current_url.trim_end_matches(',').to_string());
            }
            current_url = part.to_string();
        } else if part.starts_with("data:") {
            // Skip data: URLs
            continue;
        } else {
            // This is a descriptor (like "310w" or "2x") — URL is complete
            if !current_url.is_empty() {
                urls.push(current_url.clone());
                current_url.clear();
            }
        }
    }
    // Last URL without descriptor
    if !current_url.is_empty() {
        urls.push(current_url);
    }
    
    urls
}

fn normalize_url(url: &str) -> String {
    let without_frag = url.split('#').next().unwrap_or(url);
    if without_frag.ends_with('/') {
        without_frag.trim_end_matches('/').to_string()
    } else {
        without_frag.to_string()
    }
}