//! Type checked partial references.
//!
//! This crate provides type checked partial references for rust. Type checked partial references
//! are one solution to solve [interprocedural borrowing conflicts][interprocedural-conflicts].
//!
//! # Tutorial
//!
//! With this crate it is possible to declare typed parts (using the [`part`] macro) and to assign
//! struct fields to these parts (by [deriving `PartialRefTarget`](partial_ref_derive)).
//!
//! ```
//! use partial_ref::{part, PartialRefTarget};
//!
//! part!(pub Neighbors: Vec<Vec<usize>>);
//! part!(pub Colors: Vec<usize>);
//! part!(pub Weights: Vec<f32>);
//!
//! #[derive(PartialRefTarget, Default)]
//! pub struct Graph {
//!     #[part(Neighbors)]
//!     pub neighbors: Vec<Vec<usize>>,
//!     #[part(Colors)]
//!     pub colors: Vec<usize>,
//!     #[part(Weights)]
//!     pub weights: Vec<f32>,
//! }
//! ```
//!
//! It is then possible to convert a reference to a value using the [`IntoPartialRef`] and
//! [`IntoPartialRefMut`] traits, which are derived alongside PartialRefTarget. Note that a mutable
//! partial reference must itself be mutable for mutable access.
//!
//! ```
//! # use partial_ref::{part, PartialRefTarget};
//! # use partial_ref_tests::doctest_helpers::*;
//! use partial_ref::IntoPartialRefMut;
//!
//! let mut g = Graph::default();
//!
//! let mut g_ref = g.into_partial_ref_mut();
//! ```
//!
//! You can access parts using the [`part`](PartialRef::part) and [`part_mut`](PartialRef::part_mut)
//! methods.
//!
//! ```
//! # use partial_ref::{part, PartialRefTarget, IntoPartialRefMut};
//! # use partial_ref_tests::doctest_helpers::*;
//! # let mut g = Graph::default();
//! # let mut g_ref = g.into_partial_ref_mut();
//! use partial_ref::PartialRef;
//!
//! g_ref.part_mut(Colors).extend(&[0, 1, 0]);
//! g_ref.part_mut(Weights).extend(&[0.25, 0.5, 0.75]);
//!
//! g_ref.part_mut(Neighbors).push(vec![1, 2]);
//! g_ref.part_mut(Neighbors).push(vec![0, 2]);
//! g_ref.part_mut(Neighbors).push(vec![0, 1]);
//!
//! assert_eq!(g_ref.part(Colors).len(), g_ref.part(Neighbors).len());
//! assert_eq!(g_ref.part(Colors).len(), g_ref.part(Weights).len());
//! ```
//!
//! We can now write a function that takes parts of a reference. The type of such a partial
//! reference can be written using the [`partial`] macro, which expands to a combination of [`Mut`],
//! [`Const`] and [`Ref`]. Again the parameter `g` here must be mutable to allow mutable access to
//! the referenced value. To call such a function we use the [`borrow`](PartialRef::borrow) method,
//! which will re-borrow just the required parts.
//!
//! ```
//! # use partial_ref::{part, PartialRefTarget, IntoPartialRefMut, PartialRef};
//! # use partial_ref_tests::doctest_helpers::*;
//! # let mut g = example_graph();
//! # let mut g_ref = g.into_partial_ref_mut();
//! use partial_ref::partial;
//!
//! pub fn add_color_to_weight(
//!     mut g: partial!(Graph, mut Weights, Colors),
//!     index: usize,
//! ) {
//!     g.part_mut(Weights)[index] += g.part(Colors)[index] as f32;
//! }
//!
//! add_color_to_weight(g_ref.borrow(), 1);
//!
//! assert_eq!(g_ref.part(Weights)[1], 1.5);
//! ```
//!
//! So far everything could have been written using plain built-in references. This changes as soon
//! as we want to iterate over the neighbors while invoking our function. Usually we couldn't pass a
//! mutable reference to the graph while holding the iterator over the neighbors.
//!
//! This can be easily done using partial references which support splitting. Splitting means
//! turning a single reference into two references where each has a subset of parts, so that each
//! mutably referenced part belongs only to a single reference. This is done by the
//! [`split_borrow`](PartialRef::borrow) and [`split_part`](PartialRef::split_part) methods.
//!
//! ```
//! # use partial_ref::{part, PartialRefTarget, IntoPartialRefMut, PartialRef, partial};
//! # use partial_ref_tests::doctest_helpers::*;
//! # let mut g = example_graph();
//! # let mut g_ref = g.into_partial_ref_mut();
//! let (neighbors, mut g_ref) = g_ref.split_part_mut(Neighbors);
//! let (colors, mut g_ref) = g_ref.split_part(Colors);
//!
//! for (edges, &color) in neighbors.iter_mut().zip(colors.iter()) {
//!     edges.retain(|&neighbor| colors[neighbor] != color);
//!
//!     for &neighbor in edges.iter() {
//!         add_color_to_weight(g_ref.borrow(), neighbor);
//!     }
//! }
//! ```
//!
//! This covers the basic functionality of this library. Beyond that this library also supports:
//!
//! * Partial references to nested structs using [`Nested`] and [`nested_part`].
//! * Generic functions with bounds on available parts using [`HasPart`].
//!
//! # Notes
//!
//! Some documented items are marked with *(internal)*. Typical code using this library doesn't
//! explicitly refer to them. Nevertheless they often appear in error messages and are thus part of
//! this documentation. These items also have to be public for the inference driven meta programming
//! to work. Code that is generic over parts of partial references might also need them.
//!
//! Many traits in this crate are marked as unsafe without documenting any requirements for
//! implementations. This does not mean they are safe to implement, but rather means that they are
//! not intended to be implemented outside of this library. Feel free to file an issue if you have a
//! good reason to implement them so the requirements can be documented.
//!
//! [interprocedural-conflicts]: http://smallcultfollowing.com/babysteps/blog/2018/11/01/after-nll-interprocedural-conflicts/

