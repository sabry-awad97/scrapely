//! # Advanced Crawler Example
//!
//! A comprehensive example demonstrating advanced features of the Scrapely web crawler.
//!
//! ## Features Demonstrated
//!
//! - **Multiple Observers**: Attach multiple observers to monitor different aspects of the crawl
//! - **Real-time Statistics**: Subscribe to live statistics updates during crawling
//! - **Graceful Cancellation**: Stop crawls gracefully using cancellation tokens
//! - **Rate Limiting Strategies**: Compare token bucket vs. delay-based rate limiting
//! - **Configuration Validation**: Ensure crawler configuration is valid before running
//! - **Error Handling**: Comprehensive error tracking and reporting
//!
//! ## Running the Example
//!
//! ```bash
//! cargo run -p advanced
//! ```
//!
//! ## Examples Included
//!
//! ### 1. Multiple Observers
//! Demonstrates attaching multiple observers to a single crawler to track different metrics.
//! - **MetricsObserver**: Logs detailed events for each URL
//! - **CountingObserver**: Silently counts events and reports final statistics
//!
//! ### 2. Real-time Statistics Monitoring
//! Shows how to subscribe to live statistics updates during crawling.
//! - Subscribe to stats channel
//! - Monitor progress in a separate task
//! - React to changes in real-time
//!
//! ### 3. Graceful Cancellation
//! Demonstrates cancelling a crawl operation gracefully using a timeout.
//! - Create a cancellation token
//! - Spawn a task to cancel after a timeout
//! - Receive final statistics before cancellation
//!
//! ### 4. Rate Limiting Comparison
//! Compares two rate limiting strategies:
//! - **Token Bucket**: Allows bursts up to the configured rate (10 req/sec)
//! - **Delay-based**: Fixed delay between requests (200ms)
//!
//! ### 5. Configuration Validation
//! Shows how the crawler validates configuration and rejects invalid settings.
//! - Invalid concurrency values are rejected
//! - Valid configurations are accepted
//!
//! ## Key Concepts
//!
//! ### Observers
//!
//! Observers allow you to monitor crawl events without affecting performance:
//!
//! ```ignore
//! #[async_trait::async_trait]
//! impl CrawlObserver for MyObserver {
//!     async fn on_url_visited(&self, result: &VisitResult) {
//!         println!("Visited: {}", result.visited_url);
//!     }
//! }
//!
//! let crawler = Crawler::builder()
//!     .observe_with(Arc::new(MyObserver))
//!     .build()?;
//! ```
//!
//! ### Real-time Statistics
//!
//! Subscribe to live statistics updates:
//!
//! ```ignore
//! let mut stats_rx = crawler.subscribe_stats();
//!
//! tokio::spawn(async move {
//!     while stats_rx.changed().await.is_ok() {
//!         let stats = stats_rx.borrow();
//!         println!("URLs: {}, Items: {}", stats.urls_visited, stats.items_extracted);
//!     }
//! });
//! ```
//!
//! ### Graceful Cancellation
//!
//! Stop a crawl gracefully:
//!
//! ```ignore
//! use tokio_util::sync::CancellationToken;
//!
//! let cancel_token = CancellationToken::new();
//! let token_clone = cancel_token.clone();
//!
//! tokio::spawn(async move {
//!     tokio::time::sleep(Duration::from_secs(30)).await;
//!     token_clone.cancel();
//! });
//!
//! let stats = crawler.crawl_with_cancellation(spider, cancel_token).await;
//! ```
//!
//! ## Expected Output
//!
//! The example will demonstrate:
//! - Event logging from multiple observers
//! - Real-time progress updates
//! - Graceful cancellation with partial results
//! - Performance comparison between rate limiting strategies
//! - Configuration validation in action

use anyhow::Result;
use scrapely::{CrawlObserver, CrawlStats, Crawler, Item, ItemTrait, Spider, VisitResult};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub enum SpiderError {
    RequestFailed(String),
    ExtractionFailed(String),
}

impl std::fmt::Display for SpiderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RequestFailed(msg) => write!(f, "Request failed: {}", msg),
            Self::ExtractionFailed(msg) => write!(f, "Extraction failed: {}", msg),
        }
    }
}

