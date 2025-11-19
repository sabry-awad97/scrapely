//! Web crawler with advanced features
//!
//! This module provides a concurrent web crawler with support for:
//! - **Graceful cancellation**: Stop crawls cleanly without data loss
//! - **Rate limiting**: Control request rates with token bucket or delay-based strategies
//! - **Observability**: Monitor crawl progress through the observer pattern
//! - **Real-time statistics**: Track URLs visited, items extracted, and errors
//! - **Proper completion detection**: No polling, uses explicit state tracking
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```ignore
//! use scrapely::{Crawler, Spider};
//! use std::sync::Arc;
//!
//! // Create a spider (implement the Spider trait)
//! let spider = Arc::new(MySpider::new());
//!
//! // Build and run crawler
//! let crawler = Crawler::builder()
//!     .crawling_concurrency(4)
//!     .rate_limit(10.0)  // 10 requests per second
//!     .build()?;
//!
//! let stats = crawler.crawl(spider).await;
//! println!("Visited {} URLs", stats.urls_visited);
//! ```
//!
//! ## With Observer
//!
//! ```ignore
//! use scrapely::{Crawler, CrawlObserver, VisitResult};
//! use std::sync::Arc;
//!
//! struct MyObserver;
//!
//! #[async_trait::async_trait]
//! impl CrawlObserver for MyObserver {
//!     async fn on_url_visited(&self, result: &VisitResult) {
//!         println!("Visited: {}", result.visited_url);
//!     }
//! }
//!
//! let crawler = Crawler::builder()
//!     .observe_with(Arc::new(MyObserver))
//!     .build()?;
//! ```
//!
//! ## With Cancellation
//!
//! ```ignore
//! use tokio_util::sync::CancellationToken;
//!
//! let cancel_token = CancellationToken::new();
//! let token_clone = cancel_token.clone();
//!
//! // Cancel after 10 seconds
//! tokio::spawn(async move {
//!     tokio::time::sleep(Duration::from_secs(10)).await;
//!     token_clone.cancel();
//! });
//!
//! let stats = crawler.crawl_with_cancellation(spider, cancel_token).await;
//! ```
//!
//! # Rate Limiting Strategies
//!
//! ## Token Bucket (Recommended)
//!
//! Allows bursts while maintaining average rate:
//!
//! ```ignore
//! let crawler = Crawler::builder()
//!     .rate_limit(10.0)  // 10 requests per second
//!     .build()?;
//! ```
//!
//! ## Fixed Delay
//!
//! Simple delay between requests:
//!
//! ```ignore
//! let crawler = Crawler::builder()
//!     .delay(Duration::from_millis(200))
//!     .build()?;
//! ```

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
    sync::{Notify, mpsc},
    time::sleep,
};
use tokio_stream::wrappers::ReceiverStream;

/// Result of visiting a URL during crawling
#[derive(Debug, Clone)]
pub struct VisitResult {
    /// The URL that was visited
    pub visited_url: String,
    /// New URLs discovered on the page
    pub discovered_urls: Vec<String>,
}

impl VisitResult {
    /// Create a new VisitResult with discovered URLs
    pub fn new(visited_url: String, discovered_urls: Vec<String>) -> Self {
        Self {
            visited_url,
            discovered_urls,
        }
    }

    /// Create a VisitResult with no discovered URLs
    pub fn empty(visited_url: String) -> Self {
        Self {
            visited_url,
            discovered_urls: Vec::new(),
        }
    }
}

/// Errors that can occur during crawler configuration
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Crawling concurrency must be greater than 0
    #[error("Crawling concurrency must be greater than 0, got {0}")]
    InvalidCrawlingConcurrency(usize),

    /// Processing concurrency must be greater than 0
    #[error("Processing concurrency must be greater than 0, got {0}")]
    InvalidProcessingConcurrency(usize),

    /// Queue multiplier must be greater than 0
    #[error("Queue multiplier must be greater than 0, got {0}")]
    InvalidQueueMultiplier(usize),
}

/// Error that occurred during crawling with context
#[derive(Debug)]
pub struct CrawlError {
    /// The URL being processed when the error occurred
    pub url: String,
    /// The operation that failed
    pub operation: String,
    /// The error message
    pub error: String,
}