#[macro_use]
mod macros;

pub use partial_ref_derive::PartialRefTarget;

use std::marker::PhantomData;

/// A type that can be the target of partial references.
///
/// Implementations for this trait should be derived, see [`partial_ref_derive`].
pub trait PartialRefTarget {
    /// A partial reference will be represented by a pointer to this associated type.
    ///
    /// In most cases this is the implementing type itself. This is not a requirement though. In the
    /// future, some features will introduce PartialRefTargets that have a different associated
    /// `RawTarget` type.
    type RawTarget: ?Sized;
}

/// Helper trait to associate the target type with a [`PartialRef`] without needing a lifetime.
pub trait HasTarget {
    /// The referenced type.
    ///
    // TODO add a warning that Target::RawTarget could be != Target when such a feature lands)
    type Target: PartialRefTarget + ?Sized;
}

/// A partial reference.
///
/// This is implemented by variants of [`Ref`], [`Mut`] and [`Const`]. This is only implemented if the
/// parts of any contained [`Mut`] or [`Const`] are valid for the referenced type.
pub trait PartialRef<'a>: HasTarget + Sized {
    /// Create a partial reference from a raw pointer.
    ///
    /// # Safety
    /// This is unsafe for two reasons. It can be used to dereference a raw pointer, which is
    /// already unsafe on its own, and it can be used to construct invalid partial references, for
    /// example containing the same mutable part twice. Thus extra care is required when calling
    /// this.
    unsafe fn from_raw(ptr: *mut <Self::Target as PartialRefTarget>::RawTarget) -> Self;

    /// Access to the underlying raw pointer.
    ///
    /// Beware that this can be used even for an empty reference with no parts. Doing anything with
    /// the resulting pointer is very likely unsafe, even if the partial reference is still in
    /// scope.
    fn get_raw(&self) -> *mut <Self::Target as PartialRefTarget>::RawTarget;

    /// Partially re-borrows a partial reference.
    ///
    /// This returns a new partial reference to the same value. The returned reference can have a
    /// subset of the original reference's parts.
    ///
    /// A typical use case is passing a reference to a function that requires fewer parts than the
    /// caller.
    ///
    /// Usually the type parameters can be inferred.
    #[inline(always)]
    fn borrow<BorrowedRef, SubsetIndex>(&'a mut self) -> BorrowedRef
    where
        BorrowedRef: PartialRef<'a, Target = Self::Target>,
        Self: HasSubset<'a, BorrowedRef, SubsetIndex>,
    {
        unsafe { BorrowedRef::from_raw(self.get_raw()) }
    }

    /// Access a part of the referenced value.
    ///
    /// This returns a plain reference to a single part.
    ///
    /// The parameter is only present for type inference, its value is ignored. As all parts
    /// implement [`Default`] it is always possible to pass a default value, which is useful in
    /// generic code.
    ///
    /// Usually the type parameters can be inferred.
    #[inline(always)]
    fn part<FieldPartSpec, FieldPart, PartIndex, FieldType>(
        &'a self,
        _part: FieldPartSpec,
    ) -> &'a FieldType
    where
        FieldType: ?Sized,
        FieldPart: Part<PartType = Field<FieldType>>,
        FieldPartSpec: PartSpec<FieldPart>,
        Self: PluckConst<'a, FieldPart, PartIndex>,
        Self::Target: HasPart<FieldPart> + 'a,
    {
        unsafe {
            <Const<FieldPart, Ref<'a, Self::Target>> as PartialRef<'a>>::from_raw(self.get_raw())
                .get_part()
        }
    }

    /// Mutable access to a part of the referenced value.
    ///
    /// This returns a plain mutable reference to a single part.
    ///
    /// The parameter is only present for type inference, its value is ignored. As all parts
    /// implement [`Default`] it is always possible to pass a default value, which is useful in
    /// generic code.
    ///
    /// Usually the type parameters can be inferred.
    #[inline(always)]
    fn part_mut<FieldPartSpec, FieldPart, PartIndex, FieldType>(
        &'a mut self,
        _part: FieldPartSpec,
    ) -> &'a mut FieldType
    where
        FieldType: ?Sized,
        FieldPart: Part<PartType = Field<FieldType>>,
        FieldPartSpec: PartSpec<FieldPart>,
        Self: PluckMut<'a, FieldPart, PartIndex>,
        Self::Target: HasPart<FieldPart> + 'a,
    {
        unsafe {
            <Mut<FieldPart, Ref<'a, Self::Target>> as PartialRef<'a>>::from_raw(self.get_raw())
                .get_part_mut()
        }
    }

    /// Partially re-borrows a partial reference, splitting off the remaining parts.
    ///
    /// This is equivalent to [`borrow`](PartialRef::borrow) but also returns a second partial
    /// reference that contains all parts that can be used simultaneously with the re-borrowed
    /// reference.
    ///
    /// This means that constant parts are contained in both references, while mutable parts that
    /// are re-borrowed are missing from the second partial reference. Mutable parts that are
    /// re-borrowed as constant parts are constant parts of both references.
    ///
    /// Usually the type parameters can be inferred.
    #[inline(always)]
    fn split_borrow<BorrowedRef, SubsetIndex>(&'a mut self) -> (BorrowedRef, Self::Remainder)
    where
        BorrowedRef: PartialRef<'a, Target = Self::Target>,
        Self: HasSubset<'a, BorrowedRef, SubsetIndex>,
    {
        let ptr = self.get_raw();
        unsafe { (BorrowedRef::from_raw(ptr), Self::Remainder::from_raw(ptr)) }
    }

    /// Access a part of the referenced value, splitting off the remaining parts.
    ///
    /// This is equivalent to [`part`](PartialRef::part) but also returns a partial reference as
    /// described in [`split_borrow`](PartialRef::split_borrow).
    #[inline(always)]
    fn split_part<FieldPartSpec, FieldPart, PartIndex, FieldType>(
        &'a mut self,
        _part: FieldPartSpec,
    ) -> (&'a FieldType, Self::Remainder)
    where
        FieldType: ?Sized,
        FieldPart: Part<PartType = Field<FieldType>>,
        FieldPartSpec: PartSpec<FieldPart>,
        Self: PluckConst<'a, FieldPart, PartIndex>,
        Self::Target: HasPart<FieldPart> + 'a,
    {
        let ptr = self.get_raw();
        unsafe {
            (
                <Const<FieldPart, Ref<'a, Self::Target>> as PartialRef<'a>>::from_raw(ptr)
                    .get_part(),
                Self::Remainder::from_raw(ptr),
            )
        }
    }

    /// Mutable access to a part of the referenced value, splitting off the remaining parts.
    ///
    /// This is equivalent to [`part_mut`](PartialRef::part_mut) but also returns a partial
    /// reference as described in [`split_borrow`](PartialRef::split_borrow).
    #[inline(always)]
    fn split_part_mut<FieldPartSpec, FieldPart, PartIndex, FieldType>(
        &'a mut self,
        _part: FieldPartSpec,
    ) -> (&'a mut FieldType, Self::Remainder)
    where
        FieldType: ?Sized,
        FieldPart: Part<PartType = Field<FieldType>>,
        FieldPartSpec: PartSpec<FieldPart>,
        Self: PluckMut<'a, FieldPart, PartIndex>,
        Self::Target: HasPart<FieldPart> + 'a,
    {
        let ptr = self.get_raw();
        unsafe {
            (
                <Mut<FieldPart, Ref<'a, Self::Target>> as PartialRef<'a>>::from_raw(ptr)
                    .get_part_mut(),
                Self::Remainder::from_raw(ptr),
            )
        }
    }
}

/// Construction of partial references.
///
/// See also [`IntoPartialRefMut`].
///
/// This trait gets an automatic implementation for references (mutable or immutable) to any type
/// that has a derive statement for [`PartialRefTarget`]. Usually there is no need to implement this
/// trait manually.
pub trait IntoPartialRef<'a> {
    type Ref: PartialRef<'a>;

    /// Convert a mutable or immutable reference into a partial reference.
    fn into_partial_ref(self) -> Self::Ref;
}

/// Construction of partial references from mutable references.
///
/// This has an implementation for mutable references that implement [`IntoPartialRefMut`]. It
/// performs the same operation as [`IntoPartialRefMut`] but is only implemented for mutable
/// references. This is useful as it allows writing `value.into_partial_ref_mut()` instead of `(&mut
/// value).into_partial_ref_mut()` using auto referencing of method calls. Using just
/// [`value.into_partial_ref()`] would result in an immutable reference.
pub trait IntoPartialRefMut<'a>: IntoPartialRef<'a> {
    /// Convert a mutable reference into a partial reference.
    fn into_partial_ref_mut(self) -> Self::Ref;
}

impl<'a, T> IntoPartialRefMut<'a> for &'a mut T
where
    Self: IntoPartialRef<'a>,
{
    fn into_partial_ref_mut(self) -> Self::Ref {
        self.into_partial_ref()
    }
}

/// *(internal)* Split a part into nested parts.
///
/// This is used to implement splitting of nested parts.
pub unsafe trait SplitIntoParts<'a, ContainingPart, Reference: PartialRef<'a>> {
    /// A partial reference that has all the parts `Reference` and all parts of `Self` nested in
    /// `ContainingPart` as constant parts.
    type Result: PartialRef<'a, Target = Reference::Target>;
    /// A partial reference that has all the parts `Reference` and all parts of `Self` nested in
    /// `ContainingPart` as mutable parts.
    type ResultMut: PartialRef<'a, Target = Reference::Target>;
}

/// An empty partial reference borrowing no parts.
///
/// Partial references with a non-empty set of borrowed parts are built by nesting this type within
/// the [`Mut`] and [`Const`] types.
#[repr(transparent)]
pub struct Ref<'a, Target: PartialRefTarget + ?Sized> {
    ptr: *mut Target::RawTarget,
    phantom: PhantomData<&'a mut Target>,
}

impl<'a, Target: PartialRefTarget + ?Sized> HasTarget for Ref<'a, Target> {
    type Target = Target;
}

/// An empty reference to a valid target is a valid reference.
impl<'a, 'b: 'a, Target: PartialRefTarget + ?Sized> PartialRef<'a> for Ref<'b, Target> {
    #[inline(always)]
    unsafe fn from_raw(ptr: *mut <Self::Target as PartialRefTarget>::RawTarget) -> Self {
        Ref {
            ptr,
            phantom: PhantomData,
        }
    }

    #[inline(always)]
    fn get_raw(&self) -> *mut <Self::Target as PartialRefTarget>::RawTarget {
        self.ptr
    }
}

