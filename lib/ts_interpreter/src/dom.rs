use html_parser::{self, Dom};
use ruse_object_graph::{
    fields, scached, str_cached, Cache, CachedString, FieldsMap, GraphIndex, GraphsMap, NodeIndex,
    ObjectType, PrimitiveValue,
};
use ruse_synthesizer::context::GraphIdGenerator;

pub struct DomLoader {}

impl DomLoader {
    pub const DOM_ROOT_STR: &'static str = "document";

    pub fn document_root_name(cache: &Cache) -> CachedString {
        str_cached!(cache; Self::DOM_ROOT_STR)
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
        cache: &Cache,
    ) -> NodeIndex {
        let mut fields = FieldsMap::from([(
            str_cached!(cache; "name"),
            PrimitiveValue::String(str_cached!(cache; &element.name)).into(),
        )]);
        if let Some(id) = &element.id {
            fields.insert(
                str_cached!(cache; "id"),
                PrimitiveValue::String(str_cached!(cache; id)).into(),
            );
        }
        for (attr, val) in &element.attributes {
            match val {
                Some(s) => fields.insert(
                    str_cached!(cache; attr),
                    PrimitiveValue::String(str_cached!(cache; s)).into(),
                ),
                None => fields.insert(str_cached!(cache; attr), PrimitiveValue::Bool(true).into()),
            };
        }
        if !element.classes.is_empty() {
            let classes = element.classes.join(" ");
            fields.insert(
                str_cached!(cache; "className"),
                PrimitiveValue::String(scached!(cache; classes)).into(),
            );
        }

        let node = graphs_map.add_simple_object(
            graph_id,
            id_gen.get_id_for_node(),
            Self::element_obj_type(),
            fields.into(),
        );

        Self::add_children(id_gen, graph_id, graphs_map, node, &element.children, cache);

        node
    }

    fn add_children(
        id_gen: &GraphIdGenerator,
        graph_id: GraphIndex,
        graphs_map: &mut GraphsMap,
        parent: NodeIndex,
        children: &[html_parser::Node],
        cache: &Cache,
    ) {
        for (i, child) in children.iter().enumerate() {
            if let html_parser::Node::Element(element) = child {
                let child_node = Self::add_node(id_gen, graph_id, graphs_map, element, cache);
                graphs_map.set_edge(
                    scached!(cache; i.to_string()),
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
        cache: &Cache,
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
        Self::add_children(id_gen, graph_id, graphs_map, root, &parsed.children, cache);

        Ok(root)
    }

    pub fn load_element(
        id_gen: &GraphIdGenerator,
        graph_id: GraphIndex,
        graphs_map: &mut GraphsMap,
        html: &str,
        cache: &Cache,
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
            Self::add_node(id_gen, graph_id, graphs_map, element, cache)
        } else {
            return Err(html_parser::Error::Parsing("html is not an element".into()));
        };

        Ok(root)
    }
}