#[derive(Debug, Item, Clone)]
#[item(selector = ".quote")]
pub struct Quote {
    #[field(selector = "span.text", regex = r#"[""](.+)[""]"#)]
    pub text: String,

    #[field(selector = "small.author")]
    pub author: String,

    #[field(selector = ".tags .tag")]
    pub tags: Vec<String>,
}

pub struct QuoteSpider {
    base_url: String,
}

impl QuoteSpider {
    pub fn new(base_url: String) -> Self {
        Self { base_url }
    }
}

#[async_trait::async_trait]
impl Spider for QuoteSpider {
    type Item = Quote;
    type Error = SpiderError;

    fn start_urls(&self) -> Vec<String> {
        vec![format!("{}/page/1/", self.base_url)]
    }

    async fn scrape(&self, url: String) -> Result<(Vec<Self::Item>, Vec<String>), Self::Error> {
        let response = reqwest::get(&url)
            .await
            .map_err(|e| SpiderError::RequestFailed(e.to_string()))?;

        let html_content = response
            .text()
            .await
            .map_err(|e| SpiderError::RequestFailed(e.to_string()))?;

        let quotes = Quote::from_html(&html_content)
            .map_err(|e| SpiderError::ExtractionFailed(e.to_string()))?;

        let mut new_urls = Vec::new();
        let document = scraper::Html::parse_document(&html_content);
        if let Ok(selector) = scraper::Selector::parse("li.next a") {
            if let Some(element) = document.select(&selector).next() {
                if let Some(href) = element.value().attr("href") {
                    let next_url = if href.starts_with("http") {
                        href.to_string()
                    } else {
                        format!("{}{}", self.base_url, href)
                    };
                    new_urls.push(next_url);
                }
            }
        }

        Ok((quotes, new_urls))
    }

    async fn process(&self, item: Self::Item) -> Result<(), Self::Error> {
        // Silent processing for this example
        let _ = item;
        Ok(())
    }
}

/// Observer that tracks detailed metrics
struct MetricsObserver {
    name: String,
}

impl MetricsObserver {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl CrawlObserver for MetricsObserver {
    async fn on_url_queued(&self, url: &str) {
        println!("[{}] 📋 Queued: {}", self.name, url);
    }

    async fn on_url_visited(&self, result: &VisitResult) {
        println!(
            "[{}] ✓ Visited: {} → {} new URLs",
            self.name,
            result.visited_url,
            result.discovered_urls.len()
        );
    }

    async fn on_scrape_error(&self, url: &str, error: &str) {
        eprintln!("[{}] ✗ Error: {} - {}", self.name, url, error);
    }

    async fn on_crawl_complete(&self, stats: &CrawlStats) {
        println!(
            "\n[{}] 🎉 Complete! {} URLs in {:.2}s ({:.2} URLs/sec)",
            self.name,
            stats.urls_visited,
            stats.elapsed().as_secs_f64(),
            stats.urls_per_second()
        );
    }
}

/// Observer that counts specific events
struct CountingObserver {
    name: String,
}

impl CountingObserver {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl CrawlObserver for CountingObserver {
    async fn on_url_visited(&self, _result: &VisitResult) {
        // Count silently
    }

