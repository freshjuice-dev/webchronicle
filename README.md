<h1 style="line-height: 64px; display: flex; align-items: center; justify-content: flex-start;">
  web
  <img src="src/assets/icon.png" width="32" height="32" alt="webChronicle logo">
      Chronicle
</h1>

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)
[![GitHub release](https://img.shields.io/github/v/release/freshjuice-dev/webchronicle)](https://github.com/freshjuice-dev/webchronicle/releases/latest)
[![crates.io](https://img.shields.io/crates/v/webchronicle)](https://crates.io/crates/webchronicle)
[![GitHub stars](https://img.shields.io/github/stars/freshjuice-dev/webchronicle)](https://github.com/freshjuice-dev/webchronicle/stargazers)

A web archiver that runs on your machine. Save snapshots of websites and browse them later, offline, the way they looked on the day you saved them.

## Features

- Archive any website with one command
- Browse previous versions of pages you've saved
- Nothing leaves your computer. No cloud, no uploads, no telemetry
- Multiple domains in one snapshot
- Recursive scraping (follows links to grab whole sites)
- Automatic favicon capture from page `<head>`
- High-contrast mode support (follows OS accessibility settings)
- Dark mode follows system preferences

## Install

**From crates.io:**

```bash
cargo install webchronicle
```

**From source:**

```bash
git clone https://github.com/freshjuice-dev/webchronicle.git
cd webchronicle
cargo build --release
```

## Quick start

```bash
git clone https://github.com/freshjuice-dev/webchronicle.git
cd webchronicle
cargo build --release

./target/release/webchronicle init
# edit webchronicle.toml, add your URLs
./target/release/webchronicle scrape
./target/release/webchronicle build
./target/release/webchronicle serve --port 3000
```

## Configuration

Edit `webchronicle.toml`:

**Option 1: let webChronicle find the sitemap**

```toml
[site]
title = "webChronicle"
description = "A web archiver that runs on your machine"
base_url = "https://webchronicle.app"

[scraper]
urls = ["https://example.com"]
recursive = true
max_depth = 3
```

webChronicle tries common sitemap locations (`/sitemap.xml`, `/sitemap_index.xml`, `/wp-sitemap.xml`, etc.). If found, it scrapes every URL listed. If not, it falls back to recursive link crawling.

**Option 2: point to a sitemap directly**

```toml
[site]
title = "webChronicle"
description = "A web archiver that runs on your machine"
base_url = "https://webchronicle.app"

[scraper]
urls = ["https://example.com/sitemap.xml"]
recursive = false
```

When the URL itself is a sitemap, webChronicle parses it and scrapes every page listed. No guessing, no crawling.

Snapshots land in `scraped-websites/` with a timestamp folder per domain, plus a `ledger.toml` index:

```
scraped-websites/
├── 2025-01-15T10-30-00/
│   ├── example.com/
│   │   ├── index.html
│   │   └── favicon.png
│   └── example.org/
│       └── index.html
└── ledger.toml
```

Snapshot files stay clean. The overlay (archive date, back link) is injected by the server when serving, not baked into the HTML. Copy a snapshot folder, get the original page.

## Tech

Rust binary. Tera templates for the index page. System fonts, no frameworks.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). External contributions require the [CLA](CLA.md).

## License

Copyright (C) 2024-2026 FreshJuice. Released under [AGPL v3](LICENSE) or later.

Need a commercial license? Contact contact@freshjuice.dev.
