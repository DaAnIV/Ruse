use std::collections::HashMap;

use itertools::Itertools;
use swc::atoms::Atom;
use swc_ecma_visit::{Visit, VisitWith};

use ruse_object_graph::{class_name, ObjectType, ValueType};
use swc_ecma_ast::{self as ast, Accessibility, ClassDecl, FnDecl, Id, MethodKind};

pub(crate) fn get_value_type_from_ts_type(type_ann: &ast::TsType) -> ValueType {
    match type_ann {
        ast::TsType::TsKeywordType(t) => match t.kind {
            ast::TsKeywordTypeKind::TsAnyKeyword => todo!(),
            ast::TsKeywordTypeKind::TsUnknownKeyword => todo!(),
            ast::TsKeywordTypeKind::TsNumberKeyword => ValueType::Number,
            ast::TsKeywordTypeKind::TsObjectKeyword => todo!(),
            ast::TsKeywordTypeKind::TsBooleanKeyword => ValueType::Bool,
            ast::TsKeywordTypeKind::TsBigIntKeyword => todo!(),
            ast::TsKeywordTypeKind::TsStringKeyword => ValueType::String,
            ast::TsKeywordTypeKind::TsSymbolKeyword => todo!(),
            ast::TsKeywordTypeKind::TsVoidKeyword => todo!(),
            ast::TsKeywordTypeKind::TsUndefinedKeyword => todo!(),
            ast::TsKeywordTypeKind::TsNullKeyword => ValueType::Null,
            ast::TsKeywordTypeKind::TsNeverKeyword => todo!(),
            ast::TsKeywordTypeKind::TsIntrinsicKeyword => todo!(),
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
#[allow(unused)]
pub(crate) struct DtsVarDecl {
    pub name: Id,
    pub var_type: Option<ValueType>,
}

#[derive(Debug)]
#[allow(unused)]
pub(crate) struct DtsFnDecl {
    pub name: Id,
    pub params: Vec<Vec<ValueType>>,
    pub has_rest_param: bool,
}

#[derive(Debug)]
#[allow(unused)]
pub(crate) struct DtsFieldDecl {
    pub name: Atom,
    pub var_type: Option<ValueType>,
    pub is_public: bool,
    pub is_static: bool,
    pub is_readonly: bool,
}

#[derive(Debug)]
#[allow(unused)]
pub(crate) struct DtsMethodDecl {
    pub name: Atom,
    pub kind: MethodKind,
    pub params: Vec<Vec<ValueType>>,
    pub has_rest_param: bool,
    pub is_public: bool,
    pub is_static: bool,
}

#[derive(Debug)]
#[allow(unused)]
pub(crate) struct DtsClassDecl {
    pub name: Id,
    pub obj_type: ObjectType,
    pub props: HashMap<Atom, DtsFieldDecl>,
    pub methods: HashMap<(Atom, MethodKind), DtsMethodDecl>,
    pub constructor: Option<DtsMethodDecl>,
}

#[derive(Default)]
pub(crate) struct DtsVisitor {
    pub classes: HashMap<Id, DtsClassDecl>,
    pub globals: HashMap<Id, DtsVarDecl>,
    pub functions: HashMap<Id, DtsFnDecl>,
    current_class: Option<Id>,
}

impl DtsVisitor {
    fn pat_to_value_type(&self, pat: &ast::Pat) -> Option<ValueType> {
        match pat {
            ast::Pat::Ident(binding_ident) => {
                Some(self.value_type_from_ts_type(&binding_ident.type_ann.as_ref()?.type_ann))
            }
            ast::Pat::Array(_array_pat) => todo!(),
            ast::Pat::Rest(rest_pat) => {
                Some(self.value_type_from_ts_type(&rest_pat.type_ann.as_ref()?.type_ann))
            }
            ast::Pat::Object(_object_pat) => todo!(),
            ast::Pat::Assign(_assign_pat) => todo!(),
            ast::Pat::Invalid(_invalid) => todo!(),
            ast::Pat::Expr(_expr) => todo!(),
        }
    }

    fn get_params_types_from_pat<'a, I>(&self, params: I) -> Option<Vec<ValueType>>
    where
        I: Iterator<Item = &'a ast::Pat>,
    {
        params.map(|param| self.pat_to_value_type(param)).collect()
    }

    fn get_params_types_from_ts_fn_param<'a, I>(&self, params: I) -> Option<Vec<ValueType>>
    where
        I: Iterator<Item = &'a ast::TsFnParam>,
    {
        params
            .map(|param| match param {
                ast::TsFnParam::Ident(binding_ident) => {
                    Some(self.value_type_from_ts_type(&binding_ident.type_ann.as_ref()?.type_ann))
                }
                ast::TsFnParam::Array(_array_pat) => todo!(),
                ast::TsFnParam::Rest(rest_pat) => {
                    Some(self.value_type_from_ts_type(&rest_pat.type_ann.as_ref()?.type_ann))
                }
                ast::TsFnParam::Object(_object_pat) => todo!(),
            })
            .collect()
    }

    fn update_fn_decl(&mut self, id: Id, params: Option<Vec<ValueType>>, has_rest_param: bool) {
        match self.functions.entry(id.clone()) {
            std::collections::hash_map::Entry::Occupied(mut e) => {
                let desc: &mut DtsFnDecl = e.get_mut();
                if let Some(params) = params {
                    desc.params.push(params);
                }
                desc.has_rest_param |= has_rest_param;
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(DtsFnDecl {
                    name: id,
                    params: params.map_or_else(Vec::new, |params| vec![params]),
                    has_rest_param,
                });
            }
        }
    }
}

impl Visit for DtsVisitor {
    fn visit_var_declarator(&mut self, node: &ast::VarDeclarator) {
        let ident = node.name.as_ident().unwrap();

        if let Some(type_ann) = ident.type_ann.as_ref() {
            if let Some(fn_or_constructor_type_ann) =
                type_ann.type_ann.as_ts_fn_or_constructor_type()
            {
                match fn_or_constructor_type_ann {
                    ast::TsFnOrConstructorType::TsFnType(ts_fn_type) => {
                        let params =
                            self.get_params_types_from_ts_fn_param(ts_fn_type.params.iter());
                        let has_rest_param = ts_fn_type.params.iter().any(|param| param.is_rest());
                        self.update_fn_decl(ident.id.clone().into(), params, has_rest_param);

                        return;
                    }
                    ast::TsFnOrConstructorType::TsConstructorType(_ts_constructor_type) => {
                        todo!("Var declarator constructor type")
                    }
                }
            }
        }

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
        let params = self.get_params_types_from_pat(node.function.params.iter().map(|x| &x.pat));
        let has_rest_param = node.function.params.iter().any(|param| param.pat.is_rest());
        self.update_fn_decl(node.ident.clone().into(), params, has_rest_param);
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

    fn visit_class_prop(&mut self, node: &ast::ClassProp) {
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

    fn visit_class_method(&mut self, node: &ast::ClassMethod) {
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

        let has_rest_param = function.params.iter().any(|param| param.pat.is_rest());

        let class: &mut DtsClassDecl = self
            .classes
            .get_mut(self.current_class.as_ref().unwrap())
            .unwrap();

        let is_public =
            node.accessibility.unwrap_or(Accessibility::Public) == Accessibility::Public;

        match class.methods.entry((id.clone(), node.kind.clone())) {
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
                    kind: node.kind.clone(),
                    params: params.map_or_else(Vec::new, |params| vec![params]),
                    has_rest_param,
                    is_public,
                    is_static: node.is_static,
                });
            }
        }
    }

    fn visit_constructor(&mut self, node: &ast::Constructor) {
        let id = node.key.as_ident().unwrap().sym.clone();
        let params = if node.params.iter().all(|param| match param {
            ast::ParamOrTsParamProp::Param(param) => {
                self.pat_to_value_type(&param.pat).is_some()
            }
            ast::ParamOrTsParamProp::TsParamProp(param) => {
                param.param.as_ident().unwrap().type_ann.is_some()
            }
        }) {
            Some(
                node.params
                    .iter()
                    .map(|param| match param {
                        ast::ParamOrTsParamProp::Param(param) => {
                            self.pat_to_value_type(&param.pat).unwrap()
                        }
                        ast::ParamOrTsParamProp::TsParamProp(param) => {
                            let ident = param.param.as_ident().unwrap();
                            self.value_type_from_ts_type(&ident.type_ann.as_ref().unwrap().type_ann)
                        }
                    })
                    .collect(),
            )
        } else {
            None
        };

        let has_rest_param = node.params.iter().any(|param| match param {
            ast::ParamOrTsParamProp::Param(param) => param.pat.is_rest(),
            ast::ParamOrTsParamProp::TsParamProp(param) => match &param.param {
                ast::TsParamPropParam::Ident(ident) => ident
                    .type_ann
                    .as_ref()
                    .map_or(false, |x| x.type_ann.is_ts_rest_type()),
                ast::TsParamPropParam::Assign(assign) => assign.left.is_rest(),
            },
        });

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
                    kind: MethodKind::Method,
                    params: params.map_or_else(Vec::new, |params| vec![params]),
                    has_rest_param,
                    is_public,
                    is_static: true,
                });
            }
        }
    }

    fn visit_ts_type_alias_decl(&mut self, node: &ast::TsTypeAliasDecl) {
        match node.type_ann.as_ref() {
            ast::TsType::TsTypeLit(t) => {
                println!("visit_ts_type_alias_decl: {:?}", t);
                todo!()
            }
            _ => todo!(),
        }
    }
}

