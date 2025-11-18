//! Code generation for Item trait implementation

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Ident};

use crate::parse::{ContainerAttrs, FieldAttrs};

/// Generate the complete Item trait implementation
pub fn generate_item_impl(input: &DeriveInput, container_attrs: &ContainerAttrs) -> TokenStream {
    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Get the container selector
    let container_selector = container_attrs.selector.as_ref().unwrap();

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

            fn extract_all<E>(root: &E) -> Result<Vec<Self>, scrapely::ExtractionError>
            where
                E: scrapely::ElementRef,
            {
                let elements = root.select_all(#container_selector);
                elements
                    .iter()
                    .map(|elem| Self::extract(elem))
                    .collect()
            }

            fn from_html(html: &str) -> Result<Vec<Self>, scrapely::ExtractionError> {
                let document = scraper::Html::parse_document(html);
                Self::extract_all(&document.root_element())
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

    // Check if field is Option<T> or Vec<T>
    let is_optional = crate::types::is_option(field_type);
    let is_vec = crate::types::is_vec(field_type);

    // Check if the inner type is a nested Item
    let inner_type_for_check = crate::types::get_item_type(field_type);
    let is_nested_item = crate::types::is_likely_item_type(inner_type_for_check);

    // Handle Vec<Item> fields - select multiple elements and extract each
    if is_vec && is_nested_item {
        let inner_type = crate::types::extract_inner_type(field_type).unwrap();
        quote! {
            let #field_name = {
                let elements = element.select_all(#selector);
                let mut results = Vec::new();
                for elem in elements {
                    match <#inner_type as scrapely::ItemTrait>::extract(&elem) {
                        Ok(value) => results.push(value),
                        Err(error) => return Err(error),
                    }
                }
                results
            };
        }
    } else if is_vec {
        if let Some(attr_name) = &attrs.attr {
            // Extract from HTML attributes of multiple elements
            let inner_type = crate::types::extract_inner_type(field_type).unwrap();
            quote! {
                let #field_name = {
                    let elements = element.select_all(#selector);
                    let mut results = Vec::new();
                    for elem in elements {
                        if let Some(attr_value) = elem.attr(#attr_name) {
                            match <#inner_type as scrapely::FromHtml>::from_attr(attr_value) {
                                Ok(value) => results.push(value),
                                Err(error) => {
                                    return Err(scrapely::ExtractionError::ParseError {
                                        field: #field_name_str.to_string(),
                                        text: attr_value.to_string(),
                                        error,
                                    });
                                }
                            }
                        }
                    }
                    results
                };
            }
        } else {
            // Extract from text content of multiple elements
            let inner_type = crate::types::extract_inner_type(field_type).unwrap();
            quote! {
                let #field_name = {
                    let elements = element.select_all(#selector);
                    let mut results = Vec::new();
                    for elem in elements {
                        let text = elem.text();
                        match <#inner_type as scrapely::FromHtml>::from_text(&text) {
                            Ok(value) => results.push(value),
                            Err(error) => {
                                return Err(scrapely::ExtractionError::ParseError {
                                    field: #field_name_str.to_string(),
                                    text: text.clone(),
                                    error,
                                });
                            }
                        }
                    }
                    results
                };
            }
        }
    } else if is_nested_item && is_optional {
        // Optional nested Item field
        let inner_type = crate::types::extract_inner_type(field_type).unwrap();
        quote! {
            let #field_name = element
                .select_one(#selector)
                .and_then(|elem| <#inner_type as scrapely::ItemTrait>::extract(&elem).ok());
        }
    } else if is_nested_item {
        // Required nested Item field
        quote! {
            let #field_name = {
                let elem = element
                    .select_one(#selector)
                    .ok_or_else(|| scrapely::ExtractionError::MissingField {
                        field: #field_name_str.to_string(),
                        selector: #selector.to_string(),
                    })?;

                <#field_type as scrapely::ItemTrait>::extract(&elem)?
            };
        }
    } else if let Some(attr_name) = &attrs.attr {
        // Extract from HTML attribute
        if is_optional {
            // Optional field - return None on missing element or attribute
            let inner_type = crate::types::extract_inner_type(field_type).unwrap();
            quote! {
                let #field_name = element
                    .select_one(#selector)
                    .and_then(|elem| elem.attr(#attr_name))
                    .and_then(|attr_value| {
                        <#inner_type as scrapely::FromHtml>::from_attr(attr_value).ok()
                    });
            }
        } else if attrs.default {
            // Field with default - use Default::default() on failure
            quote! {
                let #field_name = element
                    .select_one(#selector)
                    .and_then(|elem| elem.attr(#attr_name))
                    .and_then(|attr_value| {
                        <#field_type as scrapely::FromHtml>::from_attr(attr_value).ok()
                    })
                    .unwrap_or_else(|| <#field_type as Default>::default());
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
            let inner_type = crate::types::extract_inner_type(field_type).unwrap();
            quote! {
                let #field_name = element
                    .select_one(#selector)
                    .and_then(|elem| {
                        let text = elem.text();
                        <#inner_type as scrapely::FromHtml>::from_text(&text).ok()
                    });
            }
        } else if attrs.default {
            // Field with default - use Default::default() on failure
            quote! {
                let #field_name = element
                    .select_one(#selector)
                    .and_then(|elem| {
                        let text = elem.text();
                        <#field_type as scrapely::FromHtml>::from_text(&text).ok()
                    })
                    .unwrap_or_else(|| <#field_type as Default>::default());
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