/// An empty reference contains no mutable parts and thus is safe to clone.
impl<'a, Target: PartialRefTarget> Copy for Ref<'a, Target> {}

/// An empty reference contains no mutable parts and thus is safe to clone.
impl<'a, Target: PartialRefTarget> Clone for Ref<'a, Target> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

/// A mutable part of a partial reference.
#[repr(transparent)]
pub struct Mut<Part, Reference: HasTarget> {
    ptr: *mut <<Reference as HasTarget>::Target as PartialRefTarget>::RawTarget,
    phantom: PhantomData<(Reference, Part)>,
}

impl<'a, SomePart: Part, Reference: PartialRef<'a>> HasTarget for Mut<SomePart, Reference>
where
    Reference::Target: HasPart<SomePart>,
{
    type Target = Reference::Target;
}

/// Extending a valid reference by a mutable part is still a valid reference when the reference
/// target has such a part.
impl<'a, SomePart: Part, Reference: PartialRef<'a>> PartialRef<'a> for Mut<SomePart, Reference>
where
    Reference::Target: HasPart<SomePart>,
{
    #[inline(always)]
    unsafe fn from_raw(ptr: *mut <Self::Target as PartialRefTarget>::RawTarget) -> Self {
        Mut {
            ptr,
            phantom: PhantomData,
        }
    }

    #[inline(always)]
    fn get_raw(&self) -> *mut <Self::Target as PartialRefTarget>::RawTarget {
        self.ptr
    }
}

