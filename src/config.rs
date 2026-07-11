use std::fs;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub site: SiteConfig,
    pub scraper: ScraperConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub title: String,
    pub description: String,
    pub base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScraperConfig {
    pub urls: Vec<String>,
    pub recursive: bool,
    pub max_depth: u32,
    pub url_filter: Option<Vec<String>>,
}

impl Config {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)
            .map_err(|_| format!("Cannot read {}. Run `webchronicle init` first.", path))?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn default_toml() -> String {
        r#"# webChronicle configuration
# https://webchronicle.app/

[site]
title = "webChronicle"
description = "A web archiver that runs on your machine"
base_url = "https://webchronicle.app"

[scraper]
urls = ["https://example.com"]
recursive = true
max_depth = 3
# url_filter = ["example.com"]  # optional: only scrape these domains
"#.to_string()
    }

    /// Check if a URL matches the filter (or belongs to one of the seed domains)
    pub fn url_allowed(&self, url: &str) -> bool {
        if let Some(ref filter) = self.scraper.url_filter {
            return filter.iter().any(|d| url.contains(d));
        }
        self.scraper.urls.iter().any(|seed| {
            extract_domain(seed)
                .map(|d| url.contains(&d))
                .unwrap_or(false)
        })
    }
}

fn extract_domain(url: &str) -> Option<String> {
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let domain = stripped.split('/').next()?;
    Some(domain.to_string())
}