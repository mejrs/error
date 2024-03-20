use proc_macro2::Ident;
use proc_macro2::TokenStream as TokenStream2;
use quote::ToTokens;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::punctuated::Pair;
use syn::spanned::Spanned;
use syn::Field;
use syn::Type;

pub struct Sub<'tk> {
    pub enum_name: &'tk Ident,
    pub name: &'tk Ident,
    pub source: Option<&'tk Type>,
    pub location: Option<&'tk Type>,
    pub selector_fields: Vec<&'tk Field>,
    pub selector_field_names: Vec<&'tk Ident>,
    pub all_field_names: Vec<&'tk Ident>,
    pub error_text: Vec<Text>,
    pub help_text: Vec<Text>,
}

#[derive(Debug)]
pub struct Text {
    pub lit: proc_macro2::Literal,
    pub args: Vec<TokenStream2>,
}

impl Parse for Text {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<proc_macro2::Punct>()?;
        let literal: proc_macro2::Literal = input.fork().parse()?;
        let lit: syn::LitStr = input.parse()?;
        let s = lit.value();

        let sl = s.as_str();
        let (out, args) = fmt_parse(sl).expect("cannot parse format");
        Ok(Text {
            lit: proc_macro2::Literal::string(&out),
            args: args
                .into_iter()
                .map(|(x, offset)| {
                    let span = literal
                        .subspan((offset + 1)..(offset + x.len() + 1))
                        .unwrap_or(literal.span());
                    if let Ok(mut expr) = syn::parse_str::<syn::Ident>(&x) {
                        expr.set_span(span);
                        Ok(quote::quote! { #expr })
                    } else if x == "" {
                        let span = literal
                            .subspan((offset)..(offset + 2))
                            .unwrap_or(literal.span());
                        Err(syn::Error::new(span, crate::errs::NO_FORMAT_ARG))
                    } else {
                        let expr = syn::parse_str::<syn::Expr>(&x)
                            .expect(&format!("cannot parse {x} as expr"));
                        Ok(quote::quote_spanned! {span=>
                            #expr
                        })
                    }
                })
                .collect::<Result<_, _>>()?,
        })
    }
}

pub fn parse(input: &syn::DeriveInput) -> syn::Result<Vec<Sub<'_>>> {
    let syn::Data::Enum(data) = &input.data else {
        return Err(syn::Error::new(input.span(), crate::errs::ONLY_ENUM));
    };
    let enum_name = &input.ident;

    let mut out = Vec::new();
    for Pair::Punctuated(variant, _) | Pair::End(variant) in data.variants.pairs() {
        let mut source = None;
        let mut location = None;
        let name = &variant.ident;

        let mut all_fields: Vec<&Field> = Vec::new();
        let mut selector_fields: Vec<&Field> = Vec::new();

        let mut error_text = Vec::new();
        let mut help_text = Vec::new();
        for attr in &variant.attrs {
            if let syn::AttrStyle::Inner(_) = attr.style {
                return Err(syn::Error::new(attr.span(), crate::errs::NO_INNER));
            }

            if let syn::Meta::NameValue(syn::MetaNameValue { path, value, .. }) = &attr.meta {
                // This could probably be nicer
                let value = value.into_token_stream().into();

                if path.is_ident("error") {
                    error_text.push(syn::parse(value)?);
                } else if path.is_ident("help") {
                    help_text.push(syn::parse(value)?);
                }
            }
        }
        if error_text.is_empty() {
            return Err(syn::Error::new(
                variant.span(),
                crate::errs::NEED_ERROR_TEXT,
            ));
        }

        match &variant.fields {
            syn::Fields::Named(syn::FieldsNamed { named, .. }) => {
                for field in named.pairs() {
                    let field = field.value();

                    if field
                        .attrs
                        .iter()
                        .any(|attr| attr.path().is_ident("source"))
                    {
                        if field.ident.as_ref().expect("tuple enum is not allowed") != "source" {
                            return Err(syn::Error::new(
                                field.ident.span(),
                                crate::errs::MUST_BE_NAMED_SOURCE,
                            ));
                        }
                        if source.replace(&field.ty).is_some() {
                            return Err(syn::Error::new(field.span(), crate::errs::DUPE_SOURCE));
                        }
                    } else if field
                        .attrs
                        .iter()
                        .any(|attr| attr.path().is_ident("location"))
                    {
                        if field.ident.as_ref().expect("tuple enum is not allowed") != "location" {
                            return Err(syn::Error::new(
                                field.ident.span(),
                                crate::errs::MUST_BE_NAMED_LOCATION,
                            ));
                        }
                        if location.replace(&field.ty).is_some() {
                            return Err(syn::Error::new(field.span(), crate::errs::DUPE_LOCATION));
                        }
                    } else {
                        selector_fields.push(*field);
                    }
                    all_fields.push(*field);
                }
            }
            syn::Fields::Unit => {}
            syn::Fields::Unnamed(_) => {
                return Err(syn::Error::new(
                    variant.span(),
                    crate::errs::ONLY_NAMED_FIELDS,
                ))
            }
        }

        let all_field_names: Vec<&Ident> = all_fields
            .iter()
            .map(|Field { ident, .. }| ident.as_ref().expect("cannot parse ident"))
            .collect();

        let selector_field_names: Vec<&Ident> = selector_fields
            .iter()
            .map(|Field { ident, .. }| ident.as_ref().expect("cannot parse ident"))
            .collect();

        let variant = Sub {
            enum_name,
            name,
            source,
            selector_fields,
            selector_field_names,
            all_field_names,
            error_text,
            help_text,
            location,
        };

        out.push(variant)
    }

