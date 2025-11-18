//! Attribute parsing for #[item(...)] and #[field(...)] attributes

use syn::{Attribute, Lit, Result};

/// Container-level attributes from #[item(...)]
#[derive(Debug, Default, Clone)]
pub struct ContainerAttrs {
    /// CSS selector for the root element (e.g., #[item(selector = ".quote")])
    pub selector: Option<String>,
}

/// Field-level attributes from #[field(...)]
#[derive(Debug, Default, Clone)]
pub struct FieldAttrs {
    /// CSS selector for the field (e.g., #[field(selector = "span.text")])
    pub selector: Option<String>,

    /// HTML attribute to extract from (e.g., #[field(attr = "href")])
    pub attr: Option<String>,

    /// Use default value if extraction fails (e.g., #[field(default)])
    pub default: bool,
}

impl ContainerAttrs {
    /// Parse container attributes from a list of attributes
    pub fn from_attributes(attrs: &[Attribute]) -> Result<Self> {
        let mut container_attrs = ContainerAttrs::default();

        for attr in attrs {
            if !attr.path().is_ident("item") {
                continue;
            }

            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("selector") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        container_attrs.selector = Some(s.value());
                    } else {
                        return Err(meta.error("selector must be a string literal"));
                    }
                    Ok(())
                } else {
                    Err(meta.error("unknown item attribute"))
                }
            })?;
        }

        Ok(container_attrs)
    }
}

impl FieldAttrs {
    /// Parse field attributes from a list of attributes
    pub fn from_attributes(attrs: &[Attribute]) -> Result<Self> {
        let mut field_attrs = FieldAttrs::default();

        for attr in attrs {
            if !attr.path().is_ident("field") {
                continue;
            }

            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("selector") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        field_attrs.selector = Some(s.value());
                    } else {
                        return Err(meta.error("selector must be a string literal"));
                    }
                    Ok(())
                } else if meta.path.is_ident("attr") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        field_attrs.attr = Some(s.value());
                    } else {
                        return Err(meta.error("attr must be a string literal"));
                    }
                    Ok(())
                } else if meta.path.is_ident("default") {
                    field_attrs.default = true;
                    Ok(())
                } else {
                    Err(meta.error("unknown field attribute"))
                }
            })?;
        }

        Ok(field_attrs)
    }
}
