#[cfg(test)]
mod tests {
    use std::hash::{DefaultHasher, Hash, Hasher};

    use rand::{rngs::StdRng, SeedableRng};

    use crate::dot;
    use crate::generator::object_graph_generator::*;
    use crate::graph_map_value::GraphMapWrap;
    use crate::GraphsMap;
    use crate::{fields, str_cached, Cache, NodeIndex, ObjectGraph, PrimitiveValue};

    const SEED: u64 = 10;

    #[test]
    fn eq_random_graphs() {
        let cache = Cache::new();
        let mut rng = StdRng::seed_from_u64(SEED);

        let graphs_map_1 = random_gnp_object_graph(&cache, 0, &mut rng, 10, 0.5);
        let graphs_map_2 = graphs_map_1.clone();
        assert_eq!(graphs_map_1, graphs_map_2, "Graphs are not equal");
    }

    #[test]
    fn ne_graphs_field_value() {
        let cache = Cache::new();
        let mut graphs_map_1 = GraphsMap::default();
        let mut graphs_map_2 = GraphsMap::default();

        let obj_name_a = str_cached!(&cache; "A");
        let obj_name_b = str_cached!(&cache; "B");
        let obj_name_c = str_cached!(&cache; "C");
        let root_name = str_cached!(&cache; "root");
        let field_name_a = str_cached!(&cache; "a");
        let field_name_b = str_cached!(&cache; "b");
        let field_name_c = str_cached!(&cache; "c");

        let mut g1 = ObjectGraph::new(0);
        let n11 = g1.add_simple_object(
            NodeIndex(0),
            obj_name_a.clone(),
            fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
        );

        let n12 = g1.add_simple_object(
            NodeIndex(1),
            obj_name_b.clone(),
            fields!((field_name_b.clone(), PrimitiveValue::Number(6u64.into()))),
        );
        let n13 = g1.add_simple_object(
            NodeIndex(2),
            obj_name_c.clone(),
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        );
        g1.set_edge(&n11, n12, str_cached!(&cache; "12"));
        g1.set_edge(&n11, n13, str_cached!(&cache; "13"));

        let mut g2 = ObjectGraph::new(1);
        let n22: NodeIndex = g2.add_simple_object(
            NodeIndex(0),
            obj_name_b,
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        );
        let n21 = g2.add_simple_object(
            NodeIndex(1),
            obj_name_a,
            fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
        );
        g2.set_edge(&n21, n22, str_cached!(&cache; "12"));
        let n23 = g2.add_simple_object(
            NodeIndex(2),
            obj_name_c,
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        );
        g2.set_edge(&n21, n23, str_cached!(&cache; "13"));

        graphs_map_1.insert_graph(g1.into());
        graphs_map_2.insert_graph(g2.into());
        graphs_map_1.set_as_root(root_name.clone(), 0, n11);
        graphs_map_2.set_as_root(root_name, 1, n21);
        assert_ne!(&graphs_map_1, &graphs_map_2, "Graphs are equal");
    }

    #[test]
    fn ne_graphs_field_name() {
        let cache = Cache::new();
        let mut map1 = GraphsMap::default();
        let mut map2 = GraphsMap::default();

        let obj_name_a = str_cached!(&cache; "A");
        let obj_name_b = str_cached!(&cache; "B");
        let obj_name_c = str_cached!(&cache; "C");
        let root_name = str_cached!(&cache; "root");
        let field_name_a = str_cached!(&cache; "a");
        let field_name_b = str_cached!(&cache; "b");
        let field_name_b2 = str_cached!(&cache; "b2");
        let field_name_c = str_cached!(&cache; "c");

        let mut g1 = ObjectGraph::new(0);
        let n11 = g1.add_simple_object(
            NodeIndex(0),
            obj_name_a.clone(),
            fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
        );
        let n12 = g1.add_simple_object(
            NodeIndex(1),
            obj_name_b.clone(),
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        );
        let n13 = g1.add_simple_object(
            NodeIndex(2),
            obj_name_c.clone(),
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        );
        g1.set_edge(&n11, n12, str_cached!(&cache; "12"));
        g1.set_edge(&n11, n13, str_cached!(&cache; "13"));

        let mut g2 = ObjectGraph::new(1);
        let n22: NodeIndex = g2.add_simple_object(
            NodeIndex(0),
            obj_name_b,
            fields!((field_name_b2.clone(), PrimitiveValue::Number(5u64.into()))),
        );
        let n21 = g2.add_simple_object(
            NodeIndex(1),
            obj_name_a,
            fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
        );
        g2.set_edge(&n21, n22, str_cached!(&cache; "12"));
        let n23 = g2.add_simple_object(
            NodeIndex(2),
            obj_name_c,
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        );
        g2.set_edge(&n21, n23, str_cached!(&cache; "13"));

        map1.insert_graph(g1.into());
        map1.set_as_root(root_name.clone(), 0, n11);
        map2.insert_graph(g2.into());
        map2.set_as_root(root_name, 1, n21);
        assert_ne!(map1, map2, "Graphs are equal");
    }

