use std::fs;
use std::path::Path;

const MAIN_CSS: &str = include_str!("assets/main.css");
const OVERLAY_JS: &str = include_str!("assets/overlay.js");
const EXTERNAL_LINKS_JS: &str = include_str!("assets/external-links.js");
const TRACKER_BLOCKER_JS: &str = include_str!("assets/tracker-blocker.js");
const ICON_PNG: &[u8] = include_bytes!("assets/icon.png");
const FAVICON_PNG: &[u8] = include_bytes!("assets/favicon.png");

pub fn write_assets(site_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let css_dir = site_dir.join("css");
    fs::create_dir_all(&css_dir)?;
    fs::write(css_dir.join("main.css"), MAIN_CSS)?;

    let js_dir = site_dir.join("js");
    fs::create_dir_all(&js_dir)?;
    fs::write(js_dir.join("overlay.js"), OVERLAY_JS)?;

    fs::write(site_dir.join("icon.png"), ICON_PNG)?;
    fs::write(site_dir.join("favicon.png"), FAVICON_PNG)?;

    Ok(())
}

pub fn overlay_js() -> &'static str {
    OVERLAY_JS
}

pub fn external_links_js() -> &'static str {
    EXTERNAL_LINKS_JS
}

pub fn tracker_blocker_js() -> &'static str {
    TRACKER_BLOCKER_JS
}