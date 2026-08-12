use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, LitInt, LitStr, parse_macro_input};

enum Accessor {
    Name(String),
    Index(i128),
}

#[proc_macro_derive(FromBsn, attributes(bsn))]
pub fn derive_from_bsn(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => &named.named,
            _ => return compile_error(&input, "FromBsn requires named fields"),
        },
        _ => return compile_error(&input, "FromBsn only supports structs"),
    };

    let inits = fields.iter().map(|field| {
        let ident = field.ident.as_ref().expect("named field");
        let get = match accessor(field, &ident.to_string()) {
            Accessor::Name(wire) => quote! {
                s.get(#wire).ok_or_else(|| ::sc2_core::bsn::missing_field(#wire))?
            },
            Accessor::Index(index) => {
                let literal = proc_macro2::Literal::i128_unsuffixed(index);
                let label = format!("#{index}");
                quote! {
                    s.get_index(#literal).ok_or_else(|| ::sc2_core::bsn::missing_field(#label))?
                }
            }
        };
        quote! { #ident: ::sc2_core::bsn::FromBsn::from_bsn(#get)?, }
    });

    quote! {
        impl ::sc2_core::bsn::FromBsn for #name {
            fn from_bsn(
                value: &::sc2_core::bsn::value::BsnValue,
            ) -> ::sc2_core::Result<Self> {
                let s = value
                    .as_struct()
                    .ok_or_else(|| ::sc2_core::bsn::expected_struct(stringify!(#name)))?;
                ::core::result::Result::Ok(Self { #(#inits)* })
            }
        }
    }
    .into()
}

fn accessor(field: &syn::Field, default: &str) -> Accessor {
    let mut acc = Accessor::Name(default.to_string());
    for attr in &field.attrs {
        if attr.path().is_ident("bsn") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    let literal: LitStr = meta.value()?.parse()?;
                    acc = Accessor::Name(literal.value());
                } else if meta.path.is_ident("index") {
                    let literal: LitInt = meta.value()?.parse()?;
                    acc = Accessor::Index(literal.base10_parse()?);
                }
                Ok(())
            });
        }
    }
    acc
}

fn compile_error(spanned: &impl quote::ToTokens, message: &str) -> TokenStream {
    syn::Error::new_spanned(spanned, message)
        .to_compile_error()
        .into()
}
