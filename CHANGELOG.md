# Changelog

## 2.4.0 - 2026-10-21

### Fixed

- Stop treating canonical, alternate-language, and other page metadata links as downloadable assets. These links could mark pages as already downloaded before the scraper saved them, leaving same-site links pointing to the live website. The fix covers sitemap scraping and recursive crawling while preserving stylesheet, icon, and resource preload downloads.
- Fix a regression in 2.3.0: the server looked for `.app` snapshots in a directory without the `_tld` suffix, breaking local link and asset rewriting. Resolve files using the stored `.app_tld` directory name.

Existing snapshots need to be scraped again to capture the missing pages.

### Changed

- Update all direct dependencies to their latest stable versions, including reqwest 0.13.5, scraper 0.29.0, Tera 2.4.0, TOML 1.1.8, Tokio 1.54.0, clap 4.6.7, and serde_json 1.0.154.
- Migrate the date filter to Tera 2 and register it before compiling templates.
- Refresh transitive dependencies, including security fixes in rustls 0.23.46, rustls-webpki 0.103.16, and webpki-roots 1.0.10. Remove the unmaintained fxhash dependency through the scraper upgrade.

### Tests

- Add regression tests for metadata links in sitemap scraping and recursive crawling.
- Add a smoke test for config and ledger serialization, populated and empty archive pages, date formatting, HTML escaping, and 404 rendering.

Historical entries below are reconstructed from Git history. Dates refer to the tagged commits.

## 2.3.0 - 2026-09-07

- Block requests that resolve outside the archive directory.
- Fix relative image URLs in `srcset`, accept sitemap pages without a Content-Type header, and preserve quoted text in date formats.
- Store `.app` domains under `.app_tld` directories so macOS does not treat them as application bundles.
- Update dependencies.

## 2.2.0 - 2026-09-06

- Skip CMS API endpoints and non-HTML responses when scraping pages.
- Notify the desktop app when an archived page is clicked so it can close open popovers.
- Add crates.io installation instructions and Dependabot configuration.

## 2.1.0 - 2026-07-31

- Redesign the archive index with snapshot cards, domain filtering, favicons, and high-contrast styling.
- Serve snapshot files directly from `scraped-websites/` and rewrite links at request time instead of duplicating snapshots into `_site/`.
- Fix local CSS, JavaScript, and font links; cache static assets.
- Add a library API and headless server support for desktop integration, including navigation messages, external-link interception, and optional tracker-blocker injection.
- Update the logo and add contribution guidelines and a contributor license agreement.

## 2.0.0 - 2026-07-12

- Replace the Node.js and Eleventy implementation with a standalone Rust binary.
- Add sitemap discovery, direct sitemap URLs, and parallel downloads with up to 10 threads.
- Switch configuration and the snapshot ledger to TOML, rewrite CSS URLs to local paths, and inject the archive overlay only when serving pages.
- Add cross-platform release builds for Linux, macOS, and Windows.
- Change the license to AGPL-3.0-or-later and move the repository to FreshJuice.

## 1.0.0 - 2025-11-26

- Final Node.js and Eleventy version, developed from the first working archive in November 2024.
- Capture timestamped snapshots across multiple domains with recursive scraping and configurable URL filters.
- Browse saved snapshots through a generated archive index and floating navigation panel.
- Include site icons, web app manifests, setup documentation, and updated Node.js dependencies.
