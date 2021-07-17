//! Derives instances of PartialRefTarget and associated traits for the `partial_ref` crate.
#![recursion_limit = "128"]
extern crate proc_macro;

use std::collections::HashSet;

use crate::proc_macro::TokenStream;

use proc_macro2::{Span, TokenTree};
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, parse_quote, parse_str, Attribute, Data, DeriveInput, Lifetime, LifetimeDef,
    Lit, Member, Meta, Type, TypeParen,
};

fn parse_attribute_as_type(attr: &Attribute) -> Type {
    if let Some(TokenTree::Group(group)) = attr.tokens.clone().into_iter().next() {
        let parsed_type: Type = parse_quote!(#group);
        // This avoids unnecessary parentheses around type warnings from the generated code.
        if let Type::Paren(TypeParen { elem, .. }) = parsed_type {
            return *elem;
        }
        return parsed_type;
    }

    let parse_panic = || panic!("could not parse attribute `{}`", attr.tokens.to_string());
    let meta = attr.parse_meta().unwrap_or_else(|_| parse_panic());
    if let Meta::NameValue(name_value) = meta {
        if let Lit::Str(string) = name_value.lit {
            match parse_str(&string.value()) {
                Err(_) => panic!("could not parse type `{}` in attribute", string.value()),
                Ok(parsed_type) => return parsed_type,
            }
        }
    }
    parse_panic();
    unreachable!()
}

/// If the input is non-empty remove the enclosing `<` and `>` and prepend a comma.
///
/// Does not check whether the enclosing tokens actually are `<` and `>`.
fn generics_to_extra_generics(generics: &impl ToTokens) -> proc_macro2::TokenStream {
    let mut generics_tokens = proc_macro2::TokenStream::new();
    generics.to_tokens(&mut generics_tokens);

    let mut generics_tokens = generics_tokens.into_iter().collect::<Vec<_>>();

    if !generics_tokens.is_empty() {
        generics_tokens[0] = quote!(,).into_iter().next().unwrap();
        generics_tokens.pop();
    }

    let mut extra_tokens = proc_macro2::TokenStream::new();
    extra_tokens.extend(generics_tokens);
    extra_tokens
}

/// Generate a new lifetime that doesn't conflict with the existing lifetimes.
fn fresh_lifetime<'a>(lifetimes: impl Iterator<Item = &'a LifetimeDef>, name: &str) -> Lifetime {
    let mut used_idents = HashSet::new();
    for lifetime in lifetimes {
        used_idents.insert(lifetime.lifetime.ident.to_string());
    }

    let mut lifetime_name = name.to_owned();
    let mut counter = 0;

    while used_idents.contains(&lifetime_name) {
        use std::fmt::Write;
        counter += 1;
        lifetime_name.clear();
        write!(&mut lifetime_name, "{}{}", name, counter).unwrap();
    }

    Lifetime::new(&format!("'{}", lifetime_name), Span::call_site())
}

/// Derives instances of PartialRefTarget and associated traits.
///
/// Can only be used for structs. The attribute `#[part(PartName)]` can be used on the struct itself
/// for an abstract part or on a field for a field part. Parts have to be declared separately.
/// `PartName` can be any valid rust type that implements the Part trait. For fields the field type
/// of the part has to match the actual type of the field.
///
/// Example:
///
/// ```ignore
/// use partial_ref::{PartialRefTarget, part};
///
/// #[derive(PartialRefTarget)]
/// #[part(SomeAbstractPart)]
/// struct ExampleStruct {
///     field_without_part: usize,
///     #[part(SomeFieldPart)]
///     a: usize,
///     #[part(another_crate::AnotherFieldPart)]
///     b: usize,
/// }
/// ```
///
/// Instead of `#[part(PartName)]` it is also possible to use `#[part = "PartName"]` which was the
/// only supported syntax in previous versions of this crate.
///

