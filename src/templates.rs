use std::collections::HashMap;

use serde::Serialize;
use tera::{Context, Tera, Value};

use crate::ledger::{Ledger, SnapshotEntry};

const BASE_TPL: &str = include_str!("templates/base.html");
const INDEX_TPL: &str = include_str!("templates/index.html");
const NOT_FOUND_TPL: &str = include_str!("templates/404.html");

#[derive(Debug, Clone, Serialize)]
pub struct FilmStrip {
    pub domain: String,
    pub snapshots: Vec<SnapshotEntry>,
    pub first_date: String,
    pub last_date: String,
}

pub struct Templates {
    tera: Tera,
}

impl Templates {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut tera = Tera::default();
        tera.add_raw_template("base.html", BASE_TPL)?;
        tera.add_raw_template("index.html", INDEX_TPL)?;
        tera.add_raw_template("404.html", NOT_FOUND_TPL)?;
        tera.register_filter("format_date", format_date_filter);
        Ok(Templates { tera })
    }

    pub fn render_index(
        &self,
        ledger: &Ledger,
        title: &str,
        description: &str,
        base_url: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut ctx = Context::new();
        ctx.insert("title", title);
        ctx.insert("description", description);
        ctx.insert("base_url", base_url);

        // Group snapshots by domain
        let mut by_domain: HashMap<String, Vec<SnapshotEntry>> = HashMap::new();
        for snap in ledger.values() {
            for domain in &snap.urls {
                by_domain
                    .entry(domain.clone())
                    .or_default()
                    .push(snap.clone());
            }
        }

        // Build film strips — one per domain, sorted by date (oldest first for film roll)
        let mut strips: Vec<FilmStrip> = by_domain
            .into_iter()
            .map(|(domain, mut snaps)| {
                snaps.sort_by(|a, b| a.time.cmp(&b.time));
                let first = snaps.first().map(|s| s.time.clone()).unwrap_or_default();
                let last = snaps.last().map(|s| s.time.clone()).unwrap_or_default();
                FilmStrip {
                    domain,
                    snapshots: snaps,
                    first_date: first,
                    last_date: last,
                }
            })
            .collect();
        strips.sort_by(|a, b| a.domain.cmp(&b.domain));

        let total: usize = strips.iter().map(|s| s.snapshots.len()).sum();

        ctx.insert("strips", &strips);
        ctx.insert("has_snapshots", &!strips.is_empty());
        ctx.insert("total_snapshots", &total);
        ctx.insert("total_sites", &strips.len());

        let rendered = self.tera.render("index.html", &ctx)?;
        Ok(rendered)
    }

    pub fn render_404(
        &self,
        title: &str,
        description: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut ctx = Context::new();
        ctx.insert("title", title);
        ctx.insert("description", description);
        let rendered = self.tera.render("404.html", &ctx)?;
        Ok(rendered)
    }
}

fn format_date_filter(value: &Value, args: &HashMap<String, Value>) -> Result<Value, tera::Error> {
    let timestamp = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("format_date expects a string"))?;

    let format = args
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("MMM d, yyyy 'at' h:mma");

    let parsed = parse_timestamp(timestamp)
        .ok_or_else(|| tera::Error::msg(format!("Cannot parse timestamp: {}", timestamp)))?;

    let formatted = format_chrono(&parsed, format);
    Ok(Value::String(formatted))
}

fn parse_timestamp(ts: &str) -> Option<chrono::NaiveDateTime> {
    let parts: Vec<&str> = ts.splitn(2, 'T').collect();
    if parts.len() != 2 {
        return None;
    }
    let date = parts[0];
    let time_raw = parts[1];
    let time = time_raw.replacen('-', ":", 2);
    let combined = format!("{}T{}", date, time);
    chrono::NaiveDateTime::parse_from_str(&combined, "%Y-%m-%dT%H:%M:%S").ok()
}

fn format_chrono(dt: &chrono::NaiveDateTime, format: &str) -> String {
    let chrono_fmt = format
        .replace("MMMM", "%B")
        .replace("MMM", "%b")
        .replace("yyyy", "%Y")
        .replace("yy", "%y")
        .replace("LLLL", "%B")
        .replace("LLL", "%b")
        .replace("dd", "%d")
        .replace("d", "%-d")
        .replace("hh", "%I")
        .replace("h", "%-I")
        .replace("mm", "%M")
        .replace("ss", "%S")
        .replace("a", "%p");

    dt.format(&chrono_fmt).to_string()
}