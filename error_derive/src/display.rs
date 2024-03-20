use crate::variant::Sub;
use crate::variant::Text;
use crate::ErrorEnum;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

pub(crate) fn make_impl(e: &ErrorEnum<'_>) -> TokenStream2 {
    let ErrorEnum {
        enum_name,
        variants,
        is_top_level,
        ..
    } = e;
    let arms: Vec<_> = variants.iter().map(make_arm).collect();

    let help = if *is_top_level {
        quote! {
            ::core::fmt::Display::fmt("\n", f)?;
            for error in (self as &dyn ::core::error::Error).sources() {
                if let ::core::option::Option::Some(help) = ::core::error::request_value::<::error::Help>(error) {
                    ::core::fmt::Display::fmt(&help, f)?;
                }
            }
        }
    } else {
        quote! {}
    };

    quote! {
        impl ::core::fmt::Debug for #enum_name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Display::fmt(self, f)
            }
        }

        impl ::core::fmt::Display for #enum_name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                #[allow(unused_variables)]
                match self {
                   #(#arms)*
                   __unreachable => if true { return ::core::result::Result::Err(::core::fmt::Error) }
                }

                if let Some(source) = ::core::error::Error::source(self) {
                    ::core::fmt::Display::fmt("Caused by: ", f)?;
                    ::core::fmt::Display::fmt(source, f)?;
                }

                #help

                Ok(())
            }
        }
    }
}

fn make_arm(v: &Sub<'_>) -> TokenStream2 {
    let Sub {
        enum_name,
        name,
        selector_field_names,
        error_text,
        location,
        ..
    } = v;
    let error_text_maker: Vec<_> = error_text
        .iter()
        .map(|Text { lit, args }| quote! { write!(f, #lit, #(#args),*)?; })
        .collect();

    let print_location = if location.is_some() {
        quote! {
            if let #enum_name :: #name { location , .. } = _arm {
                write!(f, " (at {})", location)?;
            }
        }
    } else {
        quote! {}
    };

    match selector_field_names.len() {
        0 => {
            quote! {
                _arm @ #enum_name :: #name { .. } => {
                    #(#error_text_maker)*
                    #print_location
                    ::core::fmt::Display::fmt("\n", f)?;
                },
            }
        }
        _ => {
            quote! {
                _arm @ #enum_name :: #name { #(#selector_field_names),* , .. } => {
                    #(#error_text_maker)*
                    #print_location
                    ::core::fmt::Display::fmt("\n", f)?;
                },
            }
        }
    }
}