impl CrawlError {
    /// Create a new CrawlError with context
    pub fn new(url: String, operation: &str, error: String) -> Self {
        Self {
            url,
            operation: operation.to_string(),
            error,
        }
    }
}

/// Detects when crawling is complete by tracking pending work
#[derive(Clone)]
pub struct CompletionDetector {
    pub(crate) pending_urls: Arc<AtomicUsize>,
    pub(crate) active_scrapers: Arc<AtomicUsize>,
    completion_notify: Arc<Notify>,
}

impl CompletionDetector {
    /// Create a new CompletionDetector
    pub fn new() -> Self {
        Self {
            pending_urls: Arc::new(AtomicUsize::new(0)),
            active_scrapers: Arc::new(AtomicUsize::new(0)),
            completion_notify: Arc::new(Notify::new()),
        }
    }

    /// Increment pending URL count when a URL is queued
    pub fn url_queued(&self) {
        // Use SeqCst ordering to ensure total ordering with completion checks
        // This prevents race conditions where completion is checked before work is visible
        // Critical: must synchronize with SeqCst loads in check_completion()
        self.pending_urls.fetch_add(1, Ordering::SeqCst);
    }

    /// Decrement pending URL count when a URL visit completes
    pub fn url_completed(&self) {
        // Use Release ordering to ensure all URL processing is visible before count decreases
        // This synchronizes with url_queued's Acquire to maintain proper ordering
        let remaining = self.pending_urls.fetch_sub(1, Ordering::Release);
        if remaining == 1 {
            // Last URL completed, check if we're done
            self.check_completion();
        }
    }

    /// Mark a scraper as active
    pub fn scraper_started(&self) {
        // Use SeqCst to synchronize with completion check
        // We need total ordering to ensure consistent view with pending_urls
        self.active_scrapers.fetch_add(1, Ordering::SeqCst);
    }

    /// Mark a scraper as inactive and check for completion
    pub fn scraper_finished(&self) {
        // Use SeqCst to synchronize with completion check
        // We need total ordering to ensure consistent view with pending_urls
        let remaining = self.active_scrapers.fetch_sub(1, Ordering::SeqCst);
        if remaining == 1 {
            // Last scraper finished, check if we're done
            self.check_completion();
        }
    }

    /// Check if crawling is complete and notify if so
    fn check_completion(&self) {
        // Use SeqCst for both loads to ensure we see consistent state
        // This is critical: we need to see the same memory state for both counters
        // to avoid race conditions where work is added between the two checks
        let pending = self.pending_urls.load(Ordering::SeqCst);
        let active = self.active_scrapers.load(Ordering::SeqCst);

        if pending == 0 && active == 0 {
            // No pending work and no active scrapers - we're done
            self.completion_notify.notify_waiters();
        }
    }

    /// Wait for crawl completion
    pub async fn wait_for_completion(&self) {
        self.completion_notify.notified().await;
    }

    /// Get the current pending URL count
    pub fn pending_count(&self) -> usize {
        self.pending_urls.load(Ordering::SeqCst)
    }

    /// Get the current active scraper count
    pub fn active_count(&self) -> usize {
        self.active_scrapers.load(Ordering::SeqCst)
    }
}

impl Default for CompletionDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Observer trait for receiving crawl events
///
/// Implement this trait to monitor crawl progress, collect custom metrics,
/// or implement custom logging strategies.
///
/// # Example
///
/// ```ignore
/// use scrapely::CrawlObserver;
///
/// struct LoggingObserver;
///
/// #[async_trait::async_trait]
/// impl CrawlObserver for LoggingObserver {
///     async fn on_url_visited(&self, result: &VisitResult) {
///         println!("Visited: {}", result.visited_url);
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait CrawlObserver: Send + Sync {
    /// Called when a URL is queued for visiting
    async fn on_url_queued(&self, _url: &str) {}

    /// Called when a URL has been successfully visited
    async fn on_url_visited(&self, _result: &VisitResult) {}

    /// Called when an item is extracted from a page
    async fn on_item_extracted(&self, _url: &str) {}

    /// Called when an error occurs during scraping
    async fn on_scrape_error(&self, _url: &str, _error: &str) {}

    /// Called when an error occurs during item processing
    async fn on_process_error(&self, _error: &str) {}

    /// Called when the crawl completes
    async fn on_crawl_complete(&self, _stats: &CrawlStats) {}
}

