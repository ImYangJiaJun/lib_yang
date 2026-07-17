//! `#[derive(Action)]` 派生宏实现。
//!
//! 解析 `#[action(name, display_name, description, public, permissions(...))]` 属性，
//! 生成 `TypedAction` impl 与 `ActionMeta` 静态聚合体。

use darling::{ast::NestedMeta, FromDeriveInput, FromMeta};
use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

#[derive(FromDeriveInput)]
#[darling(attributes(action))]
struct ActionOpts {
    name: String,
    #[darling(default)]
    display_name: Option<String>,
    #[darling(default)]
    description: Option<String>,
    #[darling(default)]
    public: bool,
    #[darling(default)]
    permissions: Option<PermissionList>,
    /// 权限匹配模式："all"（AND，默认）或 "any"（OR）
    #[darling(default)]
    permission_mode: Option<String>,
    /// HTTP method；默认 POST。
    #[darling(default)]
    method: Option<String>,
    /// HTTP path；未提供时由 Action 名生成。
    #[darling(default)]
    path: Option<String>,
    /// 成功状态码；默认 200。
    #[darling(default)]
    success_status: Option<u16>,
    /// 成功响应类别：json/download/preview/redirect；默认 json。
    #[darling(default)]
    response_kind: Option<String>,
    /// 请求媒体类型：json/multipart；默认 json。
    #[darling(default)]
    request_media: Option<String>,
    /// multipart 允许的精确 MIME 类型。
    #[darling(default)]
    content_types: Option<ContentTypeList>,
    /// multipart 非文件字段数量上限。
    #[darling(default)]
    max_fields: Option<u16>,
    /// multipart 文件数量上限。
    #[darling(default)]
    max_files: Option<u16>,
    /// multipart 单文件字节上限。
    #[darling(default)]
    max_file_bytes: Option<u64>,
    /// multipart 整个请求字节上限。
    #[darling(default)]
    max_total_bytes: Option<u64>,
}

#[derive(Debug, Default)]
struct PermissionList(Vec<String>);

#[derive(Debug, Default)]
struct ContentTypeList(Vec<String>);

impl FromMeta for PermissionList {
    fn from_list(items: &[NestedMeta]) -> darling::Result<Self> {
        let mut out = Vec::new();
        for item in items {
            if let NestedMeta::Lit(syn::Lit::Str(s)) = item {
                out.push(s.value());
            } else {
                return Err(darling::Error::custom("permissions 项必须是字符串字面量"));
            }
        }
        Ok(PermissionList(out))
    }
}

impl FromMeta for ContentTypeList {
    fn from_list(items: &[NestedMeta]) -> darling::Result<Self> {
        let mut out = Vec::new();
        for item in items {
            if let NestedMeta::Lit(syn::Lit::Str(value)) = item {
                out.push(value.value());
            } else {
                return Err(darling::Error::custom("content_types 项必须是字符串字面量"));
            }
        }
        Ok(ContentTypeList(out))
    }
}

