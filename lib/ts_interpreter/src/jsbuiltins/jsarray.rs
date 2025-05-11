use boa_engine::{
    context::intrinsics::StandardConstructor,
    js_string,
    object::{builtins::JsTypedArray, FunctionObjectBuilder},
    property::{Attribute, PropertyDescriptor},
    string::StaticJsStrings,
    JsObject, JsResult, JsSymbol, JsValue,
};
use ruse_object_graph::{
    class_name, field_name,
    value::{ObjectValue, Value},
    ClassName, ValueType,
};

use crate::{
    engine_context::{EngineContext, RuseJsGlobalObject},
    js_errors::*,
    js_object_value::JsObjectValue,
    js_value::TryFromJs,
    jsfn_wrap,
    ts_class::{BuiltinClassWrapper, TsBuiltinClass, TsClass},
};

use super::jsiterator::{JsObjectIterator, JsObjectIteratorKind};

#[derive(Debug)]
pub struct BuiltinArrayClass {
    class_name: ClassName,
    id: u64,
}

impl BuiltinArrayClass {
    pub const CLASS_NAME: &'static str = "Array";

    pub(crate) fn new(id: u64) -> Self {
        Self {
            class_name: class_name!(Self::CLASS_NAME),
            id,
        }
    }

    fn get_from_js_array(
        &self,
        js_array: &boa_engine::object::builtins::JsArray,
        engine_ctx: &mut EngineContext<'_>,
    ) -> JsResult<ObjectValue> {
        let arr_len = js_array.length(engine_ctx).unwrap() as i64;
        if arr_len == 0 {
            return engine_ctx.create_array_object(vec![], &ValueType::Null);
        }

        let elements = (0..arr_len)
            .map(|i| {
                let js_elem = js_array.at(i, engine_ctx).unwrap();
                let elem = Value::try_from_js(&js_elem, engine_ctx)?;
                Ok(elem)
            })
            .collect::<JsResult<Vec<Value>>>()?;

        let elem_type = elements[0].val_type();
        engine_ctx.create_array_object(elements, &elem_type)
    }

    fn get_from_js_typed_array(
        &self,
        js_typed_array: &JsTypedArray,
        engine_ctx: &mut EngineContext<'_>,
    ) -> Result<ObjectValue, boa_engine::JsError> {
        let arr_len = js_typed_array.length(engine_ctx).unwrap() as i64;
        if arr_len == 0 {
            return engine_ctx.create_array_object(vec![], &ValueType::Number);
        }

        let elements = (0..arr_len)
            .map(|i| {
                let js_elem = js_typed_array.at(i, engine_ctx).unwrap();
                let elem = Value::try_from_js(&js_elem, engine_ctx)?;
                assert!(elem.number_value().is_some());
                Ok(elem)
            })
            .collect::<JsResult<Vec<Value>>>()?;

        engine_ctx.create_array_object(elements, &ValueType::Number)
    }
}

impl TsClass for BuiltinArrayClass {
    fn obj_type(&self, template_types: Option<Vec<ValueType>>) -> ValueType {
        let template_types = template_types.unwrap();
        assert!(template_types.len() == 1);
        ValueType::array_value_type(&template_types[0])
    }

    fn wrap_as_js_object(
        &self,
        obj: ObjectValue,
        engine_ctx: &mut EngineContext<'_>,
    ) -> JsResult<boa_engine::JsObject> {
        JsArrayWrapper::wrap_object(&obj, engine_ctx)
    }

    fn is_parametrized(&self) -> bool {
        false
    }

    fn get_class_name(&self) -> &ClassName {
        &self.class_name
    }

    fn get_class_id(&self) -> u64 {
        self.id
    }
}

impl TsBuiltinClass for BuiltinArrayClass {
    fn is_builtin_object(&self, value: &boa_engine::JsObject) -> bool {
        value.is_array() || value.is::<boa_engine::builtins::typed_array::TypedArray>()
    }

