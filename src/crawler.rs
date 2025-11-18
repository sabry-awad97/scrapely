use std::{
    collections::HashSet,
    fmt::Display,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use futures_util::StreamExt;
use tokio::{
    sync::{Barrier, Notify, mpsc},
    time::sleep,
};
use tokio_stream::wrappers::ReceiverStream;

// Configuration constants
const DEFAULT_CRAWLING_QUEUE_MULTIPLIER: usize = 400;
const DEFAULT_PROCESSING_QUEUE_MULTIPLIER: usize = 10;
const DEFAULT_DELAY_MS: u64 = 200;
const DEFAULT_CRAWLING_CONCURRENCY: usize = 2;
const DEFAULT_PROCESSING_CONCURRENCY: usize = 2;

// Number of concurrent tasks (main coordinator + processors + scrapers)
const TASK_COUNT: usize = 3;

/// Trait for implementing web spiders
///
/// A spider defines how to crawl websites by specifying start URLs,
/// extracting data from pages, and discovering new URLs to visit.
///
/// # Example
///
/// ```ignore
/// struct MySpider;
///
/// impl Spider for MySpider {
///     type Item = String;
///     type Error = Box<dyn std::error::Error>;
///
///     fn start_urls(&self) -> Vec<String> {
///         vec!["https://example.com".to_string()]
///     }
///
///     fn scrape(&self, url: String) -> BoxFuture<'_, Result<(Vec<Self::Item>, Vec<String>), Self::Error>> {
///         Box::pin(async move {
///             // Fetch and parse the page
///             Ok((vec![], vec![]))
///         })
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait Spider: Send + Sync {
    /// The type of items extracted from pages
    type Item: Send + 'static;
    /// The error type for spider operations
    type Error: Display + Send + 'static;

    /// Return the list of URLs to start crawling from
    fn start_urls(&self) -> Vec<String>;

    /// Scrape a URL and extract items and new URLs to follow
    ///
    /// Returns a tuple of (items, new_urls) where:
    /// - items: extracted data from the page
    /// - new_urls: URLs discovered on the page to crawl next
    async fn scrape(&self, url: String) -> Result<(Vec<Self::Item>, Vec<String>), Self::Error>;

    /// Process an extracted item
    ///
    /// This is called for each item extracted during scraping.
    /// Default implementation does nothing.
    async fn process(&self, _item: Self::Item) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Statistics collected during crawling
#[derive(Debug, Default)]
pub struct CrawlStats {
    pub urls_visited: AtomicUsize,
    pub items_extracted: AtomicUsize,
    pub errors_encountered: AtomicUsize,
}

impl CrawlStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn urls_visited(&self) -> usize {
        self.urls_visited.load(Ordering::Relaxed)
    }

    pub fn items_extracted(&self) -> usize {
        self.items_extracted.load(Ordering::Relaxed)
    }

    pub fn errors_encountered(&self) -> usize {
        self.errors_encountered.load(Ordering::Relaxed)
    }
}

/// Web crawler that manages concurrent scraping and processing
///
/// The crawler coordinates multiple concurrent tasks:
/// - Scrapers that fetch and parse web pages
/// - Processors that handle extracted items
/// - A coordinator that manages URL queues and completion detection
pub struct Crawler {
    delay: Duration,
    crawling_concurrency: usize,
    processing_concurrency: usize,
    crawling_queue_multiplier: usize,
    processing_queue_multiplier: usize,
    barrier: Arc<Barrier>,
    active_spiders: Arc<AtomicUsize>,
    stats: Arc<CrawlStats>,
}

impl Crawler {
    /// Create a new crawler with default settings
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Create a crawler builder for custom configuration
    pub fn builder() -> CrawlerBuilder {
        CrawlerBuilder::default()
    }

    /// Get crawl statistics
    pub fn stats(&self) -> &Arc<CrawlStats> {
        &self.stats
    }

    fn new_with_config(
        delay: Duration,
        crawling_concurrency: usize,
        processing_concurrency: usize,
        crawling_queue_multiplier: usize,
        processing_queue_multiplier: usize,
    ) -> Self {
        let barrier = Arc::new(Barrier::new(TASK_COUNT));
        let active_spiders = Arc::new(AtomicUsize::new(0));
        let stats = Arc::new(CrawlStats::new());

        Self {
            delay,
            crawling_concurrency,
            processing_concurrency,
            crawling_queue_multiplier,
            processing_queue_multiplier,
            barrier,
            active_spiders,
            stats,
        }
    }

    fn crawling_queue_capacity(&self) -> usize {
        self.crawling_concurrency * self.crawling_queue_multiplier
    }

    fn processing_queue_capacity(&self) -> usize {
        self.processing_concurrency * self.processing_queue_multiplier
    }

    fn is_crawl_complete(
        &self,
        new_urls_capacity: usize,
        urls_capacity: usize,
        crawling_queue_capacity: usize,
    ) -> bool {
        new_urls_capacity == crawling_queue_capacity
            && urls_capacity == crawling_queue_capacity
            && self.active_spiders.load(Ordering::SeqCst) == 0
    }

    /// Start crawling with the given spider
    ///
    /// This method will block until all URLs have been visited and all items processed.
    pub async fn crawl<T, E>(&self, spider: Arc<dyn Spider<Item = T, Error = E>>)
    where
        T: Send + 'static,
        E: Display + Send + 'static,
    {
        let mut visited_urls = HashSet::<String>::new();
        let crawling_queue_capacity = self.crawling_queue_capacity();
        let processing_queue_capacity = self.processing_queue_capacity();

        let (urls_to_visit_tx, urls_to_visit_rx) = mpsc::channel::<String>(crawling_queue_capacity);
        let (items_tx, items_rx) = mpsc::channel(processing_queue_capacity);
        let (new_urls_tx, mut new_urls_rx) = mpsc::channel(crawling_queue_capacity);

        let completion_notify = Arc::new(Notify::new());

        // Initialize with start URLs
        for url in spider.start_urls() {
            visited_urls.insert(url.clone());
            let _ = urls_to_visit_tx.send(url).await;
        }

        self.launch_processors(spider.clone(), items_rx);

        self.launch_scrapers(
            spider.clone(),
            urls_to_visit_rx,
            new_urls_tx.clone(),
            items_tx,
            completion_notify.clone(),
        );

        // Coordinate URL discovery and completion detection
        self.coordinate_crawl(
            &mut visited_urls,
            urls_to_visit_tx,
            &mut new_urls_rx,
            &new_urls_tx,
            crawling_queue_capacity,
        )
        .await;

        // Wait for all tasks to complete
        self.barrier.wait().await;
    }

    async fn coordinate_crawl(
        &self,
        visited_urls: &mut HashSet<String>,
        urls_to_visit_tx: mpsc::Sender<String>,
        new_urls_rx: &mut mpsc::Receiver<(String, Vec<String>)>,
        new_urls_tx: &mpsc::Sender<(String, Vec<String>)>,
        crawling_queue_capacity: usize,
    ) {
        loop {
            tokio::select! {
                // Process newly discovered URLs
                Some((visited_url, new_urls)) = new_urls_rx.recv() => {
                    visited_urls.insert(visited_url);
                    self.stats.urls_visited.fetch_add(1, Ordering::Relaxed);

                    for url in new_urls {
                        if !visited_urls.contains(&url) {
                            visited_urls.insert(url.clone());
                            let _ = urls_to_visit_tx.send(url).await;
                        }
                    }
                }

                // Check for completion periodically
                _ = sleep(Duration::from_millis(100)) => {
                    if self.is_crawl_complete(
                        new_urls_tx.capacity(),
                        urls_to_visit_tx.capacity(),
                        crawling_queue_capacity,
                    ) {
                        break;
                    }
                }
            }
        }

        drop(urls_to_visit_tx);
    }

    fn launch_processors<T, E>(
        &self,
        spider: Arc<dyn Spider<Item = T, Error = E>>,
        items: mpsc::Receiver<T>,
    ) where
        T: Send + 'static,
        E: Display + Send + 'static,
    {
        let concurrency = self.processing_concurrency;
        let barrier = self.barrier.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            ReceiverStream::new(items)
                .for_each_concurrent(concurrency, |item| {
                    let spider = spider.clone();
                    let stats = stats.clone();
                    async move {
                        match spider.process(item).await {
                            Ok(_) => {
                                stats.items_extracted.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(err) => {
                                eprintln!("Error processing item: {}", err);
                                stats.errors_encountered.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                })
                .await;

            barrier.wait().await;
        });
    }

    fn launch_scrapers<T, E>(
        &self,
        spider: Arc<dyn Spider<Item = T, Error = E>>,
        urls_to_visit: mpsc::Receiver<String>,
        new_urls_tx: mpsc::Sender<(String, Vec<String>)>,
        items_tx: mpsc::Sender<T>,
        _completion_notify: Arc<Notify>,
    ) where
        T: Send + 'static,
        E: Display + Send + 'static,
    {
        let concurrency = self.crawling_concurrency;
        let barrier = self.barrier.clone();
        let delay = self.delay;
        let active_spiders = self.active_spiders.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            ReceiverStream::new(urls_to_visit)
                .for_each_concurrent(concurrency, |queued_url| {
                    let spider = spider.clone();
                    let new_urls_tx = new_urls_tx.clone();
                    let items_tx = items_tx.clone();
                    let active_spiders = active_spiders.clone();
                    let stats = stats.clone();

                    async move {
                        active_spiders.fetch_add(1, Ordering::SeqCst);
                        let mut urls = Vec::new();

                        match spider.scrape(queued_url.clone()).await {
                            Ok((items, new_urls)) => {
                                for item in items {
                                    let _ = items_tx.send(item).await;
                                }
                                urls = new_urls;
                            }
                            Err(err) => {
                                eprintln!("Error scraping {}: {}", queued_url, err);
                                stats.errors_encountered.fetch_add(1, Ordering::Relaxed);
                            }
                        }

                        let _ = new_urls_tx.send((queued_url, urls)).await;
                        sleep(delay).await;
                        active_spiders.fetch_sub(1, Ordering::SeqCst);
                    }
                })
                .await;

            drop(items_tx);
            barrier.wait().await;
        });
    }
}

impl Default for Crawler {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for configuring a Crawler
#[derive(Debug, Clone)]
pub struct CrawlerBuilder {
    delay: Duration,
    crawling_concurrency: usize,
    processing_concurrency: usize,
    crawling_queue_multiplier: usize,
    processing_queue_multiplier: usize,
}

impl Default for CrawlerBuilder {
    fn default() -> Self {
        Self {
            delay: Duration::from_millis(DEFAULT_DELAY_MS),
            crawling_concurrency: DEFAULT_CRAWLING_CONCURRENCY,
            processing_concurrency: DEFAULT_PROCESSING_CONCURRENCY,
            crawling_queue_multiplier: DEFAULT_CRAWLING_QUEUE_MULTIPLIER,
            processing_queue_multiplier: DEFAULT_PROCESSING_QUEUE_MULTIPLIER,
        }
    }
}

impl CrawlerBuilder {
    /// Set the delay between requests (default: 200ms)
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// Set the number of concurrent scraping tasks (default: 2)
    pub fn crawling_concurrency(mut self, concurrency: usize) -> Self {
        self.crawling_concurrency = concurrency;
        self
    }

    /// Set the number of concurrent processing tasks (default: 2)
    pub fn processing_concurrency(mut self, concurrency: usize) -> Self {
        self.processing_concurrency = concurrency;
        self
    }

    /// Set the crawling queue capacity multiplier (default: 400)
    ///
    /// The actual queue capacity will be `crawling_concurrency * multiplier`.
    /// Higher values allow more URLs to be queued but use more memory.
    pub fn crawling_queue_multiplier(mut self, multiplier: usize) -> Self {
        self.crawling_queue_multiplier = multiplier;
        self
    }

    /// Set the processing queue capacity multiplier (default: 10)
    ///
    /// The actual queue capacity will be `processing_concurrency * multiplier`.
    /// Higher values allow more items to be queued but use more memory.
    pub fn processing_queue_multiplier(mut self, multiplier: usize) -> Self {
        self.processing_queue_multiplier = multiplier;
        self
    }

    /// Build the Crawler with the configured settings
    pub fn build(self) -> Crawler {
        Crawler::new_with_config(
            self.delay,
            self.crawling_concurrency,
            self.processing_concurrency,
            self.crawling_queue_multiplier,
            self.processing_queue_multiplier,
        )
    }
}
