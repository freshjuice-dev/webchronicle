use std::collections::BTreeMap;
use std::fs;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotEntry {
    pub time: String,
    pub urls: Vec<String>,
}

pub type Ledger = BTreeMap<String, SnapshotEntry>;

pub fn read() -> Result<Ledger, Box<dyn std::error::Error>> {
    let content = fs::read_to_string("scraped-websites/ledger.toml")
        .unwrap_or_else(|_| "".to_string());
    let ledger: Ledger = toml::from_str(&content).unwrap_or_default();
    Ok(ledger)
}

pub fn write(ledger: &Ledger) -> Result<(), Box<dyn std::error::Error>> {
    let toml = toml::to_string(ledger)?;
    fs::write("scraped-websites/ledger.toml", toml)?;
    Ok(())
}

/// Scan scraped-websites/ directory, rebuild ledger.toml
pub fn update() -> Result<(), Box<dyn std::error::Error>> {
    let mut ledger = Ledger::new();

    let entries = fs::read_dir("scraped-websites")?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let timestamp = entry.file_name().to_string_lossy().to_string();
        if !timestamp.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }

        let mut domains: Vec<String> = Vec::new();
        let sub_entries = fs::read_dir(&path)?;
        for sub in sub_entries {
            let sub = sub?;
            let sub_path = sub.path();
            if !sub_path.is_dir() {
                continue;
            }
            let domain = sub.file_name().to_string_lossy().to_string();
            let index = sub_path.join("index.html");
            if index.exists() {
                domains.push(domain);
            }
        }

        if !domains.is_empty() {
            ledger.insert(
                timestamp.clone(),
                SnapshotEntry {
                    time: timestamp,
                    urls: domains,
                },
            );
        }
    }

    write(&ledger)?;
    Ok(())
}