    fn get_from_js_obj(
        &self,
        value: &boa_engine::JsObject,
        engine_ctx: &mut EngineContext<'_>,
    ) -> JsResult<ObjectValue> {
        if let Ok(js_array) = boa_engine::object::builtins::JsArray::from_object(value.clone()) {
            self.get_from_js_array(&js_array, engine_ctx)
        } else if let Ok(typed_array) =
            boa_engine::object::builtins::JsTypedArray::from_object(value.clone())
        {
            self.get_from_js_typed_array(&typed_array, engine_ctx)
        } else {
            Err(js_error_not_builtin_array())
        }
    }
}

pub(crate) struct JsArrayWrapper {}

impl JsArrayWrapper {
    fn getter_for_index(index: u64) -> Option<boa_engine::NativeFunction> {
        let field_name = field_name!(index.to_string());
        Some(unsafe {
            boa_engine::native_function::NativeFunction::from_closure(move |this, _, boa_ctx| {
                RuseJsGlobalObject::get_field(this, &field_name, &mut boa_ctx.into())
            })
        })
    }

    fn setter_for_index(index: u64) -> Option<boa_engine::NativeFunction> {
        let field_name = field_name!(index.to_string());
        Some(unsafe {
            boa_engine::native_function::NativeFunction::from_closure(move |this, args, boa_ctx| {
                RuseJsGlobalObject::set_field(this, &field_name, &args[0], &mut boa_ctx.into())?;
                Ok(boa_engine::JsValue::undefined())
            })
        })
    }
}

impl BuiltinClassWrapper for JsArrayWrapper {
    fn build_standard_constructor(
        engine_ctx: &mut EngineContext<'_>,
    ) -> JsResult<StandardConstructor> {
        let symbol_iterator = JsSymbol::iterator();
        let symbol_unscopables = JsSymbol::unscopables();

        let to_string_function =
            FunctionObjectBuilder::new(engine_ctx.realm(), jsfn_wrap!(Self::to_string))
                .name(js_string!("toString"))
                .build();
        let values_function = boa_engine::object::FunctionObjectBuilder::new(
            engine_ctx.realm(),
            jsfn_wrap!(Self::values),
        )
        .name(js_string!("values"))
        .build();
        let unscopables_object = Self::unscopables_object();

        let get_length = boa_engine::object::FunctionObjectBuilder::new(
            engine_ctx.realm(),
            jsfn_wrap!(Self::get_length),
        )
        .name(js_string!("get length"))
        .build();

        let set_length = boa_engine::object::FunctionObjectBuilder::new(
            engine_ctx.realm(),
            jsfn_wrap!(Self::set_length),
        )
        .name(js_string!("set length"))
        .build();

        let mut builder =
            boa_engine::object::ConstructorBuilder::new(engine_ctx, jsfn_wrap!(Self::constructor));

        builder
            .name("Array")
            .method(jsfn_wrap!(Self::at), js_string!("at"), 1)
            .method(jsfn_wrap!(Self::concat), js_string!("concat"), 1)
            .method(jsfn_wrap!(Self::copy_within), js_string!("copyWithin"), 2)
            .method(jsfn_wrap!(Self::entries), js_string!("entries"), 0)
            .method(jsfn_wrap!(Self::every), js_string!("every"), 1)
            .method(jsfn_wrap!(Self::fill), js_string!("fill"), 1)
            .method(jsfn_wrap!(Self::filter), js_string!("filter"), 1)
            .method(jsfn_wrap!(Self::find), js_string!("find"), 1)
            .method(jsfn_wrap!(Self::find_index), js_string!("findIndex"), 1)
            .method(jsfn_wrap!(Self::find_last), js_string!("findLast"), 1)
            .method(
                jsfn_wrap!(Self::find_last_index),
                js_string!("findLastIndex"),
                1,
            )
            .method(jsfn_wrap!(Self::flat), js_string!("flat"), 0)
            .method(jsfn_wrap!(Self::flat_map), js_string!("flatMap"), 1)
            .method(jsfn_wrap!(Self::for_each), js_string!("forEach"), 1)
            .method(jsfn_wrap!(Self::includes_value), js_string!("includes"), 1)
            .method(jsfn_wrap!(Self::index_of), js_string!("indexOf"), 1)
            .method(jsfn_wrap!(Self::join), js_string!("join"), 1)
            .method(jsfn_wrap!(Self::keys), js_string!("keys"), 0)
            .method(
                jsfn_wrap!(Self::last_index_of),
                js_string!("lastIndexOf"),
                1,
            )
            .method(jsfn_wrap!(Self::map), js_string!("map"), 1)
            .method(jsfn_wrap!(Self::pop), js_string!("pop"), 0)
            .method(jsfn_wrap!(Self::push), js_string!("push"), 1)
            .method(jsfn_wrap!(Self::reduce), js_string!("reduce"), 1)
            .method(jsfn_wrap!(Self::reduce_right), js_string!("reduceRight"), 1)
            .method(jsfn_wrap!(Self::reverse), js_string!("reverse"), 0)
            .method(jsfn_wrap!(Self::shift), js_string!("shift"), 0)
            .method(jsfn_wrap!(Self::slice), js_string!("slice"), 2)
            .method(jsfn_wrap!(Self::some), js_string!("some"), 1)
            .method(jsfn_wrap!(Self::sort), js_string!("sort"), 1)
            .method(jsfn_wrap!(Self::splice), js_string!("splice"), 2)
            .method(
                jsfn_wrap!(Self::to_locale_string),
                js_string!("toLocaleString"),
                0,
            )
            .method(jsfn_wrap!(Self::to_reversed), js_string!("toReversed"), 0)
            .method(jsfn_wrap!(Self::to_sorted), js_string!("toSorted"), 1)
            .method(jsfn_wrap!(Self::to_spliced), js_string!("toSpliced"), 2)
            .method(jsfn_wrap!(Self::unshift), js_string!("unshift"), 1)
            .method(jsfn_wrap!(Self::with), js_string!("with"), 2)
            .property(
                JsSymbol::to_string_tag(),
                StaticJsStrings::ARRAY,
                Attribute::READONLY | Attribute::NON_ENUMERABLE | Attribute::CONFIGURABLE,
            )
            .property(
                js_string!("toString"),
                to_string_function,
                Attribute::WRITABLE | Attribute::NON_ENUMERABLE | Attribute::CONFIGURABLE,
            )
            .property(
                js_string!("values"),
                values_function.clone(),
                Attribute::WRITABLE | Attribute::NON_ENUMERABLE | Attribute::CONFIGURABLE,
            )
            .property(
                symbol_iterator,
                values_function,
                Attribute::WRITABLE | Attribute::NON_ENUMERABLE | Attribute::CONFIGURABLE,
            )
            .property(
                symbol_unscopables,
                unscopables_object,
                Attribute::READONLY | Attribute::NON_ENUMERABLE | Attribute::CONFIGURABLE,
            )
            .accessor(
                StaticJsStrings::LENGTH,
                Some(get_length),
                Some(set_length),
                Attribute::CONFIGURABLE,
            );

        Ok(builder.build())
    }

