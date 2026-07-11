use std::path::Path;

use tiny_http::{Header, Response, Server};

pub fn serve(site_dir: &Path, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("0.0.0.0:{}", port);
    eprintln!("Serving {} on http://localhost:{}", site_dir.display(), port);

    let server = Server::http(&addr).map_err(|e| format!("Failed to bind: {}", e))?;

    for request in server.incoming_requests() {
        let url = request.url().to_string();
        let path = url.split('?').next().unwrap_or(&url);
        let mut file_path = site_dir.join(path.trim_start_matches('/'));

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

            // Extract timestamp + domain from path for overlay injection
            let overlay_meta = if is_html {
                extract_overlay_meta(path)
            } else {
                None
            };

            let data = std::fs::read(&file_path).unwrap_or_default();

            let response_data = if is_html {
                if let Some((timestamp, domain)) = overlay_meta {
                    inject_overlay(&data, &timestamp, &domain)
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
            let _ = request.respond(response);
        } else {
            let not_found_path = site_dir.join("404.html");
            let body = if not_found_path.exists() {
                std::fs::read(&not_found_path).unwrap_or_else(|_| b"<h1>404</h1>".to_vec())
            } else {
                b"<h1>404 Not Found</h1>".to_vec()
            };
            let mut response = Response::from_data(body).with_status_code(404);
            if let Ok(ct) = Header::from_bytes("Content-Type", "text/html; charset=utf-8".as_bytes()) {
                response = response.with_header(ct);
            }
            let _ = request.respond(response);
        }
    }

    Ok(())
}

/// Extract (timestamp, domain) from a snapshot path like /snapshots/2025-01-15T10-30-00/example.com/about/
fn extract_overlay_meta(path: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    // Expected: snapshots / timestamp / domain / ...
    if parts.len() < 3 || parts[0] != "snapshots" {
        return None;
    }
    let timestamp = parts[1].to_string();
    let domain = parts[2].to_string();
    Some((timestamp, domain))
}

/// Inject overlay inline before </body> without modifying the snapshot file
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