/// A constant (immutable) part of a partial reference.
#[repr(transparent)]
pub struct Const<Part, Reference: HasTarget> {
    ptr: *mut <<Reference as HasTarget>::Target as PartialRefTarget>::RawTarget,
    phantom: PhantomData<(Reference, Part)>,
}

impl<'a, SomePart: Part, Reference: PartialRef<'a>> HasTarget for Const<SomePart, Reference>
where
    Reference::Target: HasPart<SomePart>,
{
    type Target = Reference::Target;
}

/// Extending a valid reference by a constant part is still a valid reference when the reference
/// target has such a part.
impl<'a, SomePart: Part, Reference: PartialRef<'a>> PartialRef<'a> for Const<SomePart, Reference>
where
    Reference::Target: HasPart<SomePart>,
{
    #[inline(always)]
    unsafe fn from_raw(ptr: *mut <Self::Target as PartialRefTarget>::RawTarget) -> Self {
        Const {
            ptr,
            phantom: PhantomData,
        }
    }

    #[inline(always)]
    fn get_raw(&self) -> *mut <Self::Target as PartialRefTarget>::RawTarget {
        self.ptr
    }
}

/// A reference containing just constant parts is safe to clone.
impl<SomePart, Reference: Copy + HasTarget> Copy for Const<SomePart, Reference> {}

