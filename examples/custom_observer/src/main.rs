use anyhow::Result;
use scrapely::{CrawlObserver, CrawlStats, Crawler, Item, ItemTrait, Spider, VisitResult};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A spider for crawling quotes.toscrape.com
pub struct QuoteSpider {
    base_url: String,
}

impl QuoteSpider {
    pub fn new(base_url: String) -> Self {
        Self { base_url }
    }
}

#[derive(Debug)]
pub enum QuoteSpiderError {
    RequestFailed(String),
    ExtractionFailed(String),
}

impl std::fmt::Display for QuoteSpiderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RequestFailed(msg) => write!(f, "Request failed: {}", msg),
            Self::ExtractionFailed(msg) => write!(f, "Extraction failed: {}", msg),
        }
    }
}

#[async_trait::async_trait]
impl Spider for QuoteSpider {
    type Item = Quote;
    type Error = QuoteSpiderError;

    fn start_urls(&self) -> Vec<String> {
        vec![format!("{}/page/1/", self.base_url)]
    }

    async fn scrape(&self, url: String) -> Result<(Vec<Self::Item>, Vec<String>), Self::Error> {
        let response = reqwest::get(&url)
            .await
            .map_err(|e| QuoteSpiderError::RequestFailed(e.to_string()))?;

        let html_content = response
            .text()
            .await
            .map_err(|e| QuoteSpiderError::RequestFailed(e.to_string()))?;

        let quotes = Quote::from_html(&html_content)
            .map_err(|e| QuoteSpiderError::ExtractionFailed(e.to_string()))?;

        // Extract next page link
        let mut new_urls = Vec::new();
        let document = scraper::Html::parse_document(&html_content);
        if let Ok(selector) = scraper::Selector::parse("li.next a")
            && let Some(element) = document.select(&selector).next()
            && let Some(href) = element.value().attr("href")
        {
            let next_url = if href.starts_with("http") {
                href.to_string()
            } else {
                format!("{}{}", self.base_url, href)
            };
            new_urls.push(next_url);
        }

        Ok((quotes, new_urls))
    }

    async fn process(&self, item: Self::Item) -> Result<(), Self::Error> {
        println!("  Quote: {}", item.text);
        println!("  Author: {}", item.author);
        Ok(())
    }
}

const URL: &str = "https://quotes.toscrape.com";

#[derive(Debug, Item, Clone)]
#[item(selector = ".quote")]
pub struct Quote {
    #[field(selector = "span.text")]
    pub text: String,

    #[field(selector = "small.author")]
    pub author: String,

    #[field(selector = ".tags .tag")]
    pub tags: Vec<String>,
}

/// Custom observer that tracks detailed metrics
struct MetricsObserver {
    urls_queued: AtomicUsize,
    urls_visited: AtomicUsize,
    errors: AtomicUsize,
}

impl MetricsObserver {
    fn new() -> Self {
        Self {
            urls_queued: AtomicUsize::new(0),
            urls_visited: AtomicUsize::new(0),
            errors: AtomicUsize::new(0),
        }
    }

    fn print_summary(&self) {
        println!("\n=== Metrics Observer Summary ===");
        println!("URLs Queued: {}", self.urls_queued.load(Ordering::Relaxed));
        println!(
            "URLs Visited: {}",
            self.urls_visited.load(Ordering::Relaxed)
        );
        println!("Errors: {}", self.errors.load(Ordering::Relaxed));
    }
}

#[async_trait::async_trait]
impl CrawlObserver for MetricsObserver {
    async fn on_url_queued(&self, url: &str) {
        self.urls_queued.fetch_add(1, Ordering::Relaxed);
        println!("📥 Queued: {}", url);
    }

    async fn on_url_visited(&self, result: &VisitResult) {
        self.urls_visited.fetch_add(1, Ordering::Relaxed);
        println!(
            "✅ Visited: {} (discovered {} URLs)",
            result.visited_url,
            result.discovered_urls.len()
        );
    }

    async fn on_item_extracted(&self, _url: &str) {
        println!("📦 Item extracted");
    }

    async fn on_scrape_error(&self, url: &str, error: &str) {
        self.errors.fetch_add(1, Ordering::Relaxed);
        eprintln!("❌ Error scraping {}: {}", url, error);
    }

    async fn on_process_error(&self, error: &str) {
        self.errors.fetch_add(1, Ordering::Relaxed);
        eprintln!("❌ Processing error: {}", error);
    }

    async fn on_crawl_complete(&self, stats: &CrawlStats) {
        println!("\n🎉 Crawl Complete!");
        println!("Duration: {:.2}s", stats.elapsed().as_secs_f64());
        println!("Rate: {:.2} URLs/sec", stats.urls_per_second());
    }
}

/// Progress observer that shows a simple progress indicator
struct ProgressObserver;

#[async_trait::async_trait]
impl CrawlObserver for ProgressObserver {
    async fn on_url_visited(&self, _result: &VisitResult) {
        print!(".");
        use std::io::Write;
        std::io::stdout().flush().unwrap();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Custom Observer Example ===\n");
    println!("This example demonstrates using multiple custom observers\n");

    let spider = Arc::new(QuoteSpider::new(URL.to_string()));

    // Create custom observers
    let metrics = Arc::new(MetricsObserver::new());
    let progress = Arc::new(ProgressObserver);

    // Build crawler with multiple observers
    let crawler = Crawler::builder()
        .crawling_concurrency(2)
        .rate_limit(5.0)
        .observe_with(metrics.clone())
        .observe_with(progress)
        .build()
        .expect("Failed to build crawler");

    println!("Starting crawl with custom observers...\n");

    let stats = crawler.crawl(spider).await;

    println!("\n");
    metrics.print_summary();

    println!("\n=== Final Statistics ===");
    println!("URLs visited: {}", stats.urls_visited);
    println!("Items extracted: {}", stats.items_extracted);
    println!("Errors: {}", stats.errors_encountered);
    println!("Duration: {:.2}s", stats.elapsed().as_secs_f64());
    println!("Rate: {:.2} URLs/sec", stats.urls_per_second());

    println!("\nExample finished!");

    Ok(())
}
