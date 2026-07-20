use std::fs;

use crate::config::Config;
use crate::ledger;

/// Build the site index and update the ledger.
/// No longer creates _site — the server serves from scraped-websites/ directly
/// with runtime link rewriting.
pub fn run(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Updating ledger ...");
    ledger::update()?;

    // Write config if none exists
    let config_path = std::path::Path::new("webchronicle.toml");
    if !config_path.exists() {
        fs::write(config_path, Config::default_toml())?;
    }

    eprintln!("Build complete — server serves from scraped-websites/");
    Ok(())
}
