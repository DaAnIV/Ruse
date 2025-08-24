use std::collections::HashMap;

use itertools::Itertools;
use swc::atoms::Atom;
use swc_ecma_visit::{Visit, VisitWith};

use ruse_object_graph::{class_name, ObjectType, ValueType};
use swc_ecma_ast::{self as ast, Accessibility, ClassDecl, FnDecl, Id};

pub(crate) fn get_value_type_from_ts_type(type_ann: &ast::TsType) -> ValueType {
    match type_ann {
        ast::TsType::TsKeywordType(t) => match t.kind {
            swc_ecma_ast::TsKeywordTypeKind::TsAnyKeyword => todo!(),
            swc_ecma_ast::TsKeywordTypeKind::TsUnknownKeyword => todo!(),
            swc_ecma_ast::TsKeywordTypeKind::TsNumberKeyword => ValueType::Number,
            swc_ecma_ast::TsKeywordTypeKind::TsObjectKeyword => todo!(),
            swc_ecma_ast::TsKeywordTypeKind::TsBooleanKeyword => ValueType::Bool,
            swc_ecma_ast::TsKeywordTypeKind::TsBigIntKeyword => todo!(),
            swc_ecma_ast::TsKeywordTypeKind::TsStringKeyword => ValueType::String,
            swc_ecma_ast::TsKeywordTypeKind::TsSymbolKeyword => todo!(),
            swc_ecma_ast::TsKeywordTypeKind::TsVoidKeyword => todo!(),
            swc_ecma_ast::TsKeywordTypeKind::TsUndefinedKeyword => todo!(),
            swc_ecma_ast::TsKeywordTypeKind::TsNullKeyword => ValueType::Null,
            swc_ecma_ast::TsKeywordTypeKind::TsNeverKeyword => todo!(),
            swc_ecma_ast::TsKeywordTypeKind::TsIntrinsicKeyword => todo!(),
        },
        ast::TsType::TsThisType(_) => todo!(),
        ast::TsType::TsFnOrConstructorType(_) => todo!(),
        ast::TsType::TsTypeRef(t) => {
            let id = t.type_name.as_ident().unwrap().sym.to_string();
            match id.as_str() {
                "Array" => {
                    let type_params = t.type_params.as_ref().unwrap();
                    assert!(type_params.params.len() == 1);
                    let elem_type = get_value_type_from_ts_type(&type_params.params[0]);
                    return ValueType::array_value_type(&elem_type);
                }
                "Set" => {
                    let type_params = t.type_params.as_ref().unwrap();
                    assert!(type_params.params.len() == 1);
                    let elem_type = get_value_type_from_ts_type(&type_params.params[0]);
                    return ValueType::set_value_type(&elem_type);
                }
                "Map" => {
                    let type_params = t.type_params.as_ref().unwrap();
                    assert!(type_params.params.len() == 2);
                    let key_type = get_value_type_from_ts_type(&type_params.params[0]);
                    let value_type = get_value_type_from_ts_type(&type_params.params[1]);
                    return ValueType::map_value_type(&key_type, &value_type);
                }
                _ => {}
            }
            ValueType::class_value_type(class_name!(id))
        }
        ast::TsType::TsTypeQuery(_) => todo!(),
        ast::TsType::TsTypeLit(_) => todo!(),
        ast::TsType::TsArrayType(t) => {
            let elem_type = get_value_type_from_ts_type(t.elem_type.as_ref());
            ValueType::array_value_type(&elem_type)
        }
        ast::TsType::TsTupleType(_) => todo!(),
        ast::TsType::TsOptionalType(_) => todo!(),
        ast::TsType::TsRestType(_) => todo!(),
        ast::TsType::TsUnionOrIntersectionType(t) => {
            if let Some(u) = t.as_ts_union_type() {
                if u.types.len() == 2 {
                    let left = get_value_type_from_ts_type(&u.types[0]);
                    let right = get_value_type_from_ts_type(&u.types[1]);
                    if left == ValueType::Null && !right.is_primitive() {
                        return right;
                    } else if right == ValueType::Null && !left.is_primitive() {
                        return left;
                    }
                }
            }

            todo!()
        }
        ast::TsType::TsConditionalType(_) => todo!(),
        ast::TsType::TsInferType(_) => todo!(),
        ast::TsType::TsParenthesizedType(_) => todo!(),
        ast::TsType::TsTypeOperator(_) => todo!(),
        ast::TsType::TsIndexedAccessType(_) => todo!(),
        ast::TsType::TsMappedType(_) => todo!(),
        ast::TsType::TsLitType(_) => todo!(),
        ast::TsType::TsTypePredicate(_) => todo!(),
        ast::TsType::TsImportType(_) => todo!(),
    }
}

