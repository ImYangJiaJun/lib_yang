//! 派生宏共用工具

use proc_macro2::TokenStream;
use quote::quote;
use syn::{GenericArgument, PathArguments, Type, TypePath};

/// 把 Rust 字段类型映射到 FieldType 构造代码。
/// `Option<T>` 解包为 T 并由调用方设置 required=false。
pub fn rust_type_to_field_type(ty: &Type, max_length: usize) -> TokenStream {
    let (inner, _is_option) = unwrap_option(ty);
    let path = match inner {
        Type::Path(TypePath { path, .. }) => path,
        _ => return quote! { ::yang_base::table::FieldType::Json },
    };
    let last = match path.segments.last() {
        Some(s) => s.ident.to_string(),
        None => return quote! { ::yang_base::table::FieldType::Json },
    };
    match last.as_str() {
        "i32" | "u32" => quote! { ::yang_base::table::FieldType::Integer },
        "i64" | "u64" => quote! { ::yang_base::table::FieldType::BigInt },
        "f32" => quote! { ::yang_base::table::FieldType::Float },
        "f64" => quote! { ::yang_base::table::FieldType::Double },
        "bool" => quote! { ::yang_base::table::FieldType::Boolean },
        "String" => quote! { ::yang_base::table::FieldType::String { max_length: #max_length } },
        "NaiveDate" => quote! { ::yang_base::table::FieldType::Date },
        "NaiveDateTime" => quote! { ::yang_base::table::FieldType::DateTime },
        "DateTime" => quote! { ::yang_base::table::FieldType::Timestamp },
        "Value" => quote! { ::yang_base::table::FieldType::Json },
        _ => quote! { ::yang_base::table::FieldType::Json },
    }
}

/// 判断是否是 String 字段（用于决定是否生成 Like 变体）。
pub fn is_string_type(ty: &Type) -> bool {
    let (inner, _) = unwrap_option(ty);
    if let Type::Path(TypePath { path, .. }) = inner {
        path.segments.last().map(|s| s.ident == "String").unwrap_or(false)
    } else {
        false
    }
}

/// 判断 `Option<T>`，返回 (实际类型, 是否 Option)。
pub fn unwrap_option(ty: &Type) -> (&Type, bool) {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(seg) = path.segments.last() {
            if seg.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return (inner, true);
                    }
                }
            }
        }
    }
    (ty, false)
}

/// snake_case 转 PascalCase（生成枚举变体名）。
pub fn pascal_case(s: &str) -> String {
    let mut out = String::new();
    let mut cap = true;
    for c in s.chars() {
        if c == '_' {
            cap = true;
            continue;
        }
        if cap {
            out.extend(c.to_uppercase());
            cap = false;
        } else {
            out.push(c);
        }
    }
    out
}
