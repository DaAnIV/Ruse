use std::{collections::HashMap, sync::Arc};

use ruse_object_graph::*;
use ruse_synthesizer::context::*;
use ruse_synthesizer::opcode::ExprOpcode;
use ruse_synthesizer::value::*;
use ruse_synthesizer::*;
use swc_common::{util::take::Take, DUMMY_SP};
use swc_ecma_ast as ast;

use super::TsExprAst;

#[derive(Debug)]
pub enum LitOp {
    Null,
    Str(CachedString),
    Bool(bool),
    Num(Number),
}

#[derive(Debug)]
pub struct ArrayLitOp {
    pub size: u32,
}

impl ExprOpcode<TsExprAst> for LitOp {
    fn eval(&self, ctx: &mut Context, args: &[&LocValue], _: &Cache) -> LocValue {
        debug_assert_eq!(args.len(), 0);
        let val = match self {
            LitOp::Null => Value::Primitive(PrimitiveValue::Null),
            LitOp::Str(s) => vcstring!(s.clone()),
            LitOp::Bool(b) => vbool!(*b),
            LitOp::Num(n) => vnum!(*n),
        };

        ctx.temp_value(val)
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

impl ExprOpcode<TsExprAst> for ArrayLitOp {
    fn eval(&self, ctx: &mut Context, args: &[&LocValue], cache: &Cache) -> LocValue {
        let mut fields = FieldsMap::new();
        let mut seen_graphs = HashMap::new();
        let mut obj_keys = vec![];

        for (i, val) in (0..self.size).zip(args) {
            visit_field(
                &val.val(),
                &mut fields,
                scached!(cache; i.to_string()),
                &mut seen_graphs,
                &mut obj_keys,
            );
        }

        ctx.temp_value(create_out_object(
            seen_graphs.into_values().collect(),
            cache,
            str_cached!(cache; "Array"),
            fields,
            &obj_keys,
        ))
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
    graphs: Vec<Arc<ObjectGraph>>,
    cache: &Cache,
    obj_type: CachedString,
    fields: FieldsMap,
    obj_keys: &Vec<(CachedString, (u64, NodeIndex))>,
) -> Value {
    let (mut out, nodes_map) = ObjectGraph::union(&graphs);

    let root = out.add_root(cache.temp_string(), ObjectData::new(obj_type, fields.into()));
    for (key, old_node) in obj_keys {
        out.add_edge(root, nodes_map[old_node], &key);
    }

    out.generate_serialized_data()
        .expect("Failed to serialize new graph");

    vobj!(out.into(), root)
}

fn visit_field(
    val: &Value,
    fields: &mut FieldsMap,
    key: CachedString,
    seen_graphs: &mut HashMap<u64, Arc<ObjectGraph>>,
    obj_keys: &mut Vec<(CachedString, (u64, NodeIndex))>,
) {
    match val {
        Value::Primitive(p) => {
            fields.insert(key, p.clone());
        }
        Value::Object(o) => {
            let ptr = Arc::as_ptr(&o.graph) as u64;
            if seen_graphs.contains_key(&ptr) {
                seen_graphs.insert(ptr, o.graph.clone());
            };
            obj_keys.push((key.clone(), (ptr, o.node)));
        }
    };
}