    fn wrap_object(
        array_obj: &ObjectValue,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<boa_engine::JsObject> {
        if !array_obj.is_array() {
            return Err(js_error_not_array_value());
        }

        let global_obj = engine_ctx.global_object();
        let global_ctx = RuseJsGlobalObject::from_object(&global_obj)?;

        let proto = global_ctx.constructors().array_prototype(engine_ctx)?;

        let mut builder = boa_engine::object::ObjectInitializer::with_native_data_and_proto(
            JsObjectValue::from(array_obj.clone()),
            proto,
            engine_ctx,
        );

        let graphs_map = global_ctx.graphs_map()?;

        for i in 0..array_obj.total_field_count(graphs_map) {
            let getter = Self::getter_for_index(i as u64)
                .map(|x| x.to_js_function(builder.context().realm()));
            let setter = Self::setter_for_index(i as u64)
                .map(|x| x.to_js_function(builder.context().realm()));
            builder.accessor(i, getter, setter, boa_engine::property::Attribute::WRITABLE);
        }

        let obj = builder.build();

        Ok(obj)
    }
}

impl JsArrayWrapper {
    pub(crate) fn constructor(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn at(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn concat(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn copy_within(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn entries(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn every(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn fill(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn filter(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn find(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn find_index(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn find_last(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn find_last_index(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn flat(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn flat_map(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn for_each(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn includes_value(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn index_of(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn join(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn keys(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn last_index_of(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn map(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn pop(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn push(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn reduce(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn reduce_right(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn reverse(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn shift(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn slice(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn some(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn sort(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn splice(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn to_locale_string(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn to_reversed(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn to_sorted(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn to_spliced(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn unshift(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn with(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn to_string(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn values(
        this: &JsValue,
        _: &[JsValue],
        context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        let obj = ObjectValue::try_from_js(this, context)?;
        JsObjectIterator::create_object_iterator(
            obj,
            JsObjectIteratorKind::Value,
            ValueType::Number,
            context,
        )
    }

    pub(crate) fn get_length(
        this: &JsValue,
        _args: &[JsValue],
        context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        let global_obj = context.global_object();
        let global_ctx = RuseJsGlobalObject::from_object(&global_obj)?;
        let graphs_map = global_ctx.graphs_map()?;

        let obj = ObjectValue::try_from_js(this, context)?;
        Ok(obj.total_field_count(graphs_map).into())
    }

    pub(crate) fn set_length(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    fn unscopables_object() -> JsObject {
        // 1. Let unscopableList be OrdinaryObjectCreate(null).
        let unscopable_list = JsObject::with_null_proto();
        let true_prop = PropertyDescriptor::builder()
            .value(true)
            .writable(true)
            .enumerable(true)
            .configurable(true);
        // 2. Perform ! CreateDataPropertyOrThrow(unscopableList, "at", true).
        unscopable_list.insert_property(js_string!("at"), true_prop.clone());
        // 3. Perform ! CreateDataPropertyOrThrow(unscopableList, "copyWithin", true).
        unscopable_list.insert_property(js_string!("copyWithin"), true_prop.clone());
        // 4. Perform ! CreateDataPropertyOrThrow(unscopableList, "entries", true).
        unscopable_list.insert_property(js_string!("entries"), true_prop.clone());
        // 5. Perform ! CreateDataPropertyOrThrow(unscopableList, "fill", true).
        unscopable_list.insert_property(js_string!("fill"), true_prop.clone());
        // 6. Perform ! CreateDataPropertyOrThrow(unscopableList, "find", true).
        unscopable_list.insert_property(js_string!("find"), true_prop.clone());
        // 7. Perform ! CreateDataPropertyOrThrow(unscopableList, "findIndex", true).
        unscopable_list.insert_property(js_string!("findIndex"), true_prop.clone());
        // 8. Perform ! CreateDataPropertyOrThrow(unscopableList, "findLast", true).
        unscopable_list.insert_property(js_string!("findLast"), true_prop.clone());
        // 9. Perform ! CreateDataPropertyOrThrow(unscopableList, "findLastIndex", true).
        unscopable_list.insert_property(js_string!("findLastIndex"), true_prop.clone());
        // 10. Perform ! CreateDataPropertyOrThrow(unscopableList, "flat", true).
        unscopable_list.insert_property(js_string!("flat"), true_prop.clone());
        // 11. Perform ! CreateDataPropertyOrThrow(unscopableList, "flatMap", true).
        unscopable_list.insert_property(js_string!("flatMap"), true_prop.clone());
        // 12. Perform ! CreateDataPropertyOrThrow(unscopableList, "includes", true).
        unscopable_list.insert_property(js_string!("includes"), true_prop.clone());
        // 13. Perform ! CreateDataPropertyOrThrow(unscopableList, "keys", true).
        unscopable_list.insert_property(js_string!("keys"), true_prop.clone());
        // 14. Perform ! CreateDataPropertyOrThrow(unscopableList, "toReversed", true).
        unscopable_list.insert_property(js_string!("toReversed"), true_prop.clone());
        // 15. Perform ! CreateDataPropertyOrThrow(unscopableList, "toSorted", true).
        unscopable_list.insert_property(js_string!("toSorted"), true_prop.clone());
        // 16. Perform ! CreateDataPropertyOrThrow(unscopableList, "toSpliced", true).
        unscopable_list.insert_property(js_string!("toSpliced"), true_prop.clone());
        // 17. Perform ! CreateDataPropertyOrThrow(unscopableList, "values", true).
        unscopable_list.insert_property(js_string!("values"), true_prop);

        // 13. Return unscopableList.
        unscopable_list
    }
}
