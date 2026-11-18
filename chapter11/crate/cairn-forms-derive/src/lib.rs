//! `#[derive(Form)]`: V1 kept each control's label, format, row source
//! and AfterUpdate on its property sheet. Here they're an attribute on
//! the field, and the derive writes the `cairn_forms::Form` impl.
//!
//! ```ignore
//! #[derive(Clone, PartialEq, Form)]
//! #[form(context = AppContext)]
//! struct QuoteDraft {
//!     #[field(label = "Customer", lookup = "customers", required,
//!             after_update = customer_changed, column = 1)]
//!     customer: Option<CustomerId>,
//! }
//! ```
//!
//! Field attributes: `label`, `lookup` (a list name), `filter` (the field
//! that narrows the lookup), `after_update` (a handler function),
//! `placeholder` (a function giving faint text for an empty field),
//! `validate` (a rule function), `required`, `read_only`, `hidden`,
//! `any_text` (a lookup that only suggests), `money`,
//! `percent`, `date`, `column`, `lines`, `width`, `hint`. Fields with no
//! `#[field]` aren't on the form. The fields' order is the Tab order.
//!
//! The handlers stay ordinary functions; the derive only wires them up.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::quote;
use syn::{
    parse_macro_input, Data, DeriveInput, Expr, Fields, Ident, Lit,
    LitStr, Path, Type,
};

