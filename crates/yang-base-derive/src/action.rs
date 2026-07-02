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
}

#[derive(Debug, Default)]
struct PermissionList(Vec<String>);

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

/// 展开 `#[derive(Action)]`。
pub fn expand(input: DeriveInput) -> TokenStream {
    let opts = match ActionOpts::from_derive_input(&input) {
        Ok(o) => o,
        Err(e) => return e.write_errors(),
    };
    let struct_name = &input.ident;
    let (impl_g, ty_g, where_clause) = input.generics.split_for_impl();

    let name = opts.name.clone();
    let display_name = opts.display_name.unwrap_or_else(|| name.clone());
    let description = opts.description.unwrap_or_default();
    let is_public = opts.public;
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
            fn is_public(&self) -> bool { #is_public }
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
