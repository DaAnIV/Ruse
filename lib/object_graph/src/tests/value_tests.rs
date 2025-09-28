#[cfg(test)]
mod value_tests {
    use std::hash::{DefaultHasher, Hasher};

    use crate::{
        field_name, fields,
        graph_id_generator::GraphIdGenerator,
        graph_map_value::{GraphMapHash, GraphMapWrap},
        location::*,
        mermaid, root_name,
        value::*,
        vnum, Attributes, FieldName, GraphIndex, GraphsMap, NodeIndex, ObjectType, PrimitiveValue,
        RootName,
    };

    fn create_graph_with_two_roots(graphs_map: &mut GraphsMap, id_generator: &GraphIdGenerator) {
        let obj_name_a = ObjectType::class_obj_type("A");
        let obj_name_b = ObjectType::class_obj_type("B");
        let obj_name_c = ObjectType::class_obj_type("C");
        let obj_name_d = ObjectType::class_obj_type("C");
        let root_name1 = root_name!("root1");
        let root_name2 = root_name!("root2");
        let root_name3 = root_name!("root3");
        let root_name4 = root_name!("root4");
        let root_name5 = root_name!("root5");
        let field_name_a = field_name!("a");
        let field_name_b = field_name!("b");
        let field_name_c = field_name!("c");

        let g = id_generator.get_id_for_graph();

        graphs_map.ensure_graph(g);
        let n1 = graphs_map.add_simple_object(
            g,
            id_generator.get_id_for_node(),
            obj_name_a.clone(),
            fields!((field_name_a.clone(), PrimitiveValue::from(3u64))),
        );
        let n2 = graphs_map.add_simple_object(
            g,
            id_generator.get_id_for_node(),
            obj_name_b.clone(),
            fields!((field_name_b.clone(), PrimitiveValue::from(5u64))),
        );
        let n3 = graphs_map.add_simple_object(
            g,
            id_generator.get_id_for_node(),
            obj_name_c.clone(),
            fields!((field_name_c.clone(), PrimitiveValue::from(6u64))),
        );
        graphs_map.ensure_graph(g);
        let n4 = graphs_map.add_simple_object(
            g,
            id_generator.get_id_for_node(),
            obj_name_a.clone(),
            fields!((field_name_a.clone(), PrimitiveValue::from(3u64))),
        );
        graphs_map.ensure_graph(g);
        let n5 = graphs_map.add_simple_object(
            g,
            id_generator.get_id_for_node(),
            obj_name_d.clone(),
            fields!(),
        );
        graphs_map.ensure_graph(g);
        let n6 = graphs_map.add_simple_object(
            g,
            id_generator.get_id_for_node(),
            obj_name_d.clone(),
            fields!(),
        );
        graphs_map.set_edge(field_name!("ab"), g, n1, g, n2);
        graphs_map.set_edge(field_name!("ac"), g, n1, g, n3);

        graphs_map.set_edge(field_name!("ab"), g, n4, g, n2);
        graphs_map.set_edge(field_name!("ac"), g, n4, g, n3);

        graphs_map.set_edge(field_name!("da"), g, n6, g, n1);
        graphs_map.set_edge(field_name!("da"), g, n5, g, n4);

        graphs_map.set_as_root(root_name1, g, n1);
        graphs_map.set_as_root(root_name2, g, n1);

        graphs_map.set_as_root(root_name3, g, n4);

        graphs_map.set_as_root(root_name4, g, n5);
        graphs_map.set_as_root(root_name5, g, n6);
    }

    fn obj_value_for_root(graphs_map: &GraphsMap, root_name: &RootName) -> ObjectValue {
        let root = graphs_map.get_root(root_name).unwrap();
        ObjectValue {
            obj_type: graphs_map.obj_type(root.graph, root.node).unwrap().clone(),
            graph_id: root.graph,
            node: root.node,
        }
    }

    fn loc_value_for_root(graphs_map: &GraphsMap, root_name: &RootName) -> LocValue {
        LocValue {
            loc: Location::Root(RootLoc {
                root: root_name.clone(),
                attrs: Attributes::default(),
            }),
            val: obj_value_for_root(graphs_map, root_name).into(),
        }
    }

