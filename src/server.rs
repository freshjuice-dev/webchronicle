use std::path::{Path, PathBuf};

use tiny_http::{Header, Response, Server};

use crate::rewrite;

pub fn serve(site_dir: &Path, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    serve_with_flags(site_dir, port, ServerFlags::default())
}

#[derive(Default, Clone)]
pub struct ServerFlags {
    pub skip_overlay: bool,
    pub block_trackers: bool,
    pub tracker_domains: Vec<String>,
    pub keep_srcset: bool,
}

pub fn serve_with_flags(
    site_dir: &Path,
    port: u16,
    flags: ServerFlags,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("0.0.0.0:{}", port);
    eprintln!("Serving {} on http://localhost:{}", site_dir.display(), port);

    let server = Server::http(&addr).map_err(|e| format!("Failed to bind: {}", e))?;

    for request in server.incoming_requests() {
        let url = request.url().to_string();
        let path = url.split('?').next().unwrap_or(&url);

        // Strip /snapshots/ prefix — serve from site_dir/<timestamp>/<domain>/...
        let rel_path = path
            .trim_start_matches('/')
            .strip_prefix("snapshots/")
            .unwrap_or_else(|| path.trim_start_matches('/'));
        let mut file_path = site_dir.join(rel_path);

        if file_path.is_dir() {
            file_path = file_path.join("index.html");
        }

        if file_path.extension().is_none() && !path.ends_with('/') {
            let html_candidate = file_path.with_extension("html");
            if html_candidate.exists() {
                file_path = html_candidate;
            } else {
                let dir_candidate = file_path.join("index.html");
                if dir_candidate.exists() {
                    file_path = dir_candidate;
                }
            }
        }

        if file_path.exists() && file_path.is_file() {
            let content_type = guess_content_type(&file_path);
            let is_html = content_type.starts_with("text/html");
            let is_css = content_type.starts_with("text/css");

            let data = std::fs::read(&file_path).unwrap_or_default();

            // Extract timestamp + domain from path for rewriting + overlay injection
            let snapshot_meta = if is_html || is_css {
                extract_snapshot_meta(path, site_dir)
            } else {
                None
            };

            let response_data = if is_html {
                let html_str = String::from_utf8_lossy(&data).to_string();

                // Runtime link rewriting
                let rewritten = if let Some((ref snapshot_base, ref domain, ref file_rel)) = snapshot_meta {
                    let rel_prefix = rewrite::get_relative_prefix(&file_rel);
                    rewrite::rewrite_html(&html_str, snapshot_base, &domain, flags.keep_srcset, &rel_prefix)
                } else {
                    html_str
                };

                // JS injection
                let mut d = rewritten.into_bytes();
                if flags.skip_overlay {
                    d = inject_external_links_script(&d);
                    if flags.block_trackers {
                        d = inject_tracker_blocker(&d, &flags.tracker_domains);
                    }
                } else if let Some((_, domain, _)) = snapshot_meta.as_ref() {
                    // Legacy overlay mode (non-Tauri CLI use)
                    if let Some((timestamp, _)) = extract_overlay_meta(path) {
                        d = inject_overlay(&data, &timestamp, domain);
                    }
                }
                d
            } else if is_css {
                if let Some((ref snapshot_base, domain, file_rel)) = snapshot_meta {
                    let css_str = String::from_utf8_lossy(&data).to_string();
                    rewrite::rewrite_css(&css_str, snapshot_base, &domain, &file_rel).into_bytes()
                } else {
                    data
                }
            } else {
                data
            };

            let mut response = Response::from_data(response_data);
            if let Ok(ct) = Header::from_bytes("Content-Type", content_type.as_bytes()) {
                response = response.with_header(ct);
            }
            if let Ok(ac) = Header::from_bytes("Access-Control-Allow-Origin", "*".as_bytes()) {
                response = response.with_header(ac);
            }
            if !is_html {
                if let Ok(cc) =
                    Header::from_bytes("Cache-Control", "public, max-age=86400".as_bytes())
                {
                    response = response.with_header(cc);
                }
            }
            let _ = request.respond(response);
        } else {
            let body = b"<h1>404 Not Found</h1>".to_vec();
            let mut response = Response::from_data(body).with_status_code(404);
            if let Ok(ct) =
                Header::from_bytes("Content-Type", "text/html; charset=utf-8".as_bytes())
            {
                response = response.with_header(ct);
            }
            let _ = request.respond(response);
        }
    }

    Ok(())
}

