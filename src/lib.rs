// Re-export the Item derive macro
pub use scrapely_macros::Item;

// Core modules
mod backend;
mod error;
mod item;

// Public exports
pub use backend::ElementRef;
pub use error::{ExtractionError, ParseError};
pub use item::Item as ItemTrait;
