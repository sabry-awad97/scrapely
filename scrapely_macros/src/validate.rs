//! Validation logic for derive macro inputs

use syn::{Data, DeriveInput, Error, Result};

use crate::parse::{ContainerAttrs, FieldAttrs};

/// Validate that the derive macro is only used on structs
pub fn validate_struct_only(input: &DeriveInput) -> Result<()> {
    match &input.data {
        Data::Struct(_) => Ok(()),
        Data::Enum(_) => Err(Error::new_spanned(
            input,
            "Item can only be derived for structs, not enums",
        )),
        Data::Union(_) => Err(Error::new_spanned(
            input,
            "Item can only be derived for structs, not unions",
        )),
    }
}

/// Validate that container has a selector attribute
pub fn validate_container_selector(input: &DeriveInput, attrs: &ContainerAttrs) -> Result<()> {
    if attrs.selector.is_none() {
        return Err(Error::new_spanned(
            input,
            "Item derive requires a selector attribute: #[item(selector = \"...\")]",
        ));
    }
    Ok(())
}

/// Validate that field has a selector attribute
pub fn validate_field_selector(field: &syn::Field, attrs: &FieldAttrs) -> Result<()> {
    if attrs.selector.is_none() {
        let field_name = field
            .ident
            .as_ref()
            .map(|i| i.to_string())
            .unwrap_or_else(|| "unnamed field".to_string());

        return Err(Error::new_spanned(
            field,
            format!(
                "Field '{}' requires a selector attribute: #[field(selector = \"...\")]",
                field_name
            ),
        ));
    }
    Ok(())
}

/// Validate that field doesn't have conflicting attributes
pub fn validate_no_conflicts(field: &syn::Field, attrs: &FieldAttrs) -> Result<()> {
    // Check for conflicting attr and default
    if attrs.attr.is_some() && attrs.default {
        let field_name = field
            .ident
            .as_ref()
            .map(|i| i.to_string())
            .unwrap_or_else(|| "unnamed field".to_string());

        return Err(Error::new_spanned(
            field,
            format!(
                "Field '{}' cannot have both 'attr' and 'default' attributes",
                field_name
            ),
        ));
    }

    Ok(())
}