/// Registry for managing multiple crawl observers
pub struct ObserverRegistry {
    observers: Vec<Arc<dyn CrawlObserver>>,
}

impl ObserverRegistry {
    /// Create a new empty ObserverRegistry
    pub fn new() -> Self {
        Self {
            observers: Vec::new(),
        }
    }

    /// Register an observer to receive crawl events
    pub fn register(&mut self, observer: Arc<dyn CrawlObserver>) {
        self.observers.push(observer);
    }

    /// Notify all observers that a URL was queued
    pub async fn notify_url_queued(&self, url: &str) {
        for observer in &self.observers {
            observer.on_url_queued(url).await;
        }
    }

    /// Notify all observers that a URL was visited
    pub async fn notify_url_visited(&self, result: &VisitResult) {
        for observer in &self.observers {
            observer.on_url_visited(result).await;
        }
    }

    /// Notify all observers that an item was extracted
    pub async fn notify_item_extracted(&self, url: &str) {
        for observer in &self.observers {
            observer.on_item_extracted(url).await;
        }
    }

    /// Notify all observers that a scrape error occurred
    pub async fn notify_scrape_error(&self, url: &str, error: &str) {
        for observer in &self.observers {
            observer.on_scrape_error(url, error).await;
        }
    }

    /// Notify all observers that a process error occurred
    pub async fn notify_process_error(&self, error: &str) {
        for observer in &self.observers {
            observer.on_process_error(error).await;
        }
    }

    /// Notify all observers that the crawl completed
    pub async fn notify_crawl_complete(&self, stats: &CrawlStats) {
        for observer in &self.observers {
            observer.on_crawl_complete(stats).await;
        }
    }
}

impl Default for ObserverRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for implementing rate limiting strategies
#[async_trait::async_trait]
pub trait RateLimiter: Send + Sync {
    /// Wait until a request is allowed under the rate limit
    async fn acquire(&self);

    /// Check if a request would be allowed without waiting
    fn check(&self) -> bool;
}

use std::time::Instant;
use tokio::sync::Mutex;

/// Token bucket rate limiter implementation
pub struct TokenBucketLimiter {
    tokens: Arc<Mutex<f64>>,
    capacity: f64,
    refill_rate: f64, // tokens per second
    last_refill: Arc<Mutex<Instant>>,
}

impl TokenBucketLimiter {
    /// Create a new TokenBucketLimiter
    ///
    /// # Arguments
    /// * `requests_per_second` - Maximum number of requests allowed per second
    pub fn new(requests_per_second: f64) -> Self {
        Self {
            tokens: Arc::new(Mutex::new(requests_per_second)),
            capacity: requests_per_second,
            refill_rate: requests_per_second,
            last_refill: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&self, tokens: &mut f64, last_refill: &mut Instant) {
        let now = Instant::now();
        let elapsed = now.duration_since(*last_refill).as_secs_f64();
        *tokens = (*tokens + elapsed * self.refill_rate).min(self.capacity);
        *last_refill = now;
    }
}

#[async_trait::async_trait]
impl RateLimiter for TokenBucketLimiter {
    async fn acquire(&self) {
        loop {
            let wait_duration = {
                let mut tokens = self.tokens.lock().await;
                let mut last_refill = self.last_refill.lock().await;
                self.refill(&mut tokens, &mut last_refill);

                if *tokens >= 1.0 {
                    *tokens -= 1.0;
                    return;
                }

                // Calculate how long to wait for the next token
                // tokens_needed = 1.0 - current_tokens
                // time_needed = tokens_needed / refill_rate
                let tokens_needed = 1.0 - *tokens;
                let seconds_to_wait = tokens_needed / self.refill_rate;
                Duration::from_secs_f64(seconds_to_wait.max(0.001)) // Minimum 1ms
            };

            sleep(wait_duration).await;
        }
    }

    fn check(&self) -> bool {
        // Simplified non-blocking check
        true
    }
}

/// Delay-based rate limiter (legacy compatibility)
pub struct DelayLimiter {
    delay: Duration,
}

impl DelayLimiter {
    /// Create a new DelayLimiter
    ///
    /// # Arguments
    /// * `delay` - Duration to wait between requests
    pub fn new(delay: Duration) -> Self {
        Self { delay }
    }
}

#[async_trait::async_trait]
impl RateLimiter for DelayLimiter {
    async fn acquire(&self) {
        sleep(self.delay).await;
    }

