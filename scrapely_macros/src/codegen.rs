//! Code generation for Item trait implementation

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Ident};

use crate::parse::{ContainerAttrs, FieldAttrs};

/// Generate the complete Item trait implementation
pub fn generate_item_impl(input: &DeriveInput, _container_attrs: &ContainerAttrs) -> TokenStream {
    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Generate field extraction code
    let field_extractions = generate_field_extractions(input);

    quote! {
        impl #impl_generics scrapely::ItemTrait for #name #ty_generics #where_clause {
            fn extract<E>(element: &E) -> Result<Self, scrapely::ExtractionError>
            where
                E: scrapely::ElementRef,
            {
                #field_extractions
            }

            fn extract_all<E>(root: &E, selector: &str) -> Result<Vec<Self>, scrapely::ExtractionError>
            where
                E: scrapely::ElementRef,
            {
                let elements = root.select_all(selector);
                elements
                    .iter()
                    .map(|elem| Self::extract(elem))
                    .collect()
            }
        }
    }
}

/// Generate field extraction code for all fields
fn generate_field_extractions(input: &DeriveInput) -> TokenStream {
    match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => {
                let mut extractions = Vec::new();
                let mut field_names = Vec::new();

                for field in &fields.named {
                    let field_name = field.ident.as_ref().unwrap();
                    let field_type = &field.ty;

                    // Parse field attributes
                    let field_attrs = match FieldAttrs::from_attributes(&field.attrs) {
                        Ok(attrs) => attrs,
                        Err(_) => continue,
                    };

                    // Validate field has selector
                    if crate::validate::validate_field_selector(field, &field_attrs).is_err() {
                        continue;
                    }

                    // Generate extraction code
                    let extraction =
                        generate_field_extraction(field_name, field_type, &field_attrs);
                    extractions.push(extraction);
                    field_names.push(field_name);
                }

                quote! {
                    #(#extractions)*

                    Ok(Self {
                        #(#field_names),*
                    })
                }
            }
            _ => quote! {
                Ok(Self {})
            },
        },
        _ => quote! {
            Ok(Self {})
        },
    }
}

/// Generate extraction code for a single field
pub fn generate_field_extraction(
    field_name: &Ident,
    field_type: &syn::Type,
    attrs: &FieldAttrs,
) -> TokenStream {
    let selector = attrs.selector.as_ref().unwrap();
    let field_name_str = field_name.to_string();

    // Check if field is Option<T>
    let is_optional = crate::types::is_option(field_type);

    // Check if extracting from attribute or text
    if let Some(attr_name) = &attrs.attr {
        // Extract from HTML attribute
        if is_optional {
            // Optional field - return None on missing element or attribute
            quote! {
                let #field_name = element
                    .select_one(#selector)
                    .and_then(|elem| elem.attr(#attr_name))
                    .and_then(|attr_value| {
                        <#field_type as scrapely::FromHtml>::from_attr(attr_value).ok()
                    });
            }
        } else {
            // Required field
            quote! {
                let #field_name = {
                    let elem = element
                        .select_one(#selector)
                        .ok_or_else(|| scrapely::ExtractionError::MissingField {
                            field: #field_name_str.to_string(),
                            selector: #selector.to_string(),
                        })?;

                    let attr_value = elem
                        .attr(#attr_name)
                        .ok_or_else(|| scrapely::ExtractionError::MissingField {
                            field: #field_name_str.to_string(),
                            selector: format!("{}[{}]", #selector, #attr_name),
                        })?;

                    <#field_type as scrapely::FromHtml>::from_attr(attr_value)
                        .map_err(|error| scrapely::ExtractionError::ParseError {
                            field: #field_name_str.to_string(),
                            text: attr_value.to_string(),
                            error,
                        })?
                };
            }
        }
    } else {
        // Extract from text content
        if is_optional {
            // Optional field - return None on missing element
            quote! {
                let #field_name = element
                    .select_one(#selector)
                    .and_then(|elem| {
                        let text = elem.text();
                        <#field_type as scrapely::FromHtml>::from_text(&text).ok()
                    });
            }
        } else {
            // Required field
            quote! {
                let #field_name = {
                    let elem = element
                        .select_one(#selector)
                        .ok_or_else(|| scrapely::ExtractionError::MissingField {
                            field: #field_name_str.to_string(),
                            selector: #selector.to_string(),
                        })?;

                    let text = elem.text();

                    <#field_type as scrapely::FromHtml>::from_text(&text)
                        .map_err(|error| scrapely::ExtractionError::ParseError {
                            field: #field_name_str.to_string(),
                            text: text.clone(),
                            error,
                        })?
                };
            }
        }
    }
}
