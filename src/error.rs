//! Error types for HTML extraction and parsing operations
//!
//! This module provides error types for handling failures during HTML extraction
//! and type conversion. All errors include contextual information to help debug
//! scraping issues.

/// Errors that can occur during HTML extraction
///
/// This error type represents all possible failures when extracting data from HTML.
/// Each variant includes contextual information about what went wrong.
///
/// # Examples
///
/// ## Handling Missing Fields
///
/// ```ignore
/// use scrapely::{Item, ExtractionError};
///
/// match Quote::extract(&element) {
///     Ok(quote) => println!("Extracted: {:?}", quote),
///     Err(ExtractionError::MissingField { field, selector }) => {
///         eprintln!("Field '{}' not found with selector '{}'", field, selector);
///     }
///     Err(e) => eprintln!("Other error: {}", e),
/// }
/// ```
///
/// ## Handling Parse Errors
///
/// ```ignore
/// match Quote::extract(&element) {
///     Ok(quote) => println!("Extracted: {:?}", quote),
///     Err(ExtractionError::ParseError { field, text, error }) => {
///         eprintln!("Failed to parse field '{}' from text '{}': {}", field, text, error);
///     }
///     Err(e) => eprintln!("Other error: {}", e),
/// }
/// ```
#[derive(Debug, thiserror::Error)]
pub enum ExtractionError {
    /// Failed to parse a CSS selector
    ///
    /// This error occurs when an invalid CSS selector is provided.
    #[error("Failed to parse selector '{selector}': {error}")]
    InvalidSelector { selector: String, error: String },

    /// Required field was not found in the HTML
    ///
    /// This error occurs when a required field's selector doesn't match any elements.
    /// Optional fields (Option<T>) will not trigger this error.
    #[error("Required field '{field}' not found using selector '{selector}'")]
    MissingField { field: String, selector: String },

    /// Failed to parse extracted text into the target type
    ///
    /// This error occurs when the extracted text cannot be converted to the field's type.
    /// The original text is included to help with debugging.
    #[error("Failed to parse field '{field}' from text '{text}': {error}")]
    ParseError {
        field: String,
        text: String,
        error: ParseError,
    },

    /// Multiple errors occurred during extraction
    ///
    /// This error can occur when extracting multiple items and multiple failures happen.
    #[error("Multiple errors occurred: {errors:?}")]
    Multiple { errors: Vec<ExtractionError> },
}

/// Errors that can occur when parsing text into Rust types
///
/// This error type represents failures when converting extracted text or attributes
/// into specific Rust types. Each variant includes the original text that failed to parse.
///
/// # Examples
///
/// ```ignore
/// use scrapely::{FromHtml, ParseError};
///
/// match i32::from_text("not a number") {
///     Ok(num) => println!("Parsed: {}", num),
///     Err(ParseError::InvalidNumber { text, error }) => {
///         eprintln!("Failed to parse '{}' as number: {}", text, error);
///     }
///     Err(e) => eprintln!("Other error: {}", e),
/// }
/// ```
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// Failed to parse a number
    ///
    /// This error occurs when parsing integers (i32, u32, i64, u64, etc.).
    /// The original text and underlying parse error are included.
    #[error("Invalid number: {text}")]
    InvalidNumber {
        text: String,
        #[source]
        error: std::num::ParseIntError,
    },

    /// Failed to parse a floating point number
    ///
    /// This error occurs when parsing floats (f32, f64).
    /// The original text and underlying parse error are included.
    #[error("Invalid float: {text}")]
    InvalidFloat {
        text: String,
        #[source]
        error: std::num::ParseFloatError,
    },

    /// Failed to parse a boolean value
    ///
    /// This error occurs when parsing booleans. Valid boolean values are:
    /// - true: "true", "1", "yes", "on"
    /// - false: "false", "0", "no", "off", ""
    #[error("Invalid boolean: {text}")]
    InvalidBool { text: String },

    /// Custom parsing error
    ///
    /// This error can be used for custom FromHtml implementations.
    #[error("Custom parse error: {message}")]
    Custom { message: String },
}