    Ok(out)
}

fn fmt_parse(s: &str) -> Result<(String, Vec<(String, usize)>), ()> {
    use std::fmt::Write;

    let mut sl = s;
    let mut out = String::new();
    let mut args: Vec<(String, usize)> = Vec::new();
    loop {
        let Some((prefix, suffix)) = sl.split_once("{") else {
            out.push_str(sl);
            break;
        };
        out.push_str(prefix);
        sl = suffix;

        if let Some(("", rest)) = sl.split_once("{") {
            out.push_str("{{");
            sl = rest;
            continue; // "{{"" is an escape
        }

        let (argument, rest, formatter) = if let Some((argument, rest)) = sl.split_once("}") {
            if let Some((argument, formatter)) = argument.split_once(":") {
                (argument, rest, format!(":{formatter}"))
            } else {
                (argument, rest, String::new())
            }
        } else {
            return Err(());
        };

        if let Some(("", remainder)) = rest.split_once("}") {
            sl = remainder;
            continue; // "{{"" is an escape
        };
        sl = rest;
        write!(out, "{{{formatter}}}").unwrap();
        let offset = argument.as_ptr() as usize - s.as_ptr() as usize;
        args.push((String::from(argument), offset));
    }
    Ok((out, args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_debug() {
        let s = "cannot open cache: encountered {source} while looking for file {file:?}";
        let (out, args) = fmt_parse(s).unwrap();

        assert_eq!(
            out,
            "cannot open cache: encountered {} while looking for file {:?}"
        );
        assert_eq!(
            &args,
            &[(String::from("source"), 32), (String::from("file"), 64)]
        );
    }

    #[test]
    fn parse_escape() {
        let s = "Index {index_id} Archive {archive_id}: Crc does not match: {crc} !=  {{crc2}}";
        let (out, args) = fmt_parse(s).unwrap();

        assert_eq!(
            out,
            "Index {} Archive {}: Crc does not match: {} !=  {{crc2}}"
        );
        assert_eq!(
            &args,
            &[
                (String::from("index_id"), 7),
                (String::from("archive_id"), 26),
                (String::from("crc"), 60)
            ]
        );
    }

    #[test]
    fn parse_early_escape() {
        let s = "Index {index_id} Archive {archive_id}: Crc does not match: {{crc2}} != {crc}";
        let (out, args) = fmt_parse(s).unwrap();

        assert_eq!(
            out,
            "Index {} Archive {}: Crc does not match: {{crc2}} != {}"
        );
        assert_eq!(
            &args,
            &[
                (String::from("index_id"), 7),
                (String::from("archive_id"), 26),
                (String::from("crc"), 72)
            ]
        );
    }

    #[test]
    fn parse_nothing() {
        let s = "whatever {}";
        let (out, args) = fmt_parse(s).unwrap();
    }
}
