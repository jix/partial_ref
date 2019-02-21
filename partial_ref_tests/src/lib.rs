pub mod doctest_helpers;

pub mod isolated {
    // This module imports just the minimum required to partially test macro hygiene
    use partial_ref::{part, PartialRefTarget};
    #[derive(Debug, PartialRefTarget)]
    #[part = "PartC"]
    #[part = "PartD"]
    pub struct Foo {
        #[part = "PartA"]
        pub a: u32,
        #[part = "PartB"]
        pub b: u32,
    }

    part!(pub PartC);
    part!(pub PartD);
    part!(pub PartA: u32);
    part!(pub PartB: u32);
}

use self::isolated::*;
use partial_ref::*;

pub fn swap_a_and_b<T>(mut foo: partial!(T, mut PartA, mut PartB))
where
    T: PartialRefTarget,
    T: HasPart<PartA>,
    T: HasPart<PartB>,
{
    split_borrow!(a, b = &(mut PartA) foo);

    std::mem::swap(a.part_mut(PartA), b.part_mut(PartB))
}

pub fn swap_a_and_b_alt<T>(mut foo: partial!(T, mut PartA, mut PartB))
where
    T: PartialRefTarget,
    T: HasPart<PartA>,
    T: HasPart<PartB>,
{
    let (a, mut foo) = foo.split_part_mut(PartA);
    let (b, _) = foo.split_part_mut(PartB);

    std::mem::swap(a, b);
}

pub fn swap_a_and_b_lt<'a, T>(mut foo: partial!('a T, mut PartA, mut PartB))
where
    T: PartialRefTarget,
    T: HasPart<PartA>,
    T: HasPart<PartB>,
{
    split_borrow!(a, b = &(mut PartA) foo);

    std::mem::swap(a.part_mut(PartA), b.part_mut(PartB))
}

#[test]
fn test_swap_a_and_b() {
    let mut f = Foo { a: 1, b: 2 };

    let mut fr = f.into_partial_ref_mut();

    swap_a_and_b(fr.borrow());

    assert_eq!(*fr.part(PartA), 2);
    assert_eq!(*fr.part(PartB), 1);
}

#[test]
fn test_swap_a_and_b_alt() {
    let mut f = Foo { a: 1, b: 2 };

    let mut fr = f.into_partial_ref_mut();

    split_borrow!(x = &(mut PartC) fr);

    swap_a_and_b_alt(fr.borrow());

    assert_eq!(*fr.part(PartA), 2);
    assert_eq!(*fr.part(PartB), 1);

    drop(x);
}

#[test]
fn test_swap_a_and_b_lt() {
    let mut f = Foo { a: 1, b: 2 };

    let mut fr = f.into_partial_ref_mut();

    swap_a_and_b_lt(fr.borrow());

    assert_eq!(*fr.part(PartA), 2);
    assert_eq!(*fr.part(PartB), 1);
}

#[derive(Debug, PartialRefTarget)]
struct Bar {
    #[part = "PartFoo"]
    foo: Foo,
    #[part = "PartA"]
    a: u32,
}

part!(PartFoo: Foo);

#[derive(Debug, PartialRefTarget)]
struct Baz {
    #[part = "PartBar"]
    bar: Bar,
    #[part = "PartA"]
    a: u32,
}

part!(PartBar: Bar);

#[test]
fn test_nested_mut() {
    let mut s = Bar {
        foo: Foo { a: 1, b: 2 },
        a: 3,
    };

    let mut sr = s.into_partial_ref_mut();

    split_borrow!(foo_a = &(mut PartFoo | PartA) sr);

    std::mem::swap(foo_a.part_mut(PartFoo | PartA), sr.part_mut(PartA));

    assert_eq!(s.foo.a, 3);
    assert_eq!(s.a, 1);
}

#[test]
fn test_deeply_nested_mut() {
    let mut s = Baz {
        bar: Bar {
            foo: Foo { a: 1, b: 2 },
            a: 3,
        },
        a: 7,
    };

    let mut sr = s.into_partial_ref_mut();

    split_borrow!(foo_a = &(mut PartBar | PartFoo | PartA) sr);

    std::mem::swap(
        foo_a.part_mut(PartBar | PartFoo | PartA),
        sr.part_mut(PartA),
    );

    assert_eq!(s.bar.foo.a, 7);
    assert_eq!(s.a, 1);
}

#[test]
fn test_nested_mut_to_const() {
    let mut s = Bar {
        foo: Foo { a: 1, b: 2 },
        a: 3,
    };

    let mut sr = s.into_partial_ref_mut();

    split_borrow!(foo_a = &(PartFoo | PartA) sr);

    *sr.part_mut(PartA) = *foo_a.part(PartFoo | PartA);

    // make sure other parts are still mutable
    *sr.part_mut(PartFoo | PartB) = 10;

    assert_eq!(s.foo.a, 1);
    assert_eq!(s.foo.b, 10);
    assert_eq!(s.a, 1);
}

#[test]
fn test_nested_const() {
    let mut s = Bar {
        foo: Foo { a: 1, b: 2 },
        a: 3,
    };

    let mut sr = s.into_partial_ref_mut();

    split_borrow!(src = &(PartFoo, mut PartA) sr);

    split_borrow!(foo_a = &(PartFoo | PartA) src);

    *src.part_mut(PartA) = *foo_a.part(PartFoo | PartA);

    assert_eq!(s.foo.a, 1);
    assert_eq!(s.a, 1);
}
