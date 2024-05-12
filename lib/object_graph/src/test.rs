#[cfg(test)]
mod tests {
    use petgraph::dot::Dot;
    use rand::{rngs::StdRng, SeedableRng};

    use crate::generator::*;
    use crate::{fields, str_cached, Cache, NodeIndex, ObjectData, ObjectGraph, PrimitiveValue};

    const SEED: u64 = 10;

    macro_rules! assert_err {
        ($expression:expr, $($pattern:tt)+) => {
            match $expression {
                $($pattern)+ => (),
                ref e => panic!("expected `{}` but got `{:?}`", stringify!($($pattern)+), e),
            }
        }
    }

    #[test]
    fn eq_random_graphs() {
        let cache = Cache::new();
        let mut rng = StdRng::seed_from_u64(SEED);

        let mut g1 = random_gnp_object_graph(&cache, &mut rng, 10, 0.5);
        let mut g2 = g1.clone();
        g1.generate_serialized_data()
            .expect("Failed to serialize g1");
        g2.generate_serialized_data()
            .expect("Failed to serialize g2");
        assert_eq!(g1, g2, "Graphs are not equal");
    }

    #[test]
    fn ne_graphs_field_value() {
        let cache = Cache::new();

        let obj_name_a = str_cached!(cache; "A");
        let obj_name_b = str_cached!(cache; "B");
        let obj_name_c = str_cached!(cache; "C");
        let root_name = str_cached!(cache; "root");
        let field_name_a = str_cached!(cache; "a");
        let field_name_b = str_cached!(cache; "b");
        let field_name_c = str_cached!(cache; "c");

        let mut g1 = ObjectGraph::new();
        let n11 = g1.add_root(
            root_name.clone(),
            ObjectData::new(
                obj_name_a.clone(),
                fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
            ),
        );
        let n12 = g1.add_node(ObjectData::new(
            obj_name_b.clone(),
            fields!((field_name_b.clone(), PrimitiveValue::Number(6u64.into()))),
        ));
        let n13 = g1.add_node(ObjectData::new(
            obj_name_c.clone(),
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        ));
        g1.add_edge(n11, n12, &str_cached!(cache; "12"));
        g1.add_edge(n11, n13, &str_cached!(cache; "13"));

        let mut g2 = ObjectGraph::new();
        let n22: NodeIndex = g2.add_node(ObjectData::new(
            obj_name_b,
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        ));
        let n21 = g2.add_root(
            root_name.clone(),
            ObjectData::new(
                obj_name_a,
                fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
            ),
        );
        g2.add_edge(n21, n22, &str_cached!(cache; "12"));
        let n23 = g2.add_node(ObjectData::new(
            obj_name_c,
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        ));
        g2.add_edge(n21, n23, &str_cached!(cache; "13"));

        g1.generate_serialized_data()
            .expect("Failed to serialize g1");
        g2.generate_serialized_data()
            .expect("Failed to serialize g2");
        assert_ne!(g1, g2, "Graphs are  equal");
    }

    #[test]
    fn ne_graphs_field_name() {
        let cache = Cache::new();

        let obj_name_a = str_cached!(cache; "A");
        let obj_name_b = str_cached!(cache; "B");
        let obj_name_c = str_cached!(cache; "C");
        let root_name = str_cached!(cache; "root");
        let field_name_a = str_cached!(cache; "a");
        let field_name_b = str_cached!(cache; "b");
        let field_name_b2 = str_cached!(cache; "b2");
        let field_name_c = str_cached!(cache; "c");

        let mut g1 = ObjectGraph::new();
        let n11 = g1.add_root(
            root_name.clone(),
            ObjectData::new(
                obj_name_a.clone(),
                fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
            ),
        );
        let n12 = g1.add_node(ObjectData::new(
            obj_name_b.clone(),
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        ));
        let n13 = g1.add_node(ObjectData::new(
            obj_name_c.clone(),
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        ));
        g1.add_edge(n11, n12, &str_cached!(cache; "12"));
        g1.add_edge(n11, n13, &str_cached!(cache; "13"));

        let mut g2 = ObjectGraph::new();
        let n22: NodeIndex = g2.add_node(ObjectData::new(
            obj_name_b,
            fields!((field_name_b2.clone(), PrimitiveValue::Number(5u64.into()))),
        ));
        let n21 = g2.add_root(
            root_name.clone(),
            ObjectData::new(
                obj_name_a,
                fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
            ),
        );
        g2.add_edge(n21, n22, &str_cached!(cache; "12"));
        let n23 = g2.add_node(ObjectData::new(
            obj_name_c,
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        ));
        g2.add_edge(n21, n23, &str_cached!(cache; "13"));

        g1.generate_serialized_data()
            .expect("Failed to serialize g1");
        g2.generate_serialized_data()
            .expect("Failed to serialize g2");
        assert_ne!(g1, g2, "Graphs are equal");
    }

