//! `#[derive(TableEntity)]` 展开逻辑

use crate::util::{is_string_type, pascal_case, rust_type_to_field_type, unwrap_option};
use darling::{FromDeriveInput, FromField};
use proc_macro2::TokenStream;
use proc_macro_error::abort;
use quote::{format_ident, quote};
use syn::DeriveInput;

/// 表级属性，通过 `#[table(...)]` 指定。
#[derive(FromDeriveInput)]
#[darling(attributes(table))]
struct TableOpts {
    /// 数据库表名（必填）。
    name: String,
    /// 显示名称（可选，默认同表名）。
    #[darling(default)]
    display_name: Option<String>,
    /// 软删除字段名（可选，暂未展开生成）。
    #[darling(default)]
    soft_delete: Option<String>,
}

/// 字段级属性，通过 `#[entity(...)]` 指定。
#[derive(FromField)]
#[darling(attributes(entity))]
struct FieldOpts {
    ident: Option<syn::Ident>,
    ty: syn::Type,
    /// 是否为主键（默认 false）。
    #[darling(default)]
    primary_key: bool,
    /// 字符串最大长度（默认 255）。
    #[darling(default)]
    max_length: Option<usize>,
    /// 是否加唯一索引（默认 false）。
    #[darling(default)]
    unique: bool,
    /// 是否必填（默认：非 Option 字段为 true）。
    #[darling(default)]
    required: Option<bool>,
    /// 数据库列名（默认同字段名）。
    #[darling(default)]
    column: Option<String>,
    /// 跳过此字段，不生成到枚举和 TableConfig 中（默认 false）。
    #[darling(default)]
    skip: bool,
}

