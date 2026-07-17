use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    braced, Attribute, Error, Expr, GenericArgument, Ident, Result, Token, Type, Visibility,
};

pub(crate) struct ParamsInput {
    attrs: Vec<Attribute>,
    visibility: Visibility,
    name: Ident,
    fields: Punctuated<ParamField, Token![,]>,
}

struct ParamField {
    attrs: Vec<Attribute>,
    name: Ident,
    builder: Expr,
}

#[derive(Clone, Copy)]
enum Source {
    Body,
    Query,
    Path,
    Header,
}

impl Parse for ParamsInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let attrs = Attribute::parse_outer(input)?;
        let visibility = input.parse()?;
        let name = input.parse()?;
        let content;
        braced!(content in input);
        let fields = content.parse_terminated(ParamField::parse, Token![,])?;
        Ok(Self {
            attrs,
            visibility,
            name,
            fields,
        })
    }
}

impl Parse for ParamField {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let attrs = Attribute::parse_outer(input)?;
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let builder = input.parse()?;
        Ok(Self {
            attrs,
            name,
            builder,
        })
    }
}

pub(crate) fn expand(input: ParamsInput) -> Result<TokenStream> {
    let ParamsInput {
        attrs,
        visibility,
        name,
        fields,
    } = input;
    let deny_unknown = attrs
        .iter()
        .any(|attribute| attribute.path().is_ident("deny_unknown_fields"));
    let attrs = attrs
        .into_iter()
        .filter(|attribute| !attribute.path().is_ident("deny_unknown_fields"));

    let mut struct_fields = Vec::new();
    let mut params = Vec::new();
    let mut validations = Vec::new();
    let mut external_fields = Vec::new();
    let mut has_body = false;
    for field in fields {
        let field_name = field.name;
        let (builder_name, radio_type) = builder_kind(&field.builder)?;
        let value_type = rust_type(&builder_name, radio_type)?;
        let required = is_required(&field.builder);
        let source = param_source(&field.attrs)?;
        has_body |= matches!(source, Source::Body);
        let field_type = if required {
            quote!(#value_type)
        } else {
            quote!(::core::option::Option<#value_type>)
        };
        let attrs = field
            .attrs
            .into_iter()
            .filter(|attribute| !attribute.path().is_ident("param"));
        let serde_default = (!required).then(|| quote!(#[serde(default)]));
        let builder = field.builder;
        let source_token = source_token(source);
        validations.push(quote! {
            ::yang_base::definition::__validate_field_literal(stringify!(#field_name));
        });
        struct_fields.push(quote! {
            #(#attrs)*
            #serde_default
            pub #field_name: #field_type
        });
        params.push(quote! {
            .param(
                ::yang_base::definition::FieldName::__from_validated_literal(stringify!(#field_name)),
                #source_token,
                #builder,
            )
        });
        if !matches!(source, Source::Body) {
            let map = match source {
                Source::Query => quote!(request.query),
                Source::Path => quote!(request.path_params),
                Source::Header => quote!(request.headers),
                Source::Body => unreachable!(),
            };
            let lookup = if matches!(source, Source::Header) {
                quote!(stringify!(#field_name)
                    .replace('_', "-")
                    .to_ascii_lowercase())
            } else {
                quote!(stringify!(#field_name).to_string())
            };
            let value = external_value(&builder_name, quote!(raw), &value_type);
            external_fields.push(quote! {
                if let ::core::option::Option::Some(raw) = #map.get(&#lookup) {
                    let value = #value.map_err(|message| {
                        ::yang_base::error::BaseError::ParamInvalid(
                            stringify!(#field_name).to_string(),
                            message,
                        )
                    })?;
                    object.insert(stringify!(#field_name).to_string(), value);
                }
            });
        }
    }

    let serde_unknown = deny_unknown.then(|| quote!(#[serde(deny_unknown_fields)]));
    Ok(quote! {
        #(#attrs)*
        #[derive(::serde::Deserialize, ::schemars::JsonSchema)]
        #serde_unknown
        #visibility struct #name {
            #(#struct_fields,)*
        }

        impl ::yang_base::definition::ParamInput for #name {
            fn params() -> ::yang_base::definition::Params {
                const _: () = {
                    #(#validations)*
                };
                ::yang_base::definition::Params::new()
                    #(#params)*
            }

            fn decode(
                request: &mut ::yang_base::action::Request,
            ) -> ::core::result::Result<Self, ::yang_base::error::BaseError> {
                let mut object = if #has_body {
                    match ::core::mem::take(&mut request.body) {
                        ::serde_json::Value::Object(values) => values,
                        ::serde_json::Value::Null => ::serde_json::Map::new(),
                        _ => return ::core::result::Result::Err(
                            ::yang_base::error::BaseError::ParamInvalid(
                                "body".to_string(),
                                "请求体必须是 JSON object".to_string(),
                            ),
                        ),
                    }
                } else {
                    ::serde_json::Map::new()
                };
                #(#external_fields)*
                ::serde_json::from_value(::serde_json::Value::Object(object)).map_err(|error| {
                    ::yang_base::error::BaseError::ParamInvalid("input".to_string(), error.to_string())
                })
            }
        }
    })
}

fn param_source(attrs: &[Attribute]) -> Result<Source> {
    let mut source = Source::Body;
    for attribute in attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("param"))
    {
        attribute.parse_nested_meta(|meta| {
            if !meta.path.is_ident("source") {
                return Err(meta.error("仅支持 param(source = body|query|path|header)"));
            }
            let value: Ident = meta.value()?.parse()?;
            source = match value.to_string().as_str() {
                "body" => Source::Body,
                "query" => Source::Query,
                "path" => Source::Path,
                "header" => Source::Header,
                _ => {
                    return Err(Error::new_spanned(
                        value,
                        "参数来源必须是 body/query/path/header",
                    ))
                }
            };
            Ok(())
        })?;
    }
    Ok(source)
}

fn source_token(source: Source) -> TokenStream {
    let variant = match source {
        Source::Body => quote!(Body),
        Source::Query => quote!(Query),
        Source::Path => quote!(Path),
        Source::Header => quote!(Header),
    };
    quote!(::yang_base::definition::ParamSource::#variant)
}

fn external_value(builder: &Ident, raw: TokenStream, value_type: &TokenStream) -> TokenStream {
    match builder.to_string().as_str() {
        "Str" | "Text" | "Decimal" | "Password" => {
            quote!(::core::result::Result::<::serde_json::Value, ::std::string::String>::Ok(
                ::serde_json::Value::String(#raw.clone())
            ))
        }
        "Key" | "Int" | "Table" | "Tree" | "Timestamp" => quote! {
            #raw.parse::<i64>()
                .map(::serde_json::Value::from)
                .map_err(|error| error.to_string())
        },
        "Switch" => quote! {
            #raw.parse::<bool>()
                .map(::serde_json::Value::from)
                .map_err(|error| error.to_string())
        },
        "Radio" => quote! {
            ::serde_json::from_str::<#value_type>(#raw)
                .and_then(::serde_json::to_value)
                .map_err(|error| error.to_string())
        },
        _ => quote!(::core::result::Result::Err(
            "不支持的外部参数类型".to_string()
        )),
    }
}

fn builder_kind(expr: &Expr) -> Result<(Ident, Option<Type>)> {
    let call = match expr {
        Expr::MethodCall(method) => return builder_kind(&method.receiver),
        Expr::Call(call) => call,
        _ => return Err(Error::new_spanned(expr, "参数必须使用字段 Builder 调用")),
    };
    let Expr::Path(path) = call.func.as_ref() else {
        return Err(Error::new_spanned(expr, "无法识别字段 Builder"));
    };
    let segments = path.path.segments.iter().collect::<Vec<_>>();
    let builder = segments
        .get(segments.len().saturating_sub(2))
        .ok_or_else(|| Error::new_spanned(expr, "字段 Builder 必须形如 Str::new()"))?;
    let generic = match &builder.arguments {
        syn::PathArguments::AngleBracketed(arguments) => {
            arguments.args.iter().find_map(|argument| {
                if let GenericArgument::Type(value) = argument {
                    Some(value.clone())
                } else {
                    None
                }
            })
        }
        _ => None,
    };
    Ok((builder.ident.clone(), generic))
}

fn rust_type(builder: &Ident, radio_type: Option<Type>) -> Result<TokenStream> {
    match builder.to_string().as_str() {
        "Str" | "Text" | "Decimal" | "Password" => Ok(quote!(::std::string::String)),
        "Key" | "Int" | "Table" | "Tree" | "Timestamp" => Ok(quote!(i64)),
        "Switch" => Ok(quote!(bool)),
        "Radio" => radio_type
            .map(|value| quote!(#value))
            .ok_or_else(|| Error::new_spanned(builder, "Radio 必须声明值类型，例如 Radio::<i8>")),
        _ => Err(Error::new_spanned(builder, "不支持的参数字段 Builder")),
    }
}

fn is_required(expr: &Expr) -> bool {
    match expr {
        Expr::MethodCall(method) if method.method == "require" => method.args.first().is_some_and(
            |argument| matches!(argument, Expr::Lit(literal) if matches!(&literal.lit, syn::Lit::Bool(value) if value.value)),
        ),
        Expr::MethodCall(method) => is_required(&method.receiver),
        _ => false,
    }
}
