use anyhow::Result;
use scrapely::{CrawlObserver, CrawlStats, Crawler, Item, ItemTrait, Spider};
use std::sync::Arc;
use std::time::{Duration, Instant};

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

        // Extract next page link (limit to 3 pages for demo)
        let mut new_urls = Vec::new();
        let document = scraper::Html::parse_document(&html_content);
        if let Ok(selector) = scraper::Selector::parse("li.next a")
            && let Some(element) = document.select(&selector).next()
            && let Some(href) = element.value().attr("href")
            && !url.contains("/page/3/") // Stop after page 3
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
        println!("  {}", item.text);
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

/// Observer that tracks timing
struct TimingObserver {
    start_time: Instant,
    last_visit: std::sync::Mutex<Option<Instant>>,
}

impl TimingObserver {
    fn new() -> Self {
        Self {
            start_time: Instant::now(),
            last_visit: std::sync::Mutex::new(None),
        }
    }
}

#[async_trait::async_trait]
impl CrawlObserver for TimingObserver {
    async fn on_url_visited(&self, result: &scrapely::VisitResult) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.start_time);

        let mut last = self.last_visit.lock().unwrap();
        let interval = if let Some(last_time) = *last {
            now.duration_since(last_time)
        } else {
            Duration::from_secs(0)
        };
        *last = Some(now);

        println!(
            "[{:>6.2}s] Visited {} (interval: {:>5}ms)",
            elapsed.as_secs_f64(),
            result.visited_url,
            interval.as_millis()
        );
    }

    async fn on_crawl_complete(&self, stats: &CrawlStats) {
        println!("\n📊 Statistics:");
        println!("   Total time: {:.2}s", stats.elapsed().as_secs_f64());
        println!("   URLs visited: {}", stats.urls_visited);
        println!("   Average rate: {:.2} URLs/sec", stats.urls_per_second());
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Rate Limiting Example ===\n");
    println!("This example demonstrates different rate limiting strategies\n");

    // Example 1: Token Bucket Rate Limiter (5 requests/second)
    println!("Example 1: Token Bucket (5 req/sec)\n");
    {
        let spider = Arc::new(QuoteSpider::new(URL.to_string()));
        let observer = Arc::new(TimingObserver::new());

        let crawler = Crawler::builder()
            .rate_limit(5.0)
            .crawling_concurrency(2)
            .observe_with(observer)
            .build()
            .expect("Failed to build crawler");

        println!("Starting crawl with token bucket rate limiter...\n");
        crawler.crawl(spider).await;
    }

    println!("\n{}\n", "=".repeat(60));

    // Example 2: Delay-based Rate Limiter (500ms delay)
    println!("Example 2: Delay-based (500ms between requests)\n");
    {
        let spider = Arc::new(QuoteSpider::new(URL.to_string()));
        let observer = Arc::new(TimingObserver::new());

        let crawler = Crawler::builder()
            .delay(Duration::from_millis(500))
            .crawling_concurrency(1)
            .observe_with(observer)
            .build()
            .expect("Failed to build crawler");

        println!("Starting crawl with delay-based rate limiter...\n");
        crawler.crawl(spider).await;
    }

    println!("\n✨ All examples completed!");

    Ok(())
}
