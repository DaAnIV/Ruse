use std::sync::Arc;

use itertools::Itertools;
use ruse_object_graph::{
    dot::{self, DotConfig, SubgraphConfig},
    field_name,
    graph_map_value::GraphMapWrap,
    root_name,
    value::{ObjectValue, Value},
    vnull, vnum, GraphsMap, Number,
};
use ruse_synthesizer::{
    context::{Context, ContextArray, GraphIdGenerator, SynthesizerContext},
    op_chain,
    test::helpers::evaluate_chain,
};
use ruse_task_parser::predicate_builder::PredicateBuilder;
use ruse_ts_interpreter::{
    engine_context::EngineContext,
    test::ts_op_helpers::*,
    ts_classes::{TsClasses, TsClassesBuilder},
    ts_user_class::TsUserClass,
};

const BINARY_SEARCH_TREE_TS_PATH: &str = "../../benchmarks/tasks/classes/binary_search_tree.ts";

fn create_binary_tree_inner(
    values: &[usize],
    left: isize,
    right: isize,
    classes: &TsClasses,
    binary_tree_class: &TsUserClass,
    engine_ctx: &mut EngineContext,
) -> Value {
    if left > right {
        return vnull!();
    }

    let middle = (left + right) / 2;
    let left_tree = create_binary_tree_inner(
        values,
        left,
        middle - 1,
        classes,
        binary_tree_class,
        engine_ctx,
    );
    let right_tree = create_binary_tree_inner(
        values,
        middle + 1,
        right,
        classes,
        binary_tree_class,
        engine_ctx,
    );

    binary_tree_class
        .call_constructor(
            &[
                vnum!(Number::from(values[middle as usize])),
                left_tree,
                right_tree,
            ],
            classes,
            engine_ctx,
        )
        .unwrap()
        .into()
}

fn create_binary_tree(
    values: &[usize],
    classes: &TsClasses,
    binary_tree_class: &TsUserClass,
    engine_ctx: &mut EngineContext,
) -> Value {
    let mut sorted = values.iter().cloned().collect_vec();
    sorted.sort();
    create_binary_tree_inner(
        &sorted,
        0,
        values.len() as isize - 1,
        classes,
        binary_tree_class,
        engine_ctx,
    )
}

fn tree_dot_config(name: &str) -> DotConfig {
    let mut dot_config = DotConfig::default();
    dot_config.exclude_fields.insert("parent".to_owned());
    dot_config.subgraph = Some(SubgraphConfig {
        name: name.to_owned(),
    });
    dot_config.prefix = Some(format!("{}", dot::EscapedName(name)));

    dot_config
}

#[test]
fn tests_construct_binary_tree() {
    let mut builder = TsClassesBuilder::new();
    let binary_tree_class_name = &builder.add_ts_files(&BINARY_SEARCH_TREE_TS_PATH).unwrap()[0];
    let classes = builder.finalize();
    let id_gen = Arc::new(GraphIdGenerator::default());

    let graph_id = id_gen.get_id_for_graph();
    let mut graphs_map = GraphsMap::default();
    graphs_map.ensure_graph(graph_id);

    let mut boa_ctx = EngineContext::new_boa_ctx();
    let mut engine_ctx = EngineContext::create_engine_ctx(&mut boa_ctx, &classes);
    engine_ctx.reset_with_graph(graph_id, &mut graphs_map, &classes, &id_gen);

    let binary_tree_class = classes.get_user_class(&binary_tree_class_name).unwrap();

    let left = binary_tree_class
        .call_constructor(
            &[vnum!(Number::from(58)), vnull!(), vnull!()],
            &classes,
            &mut engine_ctx,
        )
        .unwrap();
    println!("{}", left.wrap(&graphs_map));

    let right = binary_tree_class
        .call_constructor(
            &[vnum!(Number::from(28)), vnull!(), vnull!()],
            &classes,
            &mut engine_ctx,
        )
        .unwrap();
    println!("{}", right.wrap(&graphs_map));

    let root = binary_tree_class
        .call_constructor(
            &[
                vnum!(Number::from(1)),
                Value::Object(left),
                Value::Object(right),
            ],
            &classes,
            &mut engine_ctx,
        )
        .unwrap();

    println!("{}", root.wrap(&graphs_map));
}

