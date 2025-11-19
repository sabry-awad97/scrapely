use anyhow::Result;
use scrapely::{CrawlObserver, Crawler, Item, ItemTrait, Spider, VisitResult};
use std::sync::Arc;
use std::time::Duration;

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

        // Only get first page for demo
        Ok((quotes, vec![]))
    }

    async fn process(&self, item: Self::Item) -> Result<(), Self::Error> {
        println!("  ✓ {}", item.text);
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

/// Logging observer
struct Logger;

#[async_trait::async_trait]
impl CrawlObserver for Logger {
    async fn on_url_visited(&self, result: &VisitResult) {
        println!("📄 Visited: {}", result.visited_url);
    }
}

/// Error tracking observer
struct ErrorTracker {
    errors: std::sync::Mutex<Vec<String>>,
}

impl ErrorTracker {
    fn new() -> Self {
        Self {
            errors: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn print_errors(&self) {
        let errors = self.errors.lock().unwrap();
        if !errors.is_empty() {
            println!("\n⚠️  Errors encountered:");
            for error in errors.iter() {
                println!("   - {}", error);
            }
        }
    }
}

#[async_trait::async_trait]
impl CrawlObserver for ErrorTracker {
    async fn on_scrape_error(&self, url: &str, error: &str) {
        let mut errors = self.errors.lock().unwrap();
        errors.push(format!("Scrape error at {}: {}", url, error));
    }

    async fn on_process_error(&self, error: &str) {
        let mut errors = self.errors.lock().unwrap();
        errors.push(format!("Process error: {}", error));
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Advanced Configuration Examples ===\n");
    println!("This example demonstrates various crawler configurations\n");

    let spider = Arc::new(QuoteSpider::new(URL.to_string()));

    // Example 1: Minimal configuration
    println!("1. Minimal Configuration (defaults)");
    {
        let crawler = Crawler::new();
        let stats = crawler.crawl(spider.clone()).await;
        println!("   ✓ Completed with {} URLs\n", stats.urls_visited);
    }

    // Example 2: High concurrency
    println!("2. High Concurrency Configuration");
    {
        let crawler = Crawler::builder()
            .crawling_concurrency(10)
            .processing_concurrency(5)
            .build()
            .expect("Failed to build crawler");
        let stats = crawler.crawl(spider.clone()).await;
        println!("   ✓ Completed with {} URLs\n", stats.urls_visited);
    }

    // Example 3: Conservative rate limiting
    println!("3. Conservative Rate Limiting");
    {
        let crawler = Crawler::builder()
            .rate_limit(2.0)
            .crawling_concurrency(1)
            .build()
            .expect("Failed to build crawler");
        let stats = crawler.crawl(spider.clone()).await;
        println!("   ✓ Completed with {} URLs\n", stats.urls_visited);
    }

    // Example 4: Multiple observers
    println!("4. Multiple Observers");
    {
        let logger = Arc::new(Logger);
        let error_tracker = Arc::new(ErrorTracker::new());

        let crawler = Crawler::builder()
            .observe_with(logger)
            .observe_with(error_tracker.clone())
            .build()
            .expect("Failed to build crawler");

        let stats = crawler.crawl(spider.clone()).await;
        println!("   ✓ Completed with {} URLs", stats.urls_visited);
        error_tracker.print_errors();
        println!();
    }

    // Example 5: Custom queue sizes
    println!("5. Custom Queue Sizes");
    {
        let crawler = Crawler::builder()
            .crawling_concurrency(5)
            .processing_concurrency(3)
            .crawling_queue_multiplier(100)
            .processing_queue_multiplier(20)
            .build()
            .expect("Failed to build crawler");
        let stats = crawler.crawl(spider.clone()).await;
        println!("   ✓ Completed with {} URLs\n", stats.urls_visited);
    }

    // Example 6: Delay-based rate limiting
    println!("6. Delay-based Rate Limiting");
    {
        let crawler = Crawler::builder()
            .delay(Duration::from_millis(200))
            .crawling_concurrency(2)
            .build()
            .expect("Failed to build crawler");
        let stats = crawler.crawl(spider.clone()).await;
        println!("   ✓ Completed with {} URLs\n", stats.urls_visited);
    }

    // Example 7: Configuration validation
    println!("7. Configuration Validation");
    {
        match Crawler::builder().crawling_concurrency(0).build() {
            Ok(_) => println!("   ✗ Should have failed!"),
            Err(e) => println!("   ✓ Correctly rejected invalid config: {}", e),
        }

        match Crawler::builder().processing_concurrency(0).build() {
            Ok(_) => println!("   ✗ Should have failed!"),
            Err(e) => println!("   ✓ Correctly rejected invalid config: {}", e),
        }
    }

    println!("\n{}", "=".repeat(60));
    println!("✨ All configuration examples completed!");

    Ok(())
}