#[derive(Debug)]
pub(crate) struct DtsVarDecl {
    pub name: Id,
    pub var_type: Option<ValueType>,
}

#[derive(Debug)]
pub(crate) struct DtsFnDecl {
    pub name: Id,
    pub params: Vec<Vec<ValueType>>,
}

#[derive(Debug)]
pub(crate) struct DtsFieldDecl {
    pub name: Atom,
    pub var_type: Option<ValueType>,
    pub is_public: bool,
    pub is_static: bool,
    pub is_readonly: bool,
}

#[derive(Debug)]
pub(crate) struct DtsMethodDecl {
    pub name: Atom,
    pub params: Vec<Vec<ValueType>>,
    pub is_public: bool,
    pub is_static: bool,
}

#[derive(Debug)]
pub(crate) struct DtsClassDecl {
    pub name: Id,
    pub obj_type: ObjectType,
    pub props: HashMap<Atom, DtsFieldDecl>,
    pub methods: HashMap<Atom, DtsMethodDecl>,
    pub constructor: Option<DtsMethodDecl>,
}

#[derive(Default)]
pub(crate) struct DtsVisitor {
    pub classes: HashMap<Id, DtsClassDecl>,
    pub globals: HashMap<Id, DtsVarDecl>,
    pub functions: HashMap<Id, DtsFnDecl>,
    current_class: Option<Id>,
}

impl Visit for DtsVisitor {
    fn visit_var_declarator(&mut self, node: &swc_ecma_ast::VarDeclarator) {
        let ident = node.name.as_ident().unwrap();
        self.globals.insert(
            ident.id.clone().into(),
            DtsVarDecl {
                name: ident.id.clone().into(),
                var_type: ident
                    .type_ann
                    .as_ref()
                    .map(|x| self.value_type_from_ts_type(&x.type_ann)),
            },
        );
    }

    fn visit_fn_decl(&mut self, node: &FnDecl) {
        let params = if node
            .function
            .params
            .iter()
            .all(|param| param.pat.as_ident().unwrap().type_ann.is_some())
        {
            Some(
                node.function
                    .params
                    .iter()
                    .map(|param| {
                        let ident = param.pat.as_ident().unwrap();
                        self.value_type_from_ts_type(&ident.type_ann.as_ref().unwrap().type_ann)
                    })
                    .collect(),
            )
        } else {
            None
        };

        let id = node.ident.clone().into();

        match self.functions.entry(id) {
            std::collections::hash_map::Entry::Occupied(mut e) => {
                let desc: &mut DtsFnDecl = e.get_mut();
                if let Some(params) = params {
                    desc.params.push(params);
                }
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(DtsFnDecl {
                    name: node.ident.clone().into(),
                    params: params.map_or_else(Vec::new, |params| vec![params]),
                });
            }
        }
    }

    fn visit_class_decl(&mut self, node: &ClassDecl) {
        self.current_class = Some(node.ident.clone().into());
        self.classes.insert(
            node.ident.clone().into(),
            DtsClassDecl {
                name: node.ident.clone().into(),
                obj_type: ObjectType::class_obj_type(&node.ident.clone().sym.to_string()),
                props: HashMap::new(),
                methods: HashMap::new(),
                constructor: None,
            },
        );
        <ClassDecl as VisitWith<Self>>::visit_children_with(node, self)
    }

    fn visit_class_prop(&mut self, node: &swc_ecma_ast::ClassProp) {
        let id = node.key.as_ident().unwrap().sym.clone();
        let var_type = node
            .type_ann
            .as_ref()
            .map(|x| self.value_type_from_ts_type(&x.type_ann));

        let class = self
            .classes
            .get_mut(self.current_class.as_ref().unwrap())
            .unwrap();
        class.props.insert(
            id.clone(),
            DtsFieldDecl {
                name: id,
                var_type,
                is_public: node.accessibility.unwrap_or(Accessibility::Public)
                    == Accessibility::Public,
                is_static: node.is_static,
                is_readonly: node.readonly,
            },
        );
    }

