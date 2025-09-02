use boa_engine::{
    builtins::iterable::create_iter_result_object,
    context::intrinsics::StandardConstructor,
    js_string,
    object::{builtins::JsArray, ObjectInitializer},
    JsObject, JsResult, JsValue,
};
use itertools::Itertools;
use ruse_object_graph::{value::ObjectValue, FieldName, ValueType};

use crate::{
    engine_context::{EngineContext, RuseJsGlobalObject},
    js_errors::*,
    js_value::TryIntoJs,
    jsfn_wrap,
};

pub enum JsObjectIteratorKind {
    Field,
    Value,
    FieldValue,
}

pub struct JsObjectIterator {
    obj: ObjectValue,
    fields: Box<dyn Iterator<Item = FieldName>>,
    kind: JsObjectIteratorKind,
    field_type: ValueType,
}

impl JsObjectIterator {
    pub(crate) fn build_standard_constructor(
        ctx: &mut EngineContext,
    ) -> JsResult<StandardConstructor> {
        let iterator_proto = ctx.intrinsics().objects().iterator_prototypes().iterator();

        let mut builder =
            boa_engine::object::ConstructorBuilder::new(ctx, jsfn_wrap!(Self::constructor));
        builder.name("WrappedObjectIterator");
        builder.inherit(iterator_proto);
        builder.method(jsfn_wrap!(Self::next), js_string!("next"), 0);

        Ok(builder.build())
    }

    pub fn create_object_iterator(
        obj: ObjectValue,
        kind: JsObjectIteratorKind,
        field_type: ValueType,
        ctx: &mut EngineContext,
    ) -> JsResult<JsValue> {
        assert!(
            field_type.is_primitive(),
            "Only supports primtiive field types"
        );

        let global_obj = ctx.global_object();
        let global_ctx = RuseJsGlobalObject::from_object(&global_obj)?;

        let proto = global_ctx.constructors().iter_prototype(ctx)?;

        let graphs_map = global_ctx.graphs_map()?;

        let fields = obj
            .field_names_iterator(graphs_map)
            .cloned()
            .collect_vec()
            .into_iter();

        let obj_iter = Self {
            obj,
            fields: Box::new(fields),
            kind,
            field_type,
        };

        let mut builder = ObjectInitializer::with_native_data_and_proto(obj_iter, proto, ctx);

        Ok(builder.build().into())
    }

    fn get_field_value(
        &self,
        field_name: &FieldName,
        context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        let global_obj = context.global_object();
        let global_ctx = RuseJsGlobalObject::from_object(&global_obj)?;
        let graphs_map = global_ctx.graphs_map()?;

        if let Some(value) = self.obj.get_field_value(field_name, graphs_map) {
            value.try_into_js(context)
        } else {
            Ok(JsValue::undefined())
        }
    }

    fn get_field(&self, field_name: &str) -> JsResult<JsValue> {
        match self.field_type {
            ValueType::Number => Ok(JsValue::new(
                field_name
                    .parse::<f64>()
                    .map_err(|_e| js_error_unexpected_key_type())?,
            )),
            ValueType::Bool => Ok(JsValue::new(
                field_name
                    .parse::<bool>()
                    .map_err(|_e| js_error_unexpected_key_type())?,
            )),
            ValueType::String => Ok(JsValue::new(js_string!(field_name))),
            _ => unreachable!(),
        }
    }

    fn constructor(
        _this: &JsValue,
        _: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    fn next(this: &JsValue, _: &[JsValue], context: &mut EngineContext) -> JsResult<JsValue> {
        let mut obj_iter = this
            .as_object()
            .and_then(JsObject::downcast_mut::<Self>)
            .ok_or_else(|| js_error_this_is_not_obj_iterator())?;

        if let Some(name) = obj_iter.fields.next() {
            let value: JsValue = match obj_iter.kind {
                JsObjectIteratorKind::Field => obj_iter.get_field(&name)?,
                JsObjectIteratorKind::Value => obj_iter.get_field_value(&name, context)?,
                JsObjectIteratorKind::FieldValue => JsArray::from_iter(
                    [
                        obj_iter.get_field(&name)?,
                        obj_iter.get_field_value(&name, context)?,
                    ],
                    context,
                )
                .into(),
            };
            return Ok(create_iter_result_object(value, false, context));
        } else {
            return Ok(create_iter_result_object(
                JsValue::undefined(),
                true,
                context,
            ));
        }
    }
}

impl boa_engine::gc::Finalize for JsObjectIterator {}
unsafe impl boa_engine::gc::Trace for JsObjectIterator {
    boa_engine::gc::empty_trace!();
}
impl boa_engine::JsData for JsObjectIterator {}
