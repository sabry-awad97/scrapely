use crate::{ElementRef, ExtractionError};

/// Trait for types that can be extracted from HTML elements
///
/// This trait provides a common interface for all scraped data structures.
/// It can be manually implemented or automatically derived using the `#[derive(Item)]` macro.
///
/// # Examples
///
/// ```ignore
/// use scrapely::{Item, ExtractionError, ElementRef};
///
/// #[derive(Item)]
/// #[item(selector = ".quote")]
/// struct Quote {
///     #[field(selector = "span.text")]
///     text: String,
///     
///     #[field(selector = "small.author")]
///     author: String,
/// }
/// ```
pub trait Item: Sized {
    /// Extract an instance of this type from an HTML element
    ///
    /// # Arguments
    /// * `element` - The HTML element to extract from
    ///
    /// # Returns
    /// * `Ok(Self)` - Successfully extracted item
    /// * `Err(ExtractionError)` - Failed to extract or parse
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use scraper::Html;
    /// use scrapely::Item;
    ///
    /// let html = Html::parse_fragment(r#"<div class="quote">...</div>"#);
    /// let element = html.root_element();
    /// let quote = Quote::extract(&element)?;
    /// ```
    fn extract<E>(element: &E) -> Result<Self, ExtractionError>
    where
        E: ElementRef;

    /// Extract multiple instances from a document or element
    ///
    /// # Arguments
    /// * `root` - The root element or document to search within
    /// * `selector` - CSS selector to find elements
    ///
    /// # Returns
    /// * `Ok(Vec<Self>)` - All successfully extracted items
    /// * `Err(ExtractionError)` - Failed to extract or parse
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use scraper::Html;
    /// use scrapely::Item;
    ///
    /// let html = Html::parse_document(r#"<html>...</html>"#);
    /// let quotes = Quote::extract_all(&html.root_element(), ".quote")?;
    /// ```
    fn extract_all<E>(root: &E, selector: &str) -> Result<Vec<Self>, ExtractionError>
    where
        E: ElementRef;
}
