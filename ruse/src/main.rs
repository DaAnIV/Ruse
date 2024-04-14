// use interpreter::{PreCondition, object_graph::ObjectNodeType};
// use rustpython_parser as parser;
use std::process::ExitCode;

use object_graph::{str_cached, Number};
use rand::{rngs::StdRng, SeedableRng};

// mod helpers;
// use helpers::dot_generator::AstDotGenerator;
// use ruse_ts_interpreter as ts_interpreter;
// mod synthesizer;
use ruse_object_graph as object_graph;
use ruse_synthesizer::{context::Context, vnum};
use ruse_ts_synthesizer::*;

use std::sync::Arc;
// use swc::PrintArgs;
// // use swc::ecmascript::ast::ModuleItem;
// // use swc_common::{
// //     errors::{ColorConfig, Handler},
// //     FileName, SourceMap,
// // };
// use swc_common::{
//     errors::{ColorConfig, Handler},
//     FileName, SourceMap,
// };
// use swc_ecma_ast;
// use swc_ecma_parser::{Syntax, TsConfig};

const RANGE: [usize; 8] = [5, 10, 20, 50, 100, 200, 500, 1000];

#[allow(dead_code)]
fn get_graphs_from_range(cache: &mut object_graph::Cache) -> Vec<object_graph::ObjectGraph> {
    let mut graphs = Vec::with_capacity(RANGE.len());
    for n in RANGE {
        let mut rng = StdRng::from_entropy();
        graphs.push(object_graph::generator::random_gnm_object_graph(
            cache,
            &mut rng,
            n,
            n * 4,
        ))
    }
    return graphs;
}

#[allow(dead_code)]
fn get_serialized_graphs_from_range(
    cache: &mut object_graph::Cache,
) -> Vec<object_graph::ObjectGraph> {
    let mut graphs = get_graphs_from_range(cache);
    for g in &mut graphs {
        g.generate_serialized_data()
            .expect("Failed to serialize graph");
    }
    return graphs;
}

fn main() -> ExitCode {
    let cache = object_graph::Cache::new();
    let ctx = Arc::new([
        Context::with_values(
            [
                (str_cached!(cache; "x"), vnum!(Number::from(4u64))),
                (str_cached!(cache; "y"), vnum!(Number::from(2u64))),
            ]
            .into(),
        ),
        Context::with_values(
            [
                (str_cached!(cache; "x"), vnum!(Number::from(5u64))),
                (str_cached!(cache; "y"), vnum!(Number::from(3u64))),
            ]
            .into(),
        ),
    ]);
    let opcodes = construct_opcode_list(
        &[str_cached!(cache; "x"), str_cached!(cache; "y")],
        &[-1f64, 1f64],
        &ALL_BIN_NUM_OPCODES,
        &ALL_UNARY_NUM_OPCODES,
        &ALL_UPDATE_NUM_OPCODES,
        false,
        &[],
        &[],
        &[],
    );
    let mut synthesizer = TsSynthesizer::with_context_and_opcodes(ctx.clone(), opcodes, &cache);

    println!(
        "1: Generated: {}, Bank Size: {}",
        synthesizer.statistics().generated,
        synthesizer.statistics().bank_size
    );

    for i in 2..=10 {
        synthesizer.synthesize_for_size(&ctx, i, &cache);
        println!(
            "{}: Generated: {}, Bank Size: {}",
            i,
            synthesizer.statistics().generated,
            synthesizer.statistics().bank_size
        );
    }

    // let cache = object_graph::Cache::new();
    // let mut rng = StdRng::from_entropy();

    // let mut g1 = object_graph::random_gnp_object_graph(&cache, &mut rng, 1000, 0.03);
    // let mut g2 = g1.clone();
    // g1.generate_serialized_data().expect("Failed to serialize g1");
    // g2.generate_serialized_data().expect("Failed to serialize g2");
    // assert_eq!(g1, g2, "Graphs are not equal");

    // let graphs = get_serialized_graphs_from_range(&cache);
    // for g in graphs.iter() {
    //     println!("Graph with {} nodes {} edges, serialized size is {}", g.node_count(), g.edge_count(), g.serialized.as_ref().unwrap().len());
    // }

    // let cm = Arc::<SourceMap>::default();
    // let handler = Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(cm.clone()));
    // let c = swc::Compiler::new(cm.clone());

    // let fm = cm.new_source_file(
    //     FileName::Custom("test.js".into()),
    //     "let helloWorld = \"Hello World\";".to_string(),
    // );

    // let ast = c
    //     .parse_js(
    //         fm,
    //         &handler,
    //         swc_ecma_ast::EsVersion::Es2022,
    //         Syntax::Typescript(TsConfig::default()),
    //         swc::config::IsModule::Bool(false),
    //         None,
    //     )
    //     .expect("Failed to parse");

    // println!("{:?}", c.print(&ast, PrintArgs::default()));
    // dbg!(ast);
    // let ast = parser::parse(r#"((x + 5) / (y.func()))"#, parser::Mode::Expression, "<embedded>").unwrap();
    // let expr = interpreter::Expr {
    //     expr: ast.as_expression().unwrap().body.to_owned(),
    //     out_type: Some(ObjectNodeType::Int)
    // };

    // let mut context = interpreter::Context::new();
    // let ast_evaluator = interpreter::AstEvaluator::new();

    // let pre_cond = PreCondition::new();

    // let args = vec![];

    // let post = ast_evaluator.eval_expr(&mut context, &pre_cond, &expr, &args);

    // let mut dot_generator = AstDotGenerator::new();
    // dot_generator.add_expr(*expr.expr);

    // let _out = dot_generator.create_svg("1.svg").unwrap();

    // let mut vocab = synthesizer::Vocabulary::default();
    // let enumerator = synthesizer::Enumerator::new(vocab);

    ExitCode::SUCCESS
}
