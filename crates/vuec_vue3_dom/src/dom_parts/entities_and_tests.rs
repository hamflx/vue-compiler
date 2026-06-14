fn decode_basic_entities(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use vuec_source::FileId;

    fn template_source(name: &str, source: &str) -> TemplateSource {
        TemplateSource {
            filename: name.into(),
            source: source.into(),
            file_id: FileId(0),
            base_offset: 0,
        }
    }

    fn model_dir() -> Value {
        json!({
            "name": "model",
            "exp": {
                "type": 4,
                "content": "model",
                "loc": { "source": "model" }
            },
            "modifiers": [],
            "loc": { "source": "v-model=\"model\"" }
        })
    }

    fn model_node(tag: &str, props: Vec<Value>) -> Value {
        json!({
            "type": 1,
            "tag": tag,
            "tagType": 0,
            "props": props,
        })
    }

    fn model_projection(tag: &str, props: Vec<Value>) -> Value {
        transform_model_projection(&json!({
            "dir": model_dir(),
            "node": model_node(tag, props),
            "context": {},
        }))
    }

    fn transition_element_child(props: Vec<Value>) -> Value {
        json!({
            "type": 1,
            "tag": "div",
            "tagType": 0,
            "props": props,
            "children": [],
            "loc": {
                "start": { "line": 1, "column": 13, "offset": 12 },
                "end": { "line": 1, "column": 24, "offset": 23 },
                "source": "<div></div>"
            }
        })
    }

    fn transition_projection(children: Vec<Value>) -> Value {
        transform_transition_projection(&json!({
            "node": {
                "type": 1,
                "tag": "transition",
                "tagType": 1,
                "children": children,
            },
            "context": { "isTransition": true },
        }))
    }

    include!("entities_and_tests_parts/side_effect_entities_cache.rs");
    include!("entities_and_tests_parts/model_projection.rs");
    include!("entities_and_tests_parts/transition_nesting_parse.rs");
    include!("entities_and_tests_parts/asset_url_compile.rs");
    include!("entities_and_tests_parts/directive_projection.rs");
}
