#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use crate::dot;
    use crate::field_name;
    use crate::generator::object_graph_generator::*;
    use crate::mermaid;
    use crate::root_name;
    use crate::GraphIndex;
    use crate::GraphsMap;
    use crate::ObjectType;
    use crate::{fields, NodeIndex, PrimitiveValue};

    const SEED: u64 = 10;

    #[test]
    fn eq_random_graphs() {
        let mut rng = StdRng::seed_from_u64(SEED);

        let graphs_map_1 = random_gnp_object_graph(GraphIndex(0), &mut rng, 10, 0.5);
        let graphs_map_2 = graphs_map_1.clone();
        assert_eq!(graphs_map_1, graphs_map_2, "Graphs are not equal");
    }

    #[test]
    fn ne_graphs_field_value() {
        let mut graphs_map_1 = GraphsMap::default();
        let mut graphs_map_2 = GraphsMap::default();

        let obj_name_a = ObjectType::class_obj_type("A");
        let obj_name_b = ObjectType::class_obj_type("B");
        let obj_name_c = ObjectType::class_obj_type("C");
        let root_name = root_name!("root");
        let field_name_a = field_name!("a");
        let field_name_b = field_name!("b");
        let field_name_c = field_name!("c");

        let g1 = GraphIndex(0);
        let g2 = GraphIndex(1);

        graphs_map_1.ensure_graph(GraphIndex(0));
        let n11 = graphs_map_1.add_simple_object(
            g1,
            NodeIndex(0),
            obj_name_a.clone(),
            fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
        );

        let n12 = graphs_map_1.add_simple_object(
            g1,
            NodeIndex(1),
            obj_name_b.clone(),
            fields!((field_name_b.clone(), PrimitiveValue::Number(6u64.into()))),
        );
        let n13 = graphs_map_1.add_simple_object(
            g1,
            NodeIndex(2),
            obj_name_c.clone(),
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        );
        graphs_map_1.set_edge(field_name!("12"), g1, n11, g1, n12);
        graphs_map_1.set_edge(field_name!("13"), g1, n11, g1, n13);

        graphs_map_2.ensure_graph(g2);
        let n22: NodeIndex = graphs_map_2.add_simple_object(
            g2,
            NodeIndex(0),
            obj_name_b,
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        );
        let n21 = graphs_map_2.add_simple_object(
            g2,
            NodeIndex(1),
            obj_name_a,
            fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
        );
        graphs_map_2.set_edge(field_name!("12"), g2, n21, g2, n22);
        let n23 = graphs_map_2.add_simple_object(
            g2,
            NodeIndex(2),
            obj_name_c,
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        );
        graphs_map_2.set_edge(field_name!("13"), g2, n21, g2, n23);

        graphs_map_1.set_as_root(root_name.clone(), g1, n11);
        graphs_map_2.set_as_root(root_name, g2, n21);
        assert_ne!(&graphs_map_1, &graphs_map_2, "Graphs are equal");
    }

    #[test]
    fn ne_graphs_field_name() {
        let mut map1 = GraphsMap::default();
        let mut map2 = GraphsMap::default();

        let obj_name_a = ObjectType::class_obj_type("A");
        let obj_name_b = ObjectType::class_obj_type("B");
        let obj_name_c = ObjectType::class_obj_type("C");
        let root_name = root_name!("root");
        let field_name_a = field_name!("a");
        let field_name_b = field_name!("b");
        let field_name_b2 = field_name!("b2");
        let field_name_c = field_name!("c");

        let g1 = GraphIndex(0);
        let g2 = GraphIndex(1);

        map1.ensure_graph(g1);
        let n11 = map1.add_simple_object(
            g1,
            NodeIndex(0),
            obj_name_a.clone(),
            fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
        );
        let n12 = map1.add_simple_object(
            g1,
            NodeIndex(1),
            obj_name_b.clone(),
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        );
        let n13 = map1.add_simple_object(
            g1,
            NodeIndex(2),
            obj_name_c.clone(),
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        );
        map1.set_edge(field_name!("12"), g1, n11, g1, n12);
        map1.set_edge(field_name!("13"), g1, n11, g1, n13);

        map2.ensure_graph(g2);
        let n22: NodeIndex = map2.add_simple_object(
            g2,
            NodeIndex(0),
            obj_name_b,
            fields!((field_name_b2.clone(), PrimitiveValue::Number(5u64.into()))),
        );
        let n21 = map2.add_simple_object(
            g2,
            NodeIndex(1),
            obj_name_a,
            fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
        );
        map2.set_edge(field_name!("12"), g2, n21, g2, n22);
        let n23 = map2.add_simple_object(
            g2,
            NodeIndex(2),
            obj_name_c,
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        );
        map2.set_edge(field_name!("13"), g2, n21, g2, n23);

        map1.set_as_root(root_name.clone(), g1, n11);
        map2.set_as_root(root_name, g2, n21);
        assert_ne!(map1, map2, "Graphs are equal");
    }

    #[test]
    fn ne_graphs_field_type() {
        let mut map1 = GraphsMap::new();
        let mut map2 = GraphsMap::new();

        let obj_name_a = ObjectType::class_obj_type("A");
        let obj_name_b = ObjectType::class_obj_type("B");
        let obj_name_b2 = ObjectType::class_obj_type("B2");
        let obj_name_c = ObjectType::class_obj_type("C");
        let root_name = root_name!("root");
        let field_name_a = field_name!("a");
        let field_name_b = field_name!("b");
        let field_name_c = field_name!("c");

        let g1 = GraphIndex(0);
        let g2 = GraphIndex(1);

        map1.ensure_graph(g1);
        let n11 = map1.add_simple_object(
            g1,
            NodeIndex(0),
            obj_name_a.clone(),
            fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
        );
        let n12 = map1.add_simple_object(
            g1,
            NodeIndex(1),
            obj_name_b.clone(),
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        );
        let n13 = map1.add_simple_object(
            g1,
            NodeIndex(2),
            obj_name_c.clone(),
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        );
        map1.set_edge(field_name!("12"), g1, n11, g1, n12);
        map1.set_edge(field_name!("13"), g1, n11, g1, n13);

        map2.ensure_graph(g2);
        let n22: NodeIndex = map2.add_simple_object(
            g2,
            NodeIndex(0),
            obj_name_b2,
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        );
        let n21 = map2.add_simple_object(
            g2,
            NodeIndex(1),
            obj_name_a,
            fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
        );
        map2.set_edge(field_name!("12"), g2, n21, g2, n22);
        let n23 = map2.add_simple_object(
            g2,
            NodeIndex(2),
            obj_name_c,
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        );
        map2.set_edge(field_name!("12"), g2, n21, g2, n23);

        map1.set_as_root(root_name.clone(), g1, n11);
        map2.set_as_root(root_name.clone(), g2, n21);
        assert_ne!(map1, map2, "Graphs are equal");
    }

    #[test]
    fn eq_graphs() {
        let mut map1 = GraphsMap::new();
        let mut map2 = GraphsMap::new();

        let obj_name_a = ObjectType::class_obj_type("A");
        let obj_name_b = ObjectType::class_obj_type("B");
        let obj_name_c = ObjectType::class_obj_type("C");
        let root_name = root_name!("root");
        let field_name_a = field_name!("a");
        let field_name_b = field_name!("b");
        let field_name_c = field_name!("c");

        let g1 = GraphIndex(0);
        let g2 = GraphIndex(1);

        map1.ensure_graph(g1);
        let n11 = map1.add_simple_object(
            g1,
            NodeIndex(0),
            obj_name_a.clone(),
            fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
        );
        let n12 = map1.add_simple_object(
            g1,
            NodeIndex(1),
            obj_name_b.clone(),
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        );
        let n13 = map1.add_simple_object(
            g1,
            NodeIndex(2),
            obj_name_c.clone(),
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        );
        map1.set_edge(field_name!("12"), g1, n11, g1, n12);
        map1.set_edge(field_name!("13"), g1, n11, g1, n13);

        map2.ensure_graph(g2);
        let n22: NodeIndex = map2.add_simple_object(
            g2,
            NodeIndex(0),
            obj_name_b,
            fields!((field_name_b.clone(), PrimitiveValue::Number(5u64.into()))),
        );
        let n21 = map2.add_simple_object(
            g2,
            NodeIndex(1),
            obj_name_a,
            fields!((field_name_a.clone(), PrimitiveValue::Number(3u64.into()))),
        );
        map2.set_edge(field_name!("12"), g2, n21, g2, n22);
        let n23 = map2.add_simple_object(
            g2,
            NodeIndex(2),
            obj_name_c,
            fields!((field_name_c.clone(), PrimitiveValue::Number(6u64.into()))),
        );
        map2.set_edge(field_name!("13"), g2, n21, g2, n23);

        map1.set_as_root(root_name.clone(), g1, n11);
        map2.set_as_root(root_name, g2, n21);
        assert_eq!(map1, map2, "Graphs are not equal");
    }

    // #[test]
    // fn eq_random_graphs_serialized_data() {
    //     let mut rng = StdRng::seed_from_u64(SEED);

    //     let mut g1 = random_gnp_object_graph(&mut rng, 10, 0.5);
    //     let mut g2 = g1.clone();
    //     g1.generate_serialized_data();
    //     g2.generate_serialized_data();
    //     assert_eq!(g1.serialized, g2.serialized, "Graphs are not equal");
    // }

    #[test]
    fn eq_graphs_2_rng() {
        let mut rng1 = StdRng::seed_from_u64(SEED);
        let map1 = random_gnp_object_graph(GraphIndex(0), &mut rng1, 10, 0.5);
        let mut rng2 = StdRng::seed_from_u64(SEED);
        let map2 = random_gnp_object_graph(GraphIndex(1), &mut rng2, 10, 0.5);

        assert_eq!(map1, map2, "Graphs are not equal");
    }

    // #[test]
    // fn eq_graphs_2_rng_check_hash() {
    //     let mut rng1 = StdRng::seed_from_u64(SEED);
    //     let map1 = random_gnp_object_graph(0, &mut rng1, 10, 0.5);
    //     let mut rng2 = StdRng::seed_from_u64(SEED);
    //     let map2 = random_gnp_object_graph(1, &mut rng2, 10, 0.5);

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
    //     let mut graphs_map = GraphsMap::default();

    //     let mut rng1 = StdRng::seed_from_u64(SEED);
    //     let map1 = random_gnp_object_graph(0, &mut rng1, 10, 0.5);
    //     let mut rng2 = StdRng::seed_from_u64(SEED * 10);
    //     let map2 = random_gnp_object_graph(1, &mut rng2, 10, 0.5);

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
        let mut graphs_map = GraphsMap::default();
        let graph_id = GraphIndex(0);

        graphs_map.ensure_graph(graph_id);

        let obj_name = ObjectType::class_obj_type("A");
        let field_name = field_name!("a");
        let n1 = graphs_map.add_simple_object(
            graph_id,
            NodeIndex(0),
            obj_name.clone(),
            fields!((field_name.clone(), PrimitiveValue::Number(3u64.into()))),
        );
        let n2 = graphs_map.add_simple_object(
            graph_id,
            NodeIndex(1),
            obj_name,
            fields!((field_name.clone(), PrimitiveValue::Number(4u64.into()))),
        );
        graphs_map.set_edge(field_name!("c"), graph_id, n1, graph_id, n2);
        graphs_map.set_as_root(root_name!("Root"), graph_id, n1);

        // ObjectGraph::set_graphs_map(graphs_map.into());
        println!("{}", dot::Dot::from_graphs_map(&graphs_map));
        println!("{}", mermaid::Mermaid::from_graphs_map(&graphs_map));
    }

    #[ignore]
    #[test]
    fn print_rng_graph() {
        let mut rng1 = StdRng::seed_from_u64(SEED);
        let graphs_map = random_gnp_object_graph(GraphIndex(0), &mut rng1, 5, 0.2);

        // ObjectGraph::set_graphs_map(graphs_map.into());
        println!("{}", dot::Dot::from_graphs_map(&graphs_map));
    }

    #[ignore]
    #[test]
    fn print_chain_to_rng_graph() {
        let mut rng1 = StdRng::seed_from_u64(SEED);
        let g1 = GraphIndex(0);
        let mut map1 = random_gnp_object_graph(g1, &mut rng1, 5, 0.2);

        let obj_name_a = ObjectType::class_obj_type("A");
        let root_name = root_name!("root");
        let field_name_a = field_name!("a");

        let g2 = GraphIndex(1);
        map1.ensure_graph(g2);
        let n1 = map1.add_simple_object(
            g2,
            NodeIndex(map1.node_count()),
            obj_name_a.clone(),
            fields!(),
        );

        let chained_index = NodeIndex(0);
        map1.set_edge(field_name_a, g2, n1, g1, chained_index);

        map1.set_as_root(root_name.clone(), g2, n1);

        println!("{}", dot::Dot::from_graphs_map(&map1));
        println!("{}", mermaid::Mermaid::from_graphs_map(&map1));
    }

    // #[test]
    // fn serialize_graph() {
    //     let mut rng = StdRng::seed_from_u64(SEED);

    //     let n = 10;
    //     let mut g = random_gnp_object_graph(&mut rng, n.try_into().unwrap(), 0.5);
    //     g.generate_serialized_data();
    //     println!("{:?}", Dot::new(&g.graph));
    //     println!("{:?}", g.serialized.unwrap());
    // }

    // #[test]
    // fn graph_union() {
    //     let obj_name_a = str_cached!("A");
    //     let obj_name_b = str_cached!("B");
    //     let root_name_a = str_cached!("root_a");
    //     let root_name_b = str_cached!("root_b");
    //     let field_name_a = str_cached!("a");
    //     let field_name_b = str_cached!("b");

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