    fn loc_value_for_root_field(
        graphs_map: &GraphsMap,
        root_name: &RootName,
        field_name: &FieldName,
    ) -> LocValue {
        let root_obj_val = obj_value_for_root(graphs_map, root_name);

        LocValue {
            loc: Location::ObjectField(ObjectFieldLoc {
                graph: root_obj_val.graph_id,
                node: root_obj_val.node,
                field: field_name.clone(),
                attrs: Attributes::default(),
            }),
            val: root_obj_val
                .get_field_value(field_name, graphs_map)
                .unwrap()
                .into(),
        }
    }

    fn get_hash(loc_value: &LocValue, graphs_map: &GraphsMap) -> u64 {
        let mut s = DefaultHasher::new();
        loc_value.calculate_hash(&mut s, graphs_map);
        s.finish()
    }

    #[test]
    fn print_graph() {
        let mut graphs_map = GraphsMap::default();

        let id_generator = GraphIdGenerator::with_initial_values(NodeIndex(0), GraphIndex(0));

        create_graph_with_two_roots(&mut graphs_map, &id_generator);

        println!("{}", mermaid::Mermaid::from_graphs_map(&graphs_map));
    }

    #[test]
    fn test_temp_primitive_loc_value_eq() {
        let graphs_map_1 = GraphsMap::default();
        let graphs_map_2 = GraphsMap::default();

        let loc_value_1 = LocValue {
            loc: Location::Temp,
            val: vnum!(1.into()),
        };
        let loc_value_2 = LocValue {
            loc: Location::Temp,
            val: vnum!(1.into()),
        };

        assert_eq!(
            loc_value_1.wrap(&graphs_map_1),
            loc_value_2.wrap(&graphs_map_2)
        );
        assert_eq!(
            get_hash(&loc_value_1, &graphs_map_1),
            get_hash(&loc_value_2, &graphs_map_2)
        );
    }

    #[test]
    fn test_temp_object_loc_value_eq() {
        let mut graphs_map_1 = GraphsMap::default();
        let mut graphs_map_2 = GraphsMap::default();

        let id_generator_1 = GraphIdGenerator::with_initial_values(NodeIndex(0), GraphIndex(0));
        let id_generator_2 =
            GraphIdGenerator::with_initial_values(NodeIndex(0x10), GraphIndex(0x10));

        create_graph_with_two_roots(&mut graphs_map_1, &id_generator_1);
        create_graph_with_two_roots(&mut graphs_map_2, &id_generator_2);

        let obj_value_1 = obj_value_for_root(&graphs_map_1, &root_name!("root1"));
        let obj_value_2 = obj_value_for_root(&graphs_map_2, &root_name!("root1"));

        let loc_value_1 = LocValue {
            loc: Location::Temp,
            val: obj_value_1.into(),
        };
        let loc_value_2 = LocValue {
            loc: Location::Temp,
            val: obj_value_2.into(),
        };

        assert_eq!(
            loc_value_1.wrap(&graphs_map_1),
            loc_value_2.wrap(&graphs_map_2)
        );
        assert_eq!(
            get_hash(&loc_value_1, &graphs_map_1),
            get_hash(&loc_value_2, &graphs_map_2)
        );
    }

    #[test]
    fn test_object_root_loc_value_eq() {
        let mut graphs_map_1 = GraphsMap::default();
        let mut graphs_map_2 = GraphsMap::default();

        let id_generator_1 = GraphIdGenerator::with_initial_values(NodeIndex(0), GraphIndex(0));
        let id_generator_2 =
            GraphIdGenerator::with_initial_values(NodeIndex(0x10), GraphIndex(0x10));

        create_graph_with_two_roots(&mut graphs_map_1, &id_generator_1);
        create_graph_with_two_roots(&mut graphs_map_2, &id_generator_2);

        let loc_value_1 = loc_value_for_root(&graphs_map_1, &root_name!("root1"));
        let loc_value_2 = loc_value_for_root(&graphs_map_2, &root_name!("root1"));

        assert_eq!(
            loc_value_1.wrap(&graphs_map_1),
            loc_value_2.wrap(&graphs_map_2)
        );
        assert_eq!(
            get_hash(&loc_value_1, &graphs_map_1),
            get_hash(&loc_value_2, &graphs_map_2)
        );
    }

