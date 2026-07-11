use std::path::Path;

use clap::{Parser, Subcommand};

use crate::builder;
use crate::config::Config;
use crate::scraper;
use crate::server;

#[derive(Parser)]
#[command(name = "webchronicle")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "A web archiving tool — capture and explore snapshots of webpages over time")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create webchronicle.toml and directory structure in current folder
    Init,
    /// Scrape websites from config, store snapshots + update ledger
    Scrape,
    /// Build static site from snapshots (render + inject overlay)
    Build,
    /// Serve _site/ locally on given port
    Serve {
        /// Port to serve on (default: 3000)
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
    /// Remove _site/ build output
    Clean,
}

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    let toml = Config::default_toml();
    std::fs::write("webchronicle.toml", toml)?;
    std::fs::create_dir_all("scraped-websites")?;
    eprintln!("Created webchronicle.toml and scraped-websites/");
    eprintln!("Edit webchronicle.toml to add your URLs, then run: webchronicle scrape");
    Ok(())
}

pub fn run_scrape() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load("webchronicle.toml")?;
    scraper::run(&config)?;
    Ok(())
}

pub fn run_build() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load("webchronicle.toml")?;
    builder::run(&config)?;
    Ok(())
}

pub fn run_serve(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let site_dir = Path::new("_site");
    if !site_dir.exists() {
        eprintln!("_site/ not found. Run `webchronicle build` first.");
        return Ok(());
    }
    server::serve(site_dir, port)?;
    Ok(())
}

pub fn run_clean() -> Result<(), Box<dyn std::error::Error>> {
    let site = Path::new("_site");
    if site.exists() {
        std::fs::remove_dir_all(site)?;
        eprintln!("Removed _site/");
    } else {
        eprintln!("_site/ does not exist");
    }
    Ok(())
}