    #[test]
    fn ne_graphs_field_type() {
        let cache = Cache::new();
        let mut map1 = GraphsMap::new();
        let mut map2 = GraphsMap::new();

        let obj_name_a = str_cached!(&cache; "A");
        let obj_name_b = str_cached!(&cache; "B");
        let obj_name_b2 = str_cached!(&cache; "B2");
        let obj_name_c = str_cached!(&cache; "C");
        let root_name = str_cached!(&cache; "root");
        let field_name_a = str_cached!(&cache; "a");
        let field_name_b = str_cached!(&cache; "b");
        let field_name_c = str_cached!(&cache; "c");

        let mut g1 = ObjectGraph::new(0);
        let n11 = g1.add_simple_object(
            NodeIndex(0),
            obj_name_a.clone(),
            fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
        );
        let n12 = g1.add_simple_object(
            NodeIndex(1),
            obj_name_b.clone(),
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        );
        let n13 = g1.add_simple_object(
            NodeIndex(2),
            obj_name_c.clone(),
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        );
        g1.set_edge(&n11, n12, str_cached!(&cache; "12"));
        g1.set_edge(&n11, n13, str_cached!(&cache; "13"));

        let mut g2 = ObjectGraph::new(1);
        let n22: NodeIndex = g2.add_simple_object(
            NodeIndex(0),
            obj_name_b2,
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        );
        let n21 = g2.add_simple_object(
            NodeIndex(1),
            obj_name_a,
            fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
        );
        g2.set_edge(&n21, n22, str_cached!(&cache; "12"));
        let n23 = g2.add_simple_object(
            NodeIndex(2),
            obj_name_c,
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        );
        g2.set_edge(&n21, n23, str_cached!(&cache; "13"));

        map1.insert_graph(g1.into());
        map1.set_as_root(root_name.clone(), 0, n11);
        map2.insert_graph(g2.into());
        map2.set_as_root(root_name.clone(), 1, n21);
        assert_ne!(map1, map2, "Graphs are equal");
    }

    #[test]
    fn eq_graphs() {
        let cache = Cache::new();
        let mut map1 = GraphsMap::new();
        let mut map2 = GraphsMap::new();

        let obj_name_a = str_cached!(&cache; "A");
        let obj_name_b = str_cached!(&cache; "B");
        let obj_name_c = str_cached!(&cache; "C");
        let root_name = str_cached!(&cache; "root");
        let field_name_a = str_cached!(&cache; "a");
        let field_name_b = str_cached!(&cache; "b");
        let field_name_c = str_cached!(&cache; "c");

        let mut g1 = ObjectGraph::new(0);
        let n11 = g1.add_simple_object(
            NodeIndex(0),
            obj_name_a.clone(),
            fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
        );
        let n12 = g1.add_simple_object(
            NodeIndex(1),
            obj_name_b.clone(),
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        );
        let n13 = g1.add_simple_object(
            NodeIndex(2),
            obj_name_c.clone(),
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        );
        g1.set_edge(&n11, n12, str_cached!(&cache; "12"));
        g1.set_edge(&n11, n13, str_cached!(&cache; "13"));

        let mut g2 = ObjectGraph::new(1);
        let n22: NodeIndex = g2.add_simple_object(
            NodeIndex(0),
            obj_name_b,
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        );
        let n21 = g2.add_simple_object(
            NodeIndex(1),
            obj_name_a,
            fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
        );
        g2.set_edge(&n21, n22, str_cached!(&cache; "12"));
        let n23 = g2.add_simple_object(
            NodeIndex(2),
            obj_name_c,
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        );
        g2.set_edge(&n21, n23, str_cached!(&cache; "13"));

        map1.insert_graph(g1.into());
        map1.set_as_root(root_name.clone(), 0, n11);
        map2.insert_graph(g2.into());
        map2.set_as_root(root_name, 1, n21);
        assert_eq!(map1, map2, "Graphs are not equal");
    }