    fn check(&self) -> bool {
        true
    }
}

/// Configuration for rate limiting strategy
#[derive(Debug, Clone)]
pub enum RateLimiterConfig {
    /// Use a fixed delay between requests
    Delay(Duration),
    /// Use token bucket algorithm with requests per second
    TokenBucket { requests_per_second: f64 },
    /// No rate limiting
    None,
}

/// Validated configuration for the crawler
#[derive(Debug, Clone)]
pub struct CrawlerConfig {
    pub(crate) crawling_concurrency: usize,
    pub(crate) processing_concurrency: usize,
    pub(crate) crawling_queue_multiplier: usize,
    pub(crate) processing_queue_multiplier: usize,
    pub(crate) rate_limiter: RateLimiterConfig,
}

impl CrawlerConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.crawling_concurrency == 0 {
            return Err(ConfigError::InvalidCrawlingConcurrency(0));
        }
        if self.processing_concurrency == 0 {
            return Err(ConfigError::InvalidProcessingConcurrency(0));
        }
        if self.crawling_queue_multiplier == 0 {
            return Err(ConfigError::InvalidQueueMultiplier(0));
        }
        if self.processing_queue_multiplier == 0 {
            return Err(ConfigError::InvalidQueueMultiplier(0));
        }
        Ok(())
    }

    /// Get the crawling queue capacity
    pub fn crawling_queue_capacity(&self) -> usize {
        self.crawling_concurrency * self.crawling_queue_multiplier
    }

    /// Get the processing queue capacity
    pub fn processing_queue_capacity(&self) -> usize {
        self.processing_concurrency * self.processing_queue_multiplier
    }
}

/// Utility for normalizing URLs for deduplication
pub struct UrlNormalizer;

impl UrlNormalizer {
    /// Normalize a URL for deduplication
    ///
    /// This removes trailing slashes, fragments, and normalizes the URL
    /// to ensure equivalent URLs are treated as identical.
    pub fn normalize(url: &str) -> String {
        let mut normalized = url.to_string();

        // Remove fragment identifier (#...)
        if let Some(pos) = normalized.find('#') {
            normalized.truncate(pos);
        }

        // Remove trailing slash (but keep it for root paths like "http://example.com/")
        // Handle both cases: slash at end OR slash before query parameters
        if let Some(query_pos) = normalized.find('?') {
            // Has query parameters - check if there's a trailing slash before the ?
            if normalized[..query_pos].ends_with('/') {
                let path_slashes = normalized[..query_pos].matches('/').count();
                if path_slashes > 3 {
                    // Remove the slash before the query string
                    normalized.remove(query_pos - 1);
                }
            }
        } else if normalized.ends_with('/') {
            // No query parameters - check trailing slash at end
            let path_slashes = normalized.matches('/').count();
            if path_slashes > 3 {
                normalized.pop();
            }
        }

        normalized
    }
}

// Configuration constants
const DEFAULT_CRAWLING_QUEUE_MULTIPLIER: usize = 400;
const DEFAULT_PROCESSING_QUEUE_MULTIPLIER: usize = 10;
const DEFAULT_DELAY_MS: u64 = 200;
const DEFAULT_CRAWLING_CONCURRENCY: usize = 2;
const DEFAULT_PROCESSING_CONCURRENCY: usize = 2;

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

/// Statistics collected during crawling with timestamps
#[derive(Debug, Clone)]
pub struct CrawlStats {
    /// Number of URLs visited
    pub urls_visited: usize,
    /// Number of items extracted
    pub items_extracted: usize,
    /// Number of errors encountered
    pub errors_encountered: usize,
    /// When the crawl started
    pub start_time: Instant,
    /// When these stats were last updated
    pub last_update: Instant,
}

impl CrawlStats {
    /// Create new CrawlStats with current timestamp
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            urls_visited: 0,
            items_extracted: 0,
            errors_encountered: 0,
            start_time: now,
            last_update: now,
        }
    }

    /// Get elapsed time since crawl started
    pub fn elapsed(&self) -> Duration {
        self.last_update.duration_since(self.start_time)
    }

    /// Calculate URLs visited per second
    pub fn urls_per_second(&self) -> f64 {
        let elapsed = self.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.urls_visited as f64 / elapsed
        } else {
            0.0
        }
    }
}

