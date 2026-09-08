//! Derive macros for the Dewey GUI framework.
//!
//! # `#[derive(Widget)]`
//!
//! Generates a default [`Discoverable`] implementation for a struct.
//!
//! ## Container attributes
//!
//! | Attribute | Type | Description |
//! |-----------|------|-------------|
//! | `#[widget(name = "…")]` | `String` | Widget type name (default: struct name) |
//! | `#[widget(role = "…")]` | `String` | `SemanticRole` variant (default: `Display`) |
//! | `#[widget(desc = "…")]` | `String` | Human-readable description |
//!
//! ## Example
//!
//! ```ignore
//! use dewey_derive::Widget;
//!
//! #[derive(Widget)]
//! #[widget(name = "StatusBar", role = "Display", desc = "Shows status info")]
//! struct StatusBar {
//!     text: String,
//!     #[widget(skip)]
//!     internal: usize,
//! }
//! ```
//!
//! Ignored because the generated code names `dewey::ontology`, and this crate
//! cannot depend on the one that depends on it. `tests/derive_widget.rs` in
//! `deweygui` compiles exactly this struct and checks what it generates —
//! which nothing did until then: the macro was re-exported, documented, listed
//! as complete, used by no widget, no test and no example, and built by no CI
//! job, since `--features derive` was never passed.
//!
//! Fields marked `#[widget(skip)]` are excluded from `agent_state()` output.

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

/// Derive the `Discoverable` trait for a widget struct.
#[proc_macro_derive(Widget, attributes(widget))]
pub fn derive_widget(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Parse container-level #[widget(...)] attributes
    let mut widget_name = name.to_string();
    let mut role_str = "Display".to_string();
    let mut desc = String::new();

    for attr in &input.attrs {
        if !attr.path().is_ident("widget") {
            continue;
        }
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                let value = meta.value()?;
                let s: syn::LitStr = value.parse()?;
                widget_name = s.value();
            } else if meta.path.is_ident("desc") {
                let value = meta.value()?;
                let s: syn::LitStr = value.parse()?;
                desc = s.value();
            } else if meta.path.is_ident("role") {
                let value = meta.value()?;
                let s: syn::LitStr = value.parse()?;
                role_str = s.value();
            }
            Ok(())
        });
    }

    let role_ident: proc_macro2::Ident =
        syn::parse_str(&role_str).expect("invalid SemanticRole variant");

    // Collect non-skipped field names for agent_state()
    let mut state_fields = Vec::new();
    if let syn::Data::Struct(data) = &input.data {
        if let syn::Fields::Named(fields) = &data.fields {
            for field in &fields.named {
                let skip = field.attrs.iter().any(|a| {
                    if !a.path().is_ident("widget") {
                        return false;
                    }
                    let mut found_skip = false;
                    let _ = a.parse_nested_meta(|meta| {
                        if meta.path.is_ident("skip") {
                            found_skip = true;
                        }
                        Ok(())
                    });
                    found_skip
                });
                if !skip {
                    if let Some(ident) = &field.ident {
                        let key = ident.to_string();
                        state_fields.push((key, ident.clone()));
                    }
                }
            }
        }
    }

    let state_inserts = state_fields.iter().map(|(key, ident)| {
        quote! {
            map.insert(#key.to_string(), serde_json::json!(self.#ident));
        }
    });

    let expanded = quote! {
        impl #impl_generics dewey::ontology::Discoverable for #name #ty_generics #where_clause {
            fn schema(&self) -> dewey::ontology::WidgetSchema {
                dewey::ontology::WidgetSchema::new(
                    #widget_name,
                    #desc,
                    dewey::ontology::SemanticRole::#role_ident,
                )
            }

            fn capabilities(&self) -> Vec<dewey::ontology::AgentCapability> {
                Vec::new()
            }

            fn actions(&self) -> Vec<dewey::ontology::AgentAction> {
                Vec::new()
            }

            fn semantic_role(&self) -> dewey::ontology::SemanticRole {
                dewey::ontology::SemanticRole::#role_ident
            }

            fn agent_state(&self) -> serde_json::Value {
                let mut map = serde_json::Map::new();
                #(#state_inserts)*
                serde_json::Value::Object(map)
            }

            fn execute_action(
                &mut self,
                action: &str,
                _params: &serde_json::Value,
            ) -> Result<serde_json::Value, String> {
                Err(format!("{}: unknown action '{action}'", #widget_name))
            }
        }
    };

    TokenStream::from(expanded)
}
