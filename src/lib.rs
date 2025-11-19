// Re-export the Item derive macro
pub use scrapely_macros::Item;

// Core modules
mod backend;
pub mod crawler;
mod error;
mod extract;
mod item;

// Public exports
pub use backend::ElementRef;
pub use crawler::{
    CompletionDetector, ConfigError, CrawlError, CrawlObserver, CrawlStats, Crawler,
    CrawlerBuilder, DelayLimiter, ObserverRegistry, RateLimiter, Spider, StatsTracker,
    TokenBucketLimiter, UrlNormalizer, VisitResult,
};
pub use error::{ExtractionError, ParseError};
pub use extract::FromHtml;
pub use item::Item as ItemTrait;