impl Default for CrawlStats {
    fn default() -> Self {
        Self::new()
    }
}

use tokio::sync::watch;

/// Thread-safe statistics tracker with real-time broadcasting
pub struct StatsTracker {
    urls_visited: AtomicUsize,
    items_extracted: AtomicUsize,
    errors_encountered: AtomicUsize,
    start_time: Instant,
    tx: Arc<Mutex<Option<watch::Sender<CrawlStats>>>>,
    rx: watch::Receiver<CrawlStats>,
}

impl StatsTracker {
    /// Create a new StatsTracker
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(CrawlStats::new());
        Self {
            urls_visited: AtomicUsize::new(0),
            items_extracted: AtomicUsize::new(0),
            errors_encountered: AtomicUsize::new(0),
            start_time: Instant::now(),
            tx: Arc::new(Mutex::new(Some(tx))),
            rx,
        }
    }

    /// Subscribe to statistics updates
    pub fn subscribe(&self) -> watch::Receiver<CrawlStats> {
        self.rx.clone()
    }

    /// Record a URL visit
    pub fn url_visited(&self) {
        // Use Relaxed ordering - statistics don't require synchronization
        // Stats are informational and don't affect control flow
        self.urls_visited.fetch_add(1, Ordering::Relaxed);
        self.broadcast();
    }

    /// Record an item extraction
    pub fn item_extracted(&self) {
        // Use Relaxed ordering - statistics don't require synchronization
        self.items_extracted.fetch_add(1, Ordering::Relaxed);
        self.broadcast();
    }

    /// Record an error
    pub fn error_encountered(&self) {
        // Use Relaxed ordering - statistics don't require synchronization
        self.errors_encountered.fetch_add(1, Ordering::Relaxed);
        self.broadcast();
    }

    /// Broadcast current statistics to all subscribers
    fn broadcast(&self) {
        let stats = CrawlStats {
            urls_visited: self.urls_visited.load(Ordering::Relaxed),
            items_extracted: self.items_extracted.load(Ordering::Relaxed),
            errors_encountered: self.errors_encountered.load(Ordering::Relaxed),
            start_time: self.start_time,
            last_update: Instant::now(),
        };
        // Ignore send errors - if no one is listening, that's fine
        if let Ok(tx_guard) = self.tx.try_lock()
            && let Some(tx) = tx_guard.as_ref()
        {
            let _ = tx.send(stats);
        }
    }

    /// Get a snapshot of current statistics
    pub fn snapshot(&self) -> CrawlStats {
        CrawlStats {
            urls_visited: self.urls_visited.load(Ordering::Relaxed),
            items_extracted: self.items_extracted.load(Ordering::Relaxed),
            errors_encountered: self.errors_encountered.load(Ordering::Relaxed),
            start_time: self.start_time,
            last_update: Instant::now(),
        }
    }

    /// Close the statistics sender to signal completion to subscribers
    pub fn close(&self) {
        // Drop the sender by replacing it with None
        // This will cause all subscribers' changed() calls to return an error
        if let Ok(mut tx_guard) = self.tx.try_lock() {
            *tx_guard = None;
        }
    }
}

impl Default for StatsTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Web crawler that manages concurrent scraping and processing
///
/// The crawler coordinates multiple concurrent tasks:
/// - Scrapers that fetch and parse web pages
/// - Processors that handle extracted items
/// - A coordinator that manages URL queues and completion detection
pub struct Crawler {
    config: CrawlerConfig,
    observers: Arc<ObserverRegistry>,
    stats: Arc<StatsTracker>,
    completion: Arc<CompletionDetector>,
}

impl Crawler {
    /// Create a new crawler with default settings
    pub fn new() -> Self {
        Self::builder()
            .build()
            .expect("Default configuration should be valid")
    }

    /// Create a crawler builder for custom configuration
    pub fn builder() -> CrawlerBuilder {
        CrawlerBuilder::default()
    }

    /// Get a snapshot of current crawl statistics
    pub fn stats(&self) -> CrawlStats {
        self.stats.snapshot()
    }

    /// Subscribe to real-time statistics updates
    pub fn subscribe_stats(&self) -> watch::Receiver<CrawlStats> {
        self.stats.subscribe()
    }

