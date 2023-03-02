use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

mod variant;
use variant::Sub;

mod display;
mod error_impl;

#[proc_macro_derive(Error, attributes(note, help, error, source, top_level, location))]
pub fn derive_error(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    error_impl(input).into()
}

pub(crate) struct ErrorEnum<'tk> {
    pub enum_name: &'tk proc_macro2::Ident,
    pub is_top_level: bool,
    pub variants: Vec<Sub<'tk>>,
}

fn error_impl(input: syn::DeriveInput) -> TokenStream2 {
    let mut encountered_error = None;
    let variants = match variant::parse(&input) {
        Ok(x) => x,
        Err(e) => {
            encountered_error = Some(e);
            Vec::new()
        }
    };

    let e = ErrorEnum {
        enum_name: &input.ident,
        is_top_level: input
            .attrs
            .iter()
            .any(|attr| attr.path.is_ident("top_level")),
        variants,
    };

    let selectors: Vec<_> = e.variants.iter().map(make).collect();
    let error = error_impl::make_impl(&e);
    let display_impl = display::make_impl(&e);

    let encountered_error = encountered_error.map(syn::Error::into_compile_error);
    let ret = quote! {
        #encountered_error
        #display_impl
        #error
        #(#selectors)*
    };
    ret
}

fn make(v: &Sub<'_>) -> TokenStream2 {
    let Sub {
        enum_name,
        name,
        source,
        selector_fields,
        selector_field_names,
        all_field_names,
        location,
        ..
    } = v;

    let constructor = match (source, location) {
        (Some(_), _) => {
            quote! {}
        }
        (None, Some(_)) => {
            quote! {
                impl #name {
                    #[track_caller]
                    pub fn new( #(#selector_fields),* ) -> #enum_name {
                        let location = ::core::panic::Location::caller();
                        #enum_name::#name { #(#all_field_names),* }
                    }
                }
            }
        }
        (None, None) => {
            quote! {
                impl #name {
                    #[track_caller]
                    pub fn new( #(#selector_fields),* ) -> #enum_name {
                        #enum_name::#name { #(#all_field_names),* }
                    }
                }
            }
        }
    };

    let subs = if selector_fields.is_empty() {
        quote! {
            pub struct #name;
        }
    } else {
        let fields: Vec<_> = selector_fields
            .iter()
            .map(
                |syn::Field {
                     attrs,
                     ident,
                     colon_token,
                     ty,
                     ..
                 }| {
                    quote! {
                        #(#attrs)*
                        pub #ident #colon_token #ty,
                    }
                },
            )
            .collect();
        quote! {
            pub struct #name {
                #(#fields)*
            }
        }
    };

    let deconstructor = quote! {
        let #name {#(#selector_field_names),*}
    };
    let location_get = if location.is_some() {
        quote! {
            let location = ::core::panic::Location::caller();
        }
    } else {
        quote! {}
    };

    let impls = if let Some(source) = source {
        quote! {
            impl ::error::With<::core::result::Result<::core::convert::Infallible, #source>, #enum_name> for #name {
                fn bind(self, source: ::core::result::Result<::core::convert::Infallible, #source>) -> #enum_name {
                    let source = match source {
                        Ok(f) => match f {},
                        Err(e) => e,
                    };
                    #location_get
                    #deconstructor = self;
                    #enum_name::#name { #(#all_field_names),* }
                }
            }
        }
    } else {
        quote! {
            impl <E: ::core::error::Error> ::error::With<::core::result::Result<::core::convert::Infallible, E>, #enum_name> for #name {
                fn bind(self, _: ::core::result::Result<::core::convert::Infallible, E>) -> #enum_name {
                    #location_get
                    #deconstructor = self;
                    #enum_name::#name { #(#all_field_names),* }
                }
            }

            impl ::error::With<::core::option::Option<::core::convert::Infallible>, #enum_name> for #name {
                fn bind(self, _: ::core::option::Option<::core::convert::Infallible>) -> #enum_name {
                    #location_get
                    #deconstructor = self;
                    #enum_name::#name { #(#all_field_names),* }
                }
            }
        }
    };

    quote! {
        #subs
        #impls
        #constructor
    }
}

mod errs {
    pub const ONLY_ENUM: &str = "only enum errors are supported";
    pub const ONLY_NAMED_FIELDS: &str = "only enums with named fields are supported";
    pub const DUPE_SOURCE: &str = "more than one `#[source]` attribute";
    pub const DUPE_LOCATION: &str = "more than one `#[location]` attribute";
    pub const NO_INNER: &str = "inner attributes are not supported in this position";
    pub const NEED_ERROR_TEXT: &str = "at least one `#[error = \"msg\"]` attribute is required";
    pub const MUST_BE_NAMED_SOURCE: &str = "field of #[source] must be named `source`";
    pub const MUST_BE_NAMED_LOCATION: &str = "field of #[location] must be named `location`";
    pub const NO_FORMAT_ARG: &str =
        "positional argument in format string, but no arguments were given";
}
