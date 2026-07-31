use std::fs;
use std::path::{Path, PathBuf};

use crate::assets;
use crate::config::Config;
use crate::ledger;
use crate::templates::Templates;

pub fn run(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Updating ledger ...");
    ledger::update()?;

    let config_path = Path::new("webchronicle.toml");
    if !config_path.exists() {
        fs::write(config_path, Config::default_toml())?;
    }

    let site_dir = Path::new("_site");
    fs::create_dir_all(site_dir)?;
    assets::write_assets(site_dir)?;

    copy_favicons(site_dir)?;

    let templates = Templates::new()?;
    let ledger = ledger::read()?;
    let html = templates.render_index(&ledger, &config.site.title, &config.site.description, &config.site.base_url)?;
    fs::write(site_dir.join("index.html"), html)?;

    let not_found = templates.render_404(&config.site.title, &config.site.description)?;
    fs::write(site_dir.join("404.html"), not_found)?;

    eprintln!("Build complete — _site/ ready");
    Ok(())
}

fn copy_favicons(site_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let scraped = Path::new("scraped-websites");
    if !scraped.exists() { return Ok(()); }

    for entry in fs::read_dir(scraped)? {
        let entry = entry?;
        let ts_dir = entry.path();
        if !ts_dir.is_dir() { continue; }

        for domain_entry in fs::read_dir(&ts_dir)? {
            let domain_entry = domain_entry?;
            let domain_dir = domain_entry.path();
            if !domain_dir.is_dir() { continue; }

            let domain = domain_entry.file_name().to_string_lossy().to_string();
            if let Some(fav) = find_favicon(&domain_dir) {
                let dest = site_dir.join(entry.file_name()).join(&domain).join("favicon.png");
                if let Some(parent) = dest.parent() { fs::create_dir_all(parent)?; }
                fs::copy(&fav, &dest)?;
            }
        }
    }
    Ok(())
}

fn find_favicon(dir: &Path) -> Option<PathBuf> {
    let candidates = ["favicon-32x32.png", "favicon-96x96.png", "favicon.png", "favicon.ico", "favicon.svg"];
    find_favicon_recursive(dir, &candidates)
}

fn find_favicon_recursive(dir: &Path, candidates: &[&str]) -> Option<PathBuf> {
    for name in candidates {
        let path = dir.join(name);
        if path.is_file() { return Some(path); }
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let sub = entry.path();
            if sub.is_dir() {
                if let Some(found) = find_favicon_recursive(&sub, candidates) {
                    return Some(found);
                }
            }
        }
    }
    None
}