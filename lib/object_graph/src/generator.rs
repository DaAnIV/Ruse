#[cfg(feature = "generators")]
pub mod object_graph_generator {
    use crate::*;
    use petgraph::graph::DiGraph;
    use petgraph::visit::{self, EdgeRef};
    use petgraph::visit::{VisitMap, Visitable};
    use petgraph_gen::{random_gnm_graph, random_gnp_graph};
    use rand::Rng;
    use std::collections::HashMap;

    fn get_unseen_index(n: u32, seen: &mut dyn VisitMap<u32>) -> Option<u32> {
        (0..n).find(|i| seen.visit(*i))
    }

    pub fn generate_random_str<R: Rng + ?Sized>(max_size: usize, rng: &mut R) -> String {
        let str_size = rng.gen_range(0..=max_size);
        let mut rstr = Vec::with_capacity(str_size);
        for _ in 0..max_size {
            rstr.push(rng.gen_range(('a' as u16)..=('z' as u16)));
        }
        String::from_utf16(&rstr).expect("Bad random string")
    }

    fn graph_to_random_object_graph<R: Rng + ?Sized>(
        graph_id: GraphIndex,
        rng: &mut R,
        base: DiGraph<(), ()>,
    ) -> GraphsMap {
        let mut id = NodeIndex(0);
        let n = base.node_count();
        let mut discovered = base.visit_map();

        let mut map =
            HashMap::<petgraph::graph::NodeIndex, indices::NodeIndex>::with_capacity(n);
        let mut graphs_map = GraphsMap::new();
        graphs_map.ensure_graph(graph_id);
        let root_field_string = field_name!("r");
        let data_field_string = field_name!("a");
        while let Some(r) = get_unseen_index(n.try_into().unwrap(), &mut discovered) {
            let root_fields = fields!((
                root_field_string.clone(),
                PrimitiveValue::Number(rng.next_u64().into())
            ));
            let root_name = root_name!(generate_random_str(5, rng).to_ascii_uppercase());
            let obj_type =
                ObjectType::class_obj_type(&generate_random_str(5, rng).to_ascii_uppercase());
            graphs_map.add_simple_object(graph_id, id, obj_type, root_fields);
            graphs_map.set_as_root(root_name, graph_id, id);
            map.insert(r.into(), id);
            id += 1;

            let mut bfs = visit::Bfs::new(&base, r.into());
            while let Some(nx) = bfs.next(&base) {
                if discovered.visit(nx) {
                    let data_fields = fields!((
                        data_field_string.clone(),
                        PrimitiveValue::Number(rng.next_u64().into())
                    ));
                    let obj_type = ObjectType::class_obj_type(
                        &generate_random_str(5, rng).to_ascii_uppercase(),
                    );
                    graphs_map.add_simple_object(graph_id, id, obj_type, data_fields);
                    map.insert(nx, id);
                    id += 1
                }
            }
        }

        for edge in base.edge_references() {
            let object = map.get(&edge.source()).unwrap();
            let field = map.get(&edge.target()).unwrap();
            let edge_name = field_name!(format!("{}_{}", object.index(), field.index()));
            graphs_map.set_edge(edge_name, graph_id, *object, graph_id, *field);
        }

        graphs_map
    }

    pub fn random_gnp_object_graph<R: Rng + ?Sized>(
        graph_id: GraphIndex,
        rng: &mut R,
        n: usize,
        p: f64,
    ) -> GraphsMap {
        let base: DiGraph<(), ()> = random_gnp_graph(rng, n, p);
        graph_to_random_object_graph(graph_id, rng, base)
    }

    pub fn random_gnm_object_graph<R: Rng + ?Sized>(
        graph_id: GraphIndex,
        rng: &mut R,
        n: usize,
        m: usize,
    ) -> GraphsMap {
        let base: DiGraph<(), ()> = random_gnm_graph(rng, n, m);
        graph_to_random_object_graph(graph_id, rng, base)
    }
}