#[test]
fn tests_binary_tree_contains() {
    let mut builder = TsClassesBuilder::new();
    let binary_tree_class_name = &builder.add_ts_files(&BINARY_SEARCH_TREE_TS_PATH).unwrap()[0];
    let classes = builder.finalize();
    let id_gen = Arc::new(GraphIdGenerator::default());

    let graph_id = id_gen.get_id_for_graph();
    let mut graphs_map = GraphsMap::default();
    graphs_map.ensure_graph(graph_id);

    let mut boa_ctx = EngineContext::new_boa_ctx();
    let mut engine_ctx = EngineContext::create_engine_ctx(&mut boa_ctx, &classes);
    engine_ctx.reset_with_graph(graph_id, &mut graphs_map, &classes, &id_gen);

    let binary_tree_class = classes.get_user_class(&binary_tree_class_name).unwrap();

    let left: Value = binary_tree_class
        .call_constructor(
            &[vnum!(Number::from(1)), vnull!(), vnull!()],
            &classes,
            &mut engine_ctx,
        )
        .unwrap()
        .into();
    let right: Value = binary_tree_class
        .call_constructor(
            &[vnum!(Number::from(58)), vnull!(), vnull!()],
            &classes,
            &mut engine_ctx,
        )
        .unwrap()
        .into();

    let root: Value = binary_tree_class
        .call_constructor(
            &[vnum!(Number::from(28)), left.clone(), right.clone()],
            &classes,
            &mut engine_ctx,
        )
        .unwrap()
        .into();

    let mut res;

    let contains_method_desc = binary_tree_class.description.methods["contains"].clone();

    res = binary_tree_class
        .call_method(
            &contains_method_desc,
            &root,
            &[vnum!(Number::from(1))],
            &classes,
            &mut engine_ctx,
        )
        .unwrap();
    assert_eq!(res.bool_value().unwrap(), true);
    res = binary_tree_class
        .call_method(
            &contains_method_desc,
            &root,
            &[vnum!(Number::from(28))],
            &classes,
            &mut engine_ctx,
        )
        .unwrap();
    assert_eq!(res.bool_value().unwrap(), true);
    res = binary_tree_class
        .call_method(
            &contains_method_desc,
            &root,
            &[vnum!(Number::from(58))],
            &classes,
            &mut engine_ctx,
        )
        .unwrap();
    assert_eq!(res.bool_value().unwrap(), true);

    res = binary_tree_class
        .call_method(
            &contains_method_desc,
            &root,
            &[vnum!(Number::from(2))],
            &classes,
            &mut engine_ctx,
        )
        .unwrap();
    assert_eq!(res.bool_value().unwrap(), false);
    res = binary_tree_class
        .call_method(
            &contains_method_desc,
            &root,
            &[vnum!(Number::from(0))],
            &classes,
            &mut engine_ctx,
        )
        .unwrap();
    assert_eq!(res.bool_value().unwrap(), false);
}

#[derive(Clone)]
struct TreeHelper<'a> {
    tree: ObjectValue,
    graphs_map: &'a GraphsMap,
}

#[allow(dead_code)]
impl<'a> TreeHelper<'a> {
    fn value(&self) -> usize {
        let value = self
            .tree
            .get_field_value(&field_name!("_value"), self.graphs_map)
            .unwrap();

        value.number_value().unwrap().0 as usize
    }
    fn height(&self) -> usize {
        let value = self
            .tree
            .get_field_value(&field_name!("_height"), self.graphs_map)
            .unwrap();

        value.number_value().unwrap().0 as usize
    }
    fn size(&self) -> usize {
        let value = self
            .tree
            .get_field_value(&field_name!("_size"), self.graphs_map)
            .unwrap();

        value.number_value().unwrap().0 as usize
    }
    fn left(&self) -> Option<Self> {
        let value = self
            .tree
            .get_field_value(&field_name!("_left"), self.graphs_map)?;

        Some(Self {
            tree: value.into_obj().unwrap(),
            graphs_map: self.graphs_map,
        })
    }
    fn right(&self) -> Option<Self> {
        let value = self
            .tree
            .get_field_value(&field_name!("_right"), self.graphs_map)?;

        Some(Self {
            tree: value.into_obj().unwrap(),
            graphs_map: self.graphs_map,
        })
    }

    fn find_value(&self, value: usize) -> Option<TreeHelper> {
        let mut cur_node_opt = Some(self.clone());
        while let Some(cur_node) = cur_node_opt {
            if value == cur_node.value() {
                return Some(cur_node);
            }
            if value < cur_node.value() {
                cur_node_opt = cur_node.left()
            } else {
                cur_node_opt = cur_node.right()
            }
        }

        None
    }

