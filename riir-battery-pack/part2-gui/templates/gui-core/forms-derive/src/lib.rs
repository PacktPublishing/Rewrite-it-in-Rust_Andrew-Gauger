{% if derive_macro %}
//! `#[derive(Form)]`: writes the `Form` impl for a struct from each
//! field's `#[field(label = "...")]`. Fields without `#[field]` aren't on
//! the form. The fields' order is the Tab order.
//!
//! A starting point. Chapter 10 of *Rewrite It in Rust* adds lookups,
//! validation rules and change handlers as more `#[field(...)]` keys.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, LitStr};

#[proc_macro_derive(Form, attributes(field))]
pub fn derive_form(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => &named.named,
            _ => return error(name, "#[derive(Form)] needs named fields"),
        },
        _ => return error(name, "#[derive(Form)] needs a struct"),
    };

    let mut idents = Vec::new();
    let mut specs = Vec::new();
    for f in fields {
        let Some(attr) = f.attrs.iter().find(|a| a.path().is_ident("field"))
        else {
            continue;
        };
        let ident = f.ident.clone().expect("named field");
        let mut label = ident.to_string();
        let parsed = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("label") {
                label = meta.value()?.parse::<LitStr>()?.value();
                Ok(())
            } else {
                Err(meta.error("expected `label = \"...\"`"))
            }
        });
        if let Err(e) = parsed {
            return e.to_compile_error().into();
        }
        let field_name = ident.to_string();
        specs.push(quote! {
            ::{{ crate_name }}_forms::FieldSpec { name: #field_name, label: #label }
        });
        idents.push(ident);
    }
    let n = 0..idents.len();
    let m = n.clone();

    quote! {
        impl ::{{ crate_name }}_forms::Form for #name {
            const FIELDS: &'static [::{{ crate_name }}_forms::FieldSpec] = &[ #(#specs),* ];

            fn field(&self, i: usize) -> &dyn ::{{ crate_name }}_forms::FieldValue {
                match i {
                    #( #n => &self.#idents, )*
                    _ => panic!("no field {i}"),
                }
            }

            fn field_mut(&mut self, i: usize) -> &mut dyn ::{{ crate_name }}_forms::FieldValue {
                match i {
                    #( #m => &mut self.#idents, )*
                    _ => panic!("no field {i}"),
                }
            }
        }
    }
    .into()
}

fn error(name: &syn::Ident, message: &str) -> TokenStream {
    syn::Error::new_spanned(name, message)
        .to_compile_error()
        .into()
}
{% endif %}
