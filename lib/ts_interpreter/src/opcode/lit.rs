use std::{collections::HashMap, sync::Arc};

use ruse_object_graph::*;
use ruse_synthesizer::context::*;
use ruse_synthesizer::opcode::SynthesizerExprOpcode;
use ruse_synthesizer::value::*;
use ruse_synthesizer::*;
use swc_common::{util::take::Take, DUMMY_SP};
use swc_ecma_ast as ast;

use super::TsExprAst;

pub enum LitOp {
    Null,
    Str(Arc<String>),
    Bool(bool),
    Num(Number),
}
pub struct ArrayLitOp {
    pub size: u32,
}

impl SynthesizerExprOpcode<TsExprAst> for LitOp {
    fn eval(&self, ctx: &Context, args: &[&LocValue], _: &mut Cache) -> (Context, LocValue) {
        debug_assert_eq!(args.len(), 0);
        let val = match self {
            LitOp::Null => Value::Primitive(PrimitiveValue::Null),
            LitOp::Str(s) => vcstring!(s.clone()),
            LitOp::Bool(b) => vbool!(*b),
            LitOp::Num(n) => vnum!(*n),
        };

        (
            ctx.clone(),
            LocValue {
                loc: Location::Temp,
                val: val,
            },
        )
    }

    fn to_ast(&self, children: &Vec<TsExprAst>) -> TsExprAst {
        debug_assert_eq!(children.len(), 0);

        let expr = match self {
            LitOp::Null => ast::Lit::Null(ast::Null::dummy()).into(),
            LitOp::Str(s) => ast::Lit::Str(ast::Str {
                span: DUMMY_SP,
                value: s.as_str().into(),
                raw: None,
            }),
            LitOp::Bool(b) => ast::Lit::Bool(ast::Bool {
                span: DUMMY_SP,
                value: *b,
            }),
            LitOp::Num(n) => ast::Lit::Num(ast::Number {
                span: DUMMY_SP,
                value: n.0,
                raw: None,
            }),
        };

        ast::Expr::Lit(expr).into()
    }

    fn arg_types(&self) -> &[ValueType] {
        &[]
    }
}

impl SynthesizerExprOpcode<TsExprAst> for ArrayLitOp {
    fn eval(&self, ctx: &Context, args: &[&LocValue], cache: &mut Cache) -> (Context, LocValue) {
        let mut fields = FieldsMap::new();
        let mut graphs = vec![];
        let mut seen_graphs = HashMap::new();
        let mut obj_keys = vec![];

        for (i, val) in (0..self.size).zip(args) {
            visit_field(
                &val.val,
                &mut fields,
                scached!(cache; i.to_string()),
                &mut seen_graphs,
                &mut graphs,
                cache,
                &mut obj_keys,
            );
        }

        (
            ctx.clone(),
            create_out_object(graphs, cache, fields, obj_keys),
        )
    }

    fn to_ast(&self, children: &Vec<TsExprAst>) -> TsExprAst {
        let expr = ast::ArrayLit {
            span: DUMMY_SP,
            elems: children
                .into_iter()
                .map(|x| {
                    Some(ast::ExprOrSpread {
                        spread: None,
                        expr: x.node.to_owned(),
                    })
                })
                .collect(),
        };

        ast::Expr::Array(expr).into()
    }

    fn arg_types(&self) -> &[ValueType] {
        &[]
    }
}

fn create_out_object(
    graphs: Vec<ObjectGraph>,
    cache: &mut Cache,
    fields: FieldsMap,
    obj_keys: Vec<(Arc<String>, Arc<String>)>,
) -> LocValue {
    let mut out = ObjectGraph::union(&graphs);

    let root = out.add_root(str_cached!(cache; "out"), ObjectData::new(fields.into()));
    for (key, tmp_key) in &obj_keys {
        out.add_edge(root, out.get_root(tmp_key), key.clone());
        out.remove_root(tmp_key)
    }

    LocValue {
        loc: Location::Temp,
        val: vobj!(out.into(), root),
    }
}

fn visit_field(
    val: &Value,
    fields: &mut FieldsMap,
    key: Arc<String>,
    seen_graphs: &mut HashMap<u64, usize>,
    graphs: &mut Vec<ObjectGraph>,
    cache: &mut Cache,
    obj_keys: &mut Vec<(Arc<String>, Arc<String>)>,
) {
    match val {
        Value::Primitive(p) => {
            fields.insert(key, p.clone());
        }
        Value::Object(o) => {
            let ptr = Arc::as_ptr(&o.graph) as u64;
            let new_graph = match seen_graphs.get(&ptr) {
                Some(i) => &mut graphs[*i],
                None => {
                    let tmp = (*o.graph).clone();
                    graphs.push(tmp);
                    let i = graphs.len() - 1;
                    seen_graphs.insert(ptr, i);
                    &mut graphs[i]
                }
            };
            let tmp_key = scached!(cache; format!("____tmp_{key}"));
            new_graph.set_as_root(tmp_key.clone(), o.node);
            obj_keys.push((key, tmp_key));
        }
    };
}
