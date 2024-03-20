use crate::variant::Sub;
use crate::variant::Text;
use crate::ErrorEnum;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

pub(crate) fn make_impl(e: &ErrorEnum<'_>) -> TokenStream2 {
    let ErrorEnum {
        enum_name,
        variants,
        ..
    } = e;

    let source_arms: Vec<_> = variants.iter().map(make_source_arm).collect();
    let provide_arms: Vec<_> = variants.iter().map(make_provide_arm).collect();
    quote! {
        impl ::core::error::Error for #enum_name {
            fn source(&self) -> ::core::option::Option<&(dyn ::core::error::Error + 'static)> {
                match self {
                    #(#source_arms)*
                    __unreachable => None,
                }
            }

            fn provide<'a>(&'a self, request: &mut ::core::error::Request<'a>) {
                #[allow(unused_variables)]
                match self {
                    #(#provide_arms)*
                    __unreachable => {}
                }
            }
        }
    }
}

fn make_source_arm(v: &Sub<'_>) -> TokenStream2 {
    let Sub {
        enum_name,
        name,
        source,
        ..
    } = v;
    if source.is_some() {
        quote! {
            #enum_name :: #name { source, .. } => { ::core::option::Option::Some(source) },
        }
    } else {
        quote! {
            #enum_name :: #name { .. } => ::core::option::Option::None,
        }
    }
}

fn make_provide_arm(v: &Sub<'_>) -> TokenStream2 {
    let Sub {
        enum_name,
        name,
        all_field_names,
        help_text,
        ..
    } = v;
    if !help_text.is_empty() {
        let help_formatter: Vec<_> = help_text
            .iter()
            .map(|Text { lit, args }| {
                quote! {
                   write!(&mut msg, "Help: ").unwrap();
                   writeln!(&mut msg, #lit, #(#args),*).unwrap();
                }
            })
            .collect();
        quote! {
            #enum_name :: #name { #(#all_field_names),* } => {
                request.provide_value_with::<::error::Help>(|| {
                    use core::fmt::Write;

                    let mut msg = ::std::string::String::new();
                    #(#help_formatter)*

                    ::error::Help::new(msg)
                });
            },
        }
    } else {
        quote! {
            #enum_name :: #name { .. } => {},
        }
    }
}
