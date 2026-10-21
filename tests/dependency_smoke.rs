use webchronicle::config::Config;
use webchronicle::ledger::{Ledger, SnapshotEntry};
use webchronicle::templates::Templates;

#[test]
fn config_ledger_and_templates_round_trip() {
    let config: Config = toml::from_str(&Config::default_toml()).unwrap();
    let config: Config = toml::from_str(&toml::to_string(&config).unwrap()).unwrap();
    assert_eq!(config.scraper.urls, ["https://example.com"]);

    let timestamp = "2026-10-21T12-34-56";
    let mut ledger = Ledger::new();
    ledger.insert(timestamp.into(), SnapshotEntry {
        time: timestamp.into(),
        urls: vec!["example.com".into()],
    });
    let ledger: Ledger = toml::from_str(&toml::to_string(&ledger).unwrap()).unwrap();
    assert_eq!(ledger[timestamp].urls, ["example.com"]);

    let templates = Templates::new().unwrap();
    let html = templates.render_index(&ledger, "<script>alert(1)</script>", "Archive", "/").unwrap();
    assert!(html.contains("/snapshots/2026-10-21T12-34-56/example.com/"));
    assert!(html.contains("21 Oct 2026"));
    assert!(html.contains("12:34 PM"));
    assert!(!html.contains("<script>alert(1)</script>"));
    assert!(html.contains("&lt;script&gt;"));
    let empty = templates.render_index(&Ledger::new(), "Archive", "Archive", "/").unwrap();
    assert!(empty.contains("No snapshots yet"));
    assert!(templates.render_404("Archive", "Archive").unwrap().contains("This page does not exist."));
}