    #[test]
    fn ne_graphs_field_type() {
        let cache = Cache::new();

        let obj_name_a = str_cached!(cache; "A");
        let obj_name_b = str_cached!(cache; "B");
        let obj_name_b2 = str_cached!(cache; "B2");
        let obj_name_c = str_cached!(cache; "C");
        let root_name = str_cached!(cache; "root");
        let field_name_a = str_cached!(cache; "a");
        let field_name_b = str_cached!(cache; "b");
        let field_name_c = str_cached!(cache; "c");

        let mut g1 = ObjectGraph::new();
        let n11 = g1.add_root(
            root_name.clone(),
            ObjectData::new(
                obj_name_a.clone(),
                fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
            ),
        );
        let n12 = g1.add_node(ObjectData::new(
            obj_name_b.clone(),
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        ));
        let n13 = g1.add_node(ObjectData::new(
            obj_name_c.clone(),
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        ));
        g1.add_edge(n11, n12, &str_cached!(cache; "12"));
        g1.add_edge(n11, n13, &str_cached!(cache; "13"));

        let mut g2 = ObjectGraph::new();
        let n22: NodeIndex = g2.add_node(ObjectData::new(
            obj_name_b2,
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        ));
        let n21 = g2.add_root(
            root_name.clone(),
            ObjectData::new(
                obj_name_a,
                fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
            ),
        );
        g2.add_edge(n21, n22, &str_cached!(cache; "12"));
        let n23 = g2.add_node(ObjectData::new(
            obj_name_c,
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        ));
        g2.add_edge(n21, n23, &str_cached!(cache; "13"));

        g1.generate_serialized_data()
            .expect("Failed to serialize g1");
        g2.generate_serialized_data()
            .expect("Failed to serialize g2");
        assert_ne!(g1, g2, "Graphs are equal");
    }

    #[test]
    fn eq_graphs() {
        let cache = Cache::new();

        let obj_name_a = str_cached!(cache; "A");
        let obj_name_b = str_cached!(cache; "B");
        let obj_name_c = str_cached!(cache; "C");
        let root_name = str_cached!(cache; "root");
        let field_name_a = str_cached!(cache; "a");
        let field_name_b = str_cached!(cache; "b");
        let field_name_c = str_cached!(cache; "c");

        let mut g1 = ObjectGraph::new();
        let n11 = g1.add_root(
            root_name.clone(),
            ObjectData::new(
                obj_name_a.clone(),
                fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
            ),
        );
        let n12 = g1.add_node(ObjectData::new(
            obj_name_b.clone(),
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        ));
        let n13 = g1.add_node(ObjectData::new(
            obj_name_c.clone(),
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        ));
        g1.add_edge(n11, n12, &str_cached!(cache; "12"));
        g1.add_edge(n11, n13, &str_cached!(cache; "13"));

        let mut g2 = ObjectGraph::new();
        let n22: NodeIndex = g2.add_node(ObjectData::new(
            obj_name_b,
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        ));
        let n21 = g2.add_root(
            root_name.clone(),
            ObjectData::new(
                obj_name_a,
                fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
            ),
        );
        g2.add_edge(n21, n22, &str_cached!(cache; "12"));
        let n23 = g2.add_node(ObjectData::new(
            obj_name_c,
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        ));
        g2.add_edge(n21, n23, &str_cached!(cache; "13"));

        g1.generate_serialized_data()
            .expect("Failed to serialize g1");
        g2.generate_serialized_data()
            .expect("Failed to serialize g2");
        assert_eq!(g1, g2, "Graphs are not equal");
    }

    #[test]
    fn eq_random_graphs_serialized_data() {
        let cache = Cache::new();
        let mut rng = StdRng::seed_from_u64(SEED);

        let mut g1 = random_gnp_object_graph(&cache, &mut rng, 10, 0.5);
        let mut g2 = g1.clone();
        g1.generate_serialized_data()
            .expect("Failed to serialize g1");
        g2.generate_serialized_data()
            .expect("Failed to serialize g2");
        assert_eq!(g1.serialized, g2.serialized, "Graphs are not equal");
    }

    #[test]
    fn eq_graphs_2_rng() {
        let cache = Cache::new();

        let mut rng1 = StdRng::seed_from_u64(SEED);
        let mut g1 = random_gnp_object_graph(&cache, &mut rng1, 10, 0.5);
        let mut rng2 = StdRng::seed_from_u64(SEED);
        let mut g2 = random_gnp_object_graph(&cache, &mut rng2, 10, 0.5);
        g1.generate_serialized_data()
            .expect("Failed to serialize g1");
        g2.generate_serialized_data()
            .expect("Failed to serialize g2");
        assert_eq!(g1, g2, "Graphs are not equal");
    }

    #[test]
    fn print_graph() {
        let cache = Cache::new();

        let mut graph = ObjectGraph::new();
        let obj_name = str_cached!(cache; "A");
        let field_name = str_cached!(cache; "a");
        let n1 = graph.add_node(ObjectData::new(
            obj_name.clone(),
            fields!((field_name.clone(), PrimitiveValue::Number(3u64.into()))),
        ));
        let n2 = graph.add_node(ObjectData::new(
            obj_name,
            fields!((field_name.clone(), PrimitiveValue::Number(4u64.into()))),
        ));
        graph.add_edge(n1, n2, &str_cached!(cache; "c"));
        graph
            .generate_serialized_data()
            .expect("Failed to serialize graph");
        assert_ne!(graph.hash, 0);

        println!("{:?}", Dot::new(&graph.graph));
    }

    #[test]
    fn serialize_graph() {
        let cache = Cache::new();
        let mut rng = StdRng::seed_from_u64(SEED);

        let n = 10;
        let mut g = random_gnp_object_graph(&cache, &mut rng, n.try_into().unwrap(), 0.5);
        assert_err!(g.generate_serialized_data(), Ok(()));
        println!("{:?}", Dot::new(&g.graph));
        println!("{:?}", g.serialized.unwrap());
    }
}