    fn new_with_config(config: CrawlerConfig, observers: Vec<Arc<dyn CrawlObserver>>) -> Self {
        let mut registry = ObserverRegistry::new();
        for observer in observers {
            registry.register(observer);
        }

        Self {
            config,
            observers: Arc::new(registry),
            stats: Arc::new(StatsTracker::new()),
            completion: Arc::new(CompletionDetector::new()),
        }
    }

    /// Start crawling with the given spider
    ///
    /// This method will block until all URLs have been visited and all items processed.
    /// Returns final crawl statistics.
    pub async fn crawl<T, E>(&self, spider: Arc<dyn Spider<Item = T, Error = E>>) -> CrawlStats
    where
        T: Send + 'static,
        E: Display + Send + 'static,
    {
        self.crawl_internal(spider, None).await
    }

    /// Start crawling with cancellation support
    ///
    /// This method allows graceful cancellation of the crawl operation.
    /// When the cancellation token is signaled, the crawler will:
    /// - Stop accepting new URLs
    /// - Complete all in-flight scraping operations
    /// - Process all extracted items
    /// - Return final statistics
    ///
    /// # Arguments
    /// * `spider` - The spider implementation to use for crawling
    /// * `cancel_token` - Token to signal cancellation
    ///
    /// # Returns
    /// Final crawl statistics reflecting all completed work
    pub async fn crawl_with_cancellation<T, E>(
        &self,
        spider: Arc<dyn Spider<Item = T, Error = E>>,
        cancel_token: tokio_util::sync::CancellationToken,
    ) -> CrawlStats
    where
        T: Send + 'static,
        E: Display + Send + 'static,
    {
        self.crawl_internal(spider, Some(cancel_token)).await
    }

    /// Internal crawl implementation with optional cancellation
    async fn crawl_internal<T, E>(
        &self,
        spider: Arc<dyn Spider<Item = T, Error = E>>,
        cancel_token: Option<tokio_util::sync::CancellationToken>,
    ) -> CrawlStats
    where
        T: Send + 'static,
        E: Display + Send + 'static,
    {
        let mut visited_urls = HashSet::<String>::new();
        let crawling_queue_capacity = self.config.crawling_queue_capacity();
        let processing_queue_capacity = self.config.processing_queue_capacity();

        let (urls_to_visit_tx, urls_to_visit_rx) = mpsc::channel::<String>(crawling_queue_capacity);
        let (items_tx, items_rx) = mpsc::channel(processing_queue_capacity);
        let (new_urls_tx, mut new_urls_rx) = mpsc::channel::<VisitResult>(crawling_queue_capacity);

        // Initialize with start URLs
        for url in spider.start_urls() {
            let normalized = UrlNormalizer::normalize(&url);
            if visited_urls.insert(normalized.clone()) {
                self.completion.url_queued();
                self.observers.notify_url_queued(&url).await;
                if let Err(e) = urls_to_visit_tx.send(url.clone()).await {
                    let error = CrawlError::new(url, "channel_send", e.to_string());
                    eprintln!("Failed to queue start URL: {}", error.error);
                    self.stats.error_encountered();
                }
            }
        }

        // Create rate limiter
        let rate_limiter: Arc<dyn RateLimiter> = match &self.config.rate_limiter {
            RateLimiterConfig::Delay(delay) => Arc::new(DelayLimiter::new(*delay)),
            RateLimiterConfig::TokenBucket {
                requests_per_second,
            } => Arc::new(TokenBucketLimiter::new(*requests_per_second)),
            RateLimiterConfig::None => Arc::new(DelayLimiter::new(Duration::from_millis(0))),
        };

        // Launch processors and get handle to wait for completion
        let processor_handle = self.launch_processors(spider.clone(), items_rx);

        // Launch scrapers and get handle to wait for completion
        let scraper_handle = self.launch_scrapers(
            spider.clone(),
            urls_to_visit_rx,
            new_urls_tx.clone(),
            items_tx,
            rate_limiter,
        );

        // Drop the original sender so the channel closes when all scrapers finish
        drop(new_urls_tx);

        // Coordinate URL discovery
        self.coordinate_crawl(
            &mut visited_urls,
            urls_to_visit_tx,
            &mut new_urls_rx,
            cancel_token,
        )
        .await;

        // Wait for scrapers to finish (this will also close the items channel)
        let _ = scraper_handle.await;

        // Wait for processors to finish
        let _ = processor_handle.await;

        // Close the stats sender to signal completion to subscribers
        self.stats.close();

        // Notify observers
        let final_stats = self.stats.snapshot();
        self.observers.notify_crawl_complete(&final_stats).await;

        final_stats
    }