impl DtsVisitor {
    fn value_type_from_ts_type(&self, type_ann: &ast::TsType) -> ValueType {
        match type_ann {
            ast::TsType::TsKeywordType(t) => match t.kind {
                ast::TsKeywordTypeKind::TsAnyKeyword => todo!(),
                ast::TsKeywordTypeKind::TsUnknownKeyword => todo!(),
                ast::TsKeywordTypeKind::TsNumberKeyword => ValueType::Number,
                ast::TsKeywordTypeKind::TsObjectKeyword => todo!(),
                ast::TsKeywordTypeKind::TsBooleanKeyword => ValueType::Bool,
                ast::TsKeywordTypeKind::TsBigIntKeyword => todo!(),
                ast::TsKeywordTypeKind::TsStringKeyword => ValueType::String,
                ast::TsKeywordTypeKind::TsSymbolKeyword => todo!(),
                ast::TsKeywordTypeKind::TsVoidKeyword => todo!(),
                ast::TsKeywordTypeKind::TsUndefinedKeyword => ValueType::Null,
                ast::TsKeywordTypeKind::TsNullKeyword => ValueType::Null,
                ast::TsKeywordTypeKind::TsNeverKeyword => todo!(),
                ast::TsKeywordTypeKind::TsIntrinsicKeyword => todo!(),
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
