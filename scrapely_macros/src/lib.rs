use proc_macro_error::{abort_call_site, proc_macro_error};
use syn::parse_macro_input;

use item_derive::Item;

mod codegen;
mod internals;
mod item_derive;
mod parse;
mod types;
mod validate;

/// Derive macro for the Item trait
///
/// This macro automatically implements the `Item` trait for your struct, generating
/// code to extract data from HTML elements based on CSS selectors.
///
/// # Container Attributes
///
/// - `#[item(selector = "...")]` - CSS selector for the root element (required)
///
/// # Field Attributes
///
/// - `#[field(selector = "...")]` - CSS selector for the field (required)
/// - `#[field(attr = "...")]` - Extract from HTML attribute instead of text
/// - `#[field(default)]` - Use Default::default() if extraction fails
///
/// # Supported Field Types
///
/// - Primitives: `String`, `i32`, `u32`, `i64`, `u64`, `f32`, `f64`, `bool`
/// - Optional: `Option<T>` - Returns None if element is missing or parsing fails
/// - Multiple: `Vec<T>` - Extracts from multiple elements
/// - Nested: Any type implementing `Item` - Supports hierarchical data
///
/// # Examples
///
/// ## Basic Usage
///
/// ```ignore
/// use scrapely::Item;
///
/// #[derive(Item)]
/// #[item(selector = ".product")]
/// struct Product {
///     #[field(selector = "h2.title")]
///     name: String,
///     
///     #[field(selector = "span.price")]
///     price: f64,
/// }
/// ```
///
/// ## Optional Fields
///
/// ```ignore
/// #[derive(Item)]
/// #[item(selector = ".user")]
/// struct User {
///     #[field(selector = ".name")]
///     name: String,
///     
///     #[field(selector = ".email")]
///     email: Option<String>,  // Returns None if missing
/// }
/// ```
///
/// ## Attribute Extraction
///
/// ```ignore
/// #[derive(Item)]
/// #[item(selector = "a.link")]
/// struct Link {
///     #[field(selector = "self", attr = "href")]
///     url: String,
///     
///     #[field(selector = "self")]
///     text: String,
/// }
/// ```
///
/// ## Multiple Elements
///
/// ```ignore
/// #[derive(Item)]
/// #[item(selector = ".article")]
/// struct Article {
///     #[field(selector = "h1")]
///     title: String,
///     
///     #[field(selector = ".tag")]
///     tags: Vec<String>,  // Extracts all matching elements
/// }
/// ```
///
/// ## Nested Items
///
/// ```ignore
/// #[derive(Item)]
/// #[item(selector = ".comment")]
/// struct Comment {
///     #[field(selector = ".author")]
///     author: String,
///     
///     #[field(selector = ".text")]
///     text: String,
/// }
///
/// #[derive(Item)]
/// #[item(selector = ".post")]
/// struct Post {
///     #[field(selector = "h1")]
///     title: String,
///     
///     #[field(selector = ".comment")]
///     comments: Vec<Comment>,  // Nested Item extraction
/// }
/// ```
///
/// ## Default Values
///
/// ```ignore
/// #[derive(Item)]
/// #[item(selector = ".config")]
/// struct Config {
///     #[field(selector = ".name")]
///     name: String,
///     
///     #[field(selector = ".count", default)]
///     count: i32,  // Uses 0 if missing or invalid
/// }
/// ```
#[proc_macro_error]
#[proc_macro_derive(Item, attributes(item, field))]
pub fn item_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    match Item::try_from(input) {
        Ok(item) => item.generate_impl().into(),
        Err(err) => abort_call_site!(err),
    }
}
