use scrapely::{ConfigError, Crawler};
use std::time::Duration;

#[test]
fn test_zero_crawling_concurrency_rejected() {
    let result = Crawler::builder().crawling_concurrency(0).build();

    assert!(result.is_err());
    match result {
        Err(ConfigError::InvalidCrawlingConcurrency(0)) => {}
        _ => panic!("Expected InvalidCrawlingConcurrency error"),
    }
}

#[test]
fn test_zero_processing_concurrency_rejected() {
    let result = Crawler::builder().processing_concurrency(0).build();

    assert!(result.is_err());
    match result {
        Err(ConfigError::InvalidProcessingConcurrency(0)) => {}
        _ => panic!("Expected InvalidProcessingConcurrency error"),
    }
}

#[test]
fn test_zero_crawling_queue_multiplier_rejected() {
    let result = Crawler::builder().crawling_queue_multiplier(0).build();

    assert!(result.is_err());
    match result {
        Err(ConfigError::InvalidQueueMultiplier(0)) => {}
        _ => panic!("Expected InvalidQueueMultiplier error"),
    }
}

#[test]
fn test_zero_processing_queue_multiplier_rejected() {
    let result = Crawler::builder().processing_queue_multiplier(0).build();

    assert!(result.is_err());
    match result {
        Err(ConfigError::InvalidQueueMultiplier(0)) => {}
        _ => panic!("Expected InvalidQueueMultiplier error"),
    }
}

#[test]
fn test_valid_configuration_accepted() {
    let result = Crawler::builder()
        .crawling_concurrency(4)
        .processing_concurrency(2)
        .crawling_queue_multiplier(100)
        .processing_queue_multiplier(20)
        .delay(Duration::from_millis(100))
        .build();

    assert!(result.is_ok());
}

#[test]
fn test_default_configuration_valid() {
    let result = Crawler::builder().build();
    assert!(result.is_ok());
}

#[test]
fn test_rate_limit_configuration() {
    let result = Crawler::builder().rate_limit(10.0).build();

    assert!(result.is_ok());
}