/// 展开 `#[derive(Action)]`。
pub fn expand(input: DeriveInput) -> TokenStream {
    let opts = match ActionOpts::from_derive_input(&input) {
        Ok(o) => o,
        Err(e) => return e.write_errors(),
    };
    let struct_name = &input.ident;
    let (impl_g, ty_g, where_clause) = input.generics.split_for_impl();

    let name = opts.name.clone();
    if !is_segment(&name) {
        return syn::Error::new_spanned(
            &input.ident,
            "action name 必须是小写 snake_case ASCII 标识符",
        )
        .into_compile_error();
    }
    let display_name = opts.display_name.unwrap_or_else(|| name.clone());
    let description = opts.description.unwrap_or_default();
    let is_public = opts.public;
    let method_name = opts
        .method
        .as_deref()
        .unwrap_or("POST")
        .to_ascii_uppercase();
    let method = match method_name.as_str() {
        "GET" => quote!(::yang_base::definition::HttpMethod::Get),
        "POST" => quote!(::yang_base::definition::HttpMethod::Post),
        "PUT" => quote!(::yang_base::definition::HttpMethod::Put),
        "PATCH" => quote!(::yang_base::definition::HttpMethod::Patch),
        "DELETE" => quote!(::yang_base::definition::HttpMethod::Delete),
        "OPTIONS" => quote!(::yang_base::definition::HttpMethod::Options),
        "HEAD" => quote!(::yang_base::definition::HttpMethod::Head),
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "action method 必须是 GET/POST/PUT/PATCH/DELETE/OPTIONS/HEAD",
            )
            .into_compile_error()
        }
    };
    let path = opts.path.unwrap_or_default();
    if !path.is_empty() && !path.starts_with('/') {
        return syn::Error::new_spanned(&input.ident, "action path 必须以 / 开头")
            .into_compile_error();
    }
    let success_status = opts.success_status.unwrap_or(200);
    let response_kind = match opts.response_kind.as_deref().unwrap_or("json") {
        "json" => quote!(::yang_base::definition::ActionResponseKind::Json),
        "download" => quote!(::yang_base::definition::ActionResponseKind::Download),
        "preview" => quote!(::yang_base::definition::ActionResponseKind::Preview),
        "redirect" => quote!(::yang_base::definition::ActionResponseKind::Redirect),
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "action response_kind 必须是 json/download/preview/redirect",
            )
            .into_compile_error()
        }
    };
    let has_multipart_options = opts.content_types.is_some()
        || opts.max_fields.is_some()
        || opts.max_files.is_some()
        || opts.max_file_bytes.is_some()
        || opts.max_total_bytes.is_some();
    let multipart_fn = match opts.request_media.as_deref().unwrap_or("json") {
        "json" if has_multipart_options => {
            return syn::Error::new_spanned(
                &input.ident,
                "multipart 限制只能与 request_media = \"multipart\" 一起使用",
            )
            .into_compile_error()
        }
        "json" => quote! {},
        "multipart" => {
            if !matches!(method_name.as_str(), "POST" | "PUT" | "PATCH") {
                return syn::Error::new_spanned(
                    &input.ident,
                    "multipart action method 必须是 POST/PUT/PATCH",
                )
                .into_compile_error();
            }
            let content_types = opts.content_types.unwrap_or_default().0;
            if content_types.is_empty() {
                return syn::Error::new_spanned(
                    &input.ident,
                    "multipart action 必须声明 content_types",
                )
                .into_compile_error();
            }
            if content_types.iter().any(|value| !is_exact_mime_type(value)) {
                return syn::Error::new_spanned(
                    &input.ident,
                    "content_types 必须是小写精确 MIME 类型",
                )
                .into_compile_error();
            }
            if opts.max_files == Some(0) {
                return syn::Error::new_spanned(&input.ident, "max_files 必须大于 0")
                    .into_compile_error();
            }
            if opts.max_file_bytes == Some(0) || opts.max_total_bytes == Some(0) {
                return syn::Error::new_spanned(&input.ident, "文件与请求字节上限必须大于 0")
                    .into_compile_error();
            }
            if matches!(
                (opts.max_file_bytes, opts.max_total_bytes),
                (Some(file), Some(total)) if file > total
            ) {
                return syn::Error::new_spanned(
                    &input.ident,
                    "max_file_bytes 不能大于 max_total_bytes",
                )
                .into_compile_error();
            }
            let max_fields = opts
                .max_fields
                .map(|value| quote!(.max_fields(#value)))
                .unwrap_or_default();
            let max_files = opts
                .max_files
                .map(|value| quote!(.max_files(#value)))
                .unwrap_or_default();
            let max_file_bytes = opts
                .max_file_bytes
                .map(|value| quote!(.max_file_bytes(#value)))
                .unwrap_or_default();
            let max_total_bytes = opts
                .max_total_bytes
                .map(|value| quote!(.max_total_bytes(#value)))
                .unwrap_or_default();
            quote! {
                fn multipart_spec(&self) -> ::std::option::Option<::yang_base::definition::MultipartSpec> {
                    ::std::option::Option::Some(
                        ::yang_base::definition::MultipartSpec::new([#(#content_types),*])
                            #max_fields
                            #max_files
                            #max_file_bytes
                            #max_total_bytes
                    )
                }
            }
        }
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "action request_media 必须是 json/multipart",
            )
            .into_compile_error()
        }
    };
    let perms: Vec<String> = opts.permissions.unwrap_or_default().0;

    // 解析 permission_mode：支持 "all" / "any"，默认 "all"
    let perm_mode = match opts.permission_mode.as_deref() {
        Some("any") => quote! { ::yang_base::action::PermissionMode::Any },
        _ => quote! { ::yang_base::action::PermissionMode::All },
    };

    let perm_consts: Vec<TokenStream> = perms
        .iter()
        .map(|p| {
            quote! { ::yang_base::action::Permission::from_static(#p) }
        })
        .collect();

    // API-15: 空权限直接返回 &[]（零分配），避免泛型单态化时每个 T 各生一份空 OnceLock
    let perms_fn = if perms.is_empty() {
        quote! {
            fn __action_permissions_static() -> &'static [::yang_base::action::Permission] {
                &[]
            }
        }
    } else {
        quote! {
            fn __action_permissions_static() -> &'static [::yang_base::action::Permission] {
                static PERMS: ::std::sync::OnceLock<::std::vec::Vec<::yang_base::action::Permission>>
                    = ::std::sync::OnceLock::new();
                PERMS.get_or_init(|| ::std::vec![ #( #perm_consts ),* ])
            }
        }
    };

    quote! {
        impl #impl_g #struct_name #ty_g #where_clause {
            /// 框架内部使用，由 #[derive(Action)] 派生。
            #[doc(hidden)]
            #perms_fn

            /// 框架内部使用，由 #[derive(Action)] 派生。
            #[doc(hidden)]
            fn __action_input_schema_static() -> &'static ::schemars::schema::RootSchema {
                static S: ::std::sync::OnceLock<::schemars::schema::RootSchema>
                    = ::std::sync::OnceLock::new();
                S.get_or_init(|| ::schemars::schema_for!(<Self as ::yang_base::action::TypedHandler>::Input))
            }

            /// 框架内部使用，由 #[derive(Action)] 派生。
            #[doc(hidden)]
            fn __action_output_schema_static() -> &'static ::schemars::schema::RootSchema {
                static S: ::std::sync::OnceLock<::schemars::schema::RootSchema>
                    = ::std::sync::OnceLock::new();
                S.get_or_init(|| ::schemars::schema_for!(<Self as ::yang_base::action::TypedHandler>::Output))
            }
        }

        impl #impl_g ::yang_base::action::TypedAction for #struct_name #ty_g #where_clause {
            fn name(&self) -> &'static str { #name }
            fn display_name(&self) -> &'static str { #display_name }
            fn description(&self) -> &'static str { #description }
            fn http_method(&self) -> ::yang_base::definition::HttpMethod { #method }
            fn path(&self) -> &'static str { #path }
            fn success_status(&self) -> u16 { #success_status }
            fn response_kind(&self) -> ::yang_base::definition::ActionResponseKind { #response_kind }
            fn is_public(&self) -> bool { #is_public }
            #multipart_fn
            fn permission_mode(&self) -> ::yang_base::action::PermissionMode { #perm_mode }

            fn permissions(&self) -> &'static [::yang_base::action::Permission] {
                Self::__action_permissions_static()
            }

            fn input_schema(&self) -> &'static ::schemars::schema::RootSchema {
                Self::__action_input_schema_static()
            }

            fn output_schema(&self) -> &'static ::schemars::schema::RootSchema {
                Self::__action_output_schema_static()
            }

            fn meta_static(&self) -> &'static ::yang_base::action::ActionMeta {
                static M: ::std::sync::OnceLock<::yang_base::action::ActionMeta>
                    = ::std::sync::OnceLock::new();
                M.get_or_init(|| ::yang_base::action::ActionMeta::new(
                    #name,
                    #display_name,
                    #description,
                    Self::__action_permissions_static(),
                    #perm_mode,
                    #is_public,
                    Self::__action_input_schema_static(),
                    Self::__action_output_schema_static(),
                ))
            }
        }
    }
}

fn is_segment(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|value| value.is_ascii_lowercase() || value.is_ascii_digit() || value == '_')
}

fn is_exact_mime_type(value: &str) -> bool {
    let Some((top, subtype)) = value.split_once('/') else {
        return false;
    };
    !top.is_empty()
        && !subtype.is_empty()
        && !subtype.contains('/')
        && top.bytes().all(is_mime_token_byte)
        && subtype.bytes().all(is_mime_token_byte)
}

fn is_mime_token_byte(value: u8) -> bool {
    value.is_ascii_lowercase()
        || value.is_ascii_digit()
        || matches!(
            value,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}
