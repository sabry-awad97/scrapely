//! Type conversion traits for extracting Rust types from HTML content
//!
//! This module provides the `FromHtml` trait which defines how to convert
//! HTML text content and attributes into Rust types.

use crate::ParseError;

/// Trait for types that can be parsed from HTML text or attributes
///
/// This trait provides a common interface for converting HTML content into Rust types.
/// It supports extraction from both text content and HTML attributes.
///
/// # Examples
///
/// ```ignore
/// use scrapely::FromHtml;
///
/// // Parse from text content
/// let number: i32 = FromHtml::from_text("42")?;
/// assert_eq!(number, 42);
///
/// // Parse from attribute value
/// let url: String = FromHtml::from_attr("https://example.com")?;
/// assert_eq!(url, "https://example.com");
/// ```
///
/// # Implementing FromHtml
///
/// ```ignore
/// use scrapely::{FromHtml, ParseError};
///
/// struct CustomType {
///     value: String,
/// }
///
/// impl FromHtml for CustomType {
///     fn from_text(text: &str) -> Result<Self, ParseError> {
///         Ok(CustomType {
///             value: text.trim().to_string(),
///         })
///     }
///
///     fn from_attr(attr: &str) -> Result<Self, ParseError> {
///         Self::from_text(attr)
///     }
/// }
/// ```
pub trait FromHtml: Sized {
    /// Parse a value from HTML text content
    ///
    /// This method is called when extracting from element text content.
    ///
    /// # Arguments
    /// * `text` - The text content to parse
    ///
    /// # Returns
    /// * `Ok(Self)` - Successfully parsed value
    /// * `Err(ParseError)` - Failed to parse the text
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let value = i32::from_text("123")?;
    /// assert_eq!(value, 123);
    /// ```
    fn from_text(text: &str) -> Result<Self, ParseError>;

    /// Parse a value from an HTML attribute
    ///
    /// This method is called when extracting from element attributes.
    /// By default, it delegates to `from_text`.
    ///
    /// # Arguments
    /// * `attr` - The attribute value to parse
    ///
    /// # Returns
    /// * `Ok(Self)` - Successfully parsed value
    /// * `Err(ParseError)` - Failed to parse the attribute
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let url = String::from_attr("https://example.com")?;
    /// assert_eq!(url, "https://example.com");
    /// ```
    fn from_attr(attr: &str) -> Result<Self, ParseError> {
        Self::from_text(attr)
    }
}

// Implementation for String - no parsing needed, just trim whitespace
impl FromHtml for String {
    fn from_text(text: &str) -> Result<Self, ParseError> {
        Ok(text.trim().to_string())
    }
}

// Implementation for i32
impl FromHtml for i32 {
    fn from_text(text: &str) -> Result<Self, ParseError> {
        text.trim()
            .parse()
            .map_err(|error| ParseError::InvalidNumber {
                text: text.to_string(),
                error,
            })
    }
}

// Implementation for u32
impl FromHtml for u32 {
    fn from_text(text: &str) -> Result<Self, ParseError> {
        text.trim()
            .parse()
            .map_err(|error| ParseError::InvalidNumber {
                text: text.to_string(),
                error,
            })
    }
}

// Implementation for i64
impl FromHtml for i64 {
    fn from_text(text: &str) -> Result<Self, ParseError> {
        text.trim()
            .parse()
            .map_err(|error| ParseError::InvalidNumber {
                text: text.to_string(),
                error,
            })
    }
}

// Implementation for u64
impl FromHtml for u64 {
    fn from_text(text: &str) -> Result<Self, ParseError> {
        text.trim()
            .parse()
            .map_err(|error| ParseError::InvalidNumber {
                text: text.to_string(),
                error,
            })
    }
}

// Implementation for f32
impl FromHtml for f32 {
    fn from_text(text: &str) -> Result<Self, ParseError> {
        text.trim()
            .parse()
            .map_err(|error| ParseError::InvalidFloat {
                text: text.to_string(),
                error,
            })
    }
}

// Implementation for f64
impl FromHtml for f64 {
    fn from_text(text: &str) -> Result<Self, ParseError> {
        text.trim()
            .parse()
            .map_err(|error| ParseError::InvalidFloat {
                text: text.to_string(),
                error,
            })
    }
}

// Implementation for bool
impl FromHtml for bool {
    fn from_text(text: &str) -> Result<Self, ParseError> {
        let trimmed = text.trim().to_lowercase();
        match trimmed.as_str() {
            "true" | "1" | "yes" | "on" => Ok(true),
            "false" | "0" | "no" | "off" | "" => Ok(false),
            _ => Err(ParseError::InvalidBool {
                text: text.to_string(),
            }),
        }
    }
}

// Implementation for Option<T> - returns None on parse failure instead of error
impl<T: FromHtml> FromHtml for Option<T> {
    fn from_text(text: &str) -> Result<Self, ParseError> {
        // If text is empty or whitespace, return None
        if text.trim().is_empty() {
            return Ok(None);
        }

        // Try to parse the inner type, return None on failure
        match T::from_text(text) {
            Ok(value) => Ok(Some(value)),
            Err(_) => Ok(None),
        }
    }

    fn from_attr(attr: &str) -> Result<Self, ParseError> {
        // If attribute is empty, return None
        if attr.trim().is_empty() {
            return Ok(None);
        }

        // Try to parse the inner type, return None on failure
        match T::from_attr(attr) {
            Ok(value) => Ok(Some(value)),
            Err(_) => Ok(None),
        }
    }
}

// Implementation for Vec<T> - parses comma-separated values
impl<T: FromHtml> FromHtml for Vec<T> {
    fn from_text(text: &str) -> Result<Self, ParseError> {
        // If text is empty, return empty vector
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }

        // Split by comma and parse each value
        text.split(',')
            .map(|s| T::from_text(s.trim()))
            .collect::<Result<Vec<_>, _>>()
    }

    fn from_attr(attr: &str) -> Result<Self, ParseError> {
        // If attribute is empty, return empty vector
        if attr.trim().is_empty() {
            return Ok(Vec::new());
        }

        // Split by comma and parse each value
        attr.split(',')
            .map(|s| T::from_attr(s.trim()))
            .collect::<Result<Vec<_>, _>>()
    }
}
