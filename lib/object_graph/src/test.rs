#[cfg(test)]
mod tests {
    use petgraph::dot::Dot;
    use rand::{rngs::StdRng, SeedableRng};

    use crate::{Cache, str_cached, ObjectGraph, ObjectData, fields, PrimitiveValue, NodeIndex};
    use crate::generator::*;

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
        let mut cache = Cache::new();
        let mut rng = StdRng::seed_from_u64(SEED);

        let mut g1 = random_gnp_object_graph(&mut cache, &mut rng, 10, 0.5);
        let mut g2 = g1.clone();
        g1.generate_serialized_data()
            .expect("Failed to serialize g1");
        g2.generate_serialized_data()
            .expect("Failed to serialize g2");
        assert_eq!(g1, g2, "Graphs are not equal");
    }

    #[test]
    fn eq_graphs() {
        let mut cache = Cache::new();

        let root_name = str_cached!(cache; "root");
        let field_name_a = str_cached!(cache; "a");
        let field_name_b = str_cached!(cache; "b");
        let field_name_c = str_cached!(cache; "c");

        let mut g1 = ObjectGraph::new();
        let n11 = g1.add_root(root_name.clone(), ObjectData::new(
            fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
        ));
        let n12 = g1.add_node(ObjectData::new(
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        ));
        let n13 = g1.add_node(ObjectData::new(
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        ));
        g1.add_edge(n11, n12, &str_cached!(cache; "12"));
        g1.add_edge(n11, n13, &str_cached!(cache; "13"));

        let mut g2 = ObjectGraph::new();
        let n22: NodeIndex = g2.add_node(ObjectData::new(
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        ));
        let n21 = g2.add_root(root_name.clone(), ObjectData::new(
            fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
        ));
        g2.add_edge(n21, n22, &str_cached!(cache; "12"));
        let n23 = g2.add_node(ObjectData::new(
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
        let mut cache = Cache::new();
        let mut rng = StdRng::seed_from_u64(SEED);

        let mut g1 = random_gnp_object_graph(&mut cache, &mut rng, 10, 0.5);
        let mut g2 = g1.clone();
        g1.generate_serialized_data()
            .expect("Failed to serialize g1");
        g2.generate_serialized_data()
            .expect("Failed to serialize g2");
        assert_eq!(g1.serialized, g2.serialized, "Graphs are not equal");
    }

    #[test]
    fn eq_graphs_2_rng() {
        let mut cache = Cache::new();

        let mut rng1 = StdRng::seed_from_u64(SEED);
        let mut g1 = random_gnp_object_graph(&mut cache, &mut rng1, 10, 0.5);
        let mut rng2 = StdRng::seed_from_u64(SEED);
        let mut g2 = random_gnp_object_graph(&mut cache, &mut rng2, 10, 0.5);
        g1.generate_serialized_data()
            .expect("Failed to serialize g1");
        g2.generate_serialized_data()
            .expect("Failed to serialize g2");
        assert_eq!(g1, g2, "Graphs are not equal");
    }

    #[test]
    fn print_graph() {
        let mut cache = Cache::new();

        let mut graph = ObjectGraph::new();
        let field_name = str_cached!(cache; "a");
        let n1 = graph.add_node(ObjectData::new(
            fields!((field_name.clone(), PrimitiveValue::Number(3u64.into()))),
        ));
        let n2 = graph.add_node(ObjectData::new(
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
        let mut cache = Cache::new();
        let mut rng = StdRng::seed_from_u64(SEED);

        let n = 10;
        let mut g = random_gnp_object_graph(&mut cache, &mut rng, n.try_into().unwrap(), 0.5);
        assert_err!(g.generate_serialized_data(), Ok(()));
        println!("{:?}", Dot::new(&g.graph));
        println!("{:?}", g.serialized.unwrap());
    }
}
