use ruse_object_graph::{scached, Cache, PrimitiveValue};
use ruse_synthesizer::context::*;
use ruse_synthesizer::opcode::SynthesizerExprOpcode;
use ruse_synthesizer::value::*;

use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use super::TsExprAst;

pub struct MemberOp {
    arg_types: [ValueType; 2],
}

impl MemberOp {
    pub fn new(field_access_type: ValueType) -> Self {
        Self {
            arg_types: [ValueType::Object, field_access_type],
        }
    }
}

impl SynthesizerExprOpcode<TsExprAst> for MemberOp {
    fn eval(&self, ctx: &mut Context, args: &[&LocValue], cache: &Cache) -> LocValue {
        debug_assert_eq!(args.len(), 2);

        let obj = args[0].val().obj().unwrap();
        let field_name = match args[1].val().primitive().unwrap() {
            PrimitiveValue::Number(n) => scached!(cache; n.to_string()),
            PrimitiveValue::String(s) => s.clone(),
            _ => unreachable!(),
        };
        let val = obj.get_field_value(&field_name).unwrap();
        let loc = match &args[0].loc() {
            Location::Temp => Location::Temp,
            Location::Var(l) => Location::ObjectField(ObjectFieldLoc {
                var: l.var.clone(),
                node: obj.node,
                field: field_name.clone(),
            }),
            Location::ObjectField(l) => Location::ObjectField(ObjectFieldLoc {
                var: l.var.clone(),
                node: obj.node,
                field: field_name.clone(),
            }),
        };

        ctx.get_loc_value(val, loc)
    }

    fn to_ast(&self, children: &Vec<TsExprAst>) -> TsExprAst {
        debug_assert_eq!(children.len(), 0);

        let expr = ast::MemberExpr {
            span: DUMMY_SP,
            obj: children[0].node.to_owned(),
            prop: ast::MemberProp::Computed(ast::ComputedPropName {
                span: DUMMY_SP,
                expr: children[1].node.to_owned(),
            }),
        };

        ast::Expr::Member(expr).into()
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}