/// A reference containing just constant parts is safe to clone.
impl<SomePart, Reference: Copy + HasTarget> Clone for Const<SomePart, Reference> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, SomePart, Target, FieldType> Const<SomePart, Ref<'a, Target>>
where
    FieldType: ?Sized,
    SomePart: Part<PartType = Field<FieldType>>,
    Target: ?Sized,
    Target: HasPart<SomePart>,
{
    /// Only available on single part references, used to implement the more general
    /// [`part`](PartialRef::part) method of [`PartialRef`].
    #[inline(always)]
    fn get_part(self) -> &'a FieldType {
        unsafe { &*Target::part_ptr(self.get_raw() as *const _) }
    }
}

impl<'a, SomePart, Target, FieldType> Mut<SomePart, Ref<'a, Target>>
where
    FieldType: ?Sized,
    SomePart: Part<PartType = Field<FieldType>>,
    Target: ?Sized,
    Target: HasPart<SomePart>,
{
    /// Only available on single part references, used to implement the more general
    /// [`part_mut`](PartialRef::part_mut) method of [`PartialRef`]
    #[inline(always)]
    fn get_part_mut(self) -> &'a mut FieldType {
        unsafe { &mut *Target::part_ptr_mut(self.get_raw()) }
    }
}

// Parts

/// Marker types for a part of a type.
///
/// Types implementing this trait are usually created using the [`part`] macro.
///
/// A type implementing this trait is used to identify a part of a reference target. Multiple
/// different reference targets can have a part identified by the same [`Part`] (see also
/// [`HasPart`]). A part has an associated [`PartType`], which determines what can be done with a
/// part.
pub trait Part: Default {
    type PartType: PartType;
}

/// Helper trait to strip lifetimes from a part.
///
/// Every part `SomePart<'a, ...>` should implement `PartSpec<SomePart<'b, ...>>`. This is used by
/// the [`part`](PartialRef::part), [`part_mut`](PartialRef::part_mut),
/// [`split_part`](PartialRef::split_part) and [`split_part_mut`](PartialRef::split_part_mut)
/// functions. This allows the passed parameter to have a different lifetime than the accessed part.
/// This in turn enables part selection using globals with static lifetimes as declared by the
/// [`part`] macro.
pub trait PartSpec<Part> {}

/// Implemented when a reference target has a part.
///
/// This trait provides methods for unchecked access to a part of a reference target.
/// Implementations for this are automatically created when deriving PartialRefTarget.
pub trait HasPart<SomePart: Part>: PartialRefTarget {
    /// Given a constant pointer to a target, produce a constant pointer to a part of it.
    ///
    /// # Safety
    /// Implementations may construct a temporary reference to ptr, which thus must be valid.
    unsafe fn part_ptr(ptr: *const Self::RawTarget) -> <SomePart::PartType as PartType>::Ptr;
    /// Given a mutable pointer to a target, produce a mutable pointer to a part of it.
    ///
    /// # Safety
    /// Implementations may construct a temporary reference to ptr, which thus must be valid.
    unsafe fn part_ptr_mut(ptr: *mut Self::RawTarget) -> <SomePart::PartType as PartType>::PtrMut;
}

/// Type of a part, determines what can be done with a part.
///
/// Common part types are [`Field`] and [`AbstractPart`].
pub trait PartType {
    /// Type that can be produced from a constant pointer to a reference target.
    type Ptr;
    /// Type that can be produced from a mutable pointer to a reference target.
    type PtrMut;
}

/// Type of a part that corresponds to a struct field.
///
/// Partial references containing a part of this type provide access to that field.
pub struct Field<FieldType: ?Sized>(PhantomData<FieldType>);

impl<FieldType: ?Sized> PartType for Field<FieldType> {
    type Ptr = *const FieldType;
    type PtrMut = *mut FieldType;
}

/// Type of an abstract part.
///
/// Partial reference keep track of an abstract part, without providing any operations on it.
pub struct AbstractPart;

impl PartType for AbstractPart {
    type Ptr = ();
    type PtrMut = ();
}

/// A nested part.
///
/// A nested part can be constructed from an outer part and an inner part. The outer part must be a
/// [`Field`] part, and the field's type must be a [`PartialRefTarget`] having the the inner part
/// ([`HasPart`]).
///
/// When nesting multiple times, the nested part should always be the outer part. This isn't
/// enforced, but some operations are only supported in that case.
#[derive(Default)]
pub struct Nested<Outer, Inner>(Outer, Inner);

/// Create default values for nested parts.
///
/// These are useful as parameters to pass to [`PartialRef`]'s part functions.
impl<NewInnerPart: Part, Outer: Part, Inner: Part> std::ops::BitOr<NewInnerPart>
    for Nested<Outer, Inner>
{
    type Output = Nested<Nested<Outer, Inner>, NewInnerPart>;

    fn bitor(self, _rhs: NewInnerPart) -> Self::Output {
        std::default::Default::default()
    }
}

