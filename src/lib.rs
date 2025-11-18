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
pub use crawler::{Crawler, Spider};
pub use error::{ExtractionError, ParseError};
pub use extract::FromHtml;
pub use item::Item as ItemTrait;
