// use interpreter::{PreCondition, object_graph::ObjectNodeType};
// use rustpython_parser as parser;
use std::{collections::HashMap, process::ExitCode, time::Instant};
// use std::thread;

use object_graph::{str_cached, Number};
use rand::{rngs::StdRng, SeedableRng};

// mod helpers;
// use helpers::dot_generator::AstDotGenerator;
// use ruse_ts_interpreter as ts_interpreter;
// mod synthesizer;
use ruse_object_graph as object_graph;
// use ruse_synthesizer::opcode::ExprOpcode;
use ruse_synthesizer::{
    // bank::{ContextMap, ProgBank, WorkGather},
    context::Context,
    value::{Location, ValueType},
    vbool,
    vnum,
    vstr,
};
use ruse_ts_interpreter::ts_class::TsClass;
// use ruse_ts_interpreter::opcode::{BinOp, LitOp, TsExprAst};
use ruse_ts_synthesizer::*;
// use swc_ecma_ast as ast;

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

// fn test_work_gatherer()
// {
//     unsafe { CACHE = Some(object_graph::Cache::new()) };
//     let cache = unsafe { CACHE.as_ref().unwrap() };

//     let ctx = Arc::new([
//         Context::with_values(
//             [
//                 (str_cached!(cache; "x"), vnum!(Number::from(4u64))),
//                 (str_cached!(cache; "y"), vnum!(Number::from(2u64))),
//             ]
//             .into(),
//         ),
//         Context::with_values(
//             [
//                 (str_cached!(cache; "x"), vnum!(Number::from(5u64))),
//                 (str_cached!(cache; "y"), vnum!(Number::from(3u64))),
//             ]
//             .into(),
//         ),
//     ]);

//     let mut bank = ProgBank::<TsExprAst, 2>::default();
//     for i in 0..3 {
//         let opcode = Arc::new(LitOp::Num(Number::from(i as u64)));
//         let mut ctx_map = ContextMap::<TsExprAst, 2>::default();
//         let mut p = SubProgram::<TsExprAst, 2>::with_opcode_and_context(opcode, &ctx);
//         unsafe { Arc::get_mut(&mut p).unwrap_unchecked() }.evaluate(cache);
//         ctx_map.insert_program(p);
//         bank.insert(ctx_map);
//     }

//     let bin_op: Arc<dyn ExprOpcode<TsExprAst>> = Arc::new(BinOp {
//         op: ast::BinaryOp::Add,
//         arg_types: [ValueType::Number, ValueType::Number],
//     });

//     let mut gatherer = WorkGather::new(
//         |op: &Arc<dyn ExprOpcode<TsExprAst>>, children: &Vec<Arc<SubProgram<TsExprAst, 2>>>| {
//             let cache = unsafe { CACHE.as_ref().unwrap() };
//             let mut p =
//                 SubProgram::<TsExprAst, 2>::with_opcode_and_children(op.clone(), children.clone());
//             unsafe { Arc::get_mut(&mut p).unwrap_unchecked() }.evaluate(cache);
//             println!("({:?}): {}", thread::current().id(), p);
//             println!("");
//         },
//         3,
//     );

//     gatherer.gather_work_for_next_iteration(&bank, &bin_op);
// }

