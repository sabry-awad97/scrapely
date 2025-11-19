use scrapely::*;

#[cfg(test)]
mod completion_tests {
    use super::*;

    // **Feature: crawler-enhancements, Property 6: Pending counter tracks queued URLs**
    // **Validates: Requirements 2.1, 2.2**
    #[test]
    fn test_pending_counter_accuracy() {
        let detector = CompletionDetector::new();

        // Queue 10 URLs
        for _ in 0..10 {
            detector.url_queued();
        }

        // Check pending count
        let pending = detector.pending_count();
        assert_eq!(
            pending, 10,
            "Pending count should be 10 after queueing 10 URLs"
        );

        // Complete 5 URLs
        for _ in 0..5 {
            detector.url_completed();
        }

        let pending_after = detector.pending_count();
        assert_eq!(
            pending_after, 5,
            "Pending count should be 5 after completing 5 URLs"
        );

        // Complete remaining URLs
        for _ in 0..5 {
            detector.url_completed();
        }

        let final_pending = detector.pending_count();
        assert_eq!(
            final_pending, 0,
            "Pending count should be 0 after completing all URLs"
        );
    }

    // **Feature: crawler-enhancements, Property 7: Completion detection accuracy**
    // **Validates: Requirements 2.3**
    #[tokio::test]
    async fn test_completion_detection() {
        let detector = CompletionDetector::new();

        // Queue and start processing a URL
        detector.url_queued();
        detector.scraper_started();

        // Spawn a task to complete the work
        let detector_clone = detector.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            detector_clone.scraper_finished();
            detector_clone.url_completed();
        });

        // Wait for completion
        tokio::time::timeout(
            tokio::time::Duration::from_secs(1),
            detector.wait_for_completion(),
        )
        .await
        .expect("Completion should be signaled within 1 second");

        // Verify final state
        assert_eq!(detector.pending_count(), 0);
        assert_eq!(detector.active_count(), 0);
    }

    #[test]
    fn test_scraper_tracking() {
        let detector = CompletionDetector::new();

        // Start 3 scrapers
        for _ in 0..3 {
            detector.scraper_started();
        }

        let active = detector.active_count();
        assert_eq!(active, 3, "Should have 3 active scrapers");

        // Finish 2 scrapers
        for _ in 0..2 {
            detector.scraper_finished();
        }

        let active_after = detector.active_count();
        assert_eq!(active_after, 1, "Should have 1 active scraper");
    }
}

#[cfg(test)]
mod proptest_completion {
    use super::*;
    use proptest::prelude::*;

    // **Feature: crawler-enhancements, Property 6: Pending counter tracks queued URLs**
    // **Validates: Requirements 2.1, 2.2**
    proptest! {
        #[test]
        fn prop_pending_counter_tracks_operations(
            queue_count in 1usize..100,
            complete_count in 0usize..100
        ) {
            let detector = CompletionDetector::new();

            // Queue URLs
            for _ in 0..queue_count {
                detector.url_queued();
            }

            let pending_after_queue = detector.pending_count();
            prop_assert_eq!(pending_after_queue, queue_count);

            // Complete some URLs (but not more than queued)
            let actual_complete = complete_count.min(queue_count);
            for _ in 0..actual_complete {
                detector.url_completed();
            }

            let pending_after_complete = detector.pending_count();
            prop_assert_eq!(pending_after_complete, queue_count - actual_complete);
        }
    }
}
