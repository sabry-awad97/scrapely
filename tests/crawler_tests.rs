use scrapely::*;
use std::sync::Arc;
use tokio::time::Duration;

#[cfg(test)]
mod completion_detector_tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let detector = CompletionDetector::new();
        assert_eq!(detector.pending_count(), 0);
        assert_eq!(detector.active_count(), 0);
    }

    #[test]
    fn test_url_queued_increments() {
        let detector = CompletionDetector::new();
        detector.url_queued();
        assert_eq!(detector.pending_count(), 1);

        detector.url_queued();
        detector.url_queued();
        assert_eq!(detector.pending_count(), 3);
    }

    #[test]
    fn test_url_completed_decrements() {
        let detector = CompletionDetector::new();
        detector.url_queued();
        detector.url_queued();

        detector.url_completed();
        assert_eq!(detector.pending_count(), 1);

        detector.url_completed();
        assert_eq!(detector.pending_count(), 0);
    }

    #[test]
    fn test_scraper_tracking() {
        let detector = CompletionDetector::new();

        detector.scraper_started();
        assert_eq!(detector.active_count(), 1);

        detector.scraper_started();
        detector.scraper_started();
        assert_eq!(detector.active_count(), 3);

        detector.scraper_finished();
        assert_eq!(detector.active_count(), 2);
    }

    #[tokio::test]
    async fn test_completion_signaled() {
        let detector = CompletionDetector::new();

        // Queue and start work
        detector.url_queued();
        detector.scraper_started();

        // Complete work in background
        let detector_clone = detector.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            detector_clone.scraper_finished();
            detector_clone.url_completed();
        });

        // Wait for completion (should complete within timeout)
        let result =
            tokio::time::timeout(Duration::from_secs(1), detector.wait_for_completion()).await;

        assert!(result.is_ok(), "Completion should be signaled");
    }

    #[tokio::test]
    async fn test_completion_not_signaled_with_pending_work() {
        let detector = CompletionDetector::new();

        detector.url_queued();
        detector.scraper_started();

        // Only finish scraper, not URL
        let detector_clone = detector.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            detector_clone.scraper_finished();
            // Don't complete URL - work still pending
        });

        // Should timeout because work is still pending
        let result =
            tokio::time::timeout(Duration::from_millis(200), detector.wait_for_completion()).await;

        assert!(result.is_err(), "Should timeout with pending work");
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let detector = Arc::new(CompletionDetector::new());

        // Spawn multiple tasks that queue and complete work
        let mut handles = vec![];
        for _ in 0..10 {
            let d = detector.clone();
            handles.push(tokio::spawn(async move {
                d.url_queued();
                d.scraper_started();
                tokio::time::sleep(Duration::from_millis(10)).await;
                d.scraper_finished();
                d.url_completed();
            }));
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap();
        }

        // All work should be complete
        assert_eq!(detector.pending_count(), 0);
        assert_eq!(detector.active_count(), 0);
    }
}

#[cfg(test)]
mod stats_tracker_tests {
    use super::*;

    #[test]
    fn test_initial_stats() {
        let tracker = StatsTracker::new();
        let stats = tracker.snapshot();

        assert_eq!(stats.urls_visited, 0);
        assert_eq!(stats.items_extracted, 0);
        assert_eq!(stats.errors_encountered, 0);
    }

    #[test]
    fn test_url_visited_increments() {
        let tracker = StatsTracker::new();

        tracker.url_visited();
        tracker.url_visited();
        tracker.url_visited();

        let stats = tracker.snapshot();
        assert_eq!(stats.urls_visited, 3);
    }

    #[test]
    fn test_item_extracted_increments() {
        let tracker = StatsTracker::new();

        tracker.item_extracted();
        tracker.item_extracted();

        let stats = tracker.snapshot();
        assert_eq!(stats.items_extracted, 2);
    }

    #[test]
    fn test_error_encountered_increments() {
        let tracker = StatsTracker::new();

        tracker.error_encountered();

        let stats = tracker.snapshot();
        assert_eq!(stats.errors_encountered, 1);
    }

    #[test]
    fn test_concurrent_updates() {
        use std::sync::Arc;
        use std::thread;

        let tracker = Arc::new(StatsTracker::new());
        let mut handles = vec![];

        // Spawn threads that update stats concurrently
        for _ in 0..10 {
            let t = tracker.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    t.url_visited();
                    t.item_extracted();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = tracker.snapshot();
        assert_eq!(stats.urls_visited, 1000);
        assert_eq!(stats.items_extracted, 1000);
    }

    #[tokio::test]
    async fn test_stats_broadcast() {
        let tracker = StatsTracker::new();
        let rx = tracker.subscribe();

        // Update stats
        tracker.url_visited();
        tracker.item_extracted();

        // Should receive updated stats
        tokio::time::sleep(Duration::from_millis(10)).await;

        let stats = rx.borrow().clone();
        assert!(stats.urls_visited > 0 || stats.items_extracted > 0);
    }
}

#[cfg(test)]
mod visit_result_tests {
    use super::*;

    #[test]
    fn test_new_visit_result() {
        let result = VisitResult::new(
            "https://example.com".to_string(),
            vec!["https://example.com/page1".to_string()],
        );

        assert_eq!(result.visited_url, "https://example.com");
        assert_eq!(result.discovered_urls.len(), 1);
    }

    #[test]
    fn test_empty_visit_result() {
        let result = VisitResult::empty("https://example.com".to_string());

        assert_eq!(result.visited_url, "https://example.com");
        assert_eq!(result.discovered_urls.len(), 0);
    }
}

#[cfg(test)]
mod observer_tests {
    use super::*;
    use std::sync::Mutex;

    struct TestObserver {
        urls_queued: Arc<Mutex<Vec<String>>>,
        urls_visited: Arc<Mutex<Vec<String>>>,
        errors: Arc<Mutex<Vec<String>>>,
    }

    impl TestObserver {
        fn new() -> Self {
            Self {
                urls_queued: Arc::new(Mutex::new(Vec::new())),
                urls_visited: Arc::new(Mutex::new(Vec::new())),
                errors: Arc::new(Mutex::new(Vec::new())),
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

        async fn on_scrape_error(&self, url: &str, error: &str) {
            self.errors
                .lock()
                .unwrap()
                .push(format!("{}: {}", url, error));
        }
    }

    #[tokio::test]
    async fn test_observer_registry() {
        let observer = Arc::new(TestObserver::new());
        let mut registry = ObserverRegistry::new();
        registry.register(observer.clone());

        // Test notifications
        registry.notify_url_queued("https://example.com").await;

        let result = VisitResult::new("https://example.com".to_string(), vec![]);
        registry.notify_url_visited(&result).await;

        registry
            .notify_scrape_error("https://example.com", "test error")
            .await;

        // Verify observer received events
        assert_eq!(observer.urls_queued.lock().unwrap().len(), 1);
        assert_eq!(observer.urls_visited.lock().unwrap().len(), 1);
        assert_eq!(observer.errors.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_multiple_observers() {
        let observer1 = Arc::new(TestObserver::new());
        let observer2 = Arc::new(TestObserver::new());

        let mut registry = ObserverRegistry::new();
        registry.register(observer1.clone());
        registry.register(observer2.clone());

        registry.notify_url_queued("https://example.com").await;

        // Both observers should receive the event
        assert_eq!(observer1.urls_queued.lock().unwrap().len(), 1);
        assert_eq!(observer2.urls_queued.lock().unwrap().len(), 1);
    }
}
