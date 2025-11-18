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

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Scrapely Crawler Example ===\n");

    println!("\n=== Using the Crawler ===\n");
    let spider = Arc::new(QuoteSpider::new(URL.to_string()));

    // Build crawler with custom configuration
    let crawler = Crawler::builder()
        .delay(Duration::from_millis(500))
        .crawling_concurrency(2)
        .processing_concurrency(1)
        .build();

    println!("Starting crawler...\n");
    crawler.crawl(spider).await;

    // Display statistics
    let stats = crawler.stats();
    println!("\n=== Crawl Statistics ===");
    println!("URLs visited: {}", stats.urls_visited());
    println!("Items extracted: {}", stats.items_extracted());
    println!("Errors encountered: {}", stats.errors_encountered());
    println!("\nCrawler finished!");

    Ok(())
}