    async fn on_crawl_complete(&self, stats: &CrawlStats) {
        println!(
            "[{}] Final count: {} URLs, {} items, {} errors",
            self.name, stats.urls_visited, stats.items_extracted, stats.errors_encountered
        );
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Advanced Crawler Examples ===\n");

    let base_url = "https://quotes.toscrape.com";

    // Example 1: Multiple Observers
    println!("--- Example 1: Multiple Observers ---\n");
    {
        let spider = Arc::new(QuoteSpider::new(base_url.to_string()));

        let crawler = Crawler::builder()
            .rate_limit(10.0)
            .crawling_concurrency(3)
            .observe_with(Arc::new(MetricsObserver::new("Metrics")))
            .observe_with(Arc::new(CountingObserver::new("Counter")))
            .build()?;

        let stats = crawler.crawl(spider).await;
        println!("\nResult: {} URLs visited\n", stats.urls_visited);
    }

    // Example 2: Real-time Statistics Monitoring
    println!("\n--- Example 2: Real-time Statistics Monitoring ---\n");
    {
        let spider = Arc::new(QuoteSpider::new(base_url.to_string()));

        let crawler = Crawler::builder()
            .rate_limit(15.0)
            .crawling_concurrency(4)
            .build()?;

        // Subscribe to real-time stats
        let mut stats_rx = crawler.subscribe_stats();

        // Spawn a task to monitor stats
        let monitor = tokio::spawn(async move {
            let mut last_count = 0;
            while stats_rx.changed().await.is_ok() {
                let stats = stats_rx.borrow().clone();
                if stats.urls_visited > last_count {
                    println!(
                        "📊 Progress: {} URLs, {} items ({:.2} URLs/sec)",
                        stats.urls_visited,
                        stats.items_extracted,
                        stats.urls_per_second()
                    );
                    last_count = stats.urls_visited;
                }
            }
        });

        let stats = crawler.crawl(spider).await;
        println!("\n✓ Crawl complete: {} URLs\n", stats.urls_visited);

        // Wait for monitor to finish
        let _ = monitor.await;
    }

    // Example 3: Cancellation with Timeout
    println!("\n--- Example 3: Cancellation with Timeout ---\n");
    {
        let spider = Arc::new(QuoteSpider::new(base_url.to_string()));

        let crawler = Crawler::builder()
            .rate_limit(5.0)
            .crawling_concurrency(2)
            .observe_with(Arc::new(MetricsObserver::new("Timed")))
            .build()?;

        let cancel_token = CancellationToken::new();
        let token_clone = cancel_token.clone();

        // Cancel after 3 seconds
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(3)).await;
            println!("\n⏰ Timeout reached - cancelling crawl...\n");
            token_clone.cancel();
        });

        let stats = crawler.crawl_with_cancellation(spider, cancel_token).await;
        println!(
            "\n✓ Gracefully stopped: {} URLs visited before cancellation\n",
            stats.urls_visited
        );
    }

    // Example 4: Different Rate Limiting Strategies
    println!("\n--- Example 4: Rate Limiting Comparison ---\n");
    {
        let spider = Arc::new(QuoteSpider::new(base_url.to_string()));

        // Token bucket (allows bursts)
        println!("Using token bucket rate limiter (10 req/sec):");
        let crawler = Crawler::builder()
            .rate_limit(10.0)
            .crawling_concurrency(3)
            .build()?;

        let start = std::time::Instant::now();
        let stats = crawler.crawl(spider.clone()).await;
        let duration = start.elapsed();

        println!(
            "  Completed {} URLs in {:.2}s ({:.2} URLs/sec)\n",
            stats.urls_visited,
            duration.as_secs_f64(),
            stats.urls_visited as f64 / duration.as_secs_f64()
        );

        // Delay-based (fixed delay)
        println!("Using delay-based rate limiter (200ms delay):");
        let crawler = Crawler::builder()
            .delay(Duration::from_millis(200))
            .crawling_concurrency(3)
            .build()?;

        let start = std::time::Instant::now();
        let stats = crawler.crawl(spider).await;
        let duration = start.elapsed();

        println!(
            "  Completed {} URLs in {:.2}s ({:.2} URLs/sec)\n",
            stats.urls_visited,
            duration.as_secs_f64(),
            stats.urls_visited as f64 / duration.as_secs_f64()
        );
    }

    // Example 5: Configuration Validation
    println!("\n--- Example 5: Configuration Validation ---\n");
    {
        // This will fail validation
        match Crawler::builder()
            .crawling_concurrency(0) // Invalid!
            .build()
        {
            Ok(_) => println!("  Unexpected success"),
            Err(e) => println!("  ✓ Validation caught error: {}", e),
        }

        // This will succeed
        match Crawler::builder()
            .crawling_concurrency(5)
            .processing_concurrency(3)
            .build()
        {
            Ok(_) => println!("  ✓ Valid configuration accepted"),
            Err(e) => println!("  Unexpected error: {}", e),
        }
    }

    println!("\n=== All Examples Complete ===");

    Ok(())
}
