use std::sync::Arc;

use itertools::Itertools;
use ruse_object_graph::{
    class_name, root_name, value::Value, vnull, vnum, ClassName, GraphIdGenerator, GraphsMap,
    Number, ValueType,
};
use ruse_synthesizer::{
    context::{Context, ContextArray, ValuesMap, Variable, VariableMap},
    embedding::merge_context_arrays,
    op_chain,
    synthesizer_context::SynthesizerContext,
    test::helpers::{evaluate_chain, init_log},
};
use ruse_ts_interpreter::{
    engine_context::EngineContext,
    js_worker_context::create_js_worker_context,
    test::ts_op_helpers::*,
    ts_classes::{TsClasses, TsClassesBuilder},
    ts_user_class::TsUserClass,
};

const BINARY_TREE_TS_PATH: &str = "../../tasks/classes/ruse/simple_tree.ts";

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

fn binary_tree_class_name() -> ClassName {
    class_name!("BinaryTree")
}

fn get_ctx(trees_values: &[&[usize]], classes: &TsClasses) -> Box<Context> {
    let id_gen = Arc::new(GraphIdGenerator::default());
    let mut graphs_map = GraphsMap::default();
    let binary_tree_class = classes.get_user_class(&binary_tree_class_name()).unwrap();

    let mut values = ValuesMap::default();

    for (i, tree_values) in trees_values.iter().enumerate() {
        let graph_id = id_gen.get_id_for_graph();
        graphs_map.ensure_graph(graph_id);

        let tree_value: Value = {
            let mut engine_ctx = EngineContext::create_engine_ctx(classes);
            engine_ctx.reset_with_graph(graph_id, &mut graphs_map, classes, &id_gen);

            create_binary_tree(tree_values, classes, binary_tree_class, &mut engine_ctx)
        };
        let tree_obj = tree_value.obj().unwrap().clone();
        graphs_map.set_as_root(
            root_name!(format!("tree{}", i + 1)),
            tree_obj.graph_id,
            tree_obj.node,
        );
        values.insert(root_name!(format!("tree{}", i + 1)), tree_value);
    }

    Context::with_values(values, graphs_map.into(), id_gen)
}

fn get_variables(tree_count: usize) -> VariableMap {
    let mut variables = VariableMap::default();
    for i in 1..=tree_count {
        variables.insert(
            root_name!(format!("tree{}", i)),
            Variable {
                name: root_name!(format!("tree{}", i)),
                value_type: ValueType::class_value_type(binary_tree_class_name()),
                immutable: false,
            },
        );
    }
    variables
}

#[test]
fn tests_complex_context_merge() {
    let mut builder = TsClassesBuilder::new();
    builder
        .add_files(&BINARY_TREE_TS_PATH)
        .expect("Failed to add binary search tree class");
    let classes = builder.finalize().unwrap();

    let ctx = ContextArray::from(vec![get_ctx(&[&[1], &[2], &[5, 2, 1]], &classes)]);
    let syn_ctx =
        SynthesizerContext::from_context_array_with_data(ctx.clone(), get_variables(3), classes);
    let classes_ref = syn_ctx.data.downcast_ref::<TsClasses>().unwrap();
    let binary_tree_class = classes_ref
        .get_user_class(&binary_tree_class_name())
        .unwrap();

    let tree2_op = id_op("tree2");
    let tree3_op = id_op("tree3");
    let left_op = class_getter_op(&binary_tree_class, "left");
    let set_left_op = class_setter_op(&binary_tree_class, "left");

    let op_chain_1 = op_chain!(
        "c1",
        &left_op;
        op_chain!("tree3_1", &tree3_op)
    );

    let op_chain_2 = op_chain!(
        "c2",
        &set_left_op;
        op_chain!("tree3_2", &tree3_op),
        op_chain!("tree2", &tree2_op)
    );

    let mut worker_ctx = create_js_worker_context(0);
    let p1 = evaluate_chain(
        op_chain_1,
        &syn_ctx.start_context,
        &syn_ctx,
        &mut worker_ctx,
    );
    let p2 = evaluate_chain(
        op_chain_2,
        &syn_ctx.start_context,
        &syn_ctx,
        &mut worker_ctx,
    );

    // println!("{{");
    // println!("\"c1\": \"{}\",", p1["c1"].get_code());
    // println!("\"c2\": \"{}\",", p2["c2"].get_code());
    // println!("\"p1\": {},", p1["c1"].pre_ctx()[0].json_display());
    // println!("\"q1\": {},", p1["c1"].post_ctx()[0].json_display());
    // println!("\"p2\": {},", p2["c2"].pre_ctx()[0].json_display());
    // println!("\"q2\": {}", p2["c2"].post_ctx()[0].json_display());
    // println!("}}");

    init_log();

    merge_context_arrays(
        p1["c1"].pre_ctx(),
        p1["c1"].post_ctx(),
        p2["c2"].pre_ctx(),
        p2["c2"].post_ctx(),
    )
    .unwrap();
}
