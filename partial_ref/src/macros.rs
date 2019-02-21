/// Declares a [`Part`].
///
/// Defines an empty struct type and derives all necessary instances to use it as a marker type for
/// parts.
///
/// This macro can define [`AbstractPart`]s using `part!(PartName);` or `part!(pub PartName);` and
/// [`Field`] parts using `part!(PartName: FieldType);` or `part!(pub PartName: FieldType);`.
#[macro_export]
macro_rules! part {
    ($part:ident) => { part!(@template $part () ($crate::AbstractPart)); };
    (pub $part:ident) => { part!(@template $part (pub) ($crate::AbstractPart)); };
    ($part:ident : $field:ty) => { part!(@template $part () ($crate::Field<$field>)); };
    (pub $part:ident : $field:ty) => { part!(@template $part (pub) ($crate::Field<$field>)); };
    (@template $part:ident ($($vis:tt)*) ($($part_type:tt)*)) => {
        #[derive(Default)]
        $($vis)* struct $part;
        impl $crate::Part for $part {
            type PartType = $($part_type)*;
        }

        // TODO maybe constrain InnerPart
        impl<InnerPart: $crate::Part> ::std::ops::BitOr<InnerPart> for $part {
            type Output = $crate::Nested<$part, InnerPart>;

            fn bitor(self, _rhs: InnerPart) -> Self::Output {
                std::default::Default::default()
            }
        }
    }
}

/// Concise syntax for partial reference types.
///
/// The first parameter is the reference target type, optionally preceded by a lifetime. The
/// following parameters are the referenced parts, each optionally preceded by the keyword `mut` to
/// indicate a mutable reference to that part. Nested parts can be specified using the pipe syntax
/// of [`nested_part`].
///
/// Examples:
///   * `partial!(Ty, PartA)` is `Const<PartA, Ref<Ty>>`
///   * `partial!('a Ty, mut PartA)` is `Mut<PartA, Ref<'a, Ty>>`
///   * `partial!(Ty, PartA, PartB, mut PartC)` is
///     `Const<PartA, Const<PartB, Mut<PartC, Ref<Ty>>`
///   * `partial!('a Ty, mut PartA | PartB)` is `Mut<Nested<PartA, PartB>, Ref<'a, Ty>>`
#[macro_export]
macro_rules! partial {
    ($target_lt:lifetime $target:ty) => { $crate::Ref<$target_lt, $target> };
    ($target_lt:lifetime $target:ty, $($parts:tt)*) => {
        partial!(@extend $crate::Ref<$target_lt, $target>, $($parts)*)
    };
    ($target:ty) => { $crate::Ref<$target> };
    ($target:ty, $($parts:tt)* ) => {
        partial!(@extend $crate::Ref<$target>, $($parts)*)
    };
    (@extend $target:ty) => {$target};
    (@extend $target:ty, ) => {$target};
    (@extend $target:ty, mut $($part:ty)|* , $($rest:tt)*) => {
        partial!(@extend $crate::Mut<$crate::nested_part!($($part)|*), $target>, $($rest)*)
    };
    (@extend $target:ty, mut $($part:ty)|*) => {
        partial!(@extend $target, mut $($part)|*, )
    };
    (@extend $target:ty, $($part:ty)|* , $($rest:tt)*) => {
        partial!(@extend $crate::Const<$crate::nested_part!($($part)|*), $target>, $($rest)*)
    };
    (@extend $target:ty, $($part:ty)|*) => {
        partial!(@extend $target, $($part)|*, )
    };
}

/// Expands `A | B | ... | Z` to `Nested<...Nested<A, B>, ..., Z>`
///
/// This mirrors the syntax for the expressions that can be passed to [`PartialRef`]'s part
/// functions.
#[macro_export]
macro_rules! nested_part {
    ($target:ty) => { $target };
    ($target:ty | $part:ty) => {
        $crate::Nested<$target , $part>
    };
    ($target:ty | $part:ty | $($subparts:ty)|*) => {
        nested_part!($crate::Nested<$target , $part> | $($subparts)|*)
    };
}

/// Helper macro for splitting a partial reference.
///
/// The statement `split_borrow!(target, rest = &(...) expr)` where `...` is a list of parts (same
/// syntax as used for [`partial`]) splits the partial reference returned by expr and binds the listed
/// parts to the identifier `target` and the remaining parts to the identifier `rest`. This is done
/// using [`PartialRef`]'s split_borrow method.
///
/// As shorter version is available as `split_borrow!(target = &(...) ident)` which is identical to
/// `split_borrow!(target, ident = &(...) ident)`. This shadows the existing partial reference with
/// a new binding.
#[macro_export]
macro_rules! split_borrow {
    ($target:ident = &($($parts:tt)*) $from:ident) => {
        split_borrow!($target, $from = &($($parts)*) $from)
    };
    ($target:ident, $rest:ident = &($($parts:tt)*) $from:expr) => {
        #[allow(unused_variables, unused_mut)]
        let (mut $target, mut $rest) = ($from).split_borrow::<partial!(_, $($parts)*), _>();
    };
}