/// Extract (snapshot_base_path, domain, file_rel_path) from a request path.
/// path = /snapshots/<timestamp>/<domain>/<file_rel>
/// Returns (site_dir/<timestamp>/<domain>, domain, file_rel)
fn extract_snapshot_meta(path: &str, site_dir: &Path) -> Option<(PathBuf, String, String)> {
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    if parts.len() < 3 || parts[0] != "snapshots" {
        return None;
    }
    let timestamp = parts[1];
    let domain = parts[2].to_string();
    let file_rel = if parts.len() > 3 {
        parts[3..].join("/")
    } else {
        String::new()
    };

    let snapshot_base = site_dir.join(timestamp).join(&domain);
    Some((snapshot_base, domain, file_rel))
}

/// Extract (timestamp, domain) from a snapshot path (legacy overlay mode)
fn extract_overlay_meta(path: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    if parts.len() < 3 || parts[0] != "snapshots" {
        return None;
    }
    Some((parts[1].to_string(), parts[2].to_string()))
}

/// Inject tracker blocker script inline before </head>, after external-links script
fn inject_tracker_blocker(html: &[u8], domains: &[String]) -> Vec<u8> {
    let js = crate::assets::tracker_blocker_js();
    let domains_json = serde_json::to_string(domains).unwrap_or_else(|_| "[]".into());
    let tag = format!(
        "\n<!-- webChronicle tracker blocker -->\n<script>var WC_TRACKER_DOMAINS = new Set({});</script>\n<script>{}</script>\n",
        domains_json, js
    );

    let content = String::from_utf8_lossy(html);

    if content.contains("wc-external-link") {
        if let Some(pos) = content.find("wc-external-link") {
            if let Some(end) = content[pos..].find("</script>") {
                let insert_at = pos + end + "</script>".len();
                let mut result = content.to_string();
                result.insert_str(insert_at, &tag);
                return result.into_bytes();
            }
        }
    }

    if let Some(pos) = content.find("</head>") {
        let mut result = content.to_string();
        result.insert_str(pos, &tag);
        result.into_bytes()
    } else if let Some(pos) = content.rfind("</body>") {
        let mut result = content.to_string();
        result.insert_str(pos, &tag);
        result.into_bytes()
    } else {
        let mut result = content.into_owned();
        result.push_str(&tag);
        result.into_bytes()
    }
}

/// Inject overlay inline before </body> (legacy CLI mode)
fn inject_overlay(html: &[u8], timestamp: &str, domain: &str) -> Vec<u8> {
    let js = crate::assets::overlay_js();
    let overlay_tag = format!(
        "\n<!-- webChronicle overlay -->\n<script id=\"webChronicle\" data-timestamp=\"{}\" data-domain=\"{}\">{}</script>\n",
        timestamp, domain, js
    );

    let content = String::from_utf8_lossy(html);

    if content.contains("id=\"webChronicle\"") {
        return html.to_vec();
    }

    if let Some(pos) = content.rfind("</body>") {
        let mut result = content.to_string();
        result.insert_str(pos, &overlay_tag);
        result.into_bytes()
    } else {
        let mut result = content.into_owned();
        result.push_str(&overlay_tag);
        result.into_bytes()
    }
}

/// Inject external link interceptor script inline before </head>
fn inject_external_links_script(html: &[u8]) -> Vec<u8> {
    let js = crate::assets::external_links_js();
    let tag = format!(
        "\n<!-- webChronicle external links -->\n<script>{}</script>\n",
        js
    );

    let content = String::from_utf8_lossy(html);

    if content.contains("wc-external-link") {
        return html.to_vec();
    }

    if let Some(pos) = content.find("</head>") {
        let mut result = content.to_string();
        result.insert_str(pos, &tag);
        result.into_bytes()
    } else if let Some(pos) = content.rfind("</body>") {
        let mut result = content.to_string();
        result.insert_str(pos, &tag);
        result.into_bytes()
    } else {
        let mut result = content.into_owned();
        result.push_str(&tag);
        result.into_bytes()
    }
}

fn guess_content_type(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("toml") => "application/toml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("txt") => "text/plain; charset=utf-8",
        Some("md") => "text/markdown; charset=utf-8",
        Some("xml") => "application/xml",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
    .to_string()
}