    fn visit_class_method(&mut self, node: &swc_ecma_ast::ClassMethod) {
        let id = node.key.as_ident().unwrap().sym.clone();
        let function = &node.function;
        let params = if function
            .params
            .iter()
            .all(|param| param.pat.as_ident().unwrap().type_ann.is_some())
        {
            Some(
                function
                    .params
                    .iter()
                    .map(|param| {
                        let ident = param.pat.as_ident().unwrap();
                        self.value_type_from_ts_type(&ident.type_ann.as_ref().unwrap().type_ann)
                    })
                    .collect(),
            )
        } else {
            None
        };

        let class: &mut DtsClassDecl = self
            .classes
            .get_mut(self.current_class.as_ref().unwrap())
            .unwrap();

        let is_public =
            node.accessibility.unwrap_or(Accessibility::Public) == Accessibility::Public;

        match class.methods.entry(id.clone()) {
            std::collections::hash_map::Entry::Occupied(mut e) => {
                let desc: &mut DtsMethodDecl = e.get_mut();
                if let Some(params) = params {
                    desc.params.push(params);
                }
                assert_eq!(desc.is_public, is_public);
                assert_eq!(desc.is_static, node.is_static);
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(DtsMethodDecl {
                    name: id,
                    params: params.map_or_else(Vec::new, |params| vec![params]),
                    is_public,
                    is_static: node.is_static,
                });
            }
        }
    }

    fn visit_constructor(&mut self, node: &swc_ecma_ast::Constructor) {
        let id = node.key.as_ident().unwrap().sym.clone();
        let params = if node.params.iter().all(|param| match param {
            swc_ecma_ast::ParamOrTsParamProp::Param(param) => {
                param.pat.as_ident().unwrap().type_ann.is_some()
            }
            swc_ecma_ast::ParamOrTsParamProp::TsParamProp(param) => {
                param.param.as_ident().unwrap().type_ann.is_some()
            }
        }) {
            Some(
                node.params
                    .iter()
                    .map(|param| match param {
                        swc_ecma_ast::ParamOrTsParamProp::Param(param) => {
                            let ident = param.pat.as_ident().unwrap();
                            self.value_type_from_ts_type(&ident.type_ann.as_ref().unwrap().type_ann)
                        }
                        swc_ecma_ast::ParamOrTsParamProp::TsParamProp(param) => {
                            let ident = param.param.as_ident().unwrap();
                            self.value_type_from_ts_type(&ident.type_ann.as_ref().unwrap().type_ann)
                        }
                    })
                    .collect(),
            )
        } else {
            None
        };

        let class: &mut DtsClassDecl = self
            .classes
            .get_mut(self.current_class.as_ref().unwrap())
            .unwrap();

        let is_public =
            node.accessibility.unwrap_or(Accessibility::Public) == Accessibility::Public;

        match &mut class.constructor {
            Some(desc) => {
                if let Some(params) = params {
                    desc.params.push(params);
                }
                assert_eq!(desc.is_public, is_public);
            }
            None => {
                class.constructor = Some(DtsMethodDecl {
                    name: id,
                    params: params.map_or_else(Vec::new, |params| vec![params]),
                    is_public,
                    is_static: true,
                });
            }
        }
    }
}

