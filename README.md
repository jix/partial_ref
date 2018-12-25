# partial_ref - Type checked partial references.

[![crates.io](https://img.shields.io/crates/v/partial_ref.svg)](https://crates.io/crates/partial_ref)
[![docs.rs](https://docs.rs/partial_ref/badge.svg)](https://docs.rs/partial_ref)
[![Build Status](https://travis-ci.com/jix/partial_ref.svg?branch=master)](https://travis-ci.com/jix/partial_ref)
![](https://img.shields.io/crates/l/partial_ref.svg)

This crate provides type checked partial references for rust. Type checked
partial references are one solution to solve
[interprocedural borrowing conflicts][interprocedural-conflicts].

## Example

```rust
use partial_ref::*;

part!(pub Neighbors: Vec<Vec<usize>>);
part!(pub Colors: Vec<usize>);
part!(pub Weights: Vec<f32>);

#[derive(PartialRefTarget, Default)]
pub struct Graph {
    #[part = "Neighbors"]
    pub neighbors: Vec<Vec<usize>>,
    #[part = "Colors"]
    pub colors: Vec<usize>,
    #[part = "Weights"]
    pub weights: Vec<f32>,
}

let mut g = Graph::default();
let mut g_ref = g.into_partial_ref_mut();

g_ref.part_mut(Colors).extend(&[0, 1, 0]);
g_ref.part_mut(Weights).extend(&[0.25, 0.5, 0.75]);

g_ref.part_mut(Neighbors).push(vec![1, 2]);
g_ref.part_mut(Neighbors).push(vec![0, 2]);
g_ref.part_mut(Neighbors).push(vec![0, 1]);

pub fn add_color_to_weight(
    mut g: partial!(Graph, mut Weights, Colors),
    index: usize,
) {
    g.part_mut(Weights)[index] += g.part(Colors)[index] as f32;
}

let (neighbors, mut g_ref) = g_ref.split_part_mut(Neighbors);
let (colors, mut g_ref) = g_ref.split_part(Colors);

for (edges, &color) in neighbors.iter_mut().zip(colors.iter()) {
    edges.retain(|&neighbor| colors[neighbor] != color);

    for &neighbor in edges.iter() {
        add_color_to_weight(g_ref.borrow(), neighbor);
    }
}
```

## Documentation

  * [Reference and Tutorial][docs]

## License

The partial_ref source code is licensed under either of

  * Apache License, Version 2.0
    ([LICENSE-APACHE](LICENSE-APACHE) or
    http://www.apache.org/licenses/LICENSE-2.0)
  * MIT license
    ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in partial_ref by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.

[docs]:https://docs.rs/partial_ref
[interprocedural-conflicts]:http://smallcultfollowing.com/babysteps/blog/2018/11/01/after-nll-interprocedural-conflicts/
