use proc_macro2::TokenStream;
use syn::DeriveInput;

#[derive(Debug)]
pub struct Item {}

impl TryFrom<DeriveInput> for Item {
    type Error = syn::Error;

    fn try_from(_input: DeriveInput) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl quote::ToTokens for Item {
    fn to_tokens(&self, _tokens: &mut TokenStream) {}
}

impl Item {
    pub fn generate_impl(&self) -> TokenStream {
        quote::quote!(#self)
    }
}
