use anyhow::Result;
use scrapely::{CrawlObserver, CrawlStats, Crawler, Item, ItemTrait, Spider};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

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

/// Observer that tracks cancellation
struct CancellationObserver;

#[async_trait::async_trait]
impl CrawlObserver for CancellationObserver {
    async fn on_url_visited(&self, result: &scrapely::VisitResult) {
        println!("✓ Visited: {}", result.visited_url);
    }

    async fn on_crawl_complete(&self, stats: &CrawlStats) {
        println!("\n🏁 Crawl completed");
        println!("   URLs visited: {}", stats.urls_visited);
        println!("   Duration: {:.2}s", stats.elapsed().as_secs_f64());
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Cancellation Example ===\n");
    println!("This example demonstrates graceful cancellation\n");

    // Example 1: Cancel after timeout
    println!("Example 1: Cancel after 3 seconds\n");
    {
        let spider = Arc::new(QuoteSpider::new(URL.to_string()));
        let observer = Arc::new(CancellationObserver);

        let crawler = Crawler::builder()
            .crawling_concurrency(2)
            .observe_with(observer)
            .build()
            .expect("Failed to build crawler");

        let cancel_token = CancellationToken::new();
        let token_clone = cancel_token.clone();

        // Cancel after 3 seconds
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(3)).await;
            println!("\n⏰ Timeout reached - cancelling crawl...\n");
            token_clone.cancel();
        });

        let stats = crawler.crawl_with_cancellation(spider, cancel_token).await;

        println!("\nResult: Visited {} URLs", stats.urls_visited);
        println!("Items extracted: {}", stats.items_extracted);
    }

    println!("\n{}\n", "=".repeat(60));

    // Example 2: Cancel on condition
    println!("Example 2: Cancel after 5 URLs\n");
    {
        let spider = Arc::new(QuoteSpider::new(URL.to_string()));
        let observer = Arc::new(CancellationObserver);

        let crawler = Crawler::builder()
            .crawling_concurrency(1)
            .observe_with(observer)
            .build()
            .expect("Failed to build crawler");

        let cancel_token = CancellationToken::new();
        let token_clone = cancel_token.clone();

        // Subscribe to stats and cancel when we've visited 5 URLs
        let stats_rx = crawler.subscribe_stats();
        tokio::spawn(async move {
            let mut rx = stats_rx;
            loop {
                if rx.changed().await.is_ok() {
                    let stats = rx.borrow().clone();
                    if stats.urls_visited >= 5 {
                        println!("\n🎯 Reached 5 URLs - cancelling...\n");
                        token_clone.cancel();
                        break;
                    }
                }
            }
        });

        let stats = crawler.crawl_with_cancellation(spider, cancel_token).await;

        println!("\nResult: Visited {} URLs before cancellation", stats.urls_visited);
        println!("Items extracted: {}", stats.items_extracted);
    }

    println!("\n✨ All examples completed!");

    Ok(())
}