    fn max_value(&self) -> usize {
        let mut max_value = 0;
        let mut cur_node_opt = Some(self.clone());
        while let Some(cur_node) = cur_node_opt {
            max_value = cur_node.value();
            cur_node_opt = cur_node.right()
        }

        max_value
    }

    fn min_value(&self) -> usize {
        let mut max_value = 0;
        let mut cur_node_opt = Some(self.clone());
        while let Some(cur_node) = cur_node_opt {
            max_value = cur_node.value();
            cur_node_opt = cur_node.left()
        }

        max_value
    }

    fn valid(&self) -> bool {
        if let Some(left) = self.left() {
            if !left.valid() {
                return false;
            }
            if self.value() < left.max_value() {
                return false;
            }
        }
        if let Some(right) = self.right() {
            if !right.valid() {
                return false;
            }
            if self.value() > right.min_value() {
                return false;
            }
        }

        true
    }

    fn cotnains(&self, value: usize) -> bool {
        self.find_value(value).is_some()
    }

    fn check_fields(&self, expected_value: usize, expected_height: usize, expected_size: usize) {
        assert_eq!(expected_value, self.value());
        assert_eq!(expected_height, self.height());
        assert_eq!(expected_size, self.size());
    }

    fn print_dot(&self, name: &str) {
        println!(
            "{}",
            self.tree
                .dot_display_with_config(self.graphs_map, tree_dot_config(name))
        );
    }
}

#[test]
fn auto_constructor() {
    let mut builder = TsClassesBuilder::new();
    let binary_tree_class_name = &builder.add_ts_files(&BINARY_SEARCH_TREE_TS_PATH).unwrap()[0];
    let classes = builder.finalize();
    let id_gen = Arc::new(GraphIdGenerator::default());

    let graph_id = id_gen.get_id_for_graph();
    let mut graphs_map = GraphsMap::default();
    graphs_map.ensure_graph(graph_id);
    let tree_value: Value = {
        let mut boa_ctx = EngineContext::new_boa_ctx();
        let mut engine_ctx = EngineContext::create_engine_ctx(&mut boa_ctx, &classes);
        engine_ctx.reset_with_graph(graph_id, &mut graphs_map, &classes, &id_gen);

        let binary_tree_class = classes.get_user_class(&binary_tree_class_name).unwrap();

        create_binary_tree(
            &[5, 2, 1, 3, 6, 10, 11],
            &classes,
            binary_tree_class,
            &mut engine_ctx,
        )
    };

    let tree = TreeHelper {
        tree: tree_value.into_obj().unwrap(),
        graphs_map: &graphs_map,
    };

    tree.check_fields(5, 3, 7);

    tree.left().unwrap().check_fields(2, 2, 3);
    tree.right().unwrap().check_fields(10, 2, 3);

    tree.left().unwrap().left().unwrap().check_fields(1, 1, 1);
    tree.left().unwrap().right().unwrap().check_fields(3, 1, 1);
    tree.right().unwrap().left().unwrap().check_fields(6, 1, 1);
    tree.right()
        .unwrap()
        .right()
        .unwrap()
        .check_fields(11, 1, 1);
}

fn get_ctx(
    tree_values: &[usize],
    node_to_delete_value: usize,
    binary_tree_class: &TsUserClass,
    classes: &TsClasses,
) -> Box<Context> {
    let id_gen = Arc::new(GraphIdGenerator::default());
    let graph_id = id_gen.get_id_for_graph();
    let mut graphs_map = GraphsMap::default();
    graphs_map.ensure_graph(graph_id);

    let tree_value: Value = {
        let mut boa_ctx = EngineContext::new_boa_ctx();
        let mut engine_ctx = EngineContext::create_engine_ctx(&mut boa_ctx, classes);
        engine_ctx.reset_with_graph(graph_id, &mut graphs_map, classes, &id_gen);

        create_binary_tree(tree_values, classes, binary_tree_class, &mut engine_ctx)
    };
    let tree_obj = tree_value.obj().unwrap().clone();
    graphs_map.set_as_root(root_name!("tree"), tree_obj.graph_id, tree_obj.node);

    let node_to_delete = {
        let initial_tree = TreeHelper {
            tree: tree_obj.clone(),
            graphs_map: &graphs_map,
        };
        initial_tree.find_value(node_to_delete_value).unwrap().tree
    };
    graphs_map.set_as_root(
        root_name!("node_to_delete"),
        node_to_delete.graph_id,
        node_to_delete.node,
    );

    Context::with_values(
        [
            (root_name!("tree"), tree_value),
            (root_name!("node_to_delete"), Value::Object(node_to_delete)),
        ]
        .into(),
        graphs_map.into(),
        id_gen,
    )
}

