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
            for error in (self as &dyn ::core::error::Error).sources() {
                if let ::core::option::Option::Some(help) = error.request_value::<::error::Help>() {
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
                }

                if let Some(source) = ::core::error::Error::source(self) {
                    ::core::fmt::Display::fmt("Caused by: ", f)?;
                    ::core::fmt::Display::fmt(source, f)?;
                    ::core::fmt::Display::fmt(&'\n', f)?;
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
        all_field_names,
        error_text,
        ..
    } = v;
    let error_text: Vec<_> = error_text
        .iter()
        .map(|Text { lit, args }| quote! { writeln!(f, #lit, #(#args),*)?; })
        .collect();

    quote! {
        #enum_name :: #name { #(#all_field_names),* } => {#(#error_text)*},
    }
}
