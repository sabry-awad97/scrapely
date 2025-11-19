use std::sync::Arc;

use async_trait::async_trait;
use scrapely::{Crawler, Item, ItemTrait, Spider};
use thirtyfour::{DesiredCapabilities, WebDriver, prelude::WebDriverError};
use tokio::sync::Mutex;

#[derive(thiserror::Error, Debug)]
enum AppError {
    #[error("WebDriver Error: {0}")]
    WebDriver(#[from] WebDriverError),
    #[error("Extraction Failed: {0}")]
    ExtractionFailed(String),
}

#[derive(Debug, Item)]
#[item(selector = ".product_pod")]
struct BooksItem {
    #[field(selector = "h3 a")]
    title: Option<String>,
}

struct BooksSpider {
    driver: Mutex<WebDriver>,
    base_url: String,
}

impl BooksSpider {
    async fn new() -> Result<Self, AppError> {
        let caps = DesiredCapabilities::chrome();

        let driver = WebDriver::new("http://localhost:9188", caps).await?;
        Ok(Self {
            driver: Mutex::new(driver),
            base_url: "http://books.toscrape.com".to_string(),
        })
    }

    async fn close(&self) -> Result<(), AppError> {
        let driver = self.driver.lock().await;
        driver.clone().quit().await?;
        Ok(())
    }
}

#[async_trait]
impl Spider for BooksSpider {
    type Item = BooksItem;
    type Error = AppError;

    fn start_urls(&self) -> Vec<String> {
        vec![self.base_url.to_string()]
    }

    async fn scrape(&self, url: String) -> Result<(Vec<Self::Item>, Vec<String>), AppError> {
        let html = {
            let webdriver = self.driver.lock().await;
            webdriver.goto(&url).await?;
            webdriver.source().await?
        };

        let next_pages_link = vec![];

        Ok((
            Self::Item::from_html(&html).map_err(|e| AppError::ExtractionFailed(e.to_string()))?,
            next_pages_link,
        ))
    }

    async fn process(&self, item: Self::Item) -> Result<(), AppError> {
        if let Some(title) = item.title {
            println!("Book Title: {:?}", title);
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Build crawler with custom configuration and observer
    let crawler = Crawler::builder()
        .rate_limit(5.0) // 5 requests per second using token bucket
        .crawling_concurrency(2)
        .processing_concurrency(1)
        .build()
        .expect("Failed to build crawler");

    let spider = Arc::new(BooksSpider::new().await?);

    crawler.crawl(spider.clone()).await;

    spider.close().await?;

    Ok(())
}
