//! # Quotes Example
//!
//! A comprehensive example demonstrating the Scrapely web crawler with real-world usage.
//!
//! ## Features Demonstrated
//!
//! - **Custom Spider Implementation**: Shows how to implement the `Spider` trait for crawling quotes.toscrape.com
//! - **Rate Limiting**: Token bucket-based rate limiting to respect server limits
//! - **Observers**: Custom observer implementation for monitoring crawl progress
//! - **Error Handling**: Proper error handling and reporting
//! - **Statistics Tracking**: Real-time statistics collection and reporting
//! - **Async/Await**: Full async implementation using Tokio
//!
//! ## Running the Example
//!
//! ```bash
//! cargo run -p quotes
//! ```
//!
//! ## What This Example Does
//!
//! 1. Creates a custom spider that crawls quotes.toscrape.com
//! 2. Extracts quote data (text, author, tags) from each page
//! 3. Automatically discovers and follows pagination links
//! 4. Applies rate limiting (5 requests/second) to be respectful to the server
//! 5. Uses an observer to log progress and events
//! 6. Displays final statistics including URLs visited and extraction rate
//!
//! ## Key Components
//!
//! - **QuoteSpider**: Custom spider implementation that handles scraping logic
//! - **Quote**: Data structure representing a quote (using the `Item` derive macro)
//! - **LoggingObserver**: Observer that prints crawl events and statistics
//! - **Crawler**: The main crawler with rate limiting and concurrency control
//!
//! ## Expected Output
//!
//! The example will:
//! - Display each visited URL
//! - Show the number of new URLs discovered on each page
//! - Print extracted quotes with authors and tags
//! - Display final statistics including crawl duration and request rate

use anyhow::Result;
use scrapely::{Crawler, Item, ItemTrait, Spider};
use std::sync::Arc;

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
        // Fetch the page
        let response = reqwest::get(&url)
            .await
            .map_err(|e| QuoteSpiderError::RequestFailed(e.to_string()))?;

        let html_content = response
            .text()
            .await
            .map_err(|e| QuoteSpiderError::RequestFailed(e.to_string()))?;

        // Extract quotes
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
        println!("Quote: {}", item.text);
        println!("Author: {}", item.author);
        println!("Tags: {}", item.tags.join(", "));
        println!();
        Ok(())
    }
}

const URL: &str = "https://quotes.toscrape.com";

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

/// Simple logging observer that prints crawl events
struct LoggingObserver;

#[async_trait::async_trait]
impl scrapely::CrawlObserver for LoggingObserver {
    async fn on_url_visited(&self, result: &scrapely::VisitResult) {
        println!(
            "✓ Visited: {} (found {} new URLs)",
            result.visited_url,
            result.discovered_urls.len()
        );
    }

    async fn on_scrape_error(&self, url: &str, error: &str) {
        eprintln!("✗ Error scraping {}: {}", url, error);
    }

    async fn on_crawl_complete(&self, stats: &scrapely::CrawlStats) {
        println!(
            "\n🎉 Crawl completed in {:.2}s",
            stats.elapsed().as_secs_f64()
        );
        println!("   Rate: {:.2} URLs/sec", stats.urls_per_second());
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Scrapely Crawler Example ===\n");

    println!("\n=== Using the Enhanced Crawler ===\n");
    let spider = Arc::new(QuoteSpider::new(URL.to_string()));

    // Build crawler with custom configuration and observer
    let crawler = Crawler::builder()
        .rate_limit(5.0) // 5 requests per second using token bucket
        .crawling_concurrency(2)
        .processing_concurrency(1)
        .observe_with(Arc::new(LoggingObserver))
        .build()
        .expect("Failed to build crawler");

    println!("Starting crawler with rate limiting and observer...\n");

    // Example 1: Normal crawl
    let stats = crawler.crawl(spider.clone()).await;

    // Display statistics
    println!("\n=== Final Crawl Statistics ===");
    println!("URLs visited: {}", stats.urls_visited);
    println!("Items extracted: {}", stats.items_extracted);
    println!("Errors encountered: {}", stats.errors_encountered);
    println!("Duration: {:.2}s", stats.elapsed().as_secs_f64());
    println!("Rate: {:.2} URLs/sec", stats.urls_per_second());

    println!("\nCrawler finished!");

    Ok(())
}
