//! HTML backend abstraction for supporting multiple HTML parsing libraries
//!
//! This module provides the `ElementRef` trait which abstracts over different HTML
//! parsing libraries, allowing the Item trait to work with any HTML parser that
//! implements this interface.

/// Trait representing an HTML element that can be queried with CSS selectors
///
/// This trait abstracts over different HTML parsing backends (scraper, select, etc.)
/// allowing the Item trait to work with any HTML parser that implements this interface.
///
/// # Examples
///
/// ```ignore
/// use scrapely::ElementRef;
/// use scraper::Html;
///
/// let html = Html::parse_fragment(r#"
///     <div class="container">
///         <h1>Title</h1>
///         <p class="text">Content</p>
///         <a href="/link">Link</a>
///     </div>
/// "#);
///
/// let element = html.root_element();
///
/// // Select a single element
/// if let Some(title) = element.select_one("h1") {
///     assert_eq!(title.text(), "Title");
/// }
///
/// // Select multiple elements
/// let paragraphs = element.select_all("p");
/// assert_eq!(paragraphs.len(), 1);
///
/// // Get attribute value
/// if let Some(link) = element.select_one("a") {
///     assert_eq!(link.attr("href"), Some("/link"));
/// }
/// ```
pub trait ElementRef {
    /// Select a single child element matching the CSS selector
    ///
    /// Returns `None` if no element matches or if the selector is invalid.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let element = html.root_element();
    /// let title = element.select_one("h1.title")?;
    /// ```
    fn select_one(&self, selector: &str) -> Option<Self>
    where
        Self: Sized;

    /// Select all child elements matching the CSS selector
    ///
    /// Returns an empty vector if no elements match or if the selector is invalid.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let element = html.root_element();
    /// let items = element.select_all("li.item");
    /// for item in items {
    ///     println!("{}", item.text());
    /// }
    /// ```
    fn select_all(&self, selector: &str) -> Vec<Self>
    where
        Self: Sized;

    /// Get the text content of this element
    ///
    /// This includes all text from child elements, concatenated together.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let element = html.root_element();
    /// let text = element.text();
    /// ```
    fn text(&self) -> String;

    /// Get the value of an HTML attribute
    ///
    /// Returns `None` if the attribute doesn't exist.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let link = element.select_one("a")?;
    /// if let Some(href) = link.attr("href") {
    ///     println!("Link: {}", href);
    /// }
    /// ```
    fn attr(&self, name: &str) -> Option<&str>;

    /// Get the inner HTML of this element
    ///
    /// Returns the HTML content inside this element, excluding the element's own tags.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let div = element.select_one("div")?;
    /// let html = div.inner_html();
    /// ```
    fn inner_html(&self) -> String;
}

/// Implementation of ElementRef for scraper::ElementRef
impl<'a> ElementRef for scraper::ElementRef<'a> {
    fn select_one(&self, selector: &str) -> Option<Self> {
        let selector = scraper::Selector::parse(selector).ok()?;
        self.select(&selector).next()
    }

    fn select_all(&self, selector: &str) -> Vec<Self> {
        let selector = match scraper::Selector::parse(selector) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        self.select(&selector).collect()
    }

    fn text(&self) -> String {
        self.text().collect()
    }

    fn attr(&self, name: &str) -> Option<&str> {
        self.value().attr(name)
    }

    fn inner_html(&self) -> String {
        self.inner_html()
    }
}
