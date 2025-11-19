use std::sync::Arc;

use async_trait::async_trait;
use scrapely::{Crawler, Item, ItemTrait, Spider};
use thirtyfour::{DesiredCapabilities, WebDriver, prelude::WebDriverError};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

#[derive(thiserror::Error, Debug)]
enum AppError {
    #[error("WebDriver Error: {0}")]
    WebDriver(#[from] WebDriverError),
    #[error("Extraction Failed: {0}")]
    ExtractionFailed(String),
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),
}

struct ChromeManager {
    process: Child,
}

impl ChromeManager {
    async fn new(port: u16) -> Result<Self, AppError> {
        let process = Command::new("examples/books/chromedriver/chromedriver.exe")
            .arg(format!("--port={}", port))
            .spawn()?;

        // Give it a moment to start
        sleep(Duration::from_secs(2)).await;

        Ok(Self { process })
    }

    async fn stop(&mut self) -> Result<(), AppError> {
        self.process.kill().await?;
        Ok(())
    }
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

        let document = scraper::Html::parse_document(&html);
        let next_page_selector = scraper::Selector::parse("li.next a").unwrap();

        let mut next_pages_link = vec![];
        if let Some(element) = document.select(&next_page_selector).next()
            && let Some(href) = element.value().attr("href")
            && let Ok(base) = reqwest::Url::parse(&url)
            && let Ok(next_url) = base.join(href)
        {
            next_pages_link.push(next_url.to_string());
        }

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
    // Start chromedriver
    let mut chrome_manager = ChromeManager::new(9188).await?;

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

    // Kill chromedriver
    chrome_manager.stop().await?;

    Ok(())
}
