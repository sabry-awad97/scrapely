//! Type analysis utilities for field types

use syn::{GenericArgument, PathArguments, Type, TypePath};

/// Check if a type is Option<T>
pub fn is_option(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty
        && let Some(segment) = path.segments.last()
    {
        return segment.ident == "Option";
    }
    false
}

/// Check if a type is Vec<T>
pub fn is_vec(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty
        && let Some(segment) = path.segments.last()
    {
        return segment.ident == "Vec";
    }
    false
}

/// Extract the inner type from Option<T> or Vec<T>
pub fn extract_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(TypePath { path, .. }) = ty
        && let Some(segment) = path.segments.last()
        && let PathArguments::AngleBracketed(args) = &segment.arguments
        && let Some(GenericArgument::Type(inner)) = args.args.first()
    {
        return Some(inner);
    }
    None
}

/// Check if a type is likely a custom type that implements Item
///
/// This uses a heuristic approach since we can't do trait resolution in proc macros.
/// We assume a type implements Item if it's:
/// - Not a primitive type (String, i32, etc.)
/// - Not Option or Vec (these are handled separately)
/// - A custom struct type
pub fn is_likely_item_type(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty
        && let Some(segment) = path.segments.last()
    {
        let ident = segment.ident.to_string();

        // Exclude standard types that implement FromHtml
        let standard_types = [
            "String", "str", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64",
            "u128", "usize", "f32", "f64", "bool", "Option", "Vec",
        ];

        if standard_types.contains(&ident.as_str()) {
            return false;
        }

        // If it's not a standard type, assume it's a custom Item type
        return true;
    }
    false
}

/// Get the actual type to check for Item trait, unwrapping Option/Vec if needed
pub fn get_item_type(ty: &Type) -> &Type {
    // If it's Option<T> or Vec<T>, get the inner type
    if (is_option(ty) || is_vec(ty))
        && let Some(inner) = extract_inner_type(ty)
    {
        return inner;
    }
    ty
}
