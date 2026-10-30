//! Rust attribute macros for declarative composition.
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use std::collections::HashSet;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit_mut::{self, VisitMut};
use syn::{parse_macro_input, Error, Expr, ExprMethodCall, Ident, ImplItem, ItemImpl, Member, Token};

/// Marks a `View` implementation for field level `State` dependency tracking.
///
/// Apply it to an `impl View` and list the state fields, for example:
///
/// ```ignore
/// #[track_state_reads(count, profile)]
/// impl View for Counter {
///     fn body(&self) -> AnyView {
///         text(self.count.get().value.to_string())
///     }
/// }
/// ```
///
/// Reads of `self.state.get().field` become `State::read_field` calls. Listed
/// state fields are also exposed to composition for lifetime and diagnostics.
#[proc_macro_attribute]
pub fn track_state_reads(attribute: TokenStream, item: TokenStream) -> TokenStream {
    let parser = Punctuated::<Ident, Token![,]>::parse_terminated;
    let fields = match parser.parse(attribute) {
        Ok(fields) => fields,
        Err(error) => return error.into_compile_error().into(),
    };
    if fields.is_empty() {
        return Error::new(Span::call_site(), "list the State fields that should be tracked")
            .into_compile_error()
            .into();
    }
    let mut implementation = parse_macro_input!(item as ItemImpl);
    let field_names: HashSet<String> = fields.iter().map(ToString::to_string).collect();
    if field_names.len() != fields.len() {
        return Error::new_spanned(fields, "state fields must be listed only once")
            .into_compile_error()
            .into();
    }

    let is_view_impl = implementation
        .trait_
        .as_ref()
        .is_some_and(|(_, path, _)| path.segments.last().is_some_and(|segment| segment.ident == "View"));
    if !is_view_impl {
        return Error::new_spanned(&implementation, "track_state_reads must annotate an impl of View")
            .into_compile_error()
            .into();
    }

    let mut found_body = false;
    let mut dynamic_properties_index = None;
    let mut precise_state_reads_index = None;
    for (index, member) in implementation.items.iter_mut().enumerate() {
        let ImplItem::Fn(function) = member else { continue };
        match function.sig.ident.to_string().as_str() {
            "body" => {
                found_body = true;
                let mut rewrite = StateFieldReads { fields: field_names.clone() };
                rewrite.visit_block_mut(&mut function.block);
            }
            "dynamic_properties" => dynamic_properties_index = Some(index),
            "precise_state_reads" => precise_state_reads_index = Some(index),
            _ => {}
        }
    }
    if !found_body {
        return Error::new_spanned(&implementation, "View implementation must define body(&self)")
            .into_compile_error()
            .into();
    }

    let property_fields: Vec<_> = fields.iter().map(|field| {
            let name = field.to_string();
            quote! { ::cove_composition::DynamicField::state(#name, &self.#field) }
        }).collect();
    let mut generated = Vec::new();
    if let Some(index) = dynamic_properties_index {
        let ImplItem::Fn(function) = &mut implementation.items[index] else { unreachable!() };
        let previous = std::mem::replace(&mut function.block, syn::parse_quote!({}));
        function.block = syn::parse_quote!({
            let mut __cove_dynamic_fields = (|| #previous)();
            __cove_dynamic_fields.extend(::std::vec![#(#property_fields),*]);
            __cove_dynamic_fields
        });
    } else {
        generated.push(quote! {
            fn dynamic_properties(&self) -> ::std::vec::Vec<::cove_composition::DynamicField> {
                ::std::vec![#(#property_fields),*]
            }
        });
    }
    if let Some(index) = precise_state_reads_index {
        let ImplItem::Fn(function) = &mut implementation.items[index] else { unreachable!() };
        function.block = syn::parse_quote!({ true });
    } else {
        generated.push(quote! { fn precise_state_reads(&self) -> bool { true } });
    }
    implementation.items.extend(generated.into_iter().map(|tokens| {
        ImplItem::Verbatim(tokens)
    }));

    let self_ty = &implementation.self_ty;
    let (impl_generics, _, where_clause) = implementation.generics.split_for_impl();
    let marker_impl = quote! {
        impl #impl_generics ::cove_composition::StateReadTrackedView for #self_ty #where_clause {}
    };
    quote!(#implementation #marker_impl).into()
}

struct StateFieldReads {
    fields: HashSet<String>,
}
impl StateFieldReads {
    fn receiver_field(&self, expression: &Expr) -> Option<Ident> {
        let Expr::Field(field) = expression else { return None };
        let Member::Named(name) = &field.member else { return None };
        let Expr::Path(base) = &*field.base else { return None };
        if base.qself.is_some()
            || base.path.segments.len() != 1
            || base.path.segments[0].ident != "self"
            || !self.fields.contains(&name.to_string())
        {
            return None;
        }
        Some(name.clone())
    }
}
impl VisitMut for StateFieldReads {
    fn visit_expr_mut(&mut self, expression: &mut Expr) {
        if let Expr::Index(index) = expression
            && let Some((state, field)) = self.array_field(&index.expr)
        {
            let element_index = &index.index;
            let span = index.span();
            *expression = syn::parse_quote_spanned! {span=>
                self.#state.read_array_element(
                    stringify!(#field),
                    #element_index,
                    |__cove_state| &__cove_state.#field,
                )
            };
            return;
        }
        if let Expr::Field(access) = expression {
            if let Expr::MethodCall(ExprMethodCall { receiver, method, args, .. }) = &*access.base
                && method == "get"
                && args.is_empty()
                && let Some(state) = self.receiver_field(receiver)
                && let Member::Named(field) = &access.member
            {
                let span = field.span();
                let field_name = field.clone();
                let replacement: Expr = syn::parse_quote_spanned! {span=>
                    self.#state.read_field(stringify!(#field_name), |__cove_state| __cove_state.#field_name.clone())
                };
                *expression = replacement;
                return;
            }
        }
        visit_mut::visit_expr_mut(self, expression);
    }
}

impl StateFieldReads {
    fn array_field(&self, expression: &Expr) -> Option<(Ident, Ident)> {
        let Expr::Field(field) = expression else { return None };
        let Member::Named(name) = &field.member else { return None };
        let Expr::MethodCall(ExprMethodCall { receiver, method, args, .. }) = &*field.base else { return None };
        if method != "get" || !args.is_empty() { return None; }
        let state = self.receiver_field(receiver)?;
        Some((state, name.clone()))
    }
}

