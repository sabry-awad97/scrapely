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

    // Example 2: Crawl with cancellation (commented out)
    // use tokio::time::timeout;
    // use tokio_util::sync::CancellationToken;
    //
    // let cancel_token = CancellationToken::new();
    // let token_clone = cancel_token.clone();
    //
    // // Cancel after 5 seconds
    // tokio::spawn(async move {
    //     tokio::time::sleep(Duration::from_secs(5)).await;
    //     token_clone.cancel();
    // });
    //
    // let stats = crawler.crawl_with_cancellation(spider, cancel_token).await;
    // println!("Crawl cancelled - visited {} URLs", stats.urls_visited);

    println!("\nCrawler finished!");

    Ok(())
}
