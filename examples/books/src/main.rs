use std::sync::Arc;

use async_trait::async_trait;
use scrapely::{Crawler, Item, ItemTrait, Spider};
use thirtyfour::prelude::*;
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, mpsc};
use tokio::time::{Duration, sleep};

#[derive(thiserror::Error, Debug)]
enum AppError {
    #[error("WebDriver Error: {0}")]
    WebDriver(#[from] WebDriverError),
    #[error("Extraction Failed: {0}")]
    ExtractionFailed(String),
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Json Error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Manages the ChromeDriver process lifecycle
struct ChromeManager {
    process: Child,
}

impl ChromeManager {
    /// Starts a new ChromeDriver instance on the specified port
    async fn new(port: u16) -> Result<Self, AppError> {
        let process = Command::new("examples/books/chromedriver/chromedriver.exe")
            .arg(format!("--port={}", port))
            .spawn()?;

        // Allow time for ChromeDriver to initialize
        sleep(Duration::from_secs(2)).await;

        Ok(Self { process })
    }

    /// Terminates the ChromeDriver process
    async fn stop(&mut self) -> Result<(), AppError> {
        self.process.kill().await?;
        Ok(())
    }
}

/// Represents a book item scraped from the website
#[derive(Debug, Item)]
#[item(selector = ".product_pod")]
struct BooksItem {
    #[field(selector = "h3 a")]
    title: Option<String>,
}

/// Spider implementation for scraping books.toscrape.com
/// Uses a pool of WebDriver instances for concurrent scraping
struct BooksSpider {
    tx: mpsc::Sender<WebDriver>,
    rx: Mutex<mpsc::Receiver<WebDriver>>,
    base_url: String,
    start_page: usize,
    max_page: usize,
}

impl BooksSpider {
    async fn new(concurrency: usize, start_page: usize, max_page: usize) -> Result<Self, AppError> {
        let mut caps = DesiredCapabilities::chrome();
        caps.add_arg("--disable-gpu")?;
        caps.add_arg("--no-sandbox")?;
        caps.add_arg("--disable-dev-shm-usage")?;
        caps.add_arg("--remote-debugging-port=9222")?;

        // Create the master driver
        let master_driver = WebDriver::new("http://localhost:9188", caps.clone()).await?;

        let (tx, rx) = mpsc::channel(concurrency);

        // Add master driver to pool
        tx.send(master_driver)
            .await
            .map_err(|_| AppError::ExtractionFailed("Failed to initialize driver pool".into()))?;

        // Create additional worker drivers
        for _ in 1..concurrency {
            let worker_caps = DesiredCapabilities::chrome();
            let driver = WebDriver::new("http://localhost:9188", worker_caps).await?;
            tx.send(driver).await.map_err(|_| {
                AppError::ExtractionFailed("Failed to initialize driver pool".into())
            })?;
        }

        Ok(Self {
            tx,
            rx: Mutex::new(rx),
            base_url: "http://books.toscrape.com".to_string(),
            start_page,
            max_page,
        })
    }

    /// Closes all WebDriver instances in the pool
    async fn close(&self) -> Result<(), AppError> {
        let mut rx = self.rx.lock().await;
        rx.close();

        while let Some(driver) = rx.recv().await {
            let _ = driver.quit().await;
        }

        Ok(())
    }
}

#[async_trait]
impl Spider for BooksSpider {
    type Item = BooksItem;
    type Error = AppError;

    fn start_urls(&self) -> Vec<String> {
        let mut urls = Vec::new();
        let mut page = self.start_page;

        while page <= self.max_page {
            if page == 1 {
                urls.push(self.base_url.to_string());
            } else {
                urls.push(format!("{}/catalogue/page-{}.html", self.base_url, page));
            }
            page += 5;
        }

        urls
    }

    async fn scrape(&self, url: String) -> Result<(Vec<Self::Item>, Vec<String>), AppError> {
        let driver = {
            let mut rx = self.rx.lock().await;
            rx.recv()
                .await
                .ok_or_else(|| AppError::ExtractionFailed("Pool closed".into()))?
        };

        let html = match driver.goto(&url).await {
            Ok(_) => driver.source().await.map_err(AppError::from),
            Err(e) => Err(AppError::from(e)),
        };

        // Return driver to pool
        let _ = self.tx.send(driver).await;

        let html = html?;

        let document = scraper::Html::parse_document(&html);
        let next_page_selector = scraper::Selector::parse("li.next a").unwrap();

        let mut next_pages_link = vec![];

        // Extract current page number from URL
        let current_page = if url.contains("/page-") {
            url.split("/page-")
                .nth(1)
                .and_then(|s| s.split('.').next())
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(1)
        } else {
            1
        };

        // Only enqueue next page if we haven't reached max_page
        if current_page < self.max_page
            && let Some(element) = document.select(&next_page_selector).next()
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
    // Initialize ChromeDriver
    let mut chrome_manager = ChromeManager::new(9188).await?;

    // Configure crawler with rate limiting and concurrency settings
    let crawler = Crawler::builder()
        .rate_limit(5.0)
        .crawling_concurrency(2)
        .processing_concurrency(1)
        .build()
        .expect("Failed to build crawler");

    // Initialize spider with page range (pages 1-10)
    let spider = Arc::new(BooksSpider::new(2, 1, 10).await?);

    // Execute crawl
    crawler.crawl(spider.clone()).await;

    // Cleanup resources
    spider.close().await?;
    chrome_manager.stop().await?;

    Ok(())
}
