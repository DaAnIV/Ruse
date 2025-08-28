use std::collections::HashSet;

use swc::atoms::Atom;
use swc_common::DUMMY_SP;
use swc_ecma_utils::find_pat_ids;
use swc_ecma_visit::{Visit, VisitWith};

use swc_ecma_ast::{
    BlockStmt, BlockStmtOrExpr, ClassDecl, Decl, ExportAll, ExportDecl, ExportDefaultDecl,
    ExportDefaultExpr, ExportNamedSpecifier, ExportSpecifier, Expr, ExprStmt, FnDecl, Function, Id,
    Ident, ModuleExportName, NamedExport, Param, Str, VarDeclarator,
};

#[derive(Default)]
pub(crate) struct ProgramVisitor {
    pub classes: Vec<(Id, ClassDecl)>,
    pub globals: Vec<(Id, VarDeclarator)>,
    pub functions: Vec<(Id, FnDecl)>,
    pub export: HashSet<Atom>,
}

impl Visit for ProgramVisitor {
    fn visit_var_declarator(&mut self, node: &VarDeclarator) {
        let ident = node.name.as_ident().unwrap();
        if let Some(init) = &node.init {
            match init.as_ref() {
                Expr::Fn(fn_expr) => {
                    self.functions.push((
                        ident.id.clone().into(),
                        FnDecl {
                            ident: ident.id.clone(),
                            declare: false,
                            function: fn_expr.function.clone(),
                        },
                    ));
                    return;
                }
                Expr::Arrow(arrow_expr) => {
                    self.functions.push((
                        ident.id.clone().into(),
                        FnDecl {
                            ident: ident.id.clone(),
                            declare: false,
                            function: Box::new(Function {
                                params: arrow_expr
                                    .params
                                    .iter()
                                    .cloned()
                                    .map(Param::from)
                                    .collect(),
                                decorators: vec![],
                                span: arrow_expr.span,
                                ctxt: arrow_expr.ctxt,
                                body: Some(match arrow_expr.body.as_ref() {
                                    BlockStmtOrExpr::BlockStmt(block) => block.clone(),
                                    BlockStmtOrExpr::Expr(expr) => BlockStmt {
                                        span: DUMMY_SP,
                                        ctxt: Default::default(),
                                        stmts: vec![ExprStmt {
                                            span: DUMMY_SP,
                                            expr: expr.clone(),
                                        }
                                        .into()],
                                    },
                                }),
                                is_async: arrow_expr.is_async,
                                is_generator: arrow_expr.is_generator,
                                type_params: arrow_expr.type_params.clone(),
                                return_type: arrow_expr.return_type.clone(),
                            }),
                        },
                    ));
                    return;
                }
                _ => {}
            }
        }
        self.globals.push((ident.id.clone().into(), node.clone()));
    }

    fn visit_fn_decl(&mut self, node: &FnDecl) {
        self.functions
            .push((node.ident.clone().into(), node.clone()));
    }

    fn visit_class_decl(&mut self, node: &ClassDecl) {
        self.classes.push((node.ident.clone().into(), node.clone()));
    }

    /// ```javascript
    /// export const foo = 1, bar = 2, { baz } = { baz: 3 };
    /// export let a = 1, [b] = [2];
    /// export function x() {}
    /// export class y {}
    /// ```
    fn visit_export_decl(&mut self, node: &ExportDecl) {
        match &node.decl {
            Decl::Class(ClassDecl { ident, .. }) | Decl::Fn(FnDecl { ident, .. }) => {
                self.export.insert(ident.sym.clone());
            }

            Decl::Var(v) => {
                self.export.extend(
                    find_pat_ids::<_, Ident>(&v.decls)
                        .into_iter()
                        .map(|id| (id.sym.clone())),
                );
            }
            _ => {}
        };

        <ExportDecl as VisitWith<Self>>::visit_children_with(node, self)
    }

    /// ```javascript
    /// export { foo, foo as bar, foo as "baz" };
    /// export { "foo", foo as bar, "foo" as "baz" } from "mod";
    /// export * as foo from "mod";
    /// export * as "bar" from "mod";
    /// ```
    fn visit_named_export(&mut self, n: &NamedExport) {
        if n.type_only {
            return;
        }

        let NamedExport {
            specifiers, src, ..
        } = n;

        if let Some(_src) = src {
            todo!()
        } else {
            self.export.extend(specifiers.into_iter().map(|e| match e {
                ExportSpecifier::Namespace(..) => {
                    unreachable!("`export *` without src is invalid")
                }
                ExportSpecifier::Default(..) => {
                    unreachable!("`export foo` without src is invalid")
                }
                ExportSpecifier::Named(ExportNamedSpecifier { orig, exported, .. }) => {
                    let orig = match orig {
                        ModuleExportName::Ident(id) => id,
                        ModuleExportName::Str(_) => {
                            unreachable!(r#"`export {{ "foo" }}` without src is invalid"#)
                        }
                    };

                    if let Some(exported) = exported {
                        let export_name = match exported {
                            ModuleExportName::Ident(Ident {
                                ctxt: _,
                                span: _,
                                sym,
                                ..
                            }) => sym,
                            ModuleExportName::Str(Str { span: _, value, .. }) => value,
                        };

                        export_name.clone()
                    } else {
                        orig.sym.clone()
                    }
                }
            }))
        }
    }

    /// ```javascript
    /// export default class foo {};
    /// export default class {};
    /// export default function bar () {};
    /// export default function () {};
    /// ```
    fn visit_export_default_decl(&mut self, _n: &ExportDefaultDecl) {
        todo!()
    }

    /// ```javascript
    /// export default foo;
    /// export default 1
    /// ```
    fn visit_export_default_expr(&mut self, _n: &ExportDefaultExpr) {
        todo!()
    }

    /// ```javascript
    /// export * from "mod";
    /// ```
    fn visit_export_all(&mut self, _n: &ExportAll) {
        todo!()
    }
}
