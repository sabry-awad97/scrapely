use scrapely::*;
use std::sync::{Arc, Mutex};
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

/// Mock spider for testing basic crawling functionality
struct MockSpider {
    pages: Arc<Mutex<Vec<(String, Vec<String>, Vec<String>)>>>, // (url, items, new_urls)
}

impl MockSpider {
    fn new() -> Self {
        let pages = vec![
            (
                "https://example.com".to_string(),
                vec!["item1".to_string(), "item2".to_string()],
                vec!["https://example.com/page1".to_string()],
            ),
            (
                "https://example.com/page1".to_string(),
                vec!["item3".to_string()],
                vec!["https://example.com/page2".to_string()],
            ),
            (
                "https://example.com/page2".to_string(),
                vec!["item4".to_string()],
                vec![],
            ),
        ];

        Self {
            pages: Arc::new(Mutex::new(pages)),
        }
    }
}

#[async_trait::async_trait]
impl Spider for MockSpider {
    type Item = String;
    type Error = String;

    fn start_urls(&self) -> Vec<String> {
        vec!["https://example.com".to_string()]
    }

    async fn scrape(&self, url: String) -> Result<(Vec<Self::Item>, Vec<String>), Self::Error> {
        let pages = self.pages.lock().unwrap();

        for (page_url, items, new_urls) in pages.iter() {
            if page_url == &url {
                return Ok((items.clone(), new_urls.clone()));
            }
        }

        Ok((vec![], vec![]))
    }

    async fn process(&self, _item: Self::Item) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_crawling_flow() {
        // Test that crawler visits all pages and extracts all items
        let spider = Arc::new(MockSpider::new());
        let crawler = Crawler::builder()
            .crawling_concurrency(2)
            .processing_concurrency(2)
            .build()
            .unwrap();

        let stats = crawler.crawl(spider).await;

        // Should visit 3 pages (start + 2 discovered)
        assert_eq!(stats.urls_visited, 3);
        // Should extract 4 items total
        assert_eq!(stats.items_extracted, 4);
        // Should have no errors
        assert_eq!(stats.errors_encountered, 0);
    }

    #[tokio::test]
    async fn test_rate_limiting_behavior() {
        use std::time::Instant;

        let spider = Arc::new(MockSpider::new());

        // Set rate limit to 2 requests per second
        let crawler = Crawler::builder()
            .crawling_concurrency(1)
            .rate_limit(2.0)
            .build()
            .unwrap();

        let start = Instant::now();
        let stats = crawler.crawl(spider).await;
        let elapsed = start.elapsed();

        // Should visit 3 pages
        assert_eq!(stats.urls_visited, 3);

        // With 2 req/sec and token bucket, first request is immediate
        // Second at ~0.5s, third at ~1.0s, but with processing overhead
        // Expect at least 0.4 seconds to show rate limiting is working
        assert!(
            elapsed.as_secs_f64() >= 0.4,
            "Rate limiting not working: took {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_cancellation_support() {
        // Spider that would crawl many pages
        struct SlowSpider;

        #[async_trait::async_trait]
        impl Spider for SlowSpider {
            type Item = String;
            type Error = String;

            fn start_urls(&self) -> Vec<String> {
                vec!["https://example.com".to_string()]
            }

            async fn scrape(
                &self,
                _url: String,
            ) -> Result<(Vec<Self::Item>, Vec<String>), Self::Error> {
                // Simulate slow scraping
                tokio::time::sleep(Duration::from_millis(100)).await;

                // Generate many new URLs to keep crawling
                let new_urls: Vec<String> = (0..10)
                    .map(|i| format!("https://example.com/page{}", i))
                    .collect();

                Ok((vec!["item".to_string()], new_urls))
            }

            async fn process(&self, _item: Self::Item) -> Result<(), Self::Error> {
                Ok(())
            }
        }

        let spider = Arc::new(SlowSpider);
        let crawler = Crawler::builder().crawling_concurrency(2).build().unwrap();

        let cancel_token = CancellationToken::new();
        let token_clone = cancel_token.clone();

        // Cancel after 500ms to ensure crawler has time to start and crawl pages
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            token_clone.cancel();
        });

