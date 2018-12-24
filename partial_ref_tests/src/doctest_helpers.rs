use partial_ref::{part, partial, IntoPartialRefMut, PartialRef, PartialRefTarget};

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

pub fn example_graph() -> Graph {
    let mut g = Graph::default();
    let mut g_ref = g.into_partial_ref_mut();

    g_ref.part_mut(Colors).extend(&[0, 1, 0]);
    g_ref.part_mut(Weights).extend(&[0.25, 0.5, 0.75]);

    g_ref.part_mut(Neighbors).push(vec![1, 2]);
    g_ref.part_mut(Neighbors).push(vec![0, 2]);
    g_ref.part_mut(Neighbors).push(vec![0, 1]);

    g
}

pub fn add_color_to_weight(mut g: partial!(Graph, mut Weights, Colors), index: usize) {
    g.part_mut(Weights)[index] += g.part(Colors)[index] as f32;
}
