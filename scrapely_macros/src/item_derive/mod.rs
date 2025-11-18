use proc_macro2::TokenStream;
use syn::{DeriveInput, Result};

use crate::codegen;
use crate::parse::ContainerAttrs;
use crate::validate;

#[derive(Debug)]
pub struct Item {
    input: DeriveInput,
    container_attrs: ContainerAttrs,
}

impl TryFrom<DeriveInput> for Item {
    type Error = syn::Error;

    fn try_from(input: DeriveInput) -> Result<Self> {
        // Validate that it's a struct
        validate::validate_struct_only(&input)?;

        // Parse container attributes
        let container_attrs = ContainerAttrs::from_attributes(&input.attrs)?;

        // Validate container has selector
        validate::validate_container_selector(&input, &container_attrs)?;

        Ok(Self {
            input,
            container_attrs,
        })
    }
}

impl Item {
    pub fn generate_impl(&self) -> TokenStream {
        codegen::generate_item_impl(&self.input, &self.container_attrs)
    }
}
