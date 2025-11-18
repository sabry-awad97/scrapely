use proc_macro_error::{abort_call_site, proc_macro_error};
use syn::parse_macro_input;

use item_derive::Item;

mod internals;
mod item_derive;

#[proc_macro_error]
#[proc_macro_derive(Item, attributes(item, field))]
pub fn item_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    match Item::try_from(input) {
        Ok(item) => item.generate_impl().into(),
        Err(err) => abort_call_site!(err),
    }
}