/// A nested part is itself a part.
impl<Outer, OuterFieldType, Inner> Part for Nested<Outer, Inner>
where
    Outer: Part<PartType = Field<OuterFieldType>>,
    Inner: Part,
    OuterFieldType: ?Sized,
    OuterFieldType: HasPart<Inner>,
    OuterFieldType: PartialRefTarget<RawTarget = OuterFieldType>,
{
    type PartType = Inner::PartType;
}

impl<Outer, Inner, OuterS, InnerS> PartSpec<Nested<OuterS, InnerS>> for Nested<Outer, Inner>
where
    Outer: PartSpec<OuterS>,
    Inner: PartSpec<InnerS>,
{
}

/// A reference has a nested part if it has the outer part and the nested part is valid.
impl<Reference, Outer, OuterFieldType, Inner> HasPart<Nested<Outer, Inner>> for Reference
where
    Reference: ?Sized,
    Reference: HasPart<Outer>,
    Outer: Part<PartType = Field<OuterFieldType>>,
    Inner: Part,
    OuterFieldType: ?Sized,
    OuterFieldType: HasPart<Inner>,
    OuterFieldType: PartialRefTarget<RawTarget = OuterFieldType>,
{
    #[inline(always)]
    unsafe fn part_ptr(ptr: *const Self::RawTarget) -> <Inner::PartType as PartType>::Ptr {
        <OuterFieldType as HasPart<Inner>>::part_ptr(<Self as HasPart<Outer>>::part_ptr(ptr))
    }

    #[inline(always)]
    unsafe fn part_ptr_mut(ptr: *mut Self::RawTarget) -> <Inner::PartType as PartType>::PtrMut {
        <OuterFieldType as HasPart<Inner>>::part_ptr_mut(<Self as HasPart<Outer>>::part_ptr_mut(
            ptr,
        ))
    }
}

// Type level logic for subsetting
// This is inspired by the `frunk` crate

/// *(internal)* Select the first part.
pub struct IndexHere;

/// *(internal)* Skip the first part.
pub struct IndexNext<Index>(Index);

/// *(internal)* Split the first part.
pub struct IndexSplit<NestedPartIndex, Index>(NestedPartIndex, Index);

/// *(internal)* Extracts the constant part `PluckedPart` at position `Index` from the partial
/// reference having this trait, leaving `Self::Remainder`.
///
/// Plucking a constant part still leaves the part in the remaining reference, but will change it
/// from mutable to constant.
///
/// The `Index` type can be inferred.
pub unsafe trait PluckConst<'a, PluckedPart, Index>: PartialRef<'a> {
    /// The partial reference left after plucking.
    type Remainder: PartialRef<'a, Target = Self::Target>;
}

/// *(internal)* Extracts the mutable part `PluckedPart` at position `Index` from the partial
/// reference having this trait, leaving `Self::Remainder`.
///
/// Plucking a mutable part removes it from the remaining reference.
///
/// The `Index` type can be inferred.
pub unsafe trait PluckMut<'a, PluckedPart, Index>: PartialRef<'a> {
    /// The partial reference left after plucking.
    type Remainder: PartialRef<'a, Target = Self::Target>;
}

/// *(internal)* Plucks the outermost constant part.
unsafe impl<'a, PluckedPart, Reference> PluckConst<'a, PluckedPart, IndexHere>
    for Const<PluckedPart, Reference>
where
    PluckedPart: Part,
    Reference: PartialRef<'a>,
    Reference::Target: HasPart<PluckedPart>,
{
    type Remainder = Const<PluckedPart, Reference>;
}

/// *(internal)* Plucks the first part, converting it from mutable to constant.
unsafe impl<'a, PluckedPart, Reference> PluckConst<'a, PluckedPart, IndexHere>
    for Mut<PluckedPart, Reference>
where
    PluckedPart: Part,
    Reference: PartialRef<'a>,
    Reference::Target: HasPart<PluckedPart>,
{
    type Remainder = Const<PluckedPart, Reference>;
}

/// *(internal)* Plucks the first mutable part.
unsafe impl<'a, PluckedPart, Reference> PluckMut<'a, PluckedPart, IndexHere>
    for Mut<PluckedPart, Reference>
where
    PluckedPart: Part,
    Reference: PartialRef<'a>,
    Reference::Target: HasPart<PluckedPart>,
{
    type Remainder = Reference;
}

/// *(internal)* Skips the constant first part while plucking a constant part.
unsafe impl<'a, PluckedPart, SkippedPart, Reference, Index>
    PluckConst<'a, PluckedPart, IndexNext<Index>> for Const<SkippedPart, Reference>
where
    PluckedPart: Part,
    SkippedPart: Part,
    Reference::Target: HasPart<PluckedPart>,
    Reference::Target: HasPart<SkippedPart>,
    Reference: PluckConst<'a, PluckedPart, Index>,
{
    type Remainder = Const<SkippedPart, Reference::Remainder>;
}

