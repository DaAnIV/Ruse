use html_parser::{self, Dom};
use ruse_object_graph::{
    field_name, fields, root_name, str_cached, FieldsMap, GraphIndex, GraphsMap, NodeIndex,
    ObjectType, PrimitiveValue, RootName,
};
use ruse_synthesizer::context::GraphIdGenerator;

pub struct DomLoader {}

impl DomLoader {
    pub const DOM_ROOT_STR: &'static str = "document";

    pub fn document_root_name() -> RootName {
        root_name!(Self::DOM_ROOT_STR)
    }

    pub fn document_obj_type() -> ObjectType {
        ObjectType::DOM
    }

    pub fn element_obj_type() -> ObjectType {
        ObjectType::DOMElement
    }

    fn add_node(
        id_gen: &GraphIdGenerator,
        graph_id: GraphIndex,
        graphs_map: &mut GraphsMap,
        element: &html_parser::Element,
    ) -> NodeIndex {
        let mut fields = FieldsMap::from([(
            field_name!("name"),
            PrimitiveValue::String(str_cached!(element.name.as_str())).into(),
        )]);
        if let Some(id) = &element.id {
            fields.insert(
                field_name!("id"),
                PrimitiveValue::String(str_cached!(id.as_str())).into(),
            );
        }
        for (attr, val) in &element.attributes {
            match val {
                Some(s) => fields.insert(
                    field_name!(attr.as_str()),
                    PrimitiveValue::String(str_cached!(s.as_str())).into(),
                ),
                None => fields.insert(
                    field_name!(attr.as_str()),
                    PrimitiveValue::Bool(true).into(),
                ),
            };
        }
        if !element.classes.is_empty() {
            let classes = element.classes.join(" ");
            fields.insert(
                field_name!("className"),
                PrimitiveValue::String(str_cached!(classes)).into(),
            );
        }

        let node = graphs_map.add_simple_object(
            graph_id,
            id_gen.get_id_for_node(),
            Self::element_obj_type(),
            fields.into(),
        );

        Self::add_children(id_gen, graph_id, graphs_map, node, &element.children);

        node
    }

    fn add_children(
        id_gen: &GraphIdGenerator,
        graph_id: GraphIndex,
        graphs_map: &mut GraphsMap,
        parent: NodeIndex,
        children: &[html_parser::Node],
    ) {
        for (i, child) in children.iter().enumerate() {
            if let html_parser::Node::Element(element) = child {
                let child_node = Self::add_node(id_gen, graph_id, graphs_map, element);
                graphs_map.set_edge(
                    field_name!(i.to_string()),
                    graph_id,
                    parent,
                    graph_id,
                    child_node,
                );
            }
        }
    }

    pub fn load_dom(
        id_gen: &GraphIdGenerator,
        graph_id: GraphIndex,
        graphs_map: &mut GraphsMap,
        html: &str,
    ) -> Result<NodeIndex, html_parser::Error> {
        let root = graphs_map.add_simple_object(
            graph_id,
            id_gen.get_id_for_node(),
            Self::document_obj_type(),
            fields!(),
        );

        let parsed = Dom::parse(html)?;
        if parsed.tree_type == html_parser::DomVariant::Empty {
            return Err(html_parser::Error::Parsing("html string is empty".into()));
        }
        Self::add_children(id_gen, graph_id, graphs_map, root, &parsed.children);

        Ok(root)
    }

    pub fn load_element(
        id_gen: &GraphIdGenerator,
        graph_id: GraphIndex,
        graphs_map: &mut GraphsMap,
        html: &str,
    ) -> Result<NodeIndex, html_parser::Error> {
        let parsed = Dom::parse(html)?;
        if parsed.children.len() != 1 {
            return Err(html_parser::Error::Parsing(
                "Element html contains more then 1 child in root".into(),
            ));
        }
        if parsed.tree_type != html_parser::DomVariant::DocumentFragment {
            return Err(html_parser::Error::Parsing(
                "Element must be of type document fragment".into(),
            ));
        }
        let root = if let html_parser::Node::Element(element) = &parsed.children[0] {
            Self::add_node(id_gen, graph_id, graphs_map, element)
        } else {
            return Err(html_parser::Error::Parsing("html is not an element".into()));
        };

        Ok(root)
    }
}