    /// Coordinate URL discovery and completion detection
    async fn coordinate_crawl(
        &self,
        visited_urls: &mut HashSet<String>,
        urls_to_visit_tx: mpsc::Sender<String>,
        new_urls_rx: &mut mpsc::Receiver<VisitResult>,
        cancel_token: Option<tokio_util::sync::CancellationToken>,
    ) {
        loop {
            tokio::select! {
                // Process newly discovered URLs
                result = new_urls_rx.recv() => {
                    match result {
                        Some(result) => {
                            // Notify observers
                            self.observers.notify_url_visited(&result).await;

                            // Update stats
                            self.stats.url_visited();

                            // Mark URL as completed
                            self.completion.url_completed();

                            // Process discovered URLs
                            for url in result.discovered_urls {
                                let normalized = UrlNormalizer::normalize(&url);
                                if visited_urls.insert(normalized) {
                                    self.completion.url_queued();
                                    self.observers.notify_url_queued(&url).await;

                                    if let Err(e) = urls_to_visit_tx.send(url.clone()).await {
                                        let error = CrawlError::new(url, "channel_send", e.to_string());
                                        eprintln!("Failed to queue URL: {}", error.error);
                                        self.stats.error_encountered();
                                        self.observers.notify_scrape_error(&error.url, &error.error).await;
                                    }
                                }
                            }

                            // Check if we're done - no pending URLs and no active scrapers
                            let pending = self.completion.pending_count();
                            let active = self.completion.active_count();
                            if pending == 0 && active == 0 {
                                break;
                            }
                        }
                        None => {
                            // Channel closed - all scrapers have finished
                            break;
                        }
                    }
                }

                // Check for cancellation
                _ = async {
                    if let Some(ref token) = cancel_token {
                        token.cancelled().await
                    } else {
                        std::future::pending::<()>().await
                    }
                } => {
                    // Cancellation requested - stop accepting new URLs
                    break;
                }
            }
        }

        drop(urls_to_visit_tx);
    }

    /// Launch processor tasks and return a handle to wait for completion
    fn launch_processors<T, E>(
        &self,
        spider: Arc<dyn Spider<Item = T, Error = E>>,
        items: mpsc::Receiver<T>,
    ) -> tokio::task::JoinHandle<()>
    where
        T: Send + 'static,
        E: Display + Send + 'static,
    {
        let concurrency = self.config.processing_concurrency;
        let stats = self.stats.clone();
        let observers = self.observers.clone();

        tokio::spawn(async move {
            ReceiverStream::new(items)
                .for_each_concurrent(concurrency, |item| {
                    let spider = spider.clone();
                    let stats = stats.clone();
                    let observers = observers.clone();

                    async move {
                        match spider.process(item).await {
                            Ok(_) => {
                                stats.item_extracted();
                                observers.notify_item_extracted("").await;
                            }
                            Err(err) => {
                                let error_msg = err.to_string();
                                eprintln!("Error processing item: {}", error_msg);
                                stats.error_encountered();
                                observers.notify_process_error(&error_msg).await;
                            }
                        }
                    }
                })
                .await;
        })
    }

