use crate::{ElementRef, ExtractionError};

/// Trait for types that can be extracted from HTML elements
///
/// This trait provides a common interface for all scraped data structures.
/// It can be manually implemented or automatically derived using the `#[derive(Item)]` macro.
///
/// # Deriving Item
///
/// The easiest way to use this trait is to derive it on your struct:
///
/// ```ignore
/// use scrapely::Item;
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
///
/// # Manual Implementation
///
/// You can also implement the trait manually for custom extraction logic:
///
/// ```ignore
/// use scrapely::{Item, ElementRef, ExtractionError, FromHtml};
///
/// struct CustomItem {
///     value: String,
/// }
///
/// impl Item for CustomItem {
///     fn extract<E>(element: &E) -> Result<Self, ExtractionError>
///     where
///         E: ElementRef,
///     {
///         let elem = element
///             .select_one(".value")
///             .ok_or_else(|| ExtractionError::MissingField {
///                 field: "value".to_string(),
///                 selector: ".value".to_string(),
///             })?;
///         
///         let text = elem.text();
///         let value = String::from_text(&text)
///             .map_err(|error| ExtractionError::ParseError {
///                 field: "value".to_string(),
///                 text: text.clone(),
///                 error,
///             })?;
///         
///         Ok(CustomItem { value })
///     }
///     
///     fn extract_all<E>(root: &E, selector: &str) -> Result<Vec<Self>, ExtractionError>
///     where
///         E: ElementRef,
///     {
///         let elements = root.select_all(selector);
///         elements.iter().map(|elem| Self::extract(elem)).collect()
///     }
/// }
/// ```
///
/// # Usage
///
/// Once you have a type that implements Item, you can extract data from HTML:
///
/// ```ignore
/// use scraper::Html;
/// use scrapely::Item;
///
/// let html = Html::parse_document(r#"
///     <div class="quote">
///         <span class="text">Hello, world!</span>
///         <small class="author">John Doe</small>
///     </div>
/// "#);
///
/// let quote = Quote::extract(&html.root_element())?;
/// println!("{}: {}", quote.author, quote.text);
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
    /// Uses the selector specified in `#[item(selector = "...")]` to find all matching elements.
    ///
    /// # Arguments
    /// * `root` - The root element or document to search within
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
    /// let quotes = Quote::extract_all(&html.root_element())?;
    /// ```
    fn extract_all<E>(root: &E) -> Result<Vec<Self>, ExtractionError>
    where
        E: ElementRef;
}