    #[test]
    fn test_object_root_loc_alias_value_eq() {
        let mut graphs_map_1 = GraphsMap::default();
        let mut graphs_map_2 = GraphsMap::default();

        let id_generator_1 = GraphIdGenerator::with_initial_values(NodeIndex(0), GraphIndex(0));
        let id_generator_2 =
            GraphIdGenerator::with_initial_values(NodeIndex(0x10), GraphIndex(0x10));

        create_graph_with_two_roots(&mut graphs_map_1, &id_generator_1);
        create_graph_with_two_roots(&mut graphs_map_2, &id_generator_2);

        let loc_value_1 = loc_value_for_root(&graphs_map_1, &root_name!("root1"));
        let loc_value_2 = loc_value_for_root(&graphs_map_2, &root_name!("root2"));

        assert_eq!(
            loc_value_1.wrap(&graphs_map_1),
            loc_value_2.wrap(&graphs_map_2)
        );
        assert_eq!(
            get_hash(&loc_value_1, &graphs_map_1),
            get_hash(&loc_value_2, &graphs_map_2)
        );
    }

    #[test]
    fn test_object_root_loc_similiar_value_ne() {
        let mut graphs_map_1 = GraphsMap::default();
        let mut graphs_map_2 = GraphsMap::default();

        let id_generator_1 = GraphIdGenerator::with_initial_values(NodeIndex(0), GraphIndex(0));
        let id_generator_2 =
            GraphIdGenerator::with_initial_values(NodeIndex(0x10), GraphIndex(0x10));

        create_graph_with_two_roots(&mut graphs_map_1, &id_generator_1);
        create_graph_with_two_roots(&mut graphs_map_2, &id_generator_2);

        let loc_value_1 = loc_value_for_root(&graphs_map_1, &root_name!("root1"));
        let loc_value_2 = loc_value_for_root(&graphs_map_2, &root_name!("root3"));

        assert_eq!(
            loc_value_1.val().wrap(&graphs_map_1),
            loc_value_2.val().wrap(&graphs_map_2)
        );
        assert_ne!(
            loc_value_1.wrap(&graphs_map_1),
            loc_value_2.wrap(&graphs_map_2)
        );
    }

    #[test]
    fn test_object_field_loc_value_eq() {
        let mut graphs_map_1 = GraphsMap::default();
        let mut graphs_map_2 = GraphsMap::default();

        let id_generator_1 = GraphIdGenerator::with_initial_values(NodeIndex(0), GraphIndex(0));
        let id_generator_2 =
            GraphIdGenerator::with_initial_values(NodeIndex(0x10), GraphIndex(0x10));

        create_graph_with_two_roots(&mut graphs_map_1, &id_generator_1);
        create_graph_with_two_roots(&mut graphs_map_2, &id_generator_2);

        let loc_value_1 =
            loc_value_for_root_field(&graphs_map_1, &root_name!("root1"), &field_name!("ab"));
        let loc_value_2 =
            loc_value_for_root_field(&graphs_map_2, &root_name!("root3"), &field_name!("ab"));

        assert_eq!(
            loc_value_1.val().wrap(&graphs_map_1),
            loc_value_2.val().wrap(&graphs_map_2)
        );
        assert_eq!(
            loc_value_1.wrap(&graphs_map_1),
            loc_value_2.wrap(&graphs_map_2)
        );
        assert_eq!(
            get_hash(&loc_value_1, &graphs_map_1),
            get_hash(&loc_value_2, &graphs_map_2)
        );
    }

    #[test]
    fn test_object_field_root_loc_value_eq() {
        let mut graphs_map_1 = GraphsMap::default();
        let mut graphs_map_2 = GraphsMap::default();

        let id_generator_1 = GraphIdGenerator::with_initial_values(NodeIndex(0), GraphIndex(0));
        let id_generator_2 =
            GraphIdGenerator::with_initial_values(NodeIndex(0x10), GraphIndex(0x10));

        create_graph_with_two_roots(&mut graphs_map_1, &id_generator_1);
        create_graph_with_two_roots(&mut graphs_map_2, &id_generator_2);

        let loc_value_1 =
            loc_value_for_root(&graphs_map_1, &root_name!("root1"));
        let loc_value_2 =
            loc_value_for_root_field(&graphs_map_2, &root_name!("root5"), &field_name!("da"));

        assert_eq!(
            loc_value_1.val().wrap(&graphs_map_1),
            loc_value_2.val().wrap(&graphs_map_2)
        );
        assert_eq!(
            loc_value_1.wrap(&graphs_map_1),
            loc_value_2.wrap(&graphs_map_2)
        );
        assert_eq!(
            get_hash(&loc_value_1, &graphs_map_1),
            get_hash(&loc_value_2, &graphs_map_2)
        );
    }