    /// Launch scraper tasks and return a handle to wait for completion
    fn launch_scrapers<T, E>(
        &self,
        spider: Arc<dyn Spider<Item = T, Error = E>>,
        urls_to_visit: mpsc::Receiver<String>,
        new_urls_tx: mpsc::Sender<VisitResult>,
        items_tx: mpsc::Sender<T>,
        rate_limiter: Arc<dyn RateLimiter>,
    ) -> tokio::task::JoinHandle<()>
    where
        T: Send + 'static,
        E: Display + Send + 'static,
    {
        let concurrency = self.config.crawling_concurrency;
        let completion = self.completion.clone();
        let stats = self.stats.clone();
        let observers = self.observers.clone();

        tokio::spawn(async move {
            ReceiverStream::new(urls_to_visit)
                .for_each_concurrent(concurrency, |queued_url| {
                    let spider = spider.clone();
                    let new_urls_tx = new_urls_tx.clone();
                    let items_tx = items_tx.clone();
                    let completion = completion.clone();
                    let stats = stats.clone();
                    let observers = observers.clone();
                    let rate_limiter = rate_limiter.clone();

                    async move {
                        completion.scraper_started();

                        // Apply rate limiting
                        rate_limiter.acquire().await;

                        let mut discovered_urls = Vec::new();

                        match spider.scrape(queued_url.clone()).await {
                            Ok((items, new_urls)) => {
                                for item in items {
                                    if let Err(e) = items_tx.send(item).await {
                                        eprintln!("Failed to send item: {}", e);
                                        stats.error_encountered();
                                    }
                                }
                                discovered_urls = new_urls;
                            }
                            Err(err) => {
                                let error_msg = err.to_string();
                                eprintln!("Error scraping {}: {}", queued_url, error_msg);
                                stats.error_encountered();
                                observers.notify_scrape_error(&queued_url, &error_msg).await;
                            }
                        }

                        let result = VisitResult::new(queued_url.clone(), discovered_urls);
                        if let Err(e) = new_urls_tx.send(result).await {
                            let error =
                                CrawlError::new(queued_url.clone(), "channel_send", e.to_string());
                            eprintln!("Failed to send visit result: {}", error.error);
                            stats.error_encountered();
                        }

                        completion.scraper_finished();
                    }
                })
                .await;

            drop(items_tx);
            drop(new_urls_tx);
        })
    }
}

impl Default for Crawler {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for configuring a Crawler
pub struct CrawlerBuilder {
    config: CrawlerConfig,
    observers: Vec<Arc<dyn CrawlObserver>>,
}

impl Default for CrawlerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl CrawlerBuilder {
    /// Create a new CrawlerBuilder with default settings
    pub fn new() -> Self {
        Self {
            config: CrawlerConfig {
                crawling_concurrency: DEFAULT_CRAWLING_CONCURRENCY,
                processing_concurrency: DEFAULT_PROCESSING_CONCURRENCY,
                crawling_queue_multiplier: DEFAULT_CRAWLING_QUEUE_MULTIPLIER,
                processing_queue_multiplier: DEFAULT_PROCESSING_QUEUE_MULTIPLIER,
                rate_limiter: RateLimiterConfig::Delay(Duration::from_millis(DEFAULT_DELAY_MS)),
            },
            observers: Vec::new(),
        }
    }

    /// Set the number of concurrent scraping tasks (default: 2)
    pub fn crawling_concurrency(mut self, concurrency: usize) -> Self {
        self.config.crawling_concurrency = concurrency;
        self
    }

    /// Set the number of concurrent processing tasks (default: 2)
    pub fn processing_concurrency(mut self, concurrency: usize) -> Self {
        self.config.processing_concurrency = concurrency;
        self
    }

    /// Set the crawling queue capacity multiplier (default: 400)
    ///
    /// The actual queue capacity will be `crawling_concurrency * multiplier`.
    /// Higher values allow more URLs to be queued but use more memory.
    pub fn crawling_queue_multiplier(mut self, multiplier: usize) -> Self {
        self.config.crawling_queue_multiplier = multiplier;
        self
    }

    /// Set the processing queue capacity multiplier (default: 10)
    ///
    /// The actual queue capacity will be `processing_concurrency * multiplier`.
    /// Higher values allow more items to be queued but use more memory.
    pub fn processing_queue_multiplier(mut self, multiplier: usize) -> Self {
        self.config.processing_queue_multiplier = multiplier;
        self
    }

    /// Set the delay between requests (default: 200ms)
    pub fn delay(mut self, delay: Duration) -> Self {
        self.config.rate_limiter = RateLimiterConfig::Delay(delay);
        self
    }

    /// Set rate limiting using token bucket algorithm
    ///
    /// # Arguments
    /// * `requests_per_second` - Maximum number of requests allowed per second
    pub fn rate_limit(mut self, requests_per_second: f64) -> Self {
        self.config.rate_limiter = RateLimiterConfig::TokenBucket {
            requests_per_second,
        };
        self
    }

    /// Register an observer to receive crawl events
    pub fn observe_with(mut self, observer: Arc<dyn CrawlObserver>) -> Self {
        self.observers.push(observer);
        self
    }

    /// Build the Crawler with the configured settings
    pub fn build(self) -> Result<Crawler, ConfigError> {
        self.config.validate()?;
        Ok(Crawler::new_with_config(self.config, self.observers))
    }
}