/// *(internal)* Skips the mutable first part while plucking a constant part.
unsafe impl<'a, PluckedPart, SkippedPart, Reference, Index>
    PluckConst<'a, PluckedPart, IndexNext<Index>> for Mut<SkippedPart, Reference>
where
    PluckedPart: Part,
    SkippedPart: Part,
    Reference::Target: HasPart<PluckedPart>,
    Reference::Target: HasPart<SkippedPart>,
    Reference: PluckConst<'a, PluckedPart, Index>,
{
    type Remainder = Mut<SkippedPart, Reference::Remainder>;
}

/// *(internal)* Skips the constant first part while plucking a mutable part.
unsafe impl<'a, PluckedPart, SkippedPart, Reference, Index>
    PluckMut<'a, PluckedPart, IndexNext<Index>> for Const<SkippedPart, Reference>
where
    PluckedPart: Part,
    SkippedPart: Part,
    Reference::Target: HasPart<PluckedPart>,
    Reference::Target: HasPart<SkippedPart>,
    Reference: PluckMut<'a, PluckedPart, Index>,
{
    type Remainder = Const<SkippedPart, Reference::Remainder>;
}

/// *(internal)* Skips the mutable first part while plucking a mutable part.
unsafe impl<'a, PluckedPart, SkippedPart, Reference, Index>
    PluckMut<'a, PluckedPart, IndexNext<Index>> for Mut<SkippedPart, Reference>
where
    PluckedPart: Part,
    SkippedPart: Part,
    Reference::Target: HasPart<PluckedPart>,
    Reference::Target: HasPart<SkippedPart>,
    Reference: PluckMut<'a, PluckedPart, Index>,
{
    type Remainder = Mut<SkippedPart, Reference::Remainder>;
}

/// *(internal)* Pluck a mutable nested subpart of the mutable first part.
///
/// This leaves all other subparts in the remaining reference.
unsafe impl<
        'a,
        ContainingPart,
        PluckedOuter,
        PluckedInner,
        Reference,
        NestedPartIndex,
        Index,
        OuterFieldType,
        ContainingFieldType,
    > PluckMut<'a, Nested<PluckedOuter, PluckedInner>, IndexSplit<NestedPartIndex, Index>>
    for Mut<ContainingPart, Reference>
where
    PluckedOuter: Part<PartType = Field<OuterFieldType>>,
    PluckedInner: Part,
    ContainingPart: Part<PartType = Field<ContainingFieldType>>,
    ContainingFieldType: SplitIntoParts<'a, ContainingPart, Reference>,
    ContainingFieldType: ?Sized,
    ContainingFieldType::ResultMut: PluckMut<'a, Nested<PluckedOuter, PluckedInner>, Index>,
    OuterFieldType: ?Sized,
    OuterFieldType: HasPart<PluckedInner>,
    OuterFieldType: PartialRefTarget<RawTarget = OuterFieldType>,
    Reference: PartialRef<'a>,
    Reference::Target: HasPart<ContainingPart>,
    ContainingPart: ContainsNestedPart<PluckedOuter, NestedPartIndex>,
{
    type Remainder = <ContainingFieldType::ResultMut as PluckMut<
        'a,
        Nested<PluckedOuter, PluckedInner>,
        Index,
    >>::Remainder;
}

/// *(internal)* Pluck a constant nested subpart of the mutable first part.
///
/// This leaves all other subparts as mutable in the remaining reference.
unsafe impl<
        'a,
        ContainingPart,
        PluckedOuter,
        PluckedInner,
        Reference,
        NestedPartIndex,
        Index,
        OuterFieldType,
        ContainingFieldType,
    > PluckConst<'a, Nested<PluckedOuter, PluckedInner>, IndexSplit<NestedPartIndex, Index>>
    for Mut<ContainingPart, Reference>
where
    PluckedOuter: Part<PartType = Field<OuterFieldType>>,
    PluckedInner: Part,
    ContainingPart: Part<PartType = Field<ContainingFieldType>>,
    ContainingFieldType: SplitIntoParts<'a, ContainingPart, Reference>,
    ContainingFieldType: ?Sized,
    ContainingFieldType::ResultMut: PluckConst<'a, Nested<PluckedOuter, PluckedInner>, Index>,
    OuterFieldType: ?Sized,
    OuterFieldType: HasPart<PluckedInner>,
    OuterFieldType: PartialRefTarget<RawTarget = OuterFieldType>,
    Reference: PartialRef<'a>,
    Reference::Target: HasPart<ContainingPart>,
    ContainingPart: ContainsNestedPart<PluckedOuter, NestedPartIndex>,
{
    type Remainder = <ContainingFieldType::ResultMut as PluckConst<
        'a,
        Nested<PluckedOuter, PluckedInner>,
        Index,
    >>::Remainder;
}