    // #[test]
    // fn eq_random_graphs_serialized_data() {
    //     let cache = Cache::new();
    //     let mut rng = StdRng::seed_from_u64(SEED);

    //     let mut g1 = random_gnp_object_graph(&cache, &mut rng, 10, 0.5);
    //     let mut g2 = g1.clone();
    //     g1.generate_serialized_data();
    //     g2.generate_serialized_data();
    //     assert_eq!(g1.serialized, g2.serialized, "Graphs are not equal");
    // }

    #[test]
    fn eq_graphs_2_rng() {
        let cache = Cache::new();

        let mut rng1 = StdRng::seed_from_u64(SEED);
        let map1 = random_gnp_object_graph(&cache, 0, &mut rng1, 10, 0.5);
        let mut rng2 = StdRng::seed_from_u64(SEED);
        let map2 = random_gnp_object_graph(&cache, 1, &mut rng2, 10, 0.5);

        assert_eq!(map1, map2,
            "Graphs are not equal"
        );
    }

    // #[test]
    // fn eq_graphs_2_rng_check_hash() {
    //     let cache = Cache::new();

    //     let mut rng1 = StdRng::seed_from_u64(SEED);
    //     let map1 = random_gnp_object_graph(&cache, 0, &mut rng1, 10, 0.5);
    //     let mut rng2 = StdRng::seed_from_u64(SEED);
    //     let map2 = random_gnp_object_graph(&cache, 1, &mut rng2, 10, 0.5);

    //     let mut s = DefaultHasher::new();
    //     map1.hash(&mut s);
    //     let g1_hash = s.finish();

    //     s = DefaultHasher::new();
    //     map2.hash(&mut s);
    //     let g2_hash = s.finish();
    //     assert_eq!(g1_hash, g2_hash, "Graphs hashes are not equal");
    // }

    // #[test]
    // fn neq_graphs_2_rng_check_hash() {
    //     let cache = Cache::new();
    //     let mut graphs_map = GraphsMap::default();

    //     let mut rng1 = StdRng::seed_from_u64(SEED);
    //     let map1 = random_gnp_object_graph(&cache, 0, &mut rng1, 10, 0.5);
    //     let mut rng2 = StdRng::seed_from_u64(SEED * 10);
    //     let map2 = random_gnp_object_graph(&cache, 1, &mut rng2, 10, 0.5);

    //     let mut s = DefaultHasher::new();
    //     map1.hash(&mut s);
    //     let g1_hash = s.finish();

    //     s = DefaultHasher::new();
    //     map2.hash(&mut s);
    //     let g2_hash = s.finish();

    //     assert_ne!(g1_hash, g2_hash, "Graphs hashes are equal");
    // }

