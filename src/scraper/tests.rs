use super::*;
use crate::config::{ScraperConfig, SiteConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use tiny_http::{Header, Response, Server};

struct Fixture {
    base: Url,
    dest: PathBuf,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Fixture {
    fn new() -> Self {
        let server = Server::http("127.0.0.1:0").unwrap();
        let address = server.server_addr().to_ip().unwrap();
        let base = Url::parse(&format!("http://{address}/")).unwrap();
        let dest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!(
                "scraper-hreflang-{}-{}",
                std::process::id(),
                address.port()
            ));
        fs::create_dir_all(&dest).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let request_log = Arc::clone(&requests);
        let stopping = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            while !stopping.load(Ordering::Relaxed) {
                let Some(request) = server.recv_timeout(Duration::from_millis(50)).unwrap() else {
                    continue;
                };
                let path = request.url().to_string();
                request_log.lock().unwrap().push(path.clone());
                let (content_type, body) = match path.as_str() {
                    "/fr/" => (
                        "text/html",
                        r#"<html><head>
                        <link rel="canonical" href="/canonical/">
                        <link rel="ALTERNATE" hreflang="en" href="/en/">
                        <link rel="next" href="/next/">
                        <link rel="preconnect" href="/preconnect/">
                        <link rel="dns-prefetch" href="/dns/">
                        <link rel="prefetch" as="document" href="/prefetched/">
                        <link rel="preload" as="document" href="/preloaded/">
                        <link rel="alternate STYLESHEET" href="/style.css">
                        <link rel="icon" href="/icon.png">
                        <link rel="preload" as="script" href="/app.js">
                        </head><body><a href="/en/">English</a></body></html>"#,
                    ),
                    "/en/" => ("text/html", "<html><body>English page</body></html>"),
                    "/style.css" => ("text/css", "body { color: black; }"),
                    "/icon.png" => ("image/png", "icon fixture"),
                    "/app.js" => ("text/javascript", "window.loaded = true;"),
                    _ => ("text/html", "<html>Metadata is not an asset</html>"),
                };
                request
                    .respond(
                        Response::from_string(body)
                            .with_header(Header::from_bytes("Content-Type", content_type).unwrap()),
                    )
                    .unwrap();
            }
        });
        Self {
            base,
            dest,
            requests,
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
        fs::remove_dir_all(&self.dest).unwrap();
    }
}

fn check_language_pages(use_sitemap: bool) {
    let fixture = Fixture::new();
    let client = ClientBuilder::new()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let seen = Mutex::new(HashSet::new());
    let french = fixture.base.join("fr/").unwrap();
    let english = fixture.base.join("en/").unwrap();
    if use_sitemap {
        scrape_page_sitemap(
            &client,
            french.as_str(),
            &fixture.base,
            &fixture.dest,
            &seen,
            1,
            2,
        )
        .unwrap();
        scrape_page_sitemap(
            &client,
            english.as_str(),
            &fixture.base,
            &fixture.dest,
            &seen,
            2,
            2,
        )
        .unwrap();
    } else {
        let config = Config {
            site: SiteConfig {
                title: String::new(),
                description: String::new(),
                base_url: String::new(),
            },
            scraper: ScraperConfig {
                urls: vec![french.to_string()],
                recursive: true,
                max_depth: 2,
                url_filter: None,
                use_sitemap: false,
                keep_srcset: false,
            },
        };
        scrape_page(
            &client,
            french.as_str(),
            &fixture.base,
            &fixture.dest,
            &config,
            0,
            &seen,
        )
        .unwrap();
    }
    assert!(fixture.dest.join("fr/index.html").is_file());
    let english_html = fs::read_to_string(fixture.dest.join("en/index.html"))
        .expect("alternate link must not prevent the English page from being saved");
    assert!(english_html.contains("English page"));
    for asset in ["style.css", "icon.png", "app.js"] {
        assert!(fixture.dest.join(asset).is_file(), "missing asset: {asset}");
    }
    let requests = fixture.requests.lock().unwrap();
    for path in [
        "/canonical/",
        "/next/",
        "/preconnect/",
        "/dns/",
        "/prefetched/",
        "/preloaded/",
    ] {
        assert!(
            !requests.iter().any(|request| request == path),
            "downloaded metadata: {path}"
        );
    }
    assert_eq!(requests.iter().filter(|path| *path == "/en/").count(), 1);
    let french_html = fs::read_to_string(fixture.dest.join("fr/index.html")).unwrap();
    let rewritten =
        crate::rewrite::rewrite_html(&french_html, &fixture.dest, "example.test", false, "../");
    assert!(rewritten.contains("<a href=\"../en/index.html\">English</a>"));
}

#[test]
fn sitemap_pages_are_not_claimed_by_metadata_links() {
    check_language_pages(true);
}

#[test]
fn crawled_pages_are_not_claimed_by_metadata_links() {
    check_language_pages(false);
}