async fn test_class_loader() {
    let code1 = "class User {
        constructor(public name: string, 
                    public surname: string,
                    public age: number,
                    protected is_admin: bool) {}
    }";
    let code2 = "class UserPair {
        constructor(public user1: User, 
                    public user2: User) {}
    }";

    let cache = Arc::new(object_graph::Cache::new());
    let user_class = TsClass::from_code(code1.to_string(), &cache).unwrap();
    let user_pair_class = TsClass::from_code(code2.to_string(), &cache).unwrap();

    let user1 = user_class.generate_object(
        str_cached!(cache; "student1"),
        HashMap::from([
            (str_cached!(cache; "surname"), vstr!(cache; "Doe")),
            (str_cached!(cache; "name"), vstr!(cache; "John")),
            (str_cached!(cache; "age"), vnum!(Number::from(25))),
            (str_cached!(cache; "is_admin"), vbool!(true)),
        ]),
    );

    let user2 = user_class.generate_object(
        str_cached!(cache; "student2"),
        HashMap::from([
            (str_cached!(cache; "name"), vstr!(cache; "Paul")),
            (str_cached!(cache; "age"), vnum!(Number::from(27))),
            (str_cached!(cache; "surname"), vstr!(cache; "Simon")),
            (str_cached!(cache; "is_admin"), vbool!(false)),
        ]),
    );

    let complex_user = user_pair_class.generate_object(
        str_cached!(cache; "student_pair"),
        HashMap::from([
            (str_cached!(cache; "user1"), user1),
            (str_cached!(cache; "user2"), user2),
        ]),
    );

    println!("{}", complex_user);
    let mut opcodes = construct_opcode_list(
        &[str_cached!(cache; "x")],
        &[-1f64, 1f64],
        &[str_cached!(cache; " ")],
        false,
    );
    add_num_opcodes(
        &mut opcodes,
        &ALL_BIN_NUM_OPCODES,
        &[],
        &ALL_UPDATE_NUM_OPCODES,
    );
    add_str_opcodes(&mut opcodes, &ALL_BIN_STR_OPCODES);
    opcodes.extend(user_class.member_opcodes().clone());
    opcodes.extend(user_pair_class.member_opcodes().clone());

    let ctx = Arc::new([Context::with_values(
        [(str_cached!(cache; "x"), complex_user)].into(),
    )]);

    let cache_clone = cache.clone();
    let mut synthesizer = TsSynthesizer::new(
        ctx.clone(),
        opcodes,
        Box::new(move |p| {
            let expected_outputs = [str_cached!(cache_clone; "John Doe")];
            if p.out_type() != ValueType::String {
                return false;
            }
            if p.out_value()[0].loc() != &Location::Temp {
                return false;
            }
            for (v, e) in p.out_value().iter().zip(expected_outputs) {
                let v_str = unsafe { v.val().string_value().unwrap_unchecked() };
                if v_str != e {
                    return false;
                }
            }
            return true;
        }),
        Box::new(|_p| true),
        3,
    );

    let start = Instant::now();
    for i in 1..=5 {
        let iteration_start = Instant::now();
        let res = synthesizer.run_iteration(&cache).await;
        println!(
            "Iteration {} took {:.3}s",
            i,
            iteration_start.elapsed().as_secs_f32()
        );
        println!("statistics: {}", synthesizer.statistics());

        if let Some(p) = res {
            println!(
                "Found p = {{{}}} in {:.3}s",
                p.get_code(),
                start.elapsed().as_secs_f32()
            );
            break;
        }
    }
}

#[allow(dead_code)]
async fn run_synthesizer() {
    let cache = Arc::new(object_graph::Cache::new());

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

    let mut opcodes = construct_opcode_list(
        &[str_cached!(cache; "x"), str_cached!(cache; "y")],
        &[-1f64, 1f64],
        &[],
        false,
    );

    add_num_opcodes(
        &mut opcodes,
        &ALL_BIN_NUM_OPCODES,
        &[],
        &ALL_UPDATE_NUM_OPCODES,
    );

    let mut synthesizer = TsSynthesizer::new(
        ctx.clone(),
        opcodes,
        Box::new(|x| {
            let expected_outputs = [Number::from(225), Number::from(576)];
            if x.out_type() != ValueType::Number {
                return false;
            }
            // let x_var = x.post_ctx()[0].get_var_loc_value(&str_cached!(cache; "x"));
            // let y_var = x.post_ctx()[0].get_var_loc_value(&str_cached!(cache; "y"));
            // if x_var.val() != &vnum!(Number::from(5u64)) { return false; }
            // if y_var.val() != &vnum!(Number::from(3u64)) { return false; }
            for (v, e) in x.out_value().iter().zip(expected_outputs) {
                let v_num = unsafe { v.val().number_value().unwrap_unchecked() };
                if v_num != e {
                    return false;
                }
            }
            return true;
        }),
        Box::new(|x| {
            if x.out_type() == ValueType::Number {
                // println!("({}) {{ {} }} ({}, {})", x.pre_ctx()[0], x.get_code(), x.out_value()[0].val().number_value().unwrap(), x.post_ctx()[0]);
                return x.out_value().iter().all(|v| {
                    let n = v.val().number_value().unwrap().0;
                    return n.is_finite() && n.abs() < 1000f64;
                });
            }
            return true;
        }),
        3,
    );

    let start = Instant::now();
    for i in 1..=5 {
        let iteration_start = Instant::now();
        let res = synthesizer.run_iteration(&cache).await;
        println!(
            "Iteration {} took {:.3}s",
            i,
            iteration_start.elapsed().as_secs_f32()
        );
        println!("statistics: {}", synthesizer.statistics());

        if let Some(p) = res {
            println!(
                "Found p = {{{}}} in {:.3}s",
                p.get_code(),
                start.elapsed().as_secs_f32()
            );
            break;
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    // test_work_gatherer();

    // run_synthesizer().await;

    test_class_loader().await;

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
