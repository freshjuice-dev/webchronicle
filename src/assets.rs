use std::fs;
use std::path::Path;

const MAIN_CSS: &str = include_str!("assets/main.css");
const OVERLAY_JS: &str = include_str!("assets/overlay.js");

pub fn write_assets(site_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let css_dir = site_dir.join("css");
    fs::create_dir_all(&css_dir)?;
    fs::write(css_dir.join("main.css"), MAIN_CSS)?;

    let js_dir = site_dir.join("js");
    fs::create_dir_all(&js_dir)?;
    fs::write(js_dir.join("overlay.js"), OVERLAY_JS)?;

    Ok(())
}

pub fn overlay_js() -> &'static str {
    OVERLAY_JS
}