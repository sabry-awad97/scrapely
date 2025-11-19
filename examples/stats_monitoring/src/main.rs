use anyhow::Result;
use scrapely::{Crawler, Item, ItemTrait, Spider};
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

    async fn process(&self, _item: Self::Item) -> Result<(), Self::Error> {
        // Simulate processing time
        tokio::time::sleep(Duration::from_millis(10)).await;
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

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Real-time Statistics Monitoring Example ===\n");
    println!("This example demonstrates real-time statistics monitoring\n");

    let spider = Arc::new(QuoteSpider::new(URL.to_string()));

    let crawler = Crawler::builder()
        .crawling_concurrency(3)
        .processing_concurrency(2)
        .rate_limit(10.0)
        .build()
        .expect("Failed to build crawler");

    // Subscribe to real-time statistics
    let stats_rx = crawler.subscribe_stats();

    // Spawn a task to monitor statistics in real-time
    let monitor_handle = tokio::spawn(async move {
        let mut rx = stats_rx;
        let mut last_urls = 0;
        let mut last_items = 0;

        println!("📊 Real-time Statistics Monitor\n");
        println!(
            "{:<10} {:<12} {:<12} {:<12} {:<10}",
            "Time", "URLs", "Items", "Errors", "Rate"
        );
        println!("{}", "=".repeat(60));

        loop {
            tokio::select! {
                _ = rx.changed() => {
                    let stats = rx.borrow().clone();

                    let urls_delta = stats.urls_visited.saturating_sub(last_urls);
                    let items_delta = stats.items_extracted.saturating_sub(last_items);

                    last_urls = stats.urls_visited;
                    last_items = stats.items_extracted;

                    println!(
                        "{:<10.1}s {:<12} {:<12} {:<12} {:<10.2}",
                        stats.elapsed().as_secs_f64(),
                        format!("{} (+{})", stats.urls_visited, urls_delta),
                        format!("{} (+{})", stats.items_extracted, items_delta),
                        stats.errors_encountered,
                        stats.urls_per_second()
                    );
                }
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    // Update every second even if no changes
                }
            }
        }
    });

    // Start the crawl
    println!("Starting crawl...\n");
    let final_stats = crawler.crawl(spider).await;

    // Stop monitoring
    monitor_handle.abort();

    // Display final statistics
    println!("\n{}", "=".repeat(60));
    println!("\n📈 Final Statistics:");
    println!("   URLs visited: {}", final_stats.urls_visited);
    println!("   Items extracted: {}", final_stats.items_extracted);
    println!("   Errors: {}", final_stats.errors_encountered);
    println!("   Duration: {:.2}s", final_stats.elapsed().as_secs_f64());
    println!(
        "   Average rate: {:.2} URLs/sec",
        final_stats.urls_per_second()
    );

    println!("\n✨ Monitoring complete!");

    Ok(())
}