#[proc_macro_derive(Form, attributes(form, field))]
pub fn derive_form(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// One `#[field(...)]`.
struct FieldAttr {
    ident: Ident,
    ty: Type,
    label: Option<LitStr>,
    lookup: Option<LitStr>,
    filter: Option<Ident>,
    after_update: Option<Path>,
    placeholder: Option<Path>,
    validate: Option<Path>,
    required: bool,
    read_only: bool,
    hidden: bool,
    any_text: bool,
    format: Option<&'static str>,
    column: u8,
    lines: u8,
    width: f32,
    hint: Option<LitStr>,
}

fn expand(input: DeriveInput) -> syn::Result<Tokens> {
    let name = &input.ident;
    let context = form_context(&input)?;
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            name,
            "#[derive(Form)] needs a struct",
        ));
    };
    let Fields::Named(named) = &data.fields else {
        return Err(syn::Error::new_spanned(
            name,
            "#[derive(Form)] needs named fields",
        ));
    };
    let mut fields = Vec::new();
    for f in &named.named {
        if let Some(attr) =
            f.attrs.iter().find(|a| a.path().is_ident("field"))
        {
            fields.push(parse_field(
                f.ident.clone().unwrap(),
                &f.ty,
                attr,
            )?);
        }
    }

    let specs = fields
        .iter()
        .map(|f| spec(f, &fields))
        .collect::<syn::Result<Vec<_>>>()?;
    let n = 0..fields.len();
    let idents: Vec<&Ident> =
        fields.iter().map(|f| &f.ident).collect();
    let checks = fields.iter().enumerate().filter_map(|(i, f)| {
        let rule = f.validate.as_ref()?;
        let ident = &f.ident;
        Some(quote! { #i => #rule(&self.#ident), })
    });
    let handlers = fields.iter().enumerate().filter_map(|(i, f)| {
        let handler = f.after_update.as_ref()?;
        Some(quote! { #i => #handler(self, ctx), })
    });
    let placeholders =
        fields.iter().enumerate().filter_map(|(i, f)| {
            let shown = f.placeholder.as_ref()?;
            Some(quote! { #i => Some(#shown(self, ctx)), })
        });
    let m = n.clone();

    Ok(quote! {
        impl ::cairn_forms::Form for #name {
            type Context = #context;

            const FIELDS: &'static [::cairn_forms::FieldSpec] =
                &[#(#specs),*];

            fn field(&self, i: usize) -> &dyn ::cairn_forms::FieldValue {
                match i {
                    #(#n => &self.#idents,)*
                    _ => panic!("no field {i}"),
                }
            }

            fn field_mut(
                &mut self,
                i: usize,
            ) -> &mut dyn ::cairn_forms::FieldValue {
                match i {
                    #(#m => &mut self.#idents,)*
                    _ => panic!("no field {i}"),
                }
            }

            fn check(&self, i: usize) -> Result<(), String> {
                match i {
                    #(#checks)*
                    _ => Ok(()),
                }
            }

            fn after_update(
                &mut self,
                i: usize,
                ctx: &Self::Context,
            ) -> Vec<::cairn_forms::Cmd<Self>> {
                match i {
                    #(#handlers)*
                    _ => Vec::new(),
                }
            }

            fn placeholder(
                &self,
                i: usize,
                ctx: &Self::Context,
            ) -> Option<String> {
                match i {
                    #(#placeholders)*
                    _ => None,
                }
            }
        }
    })
}

/// `#[form(context = Type)]`.
fn form_context(input: &DeriveInput) -> syn::Result<Type> {
    let mut context = None;
    for attr in
        input.attrs.iter().filter(|a| a.path().is_ident("form"))
    {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("context") {
                context = Some(meta.value()?.parse::<Type>()?);
                Ok(())
            } else {
                Err(meta.error("expected `context = Type`"))
            }
        })?;
    }
    context.ok_or_else(|| {
        syn::Error::new_spanned(
            &input.ident,
            "#[derive(Form)] needs #[form(context = Type)]",
        )
    })
}

fn parse_field(
    ident: Ident,
    ty: &Type,
    attr: &syn::Attribute,
) -> syn::Result<FieldAttr> {
    let mut f = FieldAttr {
        ident,
        ty: ty.clone(),
        label: None,
        lookup: None,
        filter: None,
        after_update: None,
        placeholder: None,
        validate: None,
        required: false,
        read_only: false,
        hidden: false,
        any_text: false,
        format: None,
        column: 0,
        lines: 1,
        width: 0.0,
        hint: None,
    };
    attr.parse_nested_meta(|meta| {
        let key = meta.path.get_ident().map(Ident::to_string);
        match key.as_deref() {
            Some("label") => f.label = Some(meta.value()?.parse()?),
            Some("lookup") => f.lookup = Some(meta.value()?.parse()?),
            Some("hint") => f.hint = Some(meta.value()?.parse()?),
            Some("filter") => f.filter = Some(meta.value()?.parse()?),
            Some("after_update") => {
                f.after_update = Some(meta.value()?.parse()?);
            }
            Some("placeholder") => {
                f.placeholder = Some(meta.value()?.parse()?);
            }
            Some("validate") => {
                f.validate = Some(meta.value()?.parse()?)
            }
            Some("required") => f.required = true,
            Some("read_only") => f.read_only = true,
            Some("hidden") => f.hidden = true,
            Some("any_text") => f.any_text = true,
            Some(fmt @ ("money" | "percent" | "date" | "text")) => {
                f.format = Some(match fmt {
                    "money" => "Money",
                    "percent" => "Percent",
                    "date" => "Date",
                    _ => "Text",
                });
            }
            Some("column") => {
                f.column = int(&meta.value()?.parse()?)?
            }
            Some("lines") => {
                f.lines = int(&meta.value()?.parse()?)?
            }
            Some("width") => {
                let width: u16 = int(&meta.value()?.parse()?)?;
                f.width = f32::from(width);
            }
            _ => return Err(meta.error("unknown #[field] attribute")),
        }
        Ok(())
    })?;
    Ok(f)
}

fn int<N: std::str::FromStr>(e: &Expr) -> syn::Result<N>
where
    N::Err: std::fmt::Display,
{
    match e {
        Expr::Lit(lit) => match &lit.lit {
            Lit::Int(i) => i.base10_parse(),
            other => Err(syn::Error::new_spanned(
                other,
                "expected a number",
            )),
        },
        other => {
            Err(syn::Error::new_spanned(other, "expected a number"))
        }
    }
}

/// The widget kind: an explicit format wins, then a lookup, then the
/// field's type.
fn kind(f: &FieldAttr, all: &[FieldAttr]) -> syn::Result<Tokens> {
    if let Some(list) = &f.lookup {
        let filter = match &f.filter {
            Some(other) => {
                let i = all
                    .iter()
                    .position(|g| g.ident == *other)
                    .ok_or_else(|| {
                        syn::Error::new_spanned(
                            other,
                            "filter names a field that isn't on the form",
                        )
                    })?;
                quote!(Some(#i))
            }
            None => quote!(None),
        };
        let limit = !f.any_text;
        return Ok(quote! {
            ::cairn_forms::Kind::Lookup {
                list: #list,
                filter: #filter,
                limit: #limit,
            }
        });
    }
    let kind = match f.format {
        Some(k) => k,
        None => match type_name(&f.ty).as_str() {
            "bool" => "Check",
            "i32" | "i64" => "Integer",
            "Decimal" => "Money",
            "f64" => "Percent",
            "Date" | "DateTime" => "Date",
            _ => "Text",
        },
    };
    let kind = Ident::new(kind, proc_macro2::Span::call_site());
    Ok(quote!(::cairn_forms::Kind::#kind))
}

/// The type's last path segment, looking inside `Option<...>`.
fn type_name(ty: &Type) -> String {
    let Type::Path(p) = ty else {
        return String::new();
    };
    let Some(last) = p.path.segments.last() else {
        return String::new();
    };
    if last.ident == "Option" {
        if let syn::PathArguments::AngleBracketed(args) =
            &last.arguments
        {
            if let Some(syn::GenericArgument::Type(inner)) =
                args.args.first()
            {
                return type_name(inner);
            }
        }
    }
    last.ident.to_string()
}

fn spec(f: &FieldAttr, all: &[FieldAttr]) -> syn::Result<Tokens> {
    let name = f.ident.to_string();
    let label = match &f.label {
        Some(l) => l.value(),
        None => name.clone(),
    };
    let kind = kind(f, all)?;
    let (column, read_only, required, lines, width, hidden) = (
        f.column,
        f.read_only,
        f.required,
        f.lines,
        f.width,
        f.hidden,
    );
    let hint = f.hint.as_ref().map(LitStr::value).unwrap_or_default();
    Ok(quote! {
        ::cairn_forms::FieldSpec {
            name: #name,
            label: #label,
            kind: #kind,
            column: #column,
            read_only: #read_only,
            required: #required,
            lines: #lines,
            width: #width,
            hint: #hint,
            hidden: #hidden,
        }
    })
}
