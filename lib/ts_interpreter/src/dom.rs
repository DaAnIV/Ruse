use html_parser::{self, Dom};
use ruse_object_graph::{
    fields, scached, str_cached, Cache, CachedString, FieldsMap, NodeIndex, ObjectData, ObjectGraph,
};
use ruse_synthesizer::value::ObjectValue;

pub struct DomLoader {}

impl DomLoader {
    pub const DOM_ROOT_STR: &'static str = "document";
    pub const DOM_CLASS_STR: &'static str = "Document";
    pub const ELEMENT_CLASS_STR: &'static str = "Element";

    pub fn document_root_name(cache: &Cache) -> CachedString {
        str_cached!(cache; Self::DOM_ROOT_STR)
    }

    pub fn document_obj_type(cache: &Cache) -> CachedString {
        str_cached!(cache; Self::DOM_CLASS_STR)
    }

    pub fn element_obj_type(cache: &Cache) -> CachedString {
        str_cached!(cache; Self::ELEMENT_CLASS_STR)
    }

    fn add_node(
        graph: &mut ObjectGraph,
        element: &html_parser::Element,
        cache: &Cache,
    ) -> NodeIndex {
        let mut fields = FieldsMap::from([(
            str_cached!(cache; "name"),
            str_cached!(cache; &element.name).into(),
        )]);
        if let Some(id) = &element.id {
            fields.insert(str_cached!(cache; "id"), str_cached!(cache; id).into());
        }
        for (attr, val) in &element.attributes {
            match val {
                Some(s) => fields.insert(str_cached!(cache; attr), str_cached!(cache; s).into()),
                None => fields.insert(
                    str_cached!(cache; attr),
                    ruse_object_graph::PrimitiveValue::Bool(true),
                ),
            };
        }
        if !element.classes.is_empty() {
            let classes = element.classes.join(" ");
            fields.insert(
                str_cached!(cache; "className"),
                scached!(cache; classes).into(),
            );
        }

        let node = graph.add_node(ObjectData::new(
            Self::element_obj_type(cache),
            fields.into(),
        ));

        Self::add_children(graph, &node, &element.children, cache);

        node
    }

    fn add_children(
        graph: &mut ObjectGraph,
        parent: &NodeIndex,
        children: &Vec<html_parser::Node>,
        cache: &Cache,
    ) {
        for (i, child) in children.iter().enumerate() {
            if let html_parser::Node::Element(element) = child {
                let child_node = Self::add_node(graph, element, cache);
                graph.add_edge(parent.clone(), child_node, &scached!(cache; i.to_string()));
            }
        }
    }

    pub fn load_dom(html: &str, cache: &Cache) -> Result<ObjectValue, html_parser::Error> {
        let mut graph = ObjectGraph::new();
        let root = graph.add_root(
            Self::document_root_name(cache),
            ObjectData::new(Self::document_obj_type(cache), fields!()),
        );

        let parsed = Dom::parse(html)?;
        if parsed.tree_type == html_parser::DomVariant::Empty {
            return Err(html_parser::Error::Parsing("html string is empty".into()));
        }
        Self::add_children(&mut graph, &root, &parsed.children, cache);

        Ok(ObjectValue {
            graph: graph.into(),
            node: root,
        })
    }

    pub fn load_element(html: &str, cache: &Cache) -> Result<ObjectValue, html_parser::Error> {
        let mut graph = ObjectGraph::new();
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
            Self::add_node(&mut graph, element, cache)
        } else {
            return Err(html_parser::Error::Parsing("html is not an element".into()));
        };

        Ok(ObjectValue {
            graph: graph.into(),
            node: root,
        })
    }
}
