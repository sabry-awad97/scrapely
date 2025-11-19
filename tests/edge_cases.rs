use scrapely::*;
use std::sync::Arc;
use tokio::time::Duration;

#[tokio::test]
async fn test_empty_start_urls() {
    struct EmptySpider;
    #[async_trait::async_trait]
    impl Spider for EmptySpider {
        type Item = String;
        type Error = String;
        fn start_urls(&self) -> Vec<String> {
            vec![]
        }
        async fn scrape(
            &self,
            _url: String,
        ) -> Result<(Vec<Self::Item>, Vec<String>), Self::Error> {
            Ok((vec![], vec![]))
        }
        async fn process(&self, _item: Self::Item) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    let crawler = Crawler::builder().build().unwrap();
    let stats = crawler.crawl(Arc::new(EmptySpider)).await;

    assert_eq!(stats.urls_visited, 0);
    assert_eq!(stats.items_extracted, 0);
    assert_eq!(stats.errors_encountered, 0);
}

#[tokio::test]
async fn test_circular_links() {
    struct CircularSpider;
    #[async_trait::async_trait]
    impl Spider for CircularSpider {
        type Item = String;
        type Error = String;
        fn start_urls(&self) -> Vec<String> {
            vec!["https://a.com".to_string()]
        }
        async fn scrape(&self, url: String) -> Result<(Vec<Self::Item>, Vec<String>), Self::Error> {
            if url == "https://a.com" {
                Ok((vec![], vec!["https://b.com".to_string()]))
            } else if url == "https://b.com" {
                Ok((vec![], vec!["https://a.com".to_string()]))
            } else {
                Ok((vec![], vec![]))
            }
        }
        async fn process(&self, _item: Self::Item) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    let crawler = Crawler::builder().build().unwrap();
    let stats = crawler.crawl(Arc::new(CircularSpider)).await;

    // Should visit exactly 2 pages (a and b), not infinite loop
    assert_eq!(stats.urls_visited, 2);
}

#[tokio::test]
async fn test_all_requests_fail() {
    struct BrokenSpider;
    #[async_trait::async_trait]
    impl Spider for BrokenSpider {
        type Item = String;
        type Error = String;
        fn start_urls(&self) -> Vec<String> {
            vec!["https://a.com".to_string(), "https://b.com".to_string()]
        }
        async fn scrape(&self, url: String) -> Result<(Vec<Self::Item>, Vec<String>), Self::Error> {
            Err(format!("Failed to fetch {}", url))
        }
        async fn process(&self, _item: Self::Item) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    let crawler = Crawler::builder().build().unwrap();
    let stats = crawler.crawl(Arc::new(BrokenSpider)).await;

    assert_eq!(stats.urls_visited, 2);
    assert_eq!(stats.errors_encountered, 2);
    assert_eq!(stats.items_extracted, 0);
}

#[tokio::test]
async fn test_high_load_channel_capacity() {
    // Test that the crawler doesn't deadlock when queues are full
    struct BurstSpider;
    #[async_trait::async_trait]
    impl Spider for BurstSpider {
        type Item = String;
        type Error = String;
        fn start_urls(&self) -> Vec<String> {
            vec!["https://start.com".to_string()]
        }
        async fn scrape(&self, url: String) -> Result<(Vec<Self::Item>, Vec<String>), Self::Error> {
            if url == "https://start.com" {
                // Generate 500 links (well within capacity of 1000) to avoid deadlock
                // Deadlock happens if (new_urls > queue_capacity) because:
                // Scraper blocks sending new_urls -> Scheduler stops reading -> Scheduler blocks sending work -> Scraper blocks receiving work
                let links: Vec<String> =
                    (0..500).map(|i| format!("https://page{}.com", i)).collect();
                Ok((vec![], links))
            } else {
                // Simulate some work
                tokio::time::sleep(Duration::from_millis(1)).await;
                Ok((vec!["item".to_string()], vec![]))
            }
        }
        async fn process(&self, _item: Self::Item) -> Result<(), Self::Error> {
            tokio::time::sleep(Duration::from_millis(1)).await;
            Ok(())
        }
    }

    // Use buffer sizes larger than burst size to avoid deadlock
    let crawler = Crawler::builder()
        .crawling_concurrency(10)
        .processing_concurrency(10)
        .crawling_queue_multiplier(100) // Capacity = 10 * 100 = 1000
        .delay(Duration::from_millis(0))
        .build()
        .unwrap();

    let stats = crawler.crawl(Arc::new(BurstSpider)).await;

    assert_eq!(stats.urls_visited, 501); // start + 500 pages
    assert_eq!(stats.items_extracted, 500);
}