    #[ignore]
    #[test]
    fn print_graph() {
        let cache = Cache::new();
        let mut graphs_map = GraphsMap::default();

        let mut graph = ObjectGraph::new(0);
        let obj_name = str_cached!(&cache; "A");
        let field_name = str_cached!(&cache; "a");
        let n1 = graph.add_simple_object(
            NodeIndex(0),
            obj_name.clone(),
            fields!((field_name.clone(), PrimitiveValue::Number(3u64.into()))),
        );
        let n2 = graph.add_simple_object(
            NodeIndex(1),
            obj_name,
            fields!((field_name.clone(), PrimitiveValue::Number(4u64.into()))),
        );
        graph.set_edge(&n1, n2, str_cached!(&cache; "c"));
        graphs_map.insert_graph(graph.into());
        graphs_map.set_as_root(str_cached!(&cache; "Root"), 0, n1);


        // ObjectGraph::set_graphs_map(graphs_map.into());
        println!("{}", dot::Dot::from_graphs_map(&graphs_map));
    }

    #[ignore]
    #[test]
    fn print_rng_graph() {
        let cache = Cache::new();

        let mut rng1 = StdRng::seed_from_u64(SEED);
        let graphs_map = random_gnp_object_graph(&cache, 0, &mut rng1, 5, 0.2);

        // ObjectGraph::set_graphs_map(graphs_map.into());
        println!("{}", dot::Dot::from_graphs_map(&graphs_map));
    }

    #[ignore]
    #[test]
    fn print_chain_to_rng_graph() {
        let cache = Cache::new();

        let mut rng1 = StdRng::seed_from_u64(SEED);
        let mut map1 = random_gnp_object_graph(&cache, 0, &mut rng1, 5, 0.2);

        let obj_name_a = str_cached!(&cache; "A");
        let root_name = str_cached!(&cache; "root");
        let field_name_a = str_cached!(&cache; "a");

        let mut g2 = ObjectGraph::new(1);
        let n1 = g2.add_simple_object(NodeIndex(map1.node_count()), obj_name_a.clone(), fields!());

        let chained_index = NodeIndex(0);
        g2.set_chain_edge(&n1, 0, chained_index, field_name_a);

        map1.insert_graph(g2.into());
        map1.set_as_root(root_name.clone(), 0, n1);

        println!("{}", dot::Dot::from_graphs_map(&map1));
    }

    // #[test]
    // fn serialize_graph() {
    //     let cache = Cache::new();
    //     let mut rng = StdRng::seed_from_u64(SEED);

    //     let n = 10;
    //     let mut g = random_gnp_object_graph(&cache, &mut rng, n.try_into().unwrap(), 0.5);
    //     g.generate_serialized_data();
    //     println!("{:?}", Dot::new(&g.graph));
    //     println!("{:?}", g.serialized.unwrap());
    // }

    // #[test]
    // fn graph_union() {
    //     let cache = Cache::new();

    //     let obj_name_a = str_cached!(&cache; "A");
    //     let obj_name_b = str_cached!(&cache; "B");
    //     let root_name_a = str_cached!(&cache; "root_a");
    //     let root_name_b = str_cached!(&cache; "root_b");
    //     let field_name_a = str_cached!(&cache; "a");
    //     let field_name_b = str_cached!(&cache; "b");

    //     let mut graph_a = ObjectGraph::new();
    //     let mut graph_b = ObjectGraph::new();
    //     let mut graph_ab = ObjectGraph::new();
    //     graph_a.add_root(
    //         root_name_a.clone(),

    //             obj_name_a.clone(),
    //             fields!((field_name_a.clone(), PrimitiveValue::Number(6u64.into()))),
    //         ),
    //     );
    //     graph_b.add_root(
    //         root_name_b.clone(),

    //             obj_name_b.clone(),
    //             fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
    //         ),
    //     );
    //     graph_ab.add_root(
    //         root_name_a.clone(),

    //             obj_name_a.clone(),
    //             fields!((field_name_a.clone(), PrimitiveValue::Number(6u64.into()))),
    //         ),
    //     );
    //     graph_ab.add_root(
    //         root_name_b.clone(),

    //             obj_name_b.clone(),
    //             fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
    //         ),
    //     );

    //     let (mut graph_union, _nodes_map) = ObjectGraph::union(&[graph_a.into(), graph_b.into()]);
    //     graph_ab.generate_serialized_data();
    //     graph_union.generate_serialized_data();
    //     assert_eq!(graph_union, graph_ab, "Graphs are not equal");
    // }
}
