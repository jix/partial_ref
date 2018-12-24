//! Derives instances of PartialRefTarget and associated traits for the `partial_ref` crate.
#![recursion_limit = "128"]
extern crate proc_macro;

use crate::proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, parse_str, Attribute, Data, DeriveInput, Lit, Member, Meta, Type};

fn parse_attribute_as_type(attr: &Attribute) -> Type {
    let parse_panic = || panic!("could not parse attribute `{}`", attr.tts.to_string());
    let meta = attr.parse_meta().unwrap_or_else(|_| parse_panic());
    match meta {
        Meta::NameValue(name_value) => match name_value.lit {
            Lit::Str(string) => match parse_str(&string.value()) {
                Err(_) => panic!("could not parse type `{}` in attribute", string.value()),
                Ok(parsed_type) => return parsed_type,
            },
            _ => (),
        },
        _ => (),
    }
    parse_panic();
    unreachable!()
}

/// Derives instances of PartialRefTarget and associated traits.
///
/// Can only be used for structs. The attribute `#[part = "PartName"]` can be used on the struct
/// itself for an abstract part or on a field for a field part. Parts have to be declared
/// separately. `PartName` can be any valid rust type that implements the Part trait. For fields the
/// field type of the part has to match the actual type of the field.
///
/// Example:
///
/// ```ignore
/// use partial_ref::{PartialRefTarget, part};
///
/// #[derive(PartialRefTarget)]
/// #[part = "SomeAbstractPart"]
/// struct ExampleStruct {
///     field_without_part: usize,
///     #[part = "SomeFieldPart"]
///     a: usize,
///     #[part = "another_crate::AnotherFieldPart"]
///     b: usize,
/// }
/// ```
// TODO figure out how to link to doc items of the partial_ref crate
#[proc_macro_derive(PartialRefTarget, attributes(part))]
pub fn derive_partial_ref_target(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let target_ident = input.ident;

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

    let mut const_type = quote!(::partial_ref::Ref<'a, #target_ident>);
    let mut mut_type = quote!(::partial_ref::Ref<'a, #target_ident>);
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
        impl<'a> ::partial_ref::IntoPartialRef<'a> for &'a #target_ident {
            type Ref = #const_type;
            #[inline(always)]
            fn into_partial_ref(self) -> Self::Ref {
                unsafe {
                    <Self::Ref as ::partial_ref::PartialRef>::from_raw(self as *const _ as *mut _)
                }
            }
        }

        impl<'a> ::partial_ref::IntoPartialRef<'a> for &'a mut #target_ident {
            type Ref = #mut_type;
            #[inline(always)]
            fn into_partial_ref(self) -> Self::Ref {
                unsafe {
                    <Self::Ref as ::partial_ref::PartialRef>::from_raw(self as *mut _)
                }
            }
        }

        unsafe impl<'a, ContainingPart, Reference>
            ::partial_ref::SplitIntoParts<'a, ContainingPart, Reference> for #target_ident
        where
            ContainingPart: ::partial_ref::Part<PartType=::partial_ref::Field<Self>>,
            Reference: ::partial_ref::PartialRef<'a>,
            Reference::Target: ::partial_ref::HasPart<ContainingPart>,
        {
            type Result = #split_const_type;
            type ResultMut = #split_mut_type;
        }

        impl ::partial_ref::PartialRefTarget for #target_ident {
            type RawTarget = Self;
        }
    }));

    for part in abstract_parts.iter() {
        result.push(TokenStream::from(quote! {
             impl ::partial_ref::HasPart<#part> for #target_ident {
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
             impl ::partial_ref::HasPart<#part> for #target_ident {
                #[inline(always)]
                unsafe fn part_ptr(
                    ptr: *const Self
                ) -> <<#part as ::partial_ref::Part>::PartType as ::partial_ref::PartType>::Ptr  {
                    &(*ptr).#member as *const _
                }

                #[inline(always)]
                unsafe fn part_ptr_mut(
                    ptr: *mut Self
                ) -> <<#part as ::partial_ref::Part>::PartType as ::partial_ref::PartType>::PtrMut {
                    &mut (*ptr).#member as *mut _
                }
            }
        }));
    }

    result.into_iter().collect()
}