    #[test]
    fn test_object_field_root_loc_value_ne() {
        let mut graphs_map_1 = GraphsMap::default();
        let mut graphs_map_2 = GraphsMap::default();

        let id_generator_1 = GraphIdGenerator::with_initial_values(NodeIndex(0), GraphIndex(0));
        let id_generator_2 =
            GraphIdGenerator::with_initial_values(NodeIndex(0x10), GraphIndex(0x10));

        create_graph_with_two_roots(&mut graphs_map_1, &id_generator_1);
        create_graph_with_two_roots(&mut graphs_map_2, &id_generator_2);

        let loc_value_1 =
            loc_value_for_root(&graphs_map_1, &root_name!("root1"));
        let loc_value_2 =
            loc_value_for_root_field(&graphs_map_2, &root_name!("root4"), &field_name!("da"));

        assert_eq!(
            loc_value_1.val().wrap(&graphs_map_1),
            loc_value_2.val().wrap(&graphs_map_2)
        );
        assert_ne!(
            loc_value_1.wrap(&graphs_map_1),
            loc_value_2.wrap(&graphs_map_2)
        );
    }


    #[test]
    fn test_primitive_field_loc_value_ne() {
        let mut graphs_map_1 = GraphsMap::default();
        let mut graphs_map_2 = GraphsMap::default();

        let id_generator_1 = GraphIdGenerator::with_initial_values(NodeIndex(0), GraphIndex(0));
        let id_generator_2 =
            GraphIdGenerator::with_initial_values(NodeIndex(0x10), GraphIndex(0x10));

        create_graph_with_two_roots(&mut graphs_map_1, &id_generator_1);
        create_graph_with_two_roots(&mut graphs_map_2, &id_generator_2);

        let loc_value_1 =
            loc_value_for_root_field(&graphs_map_1, &root_name!("root1"), &field_name!("a"));
        let loc_value_2 =
            loc_value_for_root_field(&graphs_map_2, &root_name!("root3"), &field_name!("a"));

        assert_eq!(
            loc_value_1.val().wrap(&graphs_map_1),
            loc_value_2.val().wrap(&graphs_map_2)
        );
        assert_ne!(
            loc_value_1.wrap(&graphs_map_1),
            loc_value_2.wrap(&graphs_map_2)
        );
    }

    #[test]
    fn test_primitive_field_loc_value_eq() {
        let mut graphs_map_1 = GraphsMap::default();
        let mut graphs_map_2 = GraphsMap::default();

        let id_generator_1 = GraphIdGenerator::with_initial_values(NodeIndex(0), GraphIndex(0));
        let id_generator_2 =
            GraphIdGenerator::with_initial_values(NodeIndex(0x10), GraphIndex(0x10));

        create_graph_with_two_roots(&mut graphs_map_1, &id_generator_1);
        create_graph_with_two_roots(&mut graphs_map_2, &id_generator_2);

        let loc_value_1 =
            loc_value_for_root_field(&graphs_map_1, &root_name!("root1"), &field_name!("a"));
        let loc_value_2 =
            loc_value_for_root_field(&graphs_map_2, &root_name!("root1"), &field_name!("a"));

        assert_eq!(
            loc_value_1.val().wrap(&graphs_map_1),
            loc_value_2.val().wrap(&graphs_map_2)
        );
        assert_eq!(
            loc_value_1.wrap(&graphs_map_1),
            loc_value_2.wrap(&graphs_map_2)
        );
        assert_eq!(
            get_hash(&loc_value_1, &graphs_map_1),
            get_hash(&loc_value_2, &graphs_map_2)
        );
    }
}