fn get_predicate_js(node_to_delete_value: usize) -> String {
    format!(
        "tree.size == 6 && tree.valid() && !tree.contains({})",
        node_to_delete_value
    )
}

#[test]
fn check_delete_two_children() {
    let mut builder = TsClassesBuilder::new();
    let binary_tree_class_name = &builder.add_ts_files(&BINARY_SEARCH_TREE_TS_PATH).unwrap()[0];
    let classes = builder.finalize();

    let binary_tree_class = classes.get_user_class(&binary_tree_class_name).unwrap();

    let ctx = ContextArray::from(vec![
        get_ctx(&[5, 2, 1, 3, 6, 10, 11], 10, binary_tree_class, &classes),
        get_ctx(&[5, 2, 1, 3, 6, 10, 11], 2, binary_tree_class, &classes),
        get_ctx(&[5, 2, 1, 3, 6, 10, 11], 5, binary_tree_class, &classes),
    ]);
    let syn_ctx = SynthesizerContext::from_context_array_with_data(ctx.clone(), classes);
    let classes_ref = syn_ctx.data.downcast_ref::<TsClasses>().unwrap();
    let binary_tree_class = classes_ref.get_user_class(&binary_tree_class_name).unwrap();
    let tree_objs = ctx
        .iter()
        .map(|x| {
            x.get_var_value(&root_name!("tree"))
                .unwrap()
                .into_obj()
                .unwrap()
        })
        .collect_vec();

    let initial_trees = tree_objs
        .iter()
        .zip_eq(ctx.iter())
        .map(|(o, c)| TreeHelper {
            tree: o.clone(),
            graphs_map: &c.graphs_map,
        })
        .collect_vec();
    initial_trees[0].print_dot(&format!("initial_tree"));

    let predicate = PredicateBuilder {
        output_type: None,
        output_array: None,
        state_array: None,
        predicate_js: Some(vec![
            get_predicate_js(10),
            get_predicate_js(2),
            get_predicate_js(5),
        ]),
        graphs_map: Default::default(),
    }
    .finalize();

    let node_to_delete_ident_op = id_op("node_to_delete");

    let right_op = class_method_op(&binary_tree_class, "right");
    let min_node_op = class_method_op(&binary_tree_class, "min_node");
    let swap_op = class_method_op(&binary_tree_class, "swap");
    let unlink_leaf_op = class_method_op(&binary_tree_class, "unlink_leaf");

    let op_chain = op_chain!(
        "final",
        &unlink_leaf_op;
        op_chain!("swap", &swap_op;
            op_chain!("min_node", &min_node_op;
                op_chain!("right", &right_op;
                    op_chain!("node_to_delete_2", &node_to_delete_ident_op)
                )
            ),
            op_chain!("node_to_delete_1", &node_to_delete_ident_op)
        )
    );

    let progs = evaluate_chain(op_chain, &syn_ctx.start_context, &syn_ctx);

    println!("{}", progs["final"].get_code());

    let swap_trees = tree_objs
        .iter()
        .zip_eq(progs["swap"].post_ctx().iter())
        .map(|(o, c)| TreeHelper {
            tree: o.clone(),
            graphs_map: &c.graphs_map,
        })
        .collect_vec();
    swap_trees
        .iter()
        .enumerate()
        .for_each(|(i, x)| x.print_dot(&format!("swap_trees_{}", i)));

    let final_trees = tree_objs
        .iter()
        .zip_eq(progs["final"].post_ctx().iter())
        .map(|(o, c)| TreeHelper {
            tree: o.clone(),
            graphs_map: &c.graphs_map,
        })
        .collect_vec();

    final_trees
        .iter()
        .enumerate()
        .for_each(|(i, x)| x.print_dot(&format!("final_tree_{}", i)));

    // final_tree.check_fields(final_tree.value(), 3, 6);
    // assert!(final_tree.valid());
    // assert!(!final_tree.cotnains(node_to_delete_value));

    assert!(predicate(&progs["final"], &syn_ctx));
}