/// 展开 `#[derive(TableEntity)]`。
pub fn expand(input: DeriveInput) -> TokenStream {
    let struct_name = input.ident.clone();
    let opts = match TableOpts::from_derive_input(&input) {
        Ok(o) => o,
        Err(e) => return e.write_errors(),
    };

    let fields = match &input.data {
        syn::Data::Struct(s) => &s.fields,
        _ => abort!(input, "#[derive(TableEntity)] 只支持 struct"),
    };

    // 解析字段属性，过滤 skip 字段
    let mut field_opts: Vec<(String, FieldOpts)> = Vec::new();
    for f in fields {
        let opt = match FieldOpts::from_field(f) {
            Ok(o) => o,
            Err(e) => return e.write_errors(),
        };
        if opt.skip {
            continue;
        }
        let name = match opt.ident.as_ref() {
            Some(i) => i.to_string(),
            None => abort!(f, "TableEntity 派生仅支持具名字段"),
        };
        field_opts.push((name, opt));
    }

    // 校验主键
    let pk_count = field_opts.iter().filter(|(_, o)| o.primary_key).count();
    if pk_count == 0 {
        abort!(input, "TableEntity 必须有一个字段标注 #[entity(primary_key)]");
    }
    if pk_count > 1 {
        abort!(input, "TableEntity 只能有一个主键");
    }
    let pk_idx = field_opts.iter().position(|(_, o)| o.primary_key).unwrap();
    let pk_column = field_opts[pk_idx].1.column.clone()
        .unwrap_or_else(|| field_opts[pk_idx].0.clone());
    let pk_type = field_opts[pk_idx].1.ty.clone();

    let table_name = opts.name.clone();
    let display_name = opts.display_name.unwrap_or_else(|| table_name.clone());
    let _soft_delete = opts.soft_delete; // 暂时保留供后续扩展

    // 每个字段的 (变体名, 列名, 类型, 是否 String)
    let field_variants: Vec<(syn::Ident, String, syn::Type, bool)> = field_opts.iter().map(|(n, o)| {
        let column = o.column.clone().unwrap_or_else(|| n.clone());
        let variant = format_ident!("{}", pascal_case(n));
        (variant, column, o.ty.clone(), is_string_type(&o.ty))
    }).collect();

    let field_enum_name = format_ident!("{}Field", struct_name);
    let where_enum_name = format_ident!("{}Where", struct_name);

    let field_variant_idents: Vec<_> = field_variants.iter().map(|(v, _, _, _)| v.clone()).collect();
    let field_columns: Vec<_> = field_variants.iter().map(|(_, c, _, _)| c.clone()).collect();

    // WhereCond 枚举变体定义
    let where_variants: Vec<TokenStream> = field_variants.iter().map(|(v, _, ty, is_str)| {
        let inner_ty = unwrap_option(ty).0.clone();
        if *is_str {
            quote! { #v(::yang_base::table::StringWhereOp) }
        } else {
            quote! { #v(::yang_base::table::WhereOp<#inner_ty>) }
        }
    }).collect();

    // WhereCond match 分支
    let where_match_arms: Vec<TokenStream> = field_variants.iter().map(|(v, column, _, _)| {
        let col_lit = column.as_str();
        quote! { Self::#v(op) => op.to_sql_condition(#col_lit) }
    }).collect();

    // TableConfig 字段构造
    let config_fields: Vec<TokenStream> = field_opts.iter().map(|(n, o)| {
        let column = o.column.clone().unwrap_or_else(|| n.clone());
        let ft = rust_type_to_field_type(&o.ty, o.max_length.unwrap_or(255));
        let (_inner, is_option) = unwrap_option(&o.ty);
        let required = o.required.unwrap_or(!is_option);
        quote! {
            config = config.field(
                ::yang_base::table::FieldConfig::new(#column, #ft).required(#required)
            );
        }
    }).collect();

    // 唯一索引构造（使用现有的 unique_index(Vec<String>) 方法）
    let unique_indexes: Vec<TokenStream> = field_opts.iter()
        .filter(|(_, o)| o.unique)
        .map(|(n, o)| {
            let column = o.column.clone().unwrap_or_else(|| n.clone());
            quote! {
                config = config.unique_index(vec![#column.to_string()]);
            }
        }).collect();

    quote! {
        // ===== Field 枚举 =====
        #[derive(
            ::core::fmt::Debug, ::core::clone::Clone, ::core::marker::Copy,
            ::core::cmp::PartialEq, ::core::cmp::Eq, ::core::hash::Hash,
            ::serde::Serialize, ::serde::Deserialize, ::schemars::JsonSchema,
        )]
        #[serde(rename_all = "snake_case")]
        pub enum #field_enum_name {
            #( #field_variant_idents ),*
        }

        impl ::yang_base::table::AsColumnName for #field_enum_name {
            fn column_name(&self) -> &'static str {
                match self {
                    #( Self::#field_variant_idents => #field_columns ),*
                }
            }
        }

        // ===== WhereCond 枚举 =====
        #[derive(::core::fmt::Debug, ::serde::Deserialize, ::schemars::JsonSchema)]
        #[serde(tag = "field", content = "cond", rename_all = "snake_case")]
        pub enum #where_enum_name {
            #( #where_variants ),*
        }

        impl ::yang_base::table::IntoSqlCondition for #where_enum_name {
            fn into_sql_condition(self) -> ::yang_base::table::SqlCondition {
                match self {
                    #( #where_match_arms ),*
                }
            }
        }

        // ===== TableEntity 实现 =====
        impl ::yang_base::table::TableEntity for #struct_name {
            type Pk = #pk_type;
            type Field = #field_enum_name;
            type WhereCond = #where_enum_name;
            const TABLE_NAME: &'static str = #table_name;
            const PK_FIELD: &'static str = #pk_column;

            fn table_config() -> &'static ::yang_base::table::TableConfig {
                static CONFIG: ::std::sync::OnceLock<::yang_base::table::TableConfig> =
                    ::std::sync::OnceLock::new();
                CONFIG.get_or_init(|| {
                    let mut config = ::yang_base::table::TableConfig::new(#table_name);
                    config = config.primary_key(#pk_column);
                    config = config.display_name(#display_name);
                    #( #config_fields )*
                    #( #unique_indexes )*
                    config
                })
            }
        }
    }
}
