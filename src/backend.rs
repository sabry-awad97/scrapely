//! HTML backend abstraction for supporting multiple HTML parsing libraries

/// Trait representing an HTML element that can be queried with CSS selectors
///
/// This trait abstracts over different HTML parsing backends (scraper, select, etc.)
/// allowing the Item trait to work with any HTML parser that implements this interface.
pub trait ElementRef {
    /// Select a single child element matching the CSS selector
    ///
    /// Returns `None` if no element matches or if the selector is invalid.
    fn select_one(&self, selector: &str) -> Option<Self>
    where
        Self: Sized;

    /// Select all child elements matching the CSS selector
    ///
    /// Returns an empty vector if no elements match or if the selector is invalid.
    fn select_all(&self, selector: &str) -> Vec<Self>
    where
        Self: Sized;

    /// Get the text content of this element
    ///
    /// This includes all text from child elements.
    fn text(&self) -> String;

    /// Get the value of an HTML attribute
    ///
    /// Returns `None` if the attribute doesn't exist.
    fn attr(&self, name: &str) -> Option<&str>;

    /// Get the inner HTML of this element
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