// TODO figure out how to link to doc items of the partial_ref crate
#[proc_macro_derive(PartialRefTarget, attributes(part))]
pub fn derive_partial_ref_target(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let target_ident = input.ident;

    let lt_a = fresh_lifetime(input.generics.lifetimes(), "a");

    let (impl_generics, target_generics, where_clause) = input.generics.split_for_impl();

    if where_clause.is_some() {
        panic!("cannot derive PartialRef target for structs with a where clause");
        // TODO lift this restriction
    }

    let extra_generics = generics_to_extra_generics(&impl_generics);

    let target_type = quote!(#target_ident #target_generics);

    let data_struct = match input.data {
        Data::Struct(data_struct) => data_struct,
        _ => panic!("deriving PartialRefTarget is only supported on structs"),
    };

    let mut abstract_parts: Vec<Type> = vec![];
    let mut typed_parts: Vec<(Member, Type)> = vec![];

    for attr in input.attrs.iter() {
        if attr.path.is_ident("part") {
            abstract_parts.push(parse_attribute_as_type(&attr));
        }
    }

    for (field_index, field) in data_struct.fields.iter().enumerate() {
        let mut part: Option<Type> = None;

        for attr in field.attrs.iter() {
            if attr.path.is_ident("part") {
                if part.is_some() {
                    panic!(
                        "{} has multiple parts",
                        field
                            .ident
                            .as_ref()
                            .map_or("unnamed field".to_owned(), |i| format!("field `{}`", i))
                    );
                }
                part = Some(parse_attribute_as_type(&attr));
            }
        }

        if let Some(part_type) = part {
            let member = field
                .ident
                .as_ref()
                .map_or(Member::Unnamed(field_index.into()), |ident| {
                    Member::Named(ident.clone())
                });
            typed_parts.push((member, part_type));
        }
    }

    let mut const_type = quote!(::partial_ref::Ref<#lt_a, #target_type>);
    let mut mut_type = quote!(::partial_ref::Ref<#lt_a, #target_type>);
    let mut split_const_type = quote!(Reference);
    let mut split_mut_type = quote!(Reference);

    for part in abstract_parts.iter() {
        const_type = quote!(::partial_ref::Const<#part, #const_type>);
        mut_type = quote!(::partial_ref::Mut<#part, #mut_type>);

        split_const_type = quote!(
            ::partial_ref::Const<
                ::partial_ref::Nested<ContainingPart, #part>,
                #split_const_type
            >
        );
        split_mut_type = quote!(
            ::partial_ref::Mut<
                ::partial_ref::Nested<ContainingPart, #part>,
                #split_mut_type
            >
        );
    }

    for (_, part) in typed_parts.iter() {
        const_type = quote!(::partial_ref::Const<#part, #const_type>);
        mut_type = quote!(::partial_ref::Mut<#part, #mut_type>);

        split_const_type = quote!(
            ::partial_ref::Const<
                ::partial_ref::Nested<ContainingPart, #part>,
                #split_const_type
            >
        );
        split_mut_type = quote!(
            ::partial_ref::Mut<
                ::partial_ref::Nested<ContainingPart, #part>,
                #split_mut_type
            >
        );
    }

    let mut result = vec![];

    result.push(TokenStream::from(quote! {
        impl<#lt_a #extra_generics> ::partial_ref::IntoPartialRef<#lt_a> for &#lt_a #target_type {
            type Ref = #const_type;
            #[inline(always)]
            fn into_partial_ref(self) -> Self::Ref {
                unsafe {
                    <Self::Ref as ::partial_ref::PartialRef>::from_raw(self as *const _ as *mut _)
                }
            }
        }

        impl<#lt_a #extra_generics> ::partial_ref::IntoPartialRef<#lt_a>
        for &#lt_a mut #target_type {
            type Ref = #mut_type;
            #[inline(always)]
            fn into_partial_ref(self) -> Self::Ref {
                unsafe {
                    <Self::Ref as ::partial_ref::PartialRef>::from_raw(self as *mut _)
                }
            }
        }

        unsafe impl<#lt_a #extra_generics, ContainingPart, Reference>
            ::partial_ref::SplitIntoParts<#lt_a, ContainingPart, Reference> for #target_type
        where
            ContainingPart: ::partial_ref::Part<PartType=::partial_ref::Field<Self>>,
            Reference: ::partial_ref::PartialRef<#lt_a>,
            Reference::Target: ::partial_ref::HasPart<ContainingPart>,
        {
            type Result = #split_const_type;
            type ResultMut = #split_mut_type;
        }

        impl #impl_generics ::partial_ref::PartialRefTarget for #target_type {
            type RawTarget = Self;
        }
    }));

    for part in abstract_parts.iter() {
        result.push(TokenStream::from(quote! {
             impl #impl_generics ::partial_ref::HasPart<#part> for #target_type {
                #[inline(always)]
                unsafe fn part_ptr(ptr: *const Self) -> () {
                    unreachable!()
                }

                #[inline(always)]
                unsafe fn part_ptr_mut(ptr: *mut Self) -> () {
                    unreachable!()
                }
            }
        }));
    }

    for (member, part) in typed_parts.iter() {
        result.push(TokenStream::from(quote! {
             impl #impl_generics ::partial_ref::HasPart<#part> for #target_type {
                #[inline(always)]
                unsafe fn part_ptr(
                    ptr: *const Self
                ) -> <<#part as ::partial_ref::Part>::PartType as ::partial_ref::PartType>::Ptr  {
                    ::core::ptr::addr_of!((*ptr).#member)
                }

                #[inline(always)]
                unsafe fn part_ptr_mut(
                    ptr: *mut Self
                ) -> <<#part as ::partial_ref::Part>::PartType as ::partial_ref::PartType>::PtrMut {
                    ::core::ptr::addr_of_mut!((*ptr).#member)
                }
            }
        }));
    }

    result.into_iter().collect()
}