impl DtsVisitor {
    fn value_type_from_ts_type(&self, type_ann: &ast::TsType) -> ValueType {
        match type_ann {
            ast::TsType::TsKeywordType(t) => match t.kind {
                swc_ecma_ast::TsKeywordTypeKind::TsAnyKeyword => todo!(),
                swc_ecma_ast::TsKeywordTypeKind::TsUnknownKeyword => todo!(),
                swc_ecma_ast::TsKeywordTypeKind::TsNumberKeyword => ValueType::Number,
                swc_ecma_ast::TsKeywordTypeKind::TsObjectKeyword => todo!(),
                swc_ecma_ast::TsKeywordTypeKind::TsBooleanKeyword => ValueType::Bool,
                swc_ecma_ast::TsKeywordTypeKind::TsBigIntKeyword => todo!(),
                swc_ecma_ast::TsKeywordTypeKind::TsStringKeyword => ValueType::String,
                swc_ecma_ast::TsKeywordTypeKind::TsSymbolKeyword => todo!(),
                swc_ecma_ast::TsKeywordTypeKind::TsVoidKeyword => todo!(),
                swc_ecma_ast::TsKeywordTypeKind::TsUndefinedKeyword => todo!(),
                swc_ecma_ast::TsKeywordTypeKind::TsNullKeyword => ValueType::Null,
                swc_ecma_ast::TsKeywordTypeKind::TsNeverKeyword => todo!(),
                swc_ecma_ast::TsKeywordTypeKind::TsIntrinsicKeyword => todo!(),
            },
            ast::TsType::TsThisType(_) => ValueType::Object(ObjectType::class_obj_type(
                self.current_class.as_ref().unwrap().0.as_str(),
            )),
            ast::TsType::TsFnOrConstructorType(_) => todo!(),
            ast::TsType::TsTypeRef(t) => {
                let id = t.type_name.as_ident().unwrap().sym.to_string();
                match id.as_str() {
                    "Array" => {
                        let type_params = t.type_params.as_ref().unwrap();
                        assert!(type_params.params.len() == 1);
                        let elem_type = self.value_type_from_ts_type(&type_params.params[0]);
                        return ValueType::array_value_type(&elem_type);
                    }
                    "Set" => {
                        let type_params = t.type_params.as_ref().unwrap();
                        assert!(type_params.params.len() == 1);
                        let elem_type = self.value_type_from_ts_type(&type_params.params[0]);
                        return ValueType::set_value_type(&elem_type);
                    }
                    "Map" => {
                        let type_params = t.type_params.as_ref().unwrap();
                        assert!(type_params.params.len() == 2);
                        let key_type = self.value_type_from_ts_type(&type_params.params[0]);
                        let value_type = self.value_type_from_ts_type(&type_params.params[1]);
                        return ValueType::map_value_type(&key_type, &value_type);
                    }
                    _ => {}
                }
                ValueType::class_value_type(class_name!(id))
            }
            ast::TsType::TsTypeQuery(_) => todo!(),
            ast::TsType::TsTypeLit(_) => todo!(),
            ast::TsType::TsArrayType(t) => {
                let elem_type = self.value_type_from_ts_type(t.elem_type.as_ref());
                ValueType::array_value_type(&elem_type)
            }
            ast::TsType::TsTupleType(t) => {
                let value_types = t
                    .elem_types
                    .iter()
                    .map(|x| self.value_type_from_ts_type(&x.ty.as_ref()))
                    .collect_vec();
                if value_types.iter().all_equal() {
                    return ValueType::array_value_type(&value_types[0]);
                }
                todo!("Tuple type with different element types is not yet supported")
            }
            ast::TsType::TsOptionalType(_) => todo!(),
            ast::TsType::TsRestType(_) => todo!(),
            ast::TsType::TsUnionOrIntersectionType(t) => {
                if let Some(u) = t.as_ts_union_type() {
                    if u.types.len() == 2 {
                        let left = self.value_type_from_ts_type(&u.types[0]);
                        let right = self.value_type_from_ts_type(&u.types[1]);
                        if left == ValueType::Null && !right.is_primitive() {
                            return right;
                        } else if right == ValueType::Null && !left.is_primitive() {
                            return left;
                        }
                    }
                }

                todo!()
            }
            ast::TsType::TsConditionalType(_) => todo!(),
            ast::TsType::TsInferType(_) => todo!(),
            ast::TsType::TsParenthesizedType(_) => todo!(),
            ast::TsType::TsTypeOperator(_) => todo!(),
            ast::TsType::TsIndexedAccessType(_) => todo!(),
            ast::TsType::TsMappedType(_) => todo!(),
            ast::TsType::TsLitType(_) => todo!(),
            ast::TsType::TsTypePredicate(_) => todo!(),
            ast::TsType::TsImportType(_) => todo!(),
        }
    }
}