/// *(internal)* Pluck a constant nested subpart of the constant first part.
unsafe impl<
        'a,
        ContainingPart,
        PluckedOuter,
        PluckedInner,
        Reference,
        NestedPartIndex,
        Index,
        OuterFieldType,
        ContainingFieldType,
    > PluckConst<'a, Nested<PluckedOuter, PluckedInner>, IndexSplit<NestedPartIndex, Index>>
    for Const<ContainingPart, Reference>
where
    PluckedOuter: Part<PartType = Field<OuterFieldType>>,
    PluckedInner: Part,
    ContainingPart: Part<PartType = Field<ContainingFieldType>>,
    ContainingFieldType: SplitIntoParts<'a, ContainingPart, Reference>,
    ContainingFieldType: ?Sized,
    ContainingFieldType::ResultMut: PluckConst<'a, Nested<PluckedOuter, PluckedInner>, Index>,
    OuterFieldType: ?Sized,
    OuterFieldType: HasPart<PluckedInner>,
    OuterFieldType: PartialRefTarget<RawTarget = OuterFieldType>,
    Reference: PartialRef<'a>,
    Reference::Target: HasPart<ContainingPart>,
    ContainingPart: ContainsNestedPart<PluckedOuter, NestedPartIndex>,
{
    type Remainder = Const<ContainingPart, Reference>;
}

/// *(internal)* Index for the empty subset.
pub struct SubsetIndexEnd;

/// *(internal)* Construct a subset index from a part index and another subset index.
pub struct SubsetIndexCons<PartIndex, TailIndex>(PartIndex, TailIndex);

/// *(internal)* Asserts that the parts of the partial reference `Reference` are a subset of the
/// parts of the partial reference having this trait.
///
/// A list of parts is considered a subset if they can be plucked in sequence.
pub unsafe trait HasSubset<'a, Reference, SubsetIndex>: PartialRef<'a> {
    type Remainder: PartialRef<'a, Target = Self::Target>;
}

/// *(internal)* Every reference has the empty reference as subset.
unsafe impl<'a, Reference> HasSubset<'a, Ref<'a, Reference::Target>, SubsetIndexEnd> for Reference
where
    Reference: PartialRef<'a>,
{
    type Remainder = Reference;
}

/// *(internal)* To have a reference with a constant first part as subset, pluck that part and make
/// sure the remaining references are in a subset relation.
unsafe impl<'a, SubsetPart, Reference, PluckedRef, PartIndex, TailIndex>
    HasSubset<'a, Const<SubsetPart, Reference>, SubsetIndexCons<PartIndex, TailIndex>>
    for PluckedRef
where
    PluckedRef: PluckConst<'a, SubsetPart, PartIndex>,
    <PluckedRef as PluckConst<'a, SubsetPart, PartIndex>>::Remainder:
        HasSubset<'a, Reference, TailIndex>,
    Reference: HasTarget,
{
    type Remainder =
        <<PluckedRef as PluckConst<'a, SubsetPart, PartIndex>>::Remainder as HasSubset<
            'a,
            Reference,
            TailIndex,
        >>::Remainder;
}

/// *(internal)* To have a reference with a mutable first part as subset, pluck that part and make
/// sure the remaining references are in a subset relation.
unsafe impl<'a, SubsetPart, Reference, PluckedRef, PartIndex, TailIndex>
    HasSubset<'a, Mut<SubsetPart, Reference>, SubsetIndexCons<PartIndex, TailIndex>> for PluckedRef
where
    PluckedRef: PluckMut<'a, SubsetPart, PartIndex>,
    <PluckedRef as PluckMut<'a, SubsetPart, PartIndex>>::Remainder:
        HasSubset<'a, Reference, TailIndex>,
    Reference: HasTarget,
{
    type Remainder =
        <<PluckedRef as PluckMut<'a, SubsetPart, PartIndex>>::Remainder as HasSubset<
            'a,
            Reference,
            TailIndex,
        >>::Remainder;
}

// Nesting helpers

/// *(internal)* Check whether a part is nested inside another part.
pub trait ContainsNestedPart<NestedPart, Index>: Part {}

/// *(internal)* A part contains itself as a nested part.
impl<NestedPart> ContainsNestedPart<NestedPart, IndexHere> for NestedPart where NestedPart: Part {}

/// *(internal)* A part contains a nested part if it contains the outer part of the nested part.
impl<NestedPart, Outer, Inner, OuterFieldType, Index>
    ContainsNestedPart<Nested<Outer, Inner>, IndexNext<Index>> for NestedPart
where
    NestedPart: Part,
    Inner: Part,
    NestedPart: ContainsNestedPart<Outer, Index>,
    Outer: Part<PartType = Field<OuterFieldType>>,
    OuterFieldType: ?Sized,
    OuterFieldType: HasPart<Inner>,
    OuterFieldType: PartialRefTarget<RawTarget = OuterFieldType>,
{
}