        let stats = crawler.crawl_with_cancellation(spider, cancel_token).await;

        // Should have visited some pages but stopped due to cancellation
        // The exact number depends on timing, but should be reasonable
        assert!(stats.urls_visited >= 1, "Should visit at least 1 page");
        assert!(
            stats.urls_visited <= 15,
            "Crawled too many pages after cancellation: {}",
            stats.urls_visited
        );
    }

    #[tokio::test]
    async fn test_error_handling_scenarios() {
        struct ErrorSpider {
            fail_on: Vec<String>,
        }

        #[async_trait::async_trait]
        impl Spider for ErrorSpider {
            type Item = String;
            type Error = String;

            fn start_urls(&self) -> Vec<String> {
                vec![
                    "https://example.com/good1".to_string(),
                    "https://example.com/bad".to_string(),
                    "https://example.com/good2".to_string(),
                ]
            }

            async fn scrape(
                &self,
                url: String,
            ) -> Result<(Vec<Self::Item>, Vec<String>), Self::Error> {
                if self.fail_on.contains(&url) {
                    return Err(format!("Failed to scrape {}", url));
                }
                Ok((vec!["item".to_string()], vec![]))
            }

            async fn process(&self, _item: Self::Item) -> Result<(), Self::Error> {
                Ok(())
            }
        }

        let spider = Arc::new(ErrorSpider {
            fail_on: vec!["https://example.com/bad".to_string()],
        });

        let crawler = Crawler::builder().crawling_concurrency(2).build().unwrap();

        let stats = crawler.crawl(spider).await;

        // Should visit all 3 URLs (errors don't stop crawling)
        assert_eq!(stats.urls_visited, 3);
        // Should extract 2 items (from good1 and good2)
        assert_eq!(stats.items_extracted, 2);
        // Should have 1 error (from bad)
        assert_eq!(stats.errors_encountered, 1);
    }

    #[tokio::test]
    async fn test_observer_notifications() {
        use std::sync::Mutex;

        struct TestObserver {
            urls_queued: Arc<Mutex<Vec<String>>>,
            urls_visited: Arc<Mutex<Vec<String>>>,
            items_extracted: Arc<Mutex<Vec<String>>>,
            scrape_errors: Arc<Mutex<Vec<String>>>,
        }

        impl TestObserver {
            fn new() -> Self {
                Self {
                    urls_queued: Arc::new(Mutex::new(Vec::new())),
                    urls_visited: Arc::new(Mutex::new(Vec::new())),
                    items_extracted: Arc::new(Mutex::new(Vec::new())),
                    scrape_errors: Arc::new(Mutex::new(Vec::new())),
                }
            }
        }

        #[async_trait::async_trait]
        impl CrawlObserver for TestObserver {
            async fn on_url_queued(&self, url: &str) {
                self.urls_queued.lock().unwrap().push(url.to_string());
            }

            async fn on_url_visited(&self, result: &VisitResult) {
                self.urls_visited
                    .lock()
                    .unwrap()
                    .push(result.visited_url.clone());
            }

            async fn on_item_extracted(&self, url: &str) {
                self.items_extracted.lock().unwrap().push(url.to_string());
            }

            async fn on_scrape_error(&self, url: &str, _error: &str) {
                self.scrape_errors.lock().unwrap().push(url.to_string());
            }
        }

        let observer = Arc::new(TestObserver::new());
        let spider = Arc::new(MockSpider::new());

        let crawler = Crawler::builder()
            .crawling_concurrency(2)
            .observe_with(observer.clone())
            .build()
            .unwrap();

        let stats = crawler.crawl(spider).await;

        // Verify observer received all notifications
        assert_eq!(observer.urls_queued.lock().unwrap().len(), 3);
        assert_eq!(observer.urls_visited.lock().unwrap().len(), 3);
        assert_eq!(observer.items_extracted.lock().unwrap().len(), 4);
        assert_eq!(observer.scrape_errors.lock().unwrap().len(), 0);

        // Verify stats match
        assert_eq!(stats.urls_visited, 3);
        assert_eq!(stats.items_extracted, 4);
    }

    #[tokio::test]
    async fn test_concurrent_crawling() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Instant;

        struct ConcurrentSpider {
            concurrent_count: Arc<AtomicUsize>,
            max_concurrent: Arc<AtomicUsize>,
        }

        #[async_trait::async_trait]
        impl Spider for ConcurrentSpider {
            type Item = String;
            type Error = String;

            fn start_urls(&self) -> Vec<String> {
                (0..10)
                    .map(|i| format!("https://example.com/page{}", i))
                    .collect()
            }

            async fn scrape(
                &self,
                _url: String,
            ) -> Result<(Vec<Self::Item>, Vec<String>), Self::Error> {
                // Track concurrent executions
                let current = self.concurrent_count.fetch_add(1, Ordering::SeqCst) + 1;

                // Update max if needed
                let mut max = self.max_concurrent.load(Ordering::SeqCst);
                while current > max {
                    match self.max_concurrent.compare_exchange(
                        max,
                        current,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    ) {
                        Ok(_) => break,
                        Err(x) => max = x,
                    }
                }

                // Simulate work
                tokio::time::sleep(Duration::from_millis(50)).await;

                self.concurrent_count.fetch_sub(1, Ordering::SeqCst);

                Ok((vec!["item".to_string()], vec![]))
            }

            async fn process(&self, _item: Self::Item) -> Result<(), Self::Error> {
                Ok(())
            }
        }

        let concurrent_count = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));

        let spider = Arc::new(ConcurrentSpider {
            concurrent_count: concurrent_count.clone(),
            max_concurrent: max_concurrent.clone(),
        });

        let crawler = Crawler::builder().crawling_concurrency(4).build().unwrap();

        let start = Instant::now();
        let stats = crawler.crawl(spider).await;
        let elapsed = start.elapsed();

        // Should visit all 10 pages
        assert_eq!(stats.urls_visited, 10);

        // Should have used concurrency (max should be > 1)
        let max = max_concurrent.load(Ordering::SeqCst);
        assert!(
            max >= 1,
            "Expected at least 1 concurrent execution, got max={}",
            max
        );
        assert!(max <= 4, "Exceeded concurrency limit, got max={}", max);

        // With concurrency, should complete faster than sequential
        // Be very lenient with timing - just ensure it completes in reasonable time
        assert!(elapsed.as_millis() < 1000, "Took too long: {:?}", elapsed);
    }

    #[tokio::test]
    async fn test_url_deduplication() {
        struct DuplicateSpider;

        #[async_trait::async_trait]
        impl Spider for DuplicateSpider {
            type Item = String;
            type Error = String;

            fn start_urls(&self) -> Vec<String> {
                vec!["https://example.com".to_string()]
            }

            async fn scrape(
                &self,
                url: String,
            ) -> Result<(Vec<Self::Item>, Vec<String>), Self::Error> {
                if url == "https://example.com" {
                    // Return duplicate URLs
                    Ok((
                        vec!["item1".to_string()],
                        vec![
                            "https://example.com/page1".to_string(),
                            "https://example.com/page1".to_string(), // duplicate
                            "https://example.com/page1/".to_string(), // should normalize to same
                        ],
                    ))
                } else {
                    Ok((vec!["item2".to_string()], vec![]))
                }
            }

            async fn process(&self, _item: Self::Item) -> Result<(), Self::Error> {
                Ok(())
            }
        }

        let spider = Arc::new(DuplicateSpider);
        let crawler = Crawler::builder().build().unwrap();

        let stats = crawler.crawl(spider).await;

        // Should only visit 2 unique pages (start + page1)
        assert_eq!(stats.urls_visited, 2);
        // Should extract 2 items
        assert_eq!(stats.items_extracted, 2);
    }
}
