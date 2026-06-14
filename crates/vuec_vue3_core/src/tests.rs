#[cfg(test)]
mod tests {
    use crate::*;
    use vuec_ast::PublicProjection;

    #[test]
    fn parse_transform_generate_roundtrip() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div>hello</div>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(source, Vue3CompilerOptions::default());
        assert!(result.code.contains("render"));
        assert!(result.ast_summary.contains("nodes="));
    }

    #[test]
    fn ssr_wraps_code() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div/>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = compile_ssr(source, Vue3CompilerOptions::default());
        assert!(result.code.starts_with("/* ssr */"));
    }

    #[test]
    fn generate_public_ast_emits_root_imports() {
        let ast = json!({
            "type": 0,
            "helpers": ["openBlock", "createElementBlock"],
            "components": [],
            "directives": [],
            "hoists": [],
            "imports": [{
                "exp": {
                    "type": 4,
                    "content": "_imports_0",
                    "isStatic": false,
                    "constType": 3
                },
                "path": "./logo.png"
            }],
            "cached": [],
            "temps": 0,
            "codegenNode": {
                "type": 13,
                "tag": "\"img\"",
                "props": {
                    "type": 15,
                    "properties": [{
                        "type": 16,
                        "key": {
                            "type": 4,
                            "content": "src",
                            "isStatic": true
                        },
                        "value": {
                            "type": 4,
                            "content": "_imports_0",
                            "isStatic": false
                        }
                    }]
                },
                "children": null,
                "isBlock": true,
                "isComponent": false
            }
        });

        let result = generate_public_ast(
            &ast,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("import _imports_0 from './logo.png'"));
        assert!(result
            .code
            .contains("import _imports_0 from './logo.png'\n\n\nexport function render"));
        assert!(result.code.contains("src: _imports_0"));
        assert!(!result.code.contains("_ctx._imports_0"));
    }

    #[test]
    fn generate_public_ast_separates_root_imports_from_hoists() {
        let ast = json!({
            "type": 0,
            "helpers": ["openBlock", "createElementBlock", "createElementVNode", "Fragment"],
            "components": [],
            "directives": [],
            "hoists": [{
                "type": 4,
                "content": "_imports_0 + '#fragment'",
                "isStatic": false,
                "constType": 3
            }],
            "imports": [{
                "exp": {
                    "type": 4,
                    "content": "_imports_0",
                    "isStatic": false,
                    "constType": 3
                },
                "path": "./icons.svg"
            }],
            "cached": [],
            "temps": 0,
            "codegenNode": {
                "type": 13,
                "tag": "_Fragment",
                "props": null,
                "children": [{
                    "type": 13,
                    "tag": "\"use\"",
                    "props": {
                        "type": 15,
                        "properties": [{
                            "type": 16,
                            "key": {
                                "type": 4,
                                "content": "href",
                                "isStatic": true
                            },
                            "value": {
                                "type": 4,
                                "content": "_hoisted_1",
                                "isStatic": false
                            }
                        }]
                    },
                    "children": null,
                    "isBlock": false,
                    "isComponent": false
                }],
                "patchFlag": "64",
                "isBlock": true,
                "isComponent": false
            }
        });

        let result = generate_public_ast(
            &ast,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains(
            "import _imports_0 from './icons.svg'\n\n\nconst _hoisted_1 = _imports_0 + '#fragment'\n\nexport function render"
        ));
    }

    #[test]
    fn template_base_offset_maps_nodes_to_original_file_spans() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div>{{ msg }}</div>".into(),
            file_id: FileId(7),
            base_offset: 42,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        assert_eq!(root.span.source(), Some(Span::new(FileId(7), 42, 62)));
        let element = ast.node(root.children[0]).expect("element");
        assert_eq!(element.span.source(), Some(Span::new(FileId(7), 42, 62)));
        let interpolation = ast.node(element.children[0]).expect("interpolation child");
        assert_eq!(
            interpolation.span.source(),
            Some(Span::new(FileId(7), 47, 56))
        );
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_records_hir_mir_edges_and_js_store() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div id=\"app\" :class=\"foo\" @click=\"go\">{{ msg }}</div>".into(),
            file_id: FileId(9),
            base_offset: 11,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        assert!(result
            .map
            .hir_for_ast(ast.root)
            .any(|hir| hir == result.hir.root));
        assert!(result
            .map
            .mir_for_hir(result.hir.root)
            .any(|mir| mir == result.mir.root));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["foo", "msg"]
        );
        assert_eq!(
            result
                .js
                .statements()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["go"]
        );

        let div_hir = result
            .hir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                HirNodeKind::Element(element) => Some(element),
                _ => None,
            })
            .expect("HIR element");
        assert_eq!(div_hir.props.static_attrs[0].name, "id");
        assert_eq!(div_hir.props.dynamic_bindings[0].name, "class");
        assert_eq!(div_hir.props.events[0].name, "click");

        let div_mir = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call) => Some(call),
                _ => None,
            })
            .expect("DOM MIR vnode");
        assert_eq!(div_mir.tag, Vue3DomTag::Native("div".into()));
        assert_eq!(div_mir.props.static_attrs[0].name, "id");
        assert_eq!(div_mir.props.dynamic_bindings[0].name, "class");
        assert_eq!(div_mir.props.events[0].name, "onClick");
        assert_eq!(div_mir.dynamic_props, vec!["class", "onClick"]);
        assert_eq!(div_mir.patch_flag.bits, 11);
        assert!(matches!(div_mir.children, MirChildren::Nodes(_)));
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_cache_handlers_event_slots() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><button @click="go" @[event]="run" /><Child @save="save" /><p v-once @click="once">once</p></div>"#.into(),
            file_id: FileId(32),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(
            &ast,
            &Vue3CompilerOptions {
                cache_handlers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        let mut cached = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call) => Some((
                    call.tag.clone(),
                    call.props
                        .events
                        .iter()
                        .filter_map(|event| event.cache.as_ref().map(|cache| cache.index))
                        .collect::<Vec<_>>(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        cached.retain(|(_, slots)| !slots.is_empty());

        assert_eq!(cached, vec![(Vue3DomTag::Native("button".into()), vec![0])]);
        assert!(result
            .mir
            .nodes
            .iter()
            .any(|node| matches!(node.kind, Vue3DomMirKind::Cache { index: 1 })));
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_v_on_modifier_payloads() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><button @click.stop.capture.once="go"/><input @keyup.enter.prevent="submit"/><button @click.right="menu"/><button @mouseup.right="mouseup"/><button @[event].left="dynamic"/><button @[name].right="dynamicRight"/></div>"#.into(),
            file_id: FileId(64),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let events = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call) => call.props.events.first(),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 6);

        assert_eq!(events[0].name, "onClick");
        assert_eq!(events[0].runtime_modifiers, vec!["stop"]);
        assert_eq!(events[0].key_modifiers, Vec::<String>::new());
        assert_eq!(events[0].option_modifiers, vec!["capture", "once"]);
        assert_eq!(events[0].click_event, None);

        assert_eq!(events[1].name, "onKeyup");
        assert_eq!(events[1].runtime_modifiers, vec!["prevent"]);
        assert_eq!(events[1].key_modifiers, vec!["enter"]);
        assert_eq!(events[1].option_modifiers, Vec::<String>::new());

        assert_eq!(events[2].name, "onContextmenu");
        assert_eq!(events[2].runtime_modifiers, vec!["right"]);
        assert_eq!(events[2].click_event, Some(Vue3DomClickEvent::ContextMenu));

        assert_eq!(events[3].name, "onMouseup");
        assert_eq!(events[3].runtime_modifiers, vec!["right"]);
        assert_eq!(events[3].key_modifiers, Vec::<String>::new());
        assert_eq!(events[3].click_event, None);

        assert_eq!(events[4].name, "event");
        assert!(events[4].dynamic_arg);
        assert_eq!(events[4].runtime_modifiers, vec!["left"]);
        assert_eq!(events[4].key_modifiers, vec!["left"]);

        assert_eq!(events[5].name, "name");
        assert!(events[5].dynamic_arg);
        assert_eq!(events[5].runtime_modifiers, vec!["right"]);
        assert_eq!(events[5].key_modifiers, vec!["right"]);
        assert_eq!(events[5].click_event, Some(Vue3DomClickEvent::ContextMenu));
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_v_bind_modifier_payloads() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div :foo-bar.camel="foo" :fooBar.prop="bar" :foo-bar.attr="baz" :[name].camel="value" :[propName].prop="propValue"/>"#.into(),
            file_id: FileId(67),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let hir = result
            .hir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                HirNodeKind::Element(element) => Some(element),
                _ => None,
            })
            .expect("HIR element");
        assert_eq!(hir.props.dynamic_bindings[0].modifiers, vec!["camel"]);
        assert_eq!(hir.props.dynamic_bindings[1].modifiers, vec!["prop"]);
        assert_eq!(hir.props.dynamic_bindings[2].modifiers, vec!["attr"]);

        let call = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call) => Some(call),
                _ => None,
            })
            .expect("DOM MIR vnode");
        assert_eq!(call.props.dynamic_bindings.len(), 5);
        assert!(call.props.dynamic_bindings[0].camel);
        assert!(call.props.dynamic_bindings[1].force_prop);
        assert!(call.props.dynamic_bindings[2].force_attr);
        assert!(call.props.dynamic_bindings[3].dynamic_arg);
        assert!(call.props.dynamic_bindings[3].camel);
        assert!(call.props.dynamic_bindings[4].dynamic_arg);
        assert!(call.props.dynamic_bindings[4].force_prop);
        assert_eq!(call.patch_flag.bits, 16);
        assert!(call.dynamic_props.is_empty());
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_keeps_ordered_prop_segments_and_object_spreads() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<button id="save" v-bind="base" :[name]="value" v-on="listeners" @[event]="run">Save</button>"#.into(),
            file_id: FileId(30),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["base", "name", "value", "listeners", "event"]
        );
        assert_eq!(
            result
                .js
                .statements()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["run"]
        );

        let button_hir = result
            .hir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                HirNodeKind::Element(element) => Some(element),
                _ => None,
            })
            .expect("HIR element");
        assert!(matches!(
            button_hir.props.segments.as_slice(),
            [
                HirPropSegment::StaticAttr(_),
                HirPropSegment::ObjectBinding(_),
                HirPropSegment::DynamicBinding(_),
                HirPropSegment::ObjectListeners(_),
                HirPropSegment::Event(_)
            ]
        ));
        assert_eq!(button_hir.props.object_bindings.len(), 1);
        assert_eq!(button_hir.props.object_listeners.len(), 1);
        assert!(button_hir.props.dynamic_bindings[0].dynamic_arg);
        assert!(button_hir.props.dynamic_bindings[0].dynamic_name.is_some());
        assert!(button_hir.props.events[0].dynamic_arg);
        assert!(button_hir.props.events[0].dynamic_name.is_some());

        let button_mir = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call) => Some(call),
                _ => None,
            })
            .expect("DOM MIR vnode");
        assert!(matches!(
            button_mir.props.segments.as_slice(),
            [
                Vue3DomPropSegment::StaticAttr(_),
                Vue3DomPropSegment::ObjectBinding(_),
                Vue3DomPropSegment::DynamicBinding(_),
                Vue3DomPropSegment::ObjectListeners(_),
                Vue3DomPropSegment::Event(_)
            ]
        ));
        assert!(button_mir.props.normalize.normalize_props);
        assert!(button_mir.props.normalize.guard_reactive_props);
        assert_eq!(button_mir.patch_flag.bits, 16);
        assert!(button_mir.dynamic_props.is_empty());
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_content_override_payloads() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><section v-html="raw">old</section><p v-text="msg"/><span>keep</span></div>"#.into(),
            file_id: FileId(58),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["raw", "msg"]
        );
        let content_calls = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call) if call.content.is_some() => Some(call),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(content_calls.len(), 2);
        assert!(matches!(
            content_calls[0].content,
            Some(Vue3DomContent::Html {
                expression: Some(JsExprId(0))
            })
        ));
        assert!(matches!(
            content_calls[0].props.segments.as_slice(),
            [Vue3DomPropSegment::Content(Vue3DomContent::Html { .. })]
        ));
        assert_eq!(content_calls[0].children, MirChildren::None);
        assert_eq!(content_calls[0].patch_flag.bits, 8);
        assert_eq!(content_calls[0].dynamic_props, vec!["innerHTML"]);
        assert!(matches!(
            content_calls[1].content,
            Some(Vue3DomContent::Text {
                expression: Some(JsExprId(1))
            })
        ));
        assert_eq!(content_calls[1].children, MirChildren::None);
        assert_eq!(content_calls[1].patch_flag.bits, 8);
        assert_eq!(content_calls[1].dynamic_props, vec!["textContent"]);
        let texts = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3DomMirKind::TextCall {
                    value: MirExpr::String(value),
                } => Some(value.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(!texts.contains(&"old"));
        assert!(texts.contains(&"keep"));
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_v_show_payload() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><span v-show="ok"/><p v-focus v-show="visible"/></div>"#.into(),
            file_id: FileId(68),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["ok", "visible"]
        );
        let calls = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call) => Some(call),
                _ => None,
            })
            .collect::<Vec<_>>();
        let span = calls
            .iter()
            .find(|call| call.tag == Vue3DomTag::Native("span".into()))
            .expect("span call");
        assert_eq!(span.v_show, Some(JsExprId(0)));
        assert!(span.directives.is_empty());
        assert_eq!(span.patch_flag.bits, 512);

        let paragraph = calls
            .iter()
            .find(|call| call.tag == Vue3DomTag::Native("p".into()))
            .expect("paragraph call");
        assert_eq!(paragraph.v_show, Some(JsExprId(1)));
        assert_eq!(
            paragraph
                .directives
                .iter()
                .map(|directive| directive.name.as_str())
                .collect::<Vec<_>>(),
            vec!["focus"]
        );
        assert_eq!(paragraph.patch_flag.bits, 512);
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_native_v_model_payloads() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><input v-model="text"><input type="radio" v-model="picked"><input type="checkbox" v-model.trim="checked"><input :type="kind" v-model="dynamic"><select v-model="selected"/><textarea v-model.lazy="body">old</textarea></div>"#.into(),
            file_id: FileId(61),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["text", "picked", "checked", "kind", "dynamic", "selected", "body"]
        );
        let calls = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call) if !call.models.is_empty() => Some(call),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 6);
        assert_eq!(
            calls
                .iter()
                .map(|call| call.models[0].kind)
                .collect::<Vec<_>>(),
            vec![
                Vue3DomModelKind::Text,
                Vue3DomModelKind::Radio,
                Vue3DomModelKind::Checkbox,
                Vue3DomModelKind::Dynamic,
                Vue3DomModelKind::Select,
                Vue3DomModelKind::Text,
            ]
        );
        assert_eq!(calls[2].models[0].modifiers, vec!["trim"]);
        assert_eq!(calls[5].models[0].modifiers, vec!["lazy"]);
        assert!(matches!(
            calls[0].props.segments.as_slice(),
            [Vue3DomPropSegment::Model(Vue3DomModel {
                kind: Vue3DomModelKind::Text,
                ..
            })]
        ));
        assert_eq!(calls[0].patch_flag.bits, 8);
        assert_eq!(calls[0].dynamic_props, vec!["onUpdate:modelValue"]);
        assert_eq!(calls[3].patch_flag.bits, 8);
        assert_eq!(calls[3].dynamic_props, vec!["type", "onUpdate:modelValue"]);
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_component_tags_structurally() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Child/><Transition/><component :is="view"/>"#.into(),
            file_id: FileId(32),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["view"]
        );
        let tags = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call) => Some(call.tag.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(tags.contains(&Vue3DomTag::ComponentAsset("Child".into())));
        assert!(tags.contains(&Vue3DomTag::RuntimeHelper(RuntimeHelper::Vue3Transition)));
        assert!(tags.contains(&Vue3DomTag::DynamicComponent(JsExprId(0))));

        let dynamic_component = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call)
                    if matches!(call.tag, Vue3DomTag::DynamicComponent(_)) =>
                {
                    Some(call)
                }
                _ => None,
            })
            .expect("dynamic component");
        assert_eq!(dynamic_component.props.dynamic_bindings[0].name, "is");
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_transition_persisted_prop() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Transition v-bind="base"><div v-show="ok"/></Transition><Transition><div/></Transition>"#.into(),
            file_id: FileId(70),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let transitions = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call)
                    if call.tag == Vue3DomTag::RuntimeHelper(RuntimeHelper::Vue3Transition) =>
                {
                    Some(call)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(transitions.len(), 2);
        assert!(matches!(
            transitions[0].props.segments.as_slice(),
            [
                Vue3DomPropSegment::ObjectBinding(_),
                Vue3DomPropSegment::StaticAttr(Vue3DomStaticAttr { name, value })
            ] if name == "persisted" && value.is_empty()
        ));
        assert!(transitions[0]
            .props
            .static_attrs
            .iter()
            .any(|attr| attr.name == "persisted" && attr.value.is_empty()));
        assert!(transitions[1]
            .props
            .static_attrs
            .iter()
            .all(|attr| attr.name != "persisted"));
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_lowercase_transition_persisted_prop() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<transition><div v-show="ok"/></transition>"#.into(),
            file_id: FileId(71),
            base_offset: 0,
        };
        let mut options = Vue3CompilerOptions::default();
        options.built_in_components = vec!["transition".into()];
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_dom_mir(&ast, &options);

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let transition = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call)
                    if call.tag == Vue3DomTag::RuntimeHelper(RuntimeHelper::Vue3Transition) =>
                {
                    Some(call)
                }
                _ => None,
            })
            .expect("transition");
        assert!(transition
            .props
            .static_attrs
            .iter()
            .any(|attr| attr.name == "persisted" && attr.value.is_empty()));
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_keeps_transition_persisted_override_order() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Transition :persisted="user" v-bind="base"><div v-show="ok"/></Transition><Transition v-bind="base" :persisted="user"><div v-show="ok"/></Transition>"#.into(),
            file_id: FileId(72),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let transitions = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call)
                    if call.tag == Vue3DomTag::RuntimeHelper(RuntimeHelper::Vue3Transition) =>
                {
                    Some(call)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(transitions.len(), 2);
        assert!(matches!(
            transitions[0].props.segments.as_slice(),
            [
                Vue3DomPropSegment::DynamicBinding(Vue3DomBinding {
                    name,
                    dynamic_arg: false,
                    ..
                }),
                Vue3DomPropSegment::ObjectBinding(_),
                Vue3DomPropSegment::StaticAttr(Vue3DomStaticAttr {
                    name: injected,
                    value
                })
            ] if name == "persisted" && injected == "persisted" && value.is_empty()
        ));
        assert!(matches!(
            transitions[1].props.segments.as_slice(),
            [
                Vue3DomPropSegment::ObjectBinding(_),
                Vue3DomPropSegment::DynamicBinding(Vue3DomBinding {
                    name,
                    dynamic_arg: false,
                    ..
                })
            ] if name == "persisted"
        ));
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_stable_component_slots() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Comp><template #header="{ item }"><span>{{ item.name }}</span></template><p>{{ msg }}</p></Comp>"#.into(),
            file_id: FileId(34),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(
            result
                .js
                .patterns()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["{ item }"]
        );
        let component = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call)
                    if call.tag == Vue3DomTag::ComponentAsset("Comp".into()) =>
                {
                    Some(call)
                }
                _ => None,
            })
            .expect("component");
        let MirChildren::Slots(slots) = &component.children else {
            panic!("component slots");
        };
        assert_eq!(slots.flag, Vue3SlotFlag::Stable);
        assert!(slots.dynamic_slots.is_empty());
        assert_eq!(slots.slots.len(), 2);
        assert_eq!(slots.slots[0].name, "header");
        assert_eq!(slots.slots[0].params, Some(JsPatternId(0)));
        assert_eq!(slots.slots[1].name, "default");
        assert!(slots.slots.iter().all(|slot| !slot.children.is_empty()));
        assert!(result
            .hir
            .nodes
            .iter()
            .any(|node| matches!(&node.kind, HirNodeKind::SlotDecl(slot) if slot.name == "header" && slot.params == Some(JsPatternId(0)))));
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_on_component_stable_slot_params() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Comp v-slot="{ item }"><span>{{ item.name }}</span></Comp>"#.into(),
            file_id: FileId(35),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(
            result
                .js
                .patterns()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["{ item }"]
        );
        let component = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call)
                    if call.tag == Vue3DomTag::ComponentAsset("Comp".into()) =>
                {
                    Some(call)
                }
                _ => None,
            })
            .expect("component");
        let MirChildren::Slots(slots) = &component.children else {
            panic!("component slots");
        };
        assert_eq!(slots.flag, Vue3SlotFlag::Stable);
        assert!(slots.dynamic_slots.is_empty());
        assert_eq!(slots.slots.len(), 1);
        assert_eq!(slots.slots[0].name, "default");
        assert_eq!(slots.slots[0].params, Some(JsPatternId(0)));
        assert!(!slots.slots[0].children.is_empty());
        assert!(result
            .hir
            .nodes
            .iter()
            .any(|node| matches!(&node.kind, HirNodeKind::SlotDecl(slot) if slot.name == "default" && slot.params == Some(JsPatternId(0)))));
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_forwarded_component_slot_flag() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Comp><template #default><slot /></template><template #footer><span>footer</span></template></Comp>"#.into(),
            file_id: FileId(36),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        let component = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call)
                    if call.tag == Vue3DomTag::ComponentAsset("Comp".into()) =>
                {
                    Some(call)
                }
                _ => None,
            })
            .expect("component");
        let MirChildren::Slots(slots) = &component.children else {
            panic!("component slots");
        };
        assert_eq!(slots.flag, Vue3SlotFlag::Forwarded);
        assert!(slots.dynamic_slots.is_empty());
        assert_eq!(slots.slots.len(), 2);
        assert!(slots
            .slots
            .iter()
            .any(|slot| slot.name == "default" && !slot.children.is_empty()));
        assert!(result
            .mir
            .nodes
            .iter()
            .any(|node| matches!(node.kind, Vue3DomMirKind::RenderSlot(_))));
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_dynamic_component_slots() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Comp><template #[name]="slotProps"><span>{{ slotProps.label }}</span></template><template #fallback v-if="ok">Fallback</template><template #item v-for="(item, index) in list"><span>{{ item }}</span></template></Comp>"#.into(),
            file_id: FileId(36),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        let component = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call)
                    if call.tag == Vue3DomTag::ComponentAsset("Comp".into()) =>
                {
                    Some(call)
                }
                _ => None,
            })
            .expect("component");
        assert_eq!(component.patch_flag.bits & 1024, 1024);
        let MirChildren::Slots(slots) = &component.children else {
            panic!("component slots");
        };
        assert_eq!(slots.flag, Vue3SlotFlag::Dynamic);
        assert!(slots.slots.is_empty());
        assert_eq!(slots.dynamic_slots.len(), 3);
        assert!(matches!(
            &slots.dynamic_slots[0],
            vuec_ast::Vue3DomDynamicSlot::Slot(slot)
                if matches!(slot.name, Vue3DomSlotName::Dynamic(_))
                    && slot.params == Some(JsPatternId(0))
        ));
        assert!(matches!(
            &slots.dynamic_slots[1],
            vuec_ast::Vue3DomDynamicSlot::Conditional(slot)
                if slot.condition.is_some()
                    && matches!(slot.slot.name, Vue3DomSlotName::Static(ref name) if name == "fallback")
                    && slot.slot.key.as_deref() == Some("1")
                    && slot.alternate.is_none()
        ));
        assert!(matches!(
            &slots.dynamic_slots[2],
            vuec_ast::Vue3DomDynamicSlot::For(slot)
                if slot.value_alias == JsPatternId(1)
                    && slot.key_alias == Some(JsPatternId(2))
                    && slot.index_alias.is_none()
                    && matches!(slot.slot.name, Vue3DomSlotName::Static(ref name) if name == "item")
        ));
        assert!(result
            .js
            .expressions()
            .iter()
            .any(|entry| entry.source == "list"));
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_chains_dynamic_slot_if_else_branches() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Comp><template #one v-if="ok">One</template><template #two v-else-if="maybe">Two</template><template #fallback v-else>Fallback</template></Comp>"#.into(),
            file_id: FileId(39),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        let component = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call)
                    if call.tag == Vue3DomTag::ComponentAsset("Comp".into()) =>
                {
                    Some(call)
                }
                _ => None,
            })
            .expect("component");
        let MirChildren::Slots(slots) = &component.children else {
            panic!("component slots");
        };
        assert_eq!(slots.flag, Vue3SlotFlag::Dynamic);
        assert_eq!(slots.dynamic_slots.len(), 1);
        let vuec_ast::Vue3DomDynamicSlot::Conditional(first) = &slots.dynamic_slots[0] else {
            panic!("first conditional slot");
        };
        assert!(first.condition.is_some());
        assert!(matches!(first.slot.name, Vue3DomSlotName::Static(ref name) if name == "one"));
        assert_eq!(first.slot.key.as_deref(), Some("0"));
        let Some(second) = first.alternate.as_deref() else {
            panic!("else-if alternate");
        };
        let vuec_ast::Vue3DomDynamicSlot::Conditional(second) = second else {
            panic!("second conditional slot");
        };
        assert!(second.condition.is_some());
        assert!(matches!(second.slot.name, Vue3DomSlotName::Static(ref name) if name == "two"));
        assert_eq!(second.slot.key.as_deref(), Some("1"));
        let Some(third) = second.alternate.as_deref() else {
            panic!("else alternate");
        };
        let vuec_ast::Vue3DomDynamicSlot::Slot(third) = third else {
            panic!("else slot");
        };
        assert!(matches!(third.name, Vue3DomSlotName::Static(ref name) if name == "fallback"));
        assert_eq!(third.key.as_deref(), Some("2"));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["ok", "maybe"]
        );
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_keeps_slot_outlet_target_split() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<slot name=\"header\">fallback</slot>".into(),
            file_id: FileId(10),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = Vue3Dialect::lower_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        assert!(result
            .hir
            .nodes
            .iter()
            .any(|node| matches!(node.kind, HirNodeKind::SlotOutlet(_))));
        assert!(result
            .mir
            .nodes
            .iter()
            .any(|node| matches!(node.kind, Vue3DomMirKind::RenderSlot(_))));
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_slot_outlet_payload() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<slot :name="active" foo="bar" :baz="baz">fallback {{ msg }}</slot>"#.into(),
            file_id: FileId(41),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = Vue3Dialect::lower_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        let slot = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3DomMirKind::RenderSlot(slot) => Some(slot),
                _ => None,
            })
            .expect("render slot");
        assert!(matches!(slot.name, Vue3DomSlotName::Dynamic(JsExprId(0))));
        assert_eq!(slot.props.static_attrs.len(), 1);
        assert_eq!(slot.props.static_attrs[0].name, "foo");
        assert_eq!(slot.props.static_attrs[0].value, "bar");
        assert_eq!(slot.props.dynamic_bindings.len(), 1);
        assert_eq!(slot.props.dynamic_bindings[0].name, "baz");
        assert_eq!(slot.props.dynamic_bindings[0].value, JsExprId(1));
        assert_eq!(slot.fallback.len(), 2);
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["active", "baz", "msg"]
        );
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_lowers_v_for_and_v_if_control_flow() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<li v-for="(item, key, index) in list" v-if="item.ok" :key="item.id">{{ item.name }}</li>"#.into(),
            file_id: FileId(13),
            base_offset: 3,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["list", "item.ok", "item.id", "item.name"]
        );
        assert_eq!(
            result
                .js
                .patterns()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["item", "key", "index"]
        );

        let for_node = result
            .hir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                HirNodeKind::For(hir_for) => Some((node.id, hir_for)),
                _ => None,
            })
            .expect("HIR for");
        assert!(result
            .hir
            .node(for_node.1.body)
            .is_some_and(|node| matches!(node.kind, HirNodeKind::If(_))));

        let if_hir = result
            .hir
            .node(for_node.1.body)
            .and_then(|node| match &node.kind {
                HirNodeKind::If(hir_if) => Some(hir_if),
                _ => None,
            })
            .expect("HIR if");
        assert_eq!(if_hir.branches.len(), 1);
        assert!(result
            .hir
            .node(if_hir.branches[0].body)
            .is_some_and(|node| matches!(node.kind, HirNodeKind::Element(_))));

        assert!(result
            .mir
            .nodes
            .iter()
            .any(|node| matches!(node.kind, Vue3DomMirKind::For(_))));
        assert!(result
            .mir
            .nodes
            .iter()
            .any(|node| matches!(node.kind, Vue3DomMirKind::If { condition: Some(_) })));
        assert!(result
            .map
            .hir_to_mir
            .iter()
            .any(|(hir, _)| *hir == for_node.0));

        let li_mir = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call) if call.tag == Vue3DomTag::Native("li".into()) => {
                    Some(call)
                }
                _ => None,
            })
            .expect("li vnode");
        assert_eq!(li_mir.patch_flag.bits, 1);
        assert!(li_mir.dynamic_props.is_empty());
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_v_for_memo_cache_target() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-for="{ x, y } in list" :key="x" v-memo="[x, y === z]"><span>foobar</span></div>"#.into(),
            file_id: FileId(14),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["list", "x", "[x, y === z]"]
        );
        assert_eq!(
            result
                .js
                .patterns()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["{ x, y }"]
        );

        let for_mir = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3DomMirKind::For(for_mir) => Some(for_mir),
                _ => None,
            })
            .expect("DOM MIR for");
        assert_eq!(for_mir.source, JsExprId(0));
        assert_eq!(for_mir.value_alias, JsPatternId(0));
        assert!(for_mir.key_alias.is_none());
        assert!(for_mir.index_alias.is_none());
        assert_eq!(for_mir.key, Some(MirExpr::JsExpr(JsExprId(1))));
        assert_eq!(
            for_mir.memo,
            Some(Vue3ForMemo {
                expression: JsExprId(2),
                index: 0,
            })
        );
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_style_and_props_patch_flags() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div :style="style" :title="title" @input="onInput"></div>"#.into(),
            file_id: FileId(15),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        let div_mir = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call) => Some(call),
                _ => None,
            })
            .expect("DOM MIR vnode");
        assert_eq!(div_mir.patch_flag.bits, 44);
        assert_eq!(div_mir.dynamic_props, vec!["style", "title", "onInput"]);
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_groups_if_else_branch_chains() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<p v-if="ok">yes</p><p v-else-if="maybe">maybe</p><p v-else>no</p>"#.into(),
            file_id: FileId(16),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        assert_eq!(
            result
                .hir
                .nodes
                .iter()
                .filter(|node| matches!(node.kind, HirNodeKind::If(_)))
                .count(),
            1
        );
        let hir_if = result
            .hir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                HirNodeKind::If(hir_if) => Some(hir_if),
                _ => None,
            })
            .expect("HIR if");
        assert_eq!(hir_if.branches.len(), 3);
        assert!(hir_if.branches[0].condition.is_some());
        assert!(hir_if.branches[1].condition.is_some());
        assert!(hir_if.branches[2].condition.is_none());
        assert_eq!(
            result
                .mir
                .nodes
                .iter()
                .filter(|node| matches!(node.kind, Vue3DomMirKind::If { .. }))
                .count(),
            3
        );
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["ok", "maybe"]
        );
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_static_hoist_wrappers() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><span id="one">one</span><span :id="two">two</span></div>"#.into(),
            file_id: FileId(18),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(
            &ast,
            &Vue3CompilerOptions {
                hoist_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let hoists = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match node.kind {
                Vue3DomMirKind::Hoisted { index } => Some((node.id, index)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(hoists.len(), 1);
        assert_eq!(hoists[0].1, 1);
        let hoisted_children = result
            .mir
            .node(hoists[0].0)
            .map(|node| node.children.len())
            .unwrap_or_default();
        assert_eq!(hoisted_children, 1);
        assert!(result
            .map
            .hir_to_mir
            .iter()
            .any(|(_, mir)| *mir == hoists[0].0));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["two"]
        );
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_v_once_cache_wrappers() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><p v-once>{{ msg }}</p><p v-once><span v-once>nested</span></p></div>"#
                .into(),
            file_id: FileId(19),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let caches = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match node.kind {
                Vue3DomMirKind::Cache { index } => Some((node.id, index)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(caches.len(), 2);
        assert_eq!(
            caches.iter().map(|(_, index)| *index).collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert!(caches.iter().all(|(id, _)| result
            .mir
            .node(*id)
            .is_some_and(|node| node.children.len() == 1)));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["msg"]
        );
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_v_memo_wrappers() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><p v-memo="[msg]">{{ msg }}</p><span v-for="item in list" v-memo="[item.id]">{{ item.name }}</span></div>"#.into(),
            file_id: FileId(20),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let memos = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match node.kind {
                Vue3DomMirKind::Memo { expression, index } => Some((node.id, expression, index)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(memos.len(), 1);
        assert_eq!(memos[0].1, JsExprId(0));
        assert_eq!(memos[0].2, 0);
        assert!(result
            .mir
            .node(memos[0].0)
            .is_some_and(|node| node.children.len() == 1));
        assert!(result
            .mir
            .nodes
            .iter()
            .any(|node| matches!(node.kind, Vue3DomMirKind::For(_))));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["[msg]", "msg", "list", "item.name", "[item.id]"]
        );
    }

    #[test]
    fn generate_vue3_dom_mir_emits_basic_vnode_tree_without_ast() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div :class="foo">{{ msg }}</div>"#.into(),
            file_id: FileId(26),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert_eq!(generated.ast_summary, "vue3-dom-mir-nodes=3");
        assert!(generated
            .code
            .starts_with("const { toDisplayString: _toDisplayString"));
        assert!(generated
            .code
            .contains("return function render(_ctx, _cache)"));
        assert!(generated.code.contains("_createElementBlock(\"div\""));
        assert!(generated.code.contains("class: _normalizeClass(_ctx.foo)"));
        assert!(generated
            .code
            .contains("_createTextVNode(_toDisplayString(_ctx.msg), 1 /* TEXT */)"));
        assert!(generated.code.contains("3"));
        assert!(generated.code.contains("[\"class\"]"));
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_asset_import_root_payload() {
        let mut ast = Vue3Dialect::base_parse(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<img :src="_imports_0">"#.into(),
                file_id: FileId(26),
                base_offset: 0,
            },
            &Vue3CompilerOptions::default(),
        );
        if let Some(root_node) = ast.root_node_mut() {
            if let Vue3AstKind::Root(root) = &mut root_node.kind {
                root.imports.push(vuec_ast::Vue3ImportItem {
                    name: "_imports_0".into(),
                    path: "./logo.png".into(),
                });
            }
        }

        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let root_imports = match &result.mir.node(result.mir.root).unwrap().kind {
            Vue3DomMirKind::Root(root) => &root.imports,
            _ => unreachable!("DOM MIR root kind"),
        };
        assert_eq!(root_imports.len(), 1);
        assert_eq!(root_imports[0].name, "_imports_0");
        assert_eq!(root_imports[0].path, "./logo.png");
        let img = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call) => Some(call),
                _ => None,
            })
            .unwrap();
        assert_eq!(img.dynamic_props, Vec::<String>::new());
        assert_eq!(img.patch_flag.bits, 0);
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_hoists_static_asset_import_bindings() {
        let mut ast = Vue3Dialect::base_parse(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div><img :src="_imports_0" /><span :id="dynamic"></span></div>"#.into(),
                file_id: FileId(26),
                base_offset: 0,
            },
            &Vue3CompilerOptions::default(),
        );
        if let Some(root_node) = ast.root_node_mut() {
            if let Vue3AstKind::Root(root) = &mut root_node.kind {
                root.imports.push(vuec_ast::Vue3ImportItem {
                    name: "_imports_0".into(),
                    path: "./logo.png".into(),
                });
            }
        }

        let result = lower_vue3_ast_to_dom_mir(
            &ast,
            &Vue3CompilerOptions {
                hoist_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        let hoists = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match node.kind {
                Vue3DomMirKind::Hoisted { index } => Some((node.id, index)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(hoists.len(), 1);
        assert_eq!(hoists[0].1, 1);
        assert_eq!(
            result.mir.node(hoists[0].0).map(|node| node.children.len()),
            Some(1)
        );

        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                hoist_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("import _imports_0 from './logo.png'"));
        assert!(generated
            .code
            .contains("const _hoisted_1 = _createElementVNode(\"img\", { src: _imports_0 })"));
        assert!(generated.code.contains("_hoisted_1"));
        assert!(generated.code.contains("id: _ctx.dynamic"));
        assert!(!generated.code.contains("_ctx._imports_0"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_asset_imports_from_mir_root() {
        let mut ast = Vue3Dialect::base_parse(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<img :src="_imports_0" :srcset="_imports_0 + ' 2x'">"#.into(),
                file_id: FileId(26),
                base_offset: 0,
            },
            &Vue3CompilerOptions::default(),
        );
        if let Some(root_node) = ast.root_node_mut() {
            if let Vue3AstKind::Root(root) = &mut root_node.kind {
                root.imports.push(vuec_ast::Vue3ImportItem {
                    name: "_imports_0".into(),
                    path: "./logo.png".into(),
                });
            }
        }

        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("import _imports_0 from './logo.png'"));
        assert!(
            generated.code.find("from \"vue\"").unwrap()
                < generated.code.find("./logo.png").unwrap()
        );
        assert!(generated.code.contains("src: _imports_0"));
        assert!(generated.code.contains("srcset: _imports_0 + ' 2x'"));
        assert!(!generated.code.contains("_ctx._imports_0"));
        assert!(!generated.code.contains("[\"src\""));
    }

    #[test]
    fn base_compile_hoists_static_srcset_asset_import_binding() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<img :srcset="_imports_0 + ' 2x'">"#.into(),
            file_id: FileId(26),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            hoist_static: true,
            ..Vue3CompilerOptions::default()
        };
        let mut ast = Vue3Dialect::base_parse(source.clone(), &options);
        if let Some(root_node) = ast.root_node_mut() {
            if let Vue3AstKind::Root(root) = &mut root_node.kind {
                root.imports.push(vuec_ast::Vue3ImportItem {
                    name: "_imports_0".into(),
                    path: "./logo.png".into(),
                });
            }
        }
        let mut ctx = TransformContext::default();
        Vue3Dialect::transform(&mut ast, &mut ctx, &options);
        let result = Vue3Dialect::finish_compile(ast, source, options, ctx);

        assert!(result.code.contains("import _imports_0 from './logo.png'"));
        assert!(result
            .code
            .contains("const _hoisted_1 = _imports_0 + ' 2x'"));
        assert!(result.code.contains("srcset: _hoisted_1"));
        assert!(!result.code.contains("_ctx._imports_0"));
    }

    #[test]
    fn base_compile_reuses_static_asset_fragment_binding_hoist() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r##"<svg><use :href="_imports_0 + '#fragment'"></use><use :href="_imports_0 + '#fragment'"></use></svg>"##.into(),
            file_id: FileId(26),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            hoist_static: true,
            ..Vue3CompilerOptions::default()
        };
        let mut ast = Vue3Dialect::base_parse(source.clone(), &options);
        if let Some(root_node) = ast.root_node_mut() {
            if let Vue3AstKind::Root(root) = &mut root_node.kind {
                root.imports.push(vuec_ast::Vue3ImportItem {
                    name: "_imports_0".into(),
                    path: "./icons.svg".into(),
                });
            }
        }
        let mut ctx = TransformContext::default();
        Vue3Dialect::transform(&mut ast, &mut ctx, &options);
        let result = Vue3Dialect::finish_compile(ast, source, options, ctx);

        assert!(result.code.contains("import _imports_0 from './icons.svg'"));
        assert!(result
            .code
            .contains("const _hoisted_1 = _imports_0 + '#fragment'"));
        assert_eq!(result.code.matches("const _hoisted_").count(), 1);
        assert_eq!(result.code.matches("href: _hoisted_1").count(), 2);
        assert!(!result.code.contains("_ctx._imports_0"));
    }

    #[test]
    fn generate_vue3_dom_mir_uses_function_mode_preamble_without_prefix() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div :class="foo">{{ msg }}</div>"#.into(),
            file_id: FileId(26),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        let generated =
            generate_vue3_dom_mir(&result.mir, &result.js, &Vue3CompilerOptions::default());

        assert!(generated.code.starts_with("const _Vue = Vue"));
        assert!(generated
            .code
            .contains("return function render(_ctx, _cache)"));
        assert!(generated.code.contains("with (_ctx) {"));
        assert!(generated
            .code
            .contains("const { toDisplayString: _toDisplayString"));
        assert!(generated.code.contains("class: _normalizeClass(foo)"));
        assert!(generated
            .code
            .contains("_createTextVNode(_toDisplayString(msg), 1 /* TEXT */)"));
        assert!(!generated.code.contains("export function render"));
        assert!(!generated.code.contains("_ctx.foo"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_control_flow_from_js_store() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<li v-for="item in list" v-if="item.ok">{{ item.name }}</li>"#.into(),
            file_id: FileId(27),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("_renderList(_ctx.list, (item) => {"));
        assert!(generated.code.contains("item.ok"));
        assert!(generated.code.contains("? (_openBlock()"));
        assert!(generated.code.contains("_toDisplayString(item.name)"));
        assert!(!generated.code.contains("_ctx.item"));
        assert!(generated
            .code
            .contains("_createCommentVNode(\"v-if\", true)"));
        assert!(!generated.code.contains("v-for"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_cache_and_hoist_wrappers() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><span id="one">one</span><p v-once>{{ msg }}</p></div>"#.into(),
            file_id: FileId(28),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(
            &ast,
            &Vue3CompilerOptions {
                hoist_static: true,
                ..Vue3CompilerOptions::default()
            },
        );
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                hoist_static: true,
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("_hoisted_1"));
        assert!(generated
            .code
            .contains("const _hoisted_1 = _createElementVNode(\"span\""));
        assert!(generated
            .code
            .contains("return function render(_ctx, _cache)"));
        assert!(generated.code.contains("_cache[0] || (_cache[0] ="));
        assert!(generated.code.contains("_toDisplayString(_ctx.msg)"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_v_memo_wrappers() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><p v-memo="[msg]">{{ msg }}</p></div>"#.into(),
            file_id: FileId(30),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("withMemo as _withMemo"));
        assert!(generated
            .code
            .contains("_withMemo([_ctx.msg], () => (_openBlock(), _createElementBlock(\"p\""));
        assert!(generated.code.contains("_cache, 0)"));
        assert!(generated.code.contains("_toDisplayString(_ctx.msg)"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_v_for_memo_cache_target() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-for="{ x, y } in list" :key="x" v-memo="[x, y === z]"><span>foobar</span></div>"#.into(),
            file_id: FileId(31),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("isMemoSame as _isMemoSame"));
        assert!(generated
            .code
            .contains("_renderList(_ctx.list, ({ x, y }, __, ___, _cached) =>"));
        assert!(generated.code.contains("const _memo = ([x, y === _ctx.z])"));
        assert!(generated
            .code
            .contains("_cached.key === x && _isMemoSame(_cached, _memo)"));
        assert!(generated
            .code
            .contains("const _item = (_openBlock(), _createElementBlock(\"div\""));
        assert!(generated.code.contains("_item.memo = _memo"));
        assert!(generated
            .code
            .contains("}, _cache, 0), 128 /* KEYED_FRAGMENT */)"));
        assert!(!generated.code.contains("_ctx.x"));
        assert!(!generated.code.contains("_ctx.y"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_content_override_payloads_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><section v-html="raw">old</section><p v-text="msg"/><span>keep</span></div>"#.into(),
            file_id: FileId(59),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("toDisplayString as _toDisplayString"));
        assert!(generated.code.contains("{ innerHTML: _ctx.raw }"));
        assert!(generated
            .code
            .contains("textContent: _toDisplayString(_ctx.msg)"));
        assert!(generated.code.contains("8 /* PROPS */, [\"innerHTML\"]"));
        assert!(generated.code.contains("8 /* PROPS */, [\"textContent\"]"));
        assert!(generated.code.contains("\"keep\""));
        assert!(!generated.code.contains("\"old\""));
    }

    #[test]
    fn generate_vue3_dom_mir_merges_content_override_with_object_bindings() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-bind="before" v-html="raw"/><p v-text="'hi'" v-bind="after"/>"#
                .into(),
            file_id: FileId(60),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(generated
            .code
            .contains("_mergeProps(_ctx.before, { innerHTML: _ctx.raw })"));
        assert!(generated
            .code
            .contains("_mergeProps({ textContent: 'hi' }, _ctx.after)"));
        assert!(!generated.code.contains("_toDisplayString('hi')"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_native_v_model_payloads_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><input v-model="text"><input type="radio" v-model="picked"><input type="checkbox" v-model.trim="checked"><input :type="kind" v-model="dynamic"><select v-model="selected"/><textarea v-model.lazy="body"/></div>"#.into(),
            file_id: FileId(62),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("vModelText as _vModelText"));
        assert!(generated.code.contains("vModelRadio as _vModelRadio"));
        assert!(generated.code.contains("vModelCheckbox as _vModelCheckbox"));
        assert!(generated.code.contains("vModelSelect as _vModelSelect"));
        assert!(generated.code.contains("vModelDynamic as _vModelDynamic"));
        assert!(generated.code.contains("withDirectives as _withDirectives"));
        assert!(generated.code.contains("[_vModelText, _ctx.text]"));
        assert!(generated.code.contains("[_vModelRadio, _ctx.picked]"));
        assert!(generated.code.contains("_vModelCheckbox"));
        assert!(generated.code.contains("_ctx.checked"));
        assert!(generated.code.contains("trim: true"));
        assert!(generated.code.contains("[_vModelDynamic, _ctx.dynamic]"));
        assert!(generated.code.contains("[_vModelSelect, _ctx.selected]"));
        assert!(generated.code.contains("_ctx.body"));
        assert!(generated.code.contains("lazy: true"));
        assert!(generated
            .code
            .contains("\"onUpdate:modelValue\": $event => ((_ctx.text) = $event)"));
        assert!(generated
            .code
            .contains("\"onUpdate:modelValue\": $event => ((_ctx.body) = $event)"));
        assert!(generated
            .code
            .contains("8 /* PROPS */, [\"onUpdate:modelValue\"]"));
    }

    #[test]
    fn generate_vue3_dom_mir_merges_native_v_model_with_object_bindings() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<input v-bind="before" v-model="text" class="field"><input v-model="after" v-bind="tail">"#.into(),
            file_id: FileId(63),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(generated.code.contains("_mergeProps(_ctx.before"));
        assert!(generated.code.contains("class: \"field\""));
        assert!(generated.code.contains("_mergeProps({ \"onUpdate:modelValue\": $event => ((_ctx.after) = $event) }, _ctx.tail)"));
        assert!(generated.code.contains("[_vModelDynamic, _ctx.text]"));
        assert!(generated.code.contains("[_vModelDynamic, _ctx.after]"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_runtime_directives_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-focus:foo.bar="value"><span v-show="ok"/></div>"#.into(),
            file_id: FileId(29),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("withDirectives as _withDirectives"));
        assert!(generated
            .code
            .contains("resolveDirective as _resolveDirective"));
        assert!(generated.code.contains("vShow as _vShow"));
        assert!(generated
            .code
            .contains("const _directive_focus = _resolveDirective(\"focus\")"));
        assert!(generated
            .code
            .contains("[_directive_focus, _ctx.value, \"foo\", {"));
        assert!(generated.code.contains("bar: true"));
        assert!(generated.code.contains("[_vShow, _ctx.ok]"));
        assert!(!generated.code.contains("_resolveDirective(\"show\")"));
        assert!(!result.mir.nodes.iter().any(|node| matches!(
            &node.kind,
            Vue3DomMirKind::VNodeCall(call)
                if call.directives.iter().any(|directive| directive.name == "show")
        )));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_v_show_payload_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><span v-show="ok"/><p v-focus v-show="visible"/></div>"#.into(),
            file_id: FileId(69),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("withDirectives as _withDirectives"));
        assert!(generated.code.contains("vShow as _vShow"));
        assert!(generated
            .code
            .contains("resolveDirective as _resolveDirective"));
        assert!(generated
            .code
            .contains("const _directive_focus = _resolveDirective(\"focus\")"));
        assert!(generated.code.contains("[_vShow, _ctx.ok]"));
        assert!(generated.code.contains("[_vShow, _ctx.visible]"));
        assert!(generated.code.contains("[_directive_focus]"));
        assert!(generated.code.contains("512 /* NEED_PATCH */"));
        assert!(!generated.code.contains("_resolveDirective(\"show\")"));
    }

    #[test]
    fn generate_vue3_dom_mir_preserves_dynamic_runtime_directive_args() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-focus:[arg].bar="value"/>"#.into(),
            file_id: FileId(32),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["arg", "value"]
        );
        let directive_hir = result
            .hir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                HirNodeKind::Element(element) => element.directives.first(),
                _ => None,
            })
            .expect("HIR directive");
        assert_eq!(directive_hir.argument, None);
        assert!(directive_hir.dynamic_argument.is_some());

        let directive_mir = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call) => call.directives.first(),
                _ => None,
            })
            .expect("DOM MIR directive");
        assert_eq!(directive_mir.argument, None);
        assert!(directive_mir.dynamic_argument.is_some());

        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("[_directive_focus, _ctx.value, _ctx.arg, {"));
        assert!(!generated
            .code
            .contains("[_directive_focus, _ctx.value, \"arg\""));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_merge_and_dynamic_props_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<button id="save" v-bind="base" :[name]="value" v-on="listeners" @[event]="run">Save</button>"#.into(),
            file_id: FileId(31),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(generated.code.contains("normalizeProps as _normalizeProps"));
        assert!(generated
            .code
            .contains("guardReactiveProps as _guardReactiveProps"));
        assert!(generated.code.contains("toHandlers as _toHandlers"));
        assert!(generated.code.contains("toHandlerKey as _toHandlerKey"));
        assert!(generated
            .code
            .contains("_normalizeProps(_guardReactiveProps(_mergeProps("));
        assert!(generated.code.contains("id: \"save\""));
        assert!(generated.code.contains("_ctx.base"));
        assert!(generated.code.contains("[_ctx.name || \"\"]: _ctx.value"));
        assert!(generated.code.contains("_toHandlers(_ctx.listeners, true)"));
        assert!(generated
            .code
            .contains("[_toHandlerKey(_ctx.event)]: _ctx.run"));
        assert!(generated.code.contains("16 /* FULL_PROPS */"));
        assert!(!generated.code.contains("[\"name\"]"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_v_bind_modifier_payloads_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div :foo-bar.camel="foo" :fooBar.prop="bar" :foo-bar.attr="baz" :[name].camel="value" :[propName].prop="propValue"/>"#.into(),
            file_id: FileId(68),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("camelize as _camelize"));
        assert!(generated.code.contains("fooBar: _ctx.foo"));
        assert!(generated.code.contains("\".fooBar\": _ctx.bar"));
        assert!(generated.code.contains("\"^foo-bar\": _ctx.baz"));
        assert!(generated
            .code
            .contains("[_camelize(_ctx.name || \"\")]: _ctx.value"));
        assert!(generated
            .code
            .contains("['.' + (_ctx.propName || \"\")]: _ctx.propValue"));
        assert!(generated.code.contains("16 /* FULL_PROPS */"));
    }

    #[test]
    fn generate_vue3_dom_mir_marks_static_prop_modifier_for_hydration() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div :id.prop="id"/>"#.into(),
            file_id: FileId(69),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("\".id\": _ctx.id"));
        assert!(generated.code.contains("40 /* PROPS, NEED_HYDRATION */"));
        assert!(generated.code.contains("[\".id\"]"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_cache_handlers_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<button id="save" @click="go">Save</button>"#.into(),
            file_id: FileId(32),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(
            &ast,
            &Vue3CompilerOptions {
                cache_handlers: true,
                ..Vue3CompilerOptions::default()
            },
        );
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                cache_handlers: true,
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("onClick: _cache[0] || (_cache[0] = _ctx.go)"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_v_on_modifier_payloads_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><button @click.stop.capture.once="go"/><input @keyup.enter.prevent="submit"/><button @click.right="menu"/><button @mouseup.right="mouseup"/><button @[event].left="dynamic"/><button @[name].right="dynamicRight"/></div>"#.into(),
            file_id: FileId(65),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("withModifiers as _withModifiers"));
        assert!(generated.code.contains("withKeys as _withKeys"));
        assert!(generated.code.contains("toHandlerKey as _toHandlerKey"));
        assert!(generated
            .code
            .contains("onClickCaptureOnce: _withModifiers(_ctx.go, [\"stop\"])"));
        assert!(generated.code.contains(
            "onKeyup: _withKeys(_withModifiers(_ctx.submit, [\"prevent\"]), [\"enter\"])"
        ));
        assert!(generated
            .code
            .contains("onContextmenu: _withModifiers(_ctx.menu, [\"right\"])"));
        assert!(generated
            .code
            .contains("onMouseup: _withModifiers(_ctx.mouseup, [\"right\"])"));
        assert!(generated.code.contains(
            "[_toHandlerKey(_ctx.event)]: _withKeys(_withModifiers(_ctx.dynamic, [\"left\"]), [\"left\"])"
        ));
        assert!(generated.code.contains(
            "[(_toHandlerKey(_ctx.name)) === \"onClick\" ? \"onContextmenu\" : (_toHandlerKey(_ctx.name))]: _withKeys(_withModifiers(_ctx.dynamicRight, [\"right\"]), [\"right\"])"
        ));
    }

    #[test]
    fn generate_vue3_dom_mir_caches_v_on_modifier_handler_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<button @keyup.enter.capture="submit">Save</button>"#.into(),
            file_id: FileId(66),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(
            &ast,
            &Vue3CompilerOptions {
                cache_handlers: true,
                ..Vue3CompilerOptions::default()
            },
        );
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                cache_handlers: true,
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains(
            "onKeyupCapture: _cache[0] || (_cache[0] = _withKeys(_ctx.submit, [\"enter\"]))"
        ));
        assert!(!generated
            .code
            .contains("8 /* PROPS */, [\"onKeyupCapture\"]"));
    }

    #[test]
    fn generate_vue3_dom_mir_rewrites_event_handlers_with_event_local() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<button @click="go($event, item)">Save</button>"#.into(),
            file_id: FileId(32),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("onClick: _ctx.go($event, _ctx.item)"));
        assert!(!generated.code.contains("_ctx.$event"));
    }

    #[test]
    fn generate_vue3_dom_mir_rewrites_arrow_event_handler_scopes() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<button @click="event => go(event, item)">Save</button>"#.into(),
            file_id: FileId(32),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("onClick: event => _ctx.go(event, _ctx.item)"));
        assert!(!generated.code.contains("_ctx.event"));
    }

    #[test]
    fn generate_vue3_dom_mir_imports_helpers_from_binding_rewrites() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div>{{ maybe }}</div>"#.into(),
            file_id: FileId(32),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let mut options = Vue3CompilerOptions {
            inline: true,
            mode: "module".into(),
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("maybe".into(), "setup-maybe-ref".into());
        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);

        assert!(generated.code.contains("unref as _unref"));
        assert!(generated.code.contains("_toDisplayString(_unref(maybe))"));
    }

    #[test]
    fn generate_vue3_dom_mir_imports_is_ref_for_setup_let_handlers() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<button @click="count = count + 1">Save</button>"#.into(),
            file_id: FileId(32),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let mut options = Vue3CompilerOptions {
            inline: true,
            mode: "module".into(),
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("count".into(), "setup-let".into());
        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);

        assert!(generated.code.contains("isRef as _isRef"));
        assert!(generated.code.contains(
            "_isRef(count) ? count.value = _unref(count) + 1 : count = _unref(count) + 1"
        ));
    }

    #[test]
    fn generate_vue3_dom_mir_rewrites_inline_v_model_assignments_by_binding() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source:
                r#"<div><input v-model="count"><input v-model="maybe"><input v-model="lett"></div>"#
                    .into(),
            file_id: FileId(88),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let mut options = Vue3CompilerOptions {
            inline: true,
            mode: "module".into(),
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("count".into(), "setup-ref".into());
        options
            .binding_metadata
            .insert("maybe".into(), "setup-maybe-ref".into());
        options
            .binding_metadata
            .insert("lett".into(), "setup-let".into());
        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);

        assert!(generated.code.contains("isRef as _isRef"));
        assert!(generated
            .code
            .contains("\"onUpdate:modelValue\": $event => ((count).value = $event)"));
        assert!(generated.code.contains(
            "\"onUpdate:modelValue\": $event => (_isRef(maybe) ? (maybe).value = $event : null)"
        ));
        assert!(generated
            .code
            .contains("\"onUpdate:modelValue\": $event => (_isRef(lett) ? (lett).value = $event : lett = $event)"));
    }

    #[test]
    fn generate_vue3_dom_mir_rewrites_inline_handler_assignment_bindings() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<button @click="count = 1; maybe = count; lett += count; count++; --maybe; lett--">Save</button>"#.into(),
            file_id: FileId(89),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let mut options = Vue3CompilerOptions {
            inline: true,
            mode: "module".into(),
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("count".into(), "setup-ref".into());
        options
            .binding_metadata
            .insert("maybe".into(), "setup-maybe-ref".into());
        options
            .binding_metadata
            .insert("lett".into(), "setup-let".into());
        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);

        assert!(generated.code.contains("count.value = 1"));
        assert!(generated.code.contains("maybe.value = count.value"));
        assert!(generated
            .code
            .contains("_isRef(lett) ? lett.value += count.value : lett += count.value"));
        assert!(generated.code.contains("count.value++"));
        assert!(generated.code.contains("--maybe.value"));
        assert!(generated
            .code
            .contains("_isRef(lett) ? lett.value-- : lett--"));
    }

    #[test]
    fn base_compile_rewrites_inline_v_model_and_nested_handler_assignments() {
        let mut options = Vue3CompilerOptions {
            inline: true,
            mode: "module".into(),
            prefix_identifiers: true,
            cache_handlers: true,
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("count".into(), "setup-ref".into());
        options
            .binding_metadata
            .insert("maybe".into(), "setup-maybe-ref".into());
        options
            .binding_metadata
            .insert("lett".into(), "setup-let".into());
        options
            .binding_metadata
            .insert("v".into(), "setup-let".into());
        options
            .binding_metadata
            .insert("val".into(), "literal-const".into());

        let result = Vue3Dialect::base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div><input v-model="count"/><input v-model="maybe"/><input v-model="lett"/><button @click="() => { v = lett }"/><button @click="() => {
  let a = '' + lett
  v = a
}"/><button @click="() => {
  (() => {
    let x = a
    (() => {
      let z = x
      let z2 = z
    })
    let lz = z
  })
  v = a
}"/><button @click="({ count } = val); [maybe] = val; ({ lett } = val)"/></div>"#.into(),
                file_id: FileId(91),
                base_offset: 0,
            },
            options,
        );

        assert!(result.preamble.contains("vModelText as _vModelText"));
        assert!(result.code.contains("[_vModelText, count.value]"));
        assert!(result.code.contains("(count).value = $event"));
        assert!(result
            .code
            .contains("_isRef(maybe) ? (maybe).value = $event : null"));
        assert!(result
            .code
            .contains("_isRef(lett) ? (lett).value = $event : lett = $event"));
        assert!(result
            .code
            .contains("_isRef(v) ? v.value = _unref(lett) : v = _unref(lett)"));
        assert!(result.code.contains("_isRef(v) ? v.value = a : v = a"));
        assert!(result
            .code
            .contains("_isRef(v) ? v.value = _ctx.a : v = _ctx.a"));
        assert!(result.code.contains("({ count: count.value } = val)"));
        assert!(result.code.contains("[maybe.value] = val"));
        assert!(result.code.contains("({ lett: lett } = val)"));
    }

    #[test]
    fn base_compile_marks_cached_native_v_model_need_patch() {
        let mut options = Vue3CompilerOptions {
            inline: true,
            mode: "module".into(),
            prefix_identifiers: true,
            cache_handlers: true,
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("count".into(), "setup-ref".into());
        let result = Vue3Dialect::base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<input v-model="count">"#.into(),
                file_id: FileId(91),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains(
            "\"onUpdate:modelValue\": _cache[0] || (_cache[0] = $event => ((count).value = $event))"
        ));
        assert!(result.code.contains("512 /* NEED_PATCH */"));
        assert!(!result
            .code
            .contains("8 /* PROPS */, [\"onUpdate:modelValue\"]"));
    }

    #[test]
    fn base_compile_emits_inline_component_v_model_props_and_diagnostics() {
        let mut options = Vue3CompilerOptions {
            inline: true,
            mode: "module".into(),
            prefix_identifiers: true,
            cache_handlers: true,
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("name".into(), "setup-let".into());
        let result = Vue3Dialect::base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<MyComponent v-model="name" />"#.into(),
                file_id: FileId(92),
                base_offset: 0,
            },
            options,
        );

        assert!(result.preamble.contains("unref as _unref"));
        assert!(result.preamble.contains("isRef as _isRef"));
        assert!(result.code.contains("modelValue: _unref(name)"));
        assert!(result.code.contains(
            "\"onUpdate:modelValue\": _cache[0] || (_cache[0] = $event => (_isRef(name) ? (name).value = $event : name = $event))"
        ));
        assert!(result.code.contains("8 /* PROPS */, [\"modelValue\"]"));
        assert!(result.diagnostics.is_empty());

        let mut invalid_options = Vue3CompilerOptions {
            inline: true,
            mode: "module".into(),
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        };
        invalid_options
            .binding_metadata
            .insert("foo".into(), "literal-const".into());
        let invalid = Vue3Dialect::base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<input v-model="foo" />"#.into(),
                file_id: FileId(92),
                base_offset: 0,
            },
            invalid_options,
        );

        assert_eq!(invalid.diagnostics.len(), 1);
        assert_eq!(invalid.diagnostics[0].code, "45");
        assert!(invalid.diagnostics[0]
            .message
            .contains("v-model cannot be used on a const binding"));

        let empty = Vue3Dialect::base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<input v-model="" />"#.into(),
                file_id: FileId(92),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );
        assert_eq!(empty.diagnostics.len(), 1);
        assert_eq!(empty.diagnostics[0].code, "42");
    }

    #[test]
    fn generate_vue3_dom_mir_emits_component_tags_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><Child/><Transition/><component :is="view"/></div>"#.into(),
            file_id: FileId(33),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("resolveComponent as _resolveComponent"));
        assert!(generated
            .code
            .contains("resolveDynamicComponent as _resolveDynamicComponent"));
        assert!(generated.code.contains("Transition as _Transition"));
        assert!(generated
            .code
            .contains("const _component_Child = _resolveComponent(\"Child\")"));
        assert!(generated.code.contains("_createVNode(_component_Child"));
        assert!(generated.code.contains("_createVNode(_Transition"));
        assert!(generated
            .code
            .contains("_createVNode(_resolveDynamicComponent(_ctx.view)"));
        assert!(!generated.code.contains("_component_Transition"));
        assert!(!generated.code.contains("is: _ctx.view"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_stable_component_slots_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Comp><template #header="{ item }"><span>{{ item.name }}</span></template><p>{{ msg }}</p></Comp>"#.into(),
            file_id: FileId(35),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("withCtx as _withCtx"));
        assert!(generated
            .code
            .contains("const _component_Comp = _resolveComponent(\"Comp\")"));
        assert!(generated.code.contains("header: _withCtx(({ item }) => ["));
        assert!(generated.code.contains("_toDisplayString(item.name)"));
        assert!(generated.code.contains("default: _withCtx(() => ["));
        assert!(generated.code.contains("_toDisplayString(_ctx.msg)"));
        assert!(!generated.code.contains("_ctx.item"));
        assert!(generated.code.contains("_: 1"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_transition_persisted_prop_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Transition v-bind="base"><div v-show="ok"/></Transition>"#.into(),
            file_id: FileId(71),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("Transition as _Transition"));
        assert!(generated.code.contains("withCtx as _withCtx"));
        assert!(generated.code.contains("vShow as _vShow"));
        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(generated.code.contains("_createBlock(_Transition"));
        assert!(generated.code.contains("_mergeProps(_ctx.base"));
        assert!(generated.code.contains("_ctx.base"));
        assert!(generated.code.contains("{ persisted: \"\" }"));
        assert!(generated.code.contains("[_vShow, _ctx.ok]"));
        assert!(generated.code.contains("512 /* NEED_PATCH */"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_dynamic_component_slots_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Comp><template #[name]="slotProps"><span>{{ slotProps.label }}</span></template><template #fallback v-if="ok">Fallback</template><template #item v-for="(item, index) in list"><span>{{ item }}</span></template></Comp>"#.into(),
            file_id: FileId(37),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("createSlots as _createSlots"));
        assert!(generated.code.contains("withCtx as _withCtx"));
        assert!(generated.code.contains("renderList as _renderList"));
        assert!(generated.code.contains("_createSlots({"));
        assert!(generated.code.contains("_: 2"));
        assert!(generated.code.contains("1024 /* DYNAMIC_SLOTS */"));
        assert!(generated.code.contains("name: _ctx.name"));
        assert!(generated.code.contains("fn: _withCtx((slotProps) => ["));
        assert!(generated.code.contains("_toDisplayString(slotProps.label)"));
        assert!(generated.code.contains("_ctx.ok"));
        assert!(generated.code.contains(": undefined"));
        assert!(generated.code.contains("name: \"fallback\""));
        assert!(generated.code.contains("key: \"1\""));
        assert!(generated
            .code
            .contains("_renderList(_ctx.list, (item, index) => {"));
        assert!(generated.code.contains("name: \"item\""));
        assert!(generated.code.contains("_toDisplayString(item)"));
        assert!(!generated.code.contains("_ctx.slotProps"));
        assert!(!generated.code.contains("_ctx.item"));
        assert!(!generated.code.contains("_ctx.index"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_dynamic_slot_if_else_alternates_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Comp><template #one v-if="ok">One</template><template #two v-else-if="maybe">Two</template><template #fallback v-else>Fallback</template></Comp>"#.into(),
            file_id: FileId(40),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("createSlots as _createSlots"));
        assert!(generated.code.contains("_ctx.ok"));
        assert!(generated.code.contains("_ctx.maybe"));
        assert!(generated.code.contains("name: \"one\""));
        assert!(generated.code.contains("name: \"two\""));
        assert!(generated.code.contains("name: \"fallback\""));
        assert!(generated.code.contains("key: \"0\""));
        assert!(generated.code.contains("key: \"1\""));
        assert!(generated.code.contains("key: \"2\""));
        let ok_offset = generated.code.find("_ctx.ok").expect("ok condition");
        let maybe_offset = generated.code.find("_ctx.maybe").expect("maybe condition");
        let one_offset = generated.code.find("name: \"one\"").expect("one slot");
        let two_offset = generated.code.find("name: \"two\"").expect("two slot");
        let fallback_offset = generated
            .code
            .find("name: \"fallback\"")
            .expect("fallback slot");
        assert!(ok_offset < one_offset);
        assert!(one_offset < maybe_offset);
        assert!(maybe_offset < two_offset);
        assert!(two_offset < fallback_offset);
        assert!(!generated.code.contains(": undefined"));
        assert_eq!(generated.code.matches("name: \"one\"").count(), 1);
        assert_eq!(generated.code.matches("name: \"two\"").count(), 1);
        assert_eq!(generated.code.matches("name: \"fallback\"").count(), 1);
    }

    #[test]
    fn generate_vue3_dom_mir_emits_forwarded_component_slot_flag() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Comp><template #default><slot /></template></Comp>"#.into(),
            file_id: FileId(38),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("renderSlot as _renderSlot"));
        assert!(generated
            .code
            .contains("_renderSlot(_ctx.$slots, \"default\")"));
        assert!(generated.code.contains("_: 3"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_slot_outlet_payload_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<slot :name="active" foo="bar" :baz="baz">fallback {{ msg }}</slot>"#.into(),
            file_id: FileId(42),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("renderSlot as _renderSlot"));
        assert!(generated
            .code
            .contains("toDisplayString as _toDisplayString"));
        assert!(generated
            .code
            .contains("_renderSlot(_ctx.$slots, _ctx.active, {"));
        assert!(generated.code.contains("foo: \"bar\""));
        assert!(generated.code.contains("baz: _ctx.baz"));
        assert!(generated.code.contains("() => ["));
        assert!(generated
            .code
            .contains("_createTextVNode(_toDisplayString(_ctx.msg), 1 /* TEXT */)"));
        assert!(!generated.code.contains("name: _ctx.active"));
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_records_target_split_edges_and_js_store() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div id=\"app\" :class=\"foo\">{{ msg }}</div>".into(),
            file_id: FileId(11),
            base_offset: 5,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        assert!(result
            .map
            .hir_for_ast(ast.root)
            .any(|hir| hir == result.hir.root));
        assert!(result
            .map
            .mir_for_hir(result.hir.root)
            .any(|mir| mir == result.mir.root));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["foo", "msg"]
        );

        let pushes = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3SsrMirKind::PushString(value) => Some(value.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(pushes, vec!["<div id=\"app\"", ">", "</div>"]);
        assert!(result.mir.nodes.iter().any(|node| matches!(
            &node.kind,
            Vue3SsrMirKind::RenderAttrs(attrs)
                if attrs.props.dynamic_bindings.len() == 1
                    && attrs.props.dynamic_bindings[0].name == "class"
                    && attrs.props.dynamic_bindings[0].value == JsExprId(0)
        )));
        assert!(result.mir.nodes.iter().any(|node| matches!(
            node.kind,
            Vue3SsrMirKind::PushInterpolated(MirExpr::JsExpr(_))
        )));
        assert!(result
            .mir
            .nodes
            .iter()
            .all(|node| !matches!(node.kind, Vue3SsrMirKind::RenderComponent(_))));
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_projects_asset_import_root_payload() {
        let mut ast = Vue3Dialect::base_parse(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<img :src="_imports_0">"#.into(),
                file_id: FileId(72),
                base_offset: 0,
            },
            &Vue3CompilerOptions::default(),
        );
        if let Some(root_node) = ast.root_node_mut() {
            if let Vue3AstKind::Root(root) = &mut root_node.kind {
                root.imports.push(vuec_ast::Vue3ImportItem {
                    name: "_imports_0".into(),
                    path: "./logo.png".into(),
                });
            }
        }

        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let root_imports = match &result.mir.node(result.mir.root).unwrap().kind {
            Vue3SsrMirKind::Root(root) => &root.imports,
            _ => unreachable!("SSR MIR root kind"),
        };
        assert_eq!(root_imports.len(), 1);
        assert_eq!(root_imports[0].name, "_imports_0");
        assert_eq!(root_imports[0].path, "./logo.png");
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_keeps_component_and_slot_target_split() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<Comp><slot name=\"header\">fallback</slot></Comp>".into(),
            file_id: FileId(12),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = Vue3Dialect::lower_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert!(result
            .hir
            .nodes
            .iter()
            .any(|node| matches!(node.kind, HirNodeKind::Component(_))));
        assert!(result
            .hir
            .nodes
            .iter()
            .any(|node| matches!(node.kind, HirNodeKind::SlotOutlet(_))));
        assert!(result.mir.nodes.iter().any(|node| matches!(
            &node.kind,
            Vue3SsrMirKind::RenderComponent(component)
                if matches!(component.tag, MirExpr::String(_))
                    && component.props.static_attrs.is_empty()
        )));
        assert!(result.mir.nodes.iter().any(|node| matches!(
            &node.kind,
            Vue3SsrMirKind::RenderSlot(slot)
                if matches!(slot.name, Vue3DomSlotName::Static(ref name) if name == "header")
                    && slot.fallback.len() == 1
        )));
        assert!(result
            .mir
            .nodes
            .iter()
            .all(|node| !matches!(node.kind, Vue3SsrMirKind::PushInterpolated(_))));
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_projects_slot_outlet_payload() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<slot :name="active" foo="bar" :baz="baz">fallback {{ msg }}</slot>"#.into(),
            file_id: FileId(42),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = Vue3Dialect::lower_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let slot = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::RenderSlot(slot) => Some(slot),
                _ => None,
            })
            .expect("ssr render slot");
        assert!(matches!(slot.name, Vue3DomSlotName::Dynamic(JsExprId(0))));
        assert_eq!(slot.props.static_attrs.len(), 1);
        assert_eq!(slot.props.static_attrs[0].name, "foo");
        assert_eq!(slot.props.static_attrs[0].value, "bar");
        assert_eq!(slot.props.dynamic_bindings.len(), 1);
        assert_eq!(slot.props.dynamic_bindings[0].name, "baz");
        assert_eq!(slot.props.dynamic_bindings[0].value, JsExprId(1));
        assert_eq!(slot.fallback.len(), 2);
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["active", "baz", "msg"]
        );
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_lowers_v_for_and_v_if_control_flow() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<li v-for="item in list" v-if="item.ok">{{ item.name }}</li>"#.into(),
            file_id: FileId(14),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["list", "item.ok", "item.name"]
        );
        assert_eq!(
            result
                .js
                .patterns()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["item"]
        );

        assert!(result
            .hir
            .nodes
            .iter()
            .any(|node| matches!(node.kind, HirNodeKind::For(_))));
        assert!(result
            .hir
            .nodes
            .iter()
            .any(|node| matches!(node.kind, HirNodeKind::If(_))));
        let for_mir = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::For(for_mir) => Some(for_mir),
                _ => None,
            })
            .expect("SSR MIR for");
        assert_eq!(for_mir.source, JsExprId(0));
        assert_eq!(for_mir.value_alias, JsPatternId(0));
        assert!(for_mir.key_alias.is_none());
        assert!(for_mir.index_alias.is_none());
        assert!(result.mir.nodes.iter().any(|node| matches!(
            node.kind,
            Vue3SsrMirKind::If {
                condition: Some(_),
                ..
            }
        )));
        assert!(result
            .mir
            .nodes
            .iter()
            .any(|node| matches!(node.kind, Vue3SsrMirKind::PushInterpolated(_))));
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_projects_v_for_alias_payload() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<li v-for="(item, key, index) in list">{{ key }}:{{ index }}:{{ item.name }}</li>"#.into(),
            file_id: FileId(18),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["list", "key", "index", "item.name"]
        );
        assert_eq!(
            result
                .js
                .patterns()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["item", "key", "index"]
        );

        let for_mir = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::For(for_mir) => Some(for_mir),
                _ => None,
            })
            .expect("SSR MIR for");
        assert_eq!(for_mir.source, JsExprId(0));
        assert_eq!(for_mir.value_alias, JsPatternId(0));
        assert_eq!(for_mir.key_alias, Some(JsPatternId(1)));
        assert_eq!(for_mir.index_alias, Some(JsPatternId(2)));
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_projects_builtin_component_payloads() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r##"<Teleport :to="target" :disabled="off"><div id="x"/></Teleport><Suspense><template #default><Foo/></template><template #fallback>loading</template></Suspense>"##.into(),
            file_id: FileId(19),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let teleport = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::Teleport(teleport) => Some(teleport),
                _ => None,
            })
            .expect("SSR MIR teleport");
        assert_eq!(teleport.target, MirExpr::JsExpr(JsExprId(0)));
        assert_eq!(teleport.disabled, MirExpr::JsExpr(JsExprId(1)));

        let suspense = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::Suspense(suspense) => Some(suspense),
                _ => None,
            })
            .expect("SSR MIR suspense");
        assert_eq!(suspense.slots.flag, Vue3SlotFlag::Stable);
        assert_eq!(
            suspense
                .slots
                .slots
                .iter()
                .map(|slot| slot.name.as_str())
                .collect::<Vec<_>>(),
            vec!["default", "fallback"]
        );
        assert!(result
            .mir
            .nodes
            .iter()
            .all(|node| !matches!(&node.kind, Vue3SsrMirKind::RenderComponent(component) if matches!(&component.tag, MirExpr::String(tag) if tag == "Teleport" || tag == "Suspense"))));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["target", "off"]
        );
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_groups_if_else_branch_chains() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<p v-if="ok">yes</p><p v-else-if="maybe">maybe</p><p v-else>no</p>"#.into(),
            file_id: FileId(17),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let hir_if = result
            .hir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                HirNodeKind::If(hir_if) => Some(hir_if),
                _ => None,
            })
            .expect("HIR if");
        assert_eq!(hir_if.branches.len(), 3);
        assert!(hir_if.branches[0].condition.is_some());
        assert!(hir_if.branches[1].condition.is_some());
        assert!(hir_if.branches[2].condition.is_none());
        assert_eq!(
            result
                .mir
                .nodes
                .iter()
                .filter(|node| matches!(node.kind, Vue3SsrMirKind::If { .. }))
                .count(),
            3
        );
        let root_if = result
            .mir
            .nodes
            .iter()
            .find(|node| {
                matches!(
                    node.kind,
                    Vue3SsrMirKind::If {
                        condition: Some(_),
                        ..
                    }
                )
            })
            .expect("root if");
        let nested_if = root_if
            .children
            .iter()
            .filter(|child_id| {
                result
                    .mir
                    .node(**child_id)
                    .is_some_and(|child| matches!(child.kind, Vue3SsrMirKind::If { .. }))
            })
            .count();
        assert_eq!(nested_if, 1);
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["ok", "maybe"]
        );
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_basic_pushes_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div>Hello {{ msg }}</div>"#.into(),
            file_id: FileId(43),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert_eq!(generated.ast_summary, "vue3-ssr-mir-nodes=6");
        assert!(generated.code.contains("ssrInterpolate as _ssrInterpolate"));
        assert!(generated.code.contains("ssrRenderAttrs as _ssrRenderAttrs"));
        assert!(generated
            .code
            .contains("export function ssrRender(_ctx, _push, _parent, _attrs)"));
        assert!(generated.code.contains("<div${"));
        assert!(generated.code.contains("_ssrRenderAttrs(_attrs)"));
        assert!(generated.code.contains("}>Hello ${"));
        assert!(generated.code.contains("_ssrInterpolate(_ctx.msg)"));
        assert!(generated.code.contains("}</div>`)"));
        assert!(!generated.code.contains("Vue3Ast"));
    }

    #[test]
    fn generate_vue3_ssr_mir_merges_root_text_and_interpolation_slices() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"foo {{ bar }} baz"#.into(),
            file_id: FileId(43),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("ssrInterpolate: _ssrInterpolate"));
        assert!(generated.code.contains("_push(`foo ${"));
        assert!(generated.code.contains("_ssrInterpolate(_ctx.bar)"));
        assert!(generated.code.contains("} baz`)"));
        assert!(generated
            .code
            .contains(r#"_push(`foo ${_ssrInterpolate(_ctx.bar)} baz`)"#));
        assert!(!generated.code.contains("<!--[-->"));
        assert!(!generated.code.contains("_push(\"foo \")"));
        assert!(!generated.code.contains("_push(_ssrInterpolate(_ctx.bar));"));
    }

    #[test]
    fn generate_vue3_ssr_mir_escapes_text_for_static_html_and_template_literals() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div>&lt;foo&gt;\$bar</div>"#.into(),
            file_id: FileId(43),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("&lt;foo&gt;\\\\\\$bar</div>`"));
        assert!(generated.code.contains("_ssrRenderAttrs(_attrs)"));
        assert!(!generated.code.contains("<foo>"));
    }

    #[test]
    fn generate_vue3_ssr_mir_merges_static_root_fragments() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"foo<!--x--><div></div><span>bar</span>"#.into(),
            file_id: FileId(43),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("_push(`<!--[-->foo<!--x--><div></div><span>bar</span><!--]-->`)"));
        assert!(!generated.code.contains("_push(\"foo\")"));
        assert!(!generated.code.contains("_push(\"<!--x-->\")"));
        assert!(!generated.code.contains("_push(\"<div\")"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_asset_imports_from_mir_root() {
        let mut ast = Vue3Dialect::base_parse(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<img :src="_imports_0">"#.into(),
                file_id: FileId(73),
                base_offset: 0,
            },
            &Vue3CompilerOptions::default(),
        );
        if let Some(root_node) = ast.root_node_mut() {
            if let Vue3AstKind::Root(root) = &mut root_node.kind {
                root.imports.push(vuec_ast::Vue3ImportItem {
                    name: "_imports_0".into(),
                    path: "./logo.png".into(),
                });
            }
        }

        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("import _imports_0 from './logo.png'"));
        assert!(generated
            .code
            .contains("_ssrRenderAttrs(_mergeProps({ src: _imports_0 }, _attrs))"));
        assert!(!generated.code.contains("_ctx._imports_0"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_attrs_payload_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div id="app" :class="klass" :style="style" :title="title" :[name]="value" v-bind="extra">Hi</div>"#.into(),
            file_id: FileId(46),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let attrs = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::RenderAttrs(attrs) => Some(attrs),
                _ => None,
            })
            .expect("ssr attrs");
        assert_eq!(attrs.props.dynamic_bindings.len(), 4);
        assert_eq!(attrs.props.object_bindings.len(), 1);
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["klass", "style", "title", "name", "value", "extra"]
        );
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(!generated.code.contains("normalizeClass as _normalizeClass"));
        assert!(!generated.code.contains("normalizeProps as _normalizeProps"));
        assert!(!generated
            .code
            .contains("guardReactiveProps as _guardReactiveProps"));
        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(generated.code.contains("ssrRenderAttrs as _ssrRenderAttrs"));
        assert!(generated.code.contains("_ssrRenderAttrs(_mergeProps("));
        assert!(generated.code.contains("id: \"app\""));
        assert!(generated.code.contains("class: _ctx.klass"));
        assert!(generated.code.contains("style: _ctx.style"));
        assert!(generated.code.contains("title: _ctx.title"));
        assert!(generated.code.contains("[_ctx.name || \"\"]: _ctx.value"));
        assert!(generated.code.contains("_ctx.extra"));
        assert!(generated.code.contains("_attrs"));
        assert!(generated.code.contains(">Hi</div>`)"));
    }

    #[test]
    fn generate_vue3_ssr_mir_ignores_dom_bind_prefix_modifiers() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div :foo-bar.camel="foo" :id.prop="id" :role.attr="role" :[name].camel="value">Hi</div>"#.into(),
            file_id: FileId(70),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("camelize as _camelize"));
        assert!(!generated.code.contains("normalizeProps as _normalizeProps"));
        assert!(generated.code.contains("ssrRenderAttrs as _ssrRenderAttrs"));
        assert!(generated.code.contains("fooBar: _ctx.foo"));
        assert!(generated.code.contains("\".id\": _ctx.id"));
        assert!(generated.code.contains("\"^role\": _ctx.role"));
        assert!(generated
            .code
            .contains("[_camelize(_ctx.name || \"\")]: _ctx.value"));
        assert!(generated.code.contains("_attrs"));
    }

    #[test]
    fn generate_vue3_ssr_mir_rebuilds_element_attrs_like_public_ssr() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><div key="1" ref="el"></div><div class="foo" :class="bar"></div><input type="checkbox" :checked="checked"><div class="foo" v-bind:[key]="value"></div></div>"#.into(),
            file_id: FileId(74),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(!generated.code.contains(" key="));
        assert!(!generated.code.contains(" ref="));
        assert!(generated
            .code
            .contains(r#"_ssrRenderClass([_ctx.bar, "foo"])"#));
        assert!(generated
            .code
            .contains(r#"(_ssrIncludeBooleanAttr(_ctx.checked)) ? " checked" : """#));
        assert!(generated.code.contains("class: \"foo\""));
        assert!(generated.code.contains("[_ctx.key || \"\"]: _ctx.value"));
        assert!(!generated.code.contains("<div class=\"foo\"${"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_directive_attrs_and_textarea_value_fallback() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source:
                r#"<div><textarea v-bind="obj">fallback</textarea><section v-xxx:x.y="z"/></div>"#
                    .into(),
            file_id: FileId(75),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("resolveDirective: _resolveDirective"));
        assert!(generated
            .code
            .contains("ssrGetDirectiveProps: _ssrGetDirectiveProps"));
        assert!(generated.code.contains("let _temp0"));
        assert!(generated
            .code
            .contains(r#"_ssrRenderAttrs(_temp0 = _ctx.obj, "textarea")"#));
        assert!(generated
            .code
            .contains(r#"_ssrInterpolate(("value" in _temp0) ? _temp0.value : "fallback")"#));
        assert!(generated
            .code
            .contains(r#"_ssrGetDirectiveProps(_ctx, _directive_xxx, _ctx.z, "x", { y: true })"#));
        assert!(generated.code.contains(
            r#"("textContent" in _temp0) ? _ssrInterpolate(_temp0.textContent) : _temp0.innerHTML ?? ''"#
        ));
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_projects_v_show_style_payload() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div id="app" style="color:red" :style="style" v-show="ok">Hi</div>"#.into(),
            file_id: FileId(47),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let attrs = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::RenderAttrs(attrs) => Some(attrs),
                _ => None,
            })
            .expect("ssr attrs");
        assert_eq!(attrs.v_show, Some(JsExprId(1)));
        assert_eq!(attrs.props.static_attrs.len(), 1);
        assert_eq!(attrs.props.static_attrs[0].name, "style");
        assert_eq!(attrs.props.static_attrs[0].value, "color:red");
        assert_eq!(attrs.props.dynamic_bindings.len(), 1);
        assert_eq!(attrs.props.dynamic_bindings[0].name, "style");
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["style", "ok"]
        );
        let pushes = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3SsrMirKind::PushString(value) => Some(value.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(pushes, vec!["<div id=\"app\"", ">", "Hi", "</div>"]);
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_projects_v_model_payloads() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><input v-model="bar"><input type="radio" value="foo" v-model="bar"><input type="checkbox" :true-value="foo" v-model="baz"><input :type="kind" :value="value" v-model="model"><textarea v-model="text">old</textarea><select v-model="picked"><option value="1"></option></select></div>"#.into(),
            file_id: FileId(50),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let models = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3SsrMirKind::RenderAttrs(attrs) => attrs.v_model.as_ref(),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(models.len(), 6);
        assert!(matches!(models[0].kind, Vue3SsrModelKind::InputValue));
        assert!(matches!(
            models[1].kind,
            Vue3SsrModelKind::InputRadio {
                value: MirExpr::String(ref value)
            } if value == "foo"
        ));
        assert!(matches!(
            models[2].kind,
            Vue3SsrModelKind::InputCheckboxTrueValue {
                true_value: MirExpr::JsExpr(JsExprId(2))
            }
        ));
        assert!(matches!(
            models[3].kind,
            Vue3SsrModelKind::InputDynamicType {
                type_expr: JsExprId(4),
                value: MirExpr::JsExpr(JsExprId(5))
            }
        ));
        assert!(matches!(models[4].kind, Vue3SsrModelKind::Textarea));
        assert!(matches!(
            models[5].kind,
            Vue3SsrModelKind::SelectOption {
                value: MirExpr::String(ref value)
            } if value == "1"
        ));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["bar", "bar", "foo", "baz", "kind", "value", "model", "text", "picked"]
        );
        let pushes = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3SsrMirKind::PushString(value) => Some(value.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(!pushes.contains(&"old"));
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_projects_content_override_payloads() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><section v-html="raw">old</section><p v-text="msg"/><span>keep</span></div>"#.into(),
            file_id: FileId(56),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        assert!(result.mir.nodes.iter().any(|node| matches!(
            node.kind,
            Vue3SsrMirKind::RenderContent(Vue3SsrContent::Html {
                expression: JsExprId(0)
            })
        )));
        assert!(result.mir.nodes.iter().any(|node| matches!(
            node.kind,
            Vue3SsrMirKind::RenderContent(Vue3SsrContent::Text {
                expression: JsExprId(1)
            })
        )));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["raw", "msg"]
        );
        let pushes = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3SsrMirKind::PushString(value) => Some(value.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(!pushes.contains(&"old"));
        assert!(pushes.contains(&"<p"));
        assert!(pushes.contains(&">"));
        assert!(pushes.contains(&"</p>"));
        assert!(pushes.contains(&"keep"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_v_show_style_payload_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><span style="color:red" :style="style" v-show="ok"/></div>"#.into(),
            file_id: FileId(48),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("ssrRenderStyle as _ssrRenderStyle"));
        assert!(!generated.code.contains("mergeProps as _mergeProps"));
        assert!(!generated.code.contains("style=\\\"color:red\\\""));
        assert!(generated.code.contains("ssrRenderAttrs as _ssrRenderAttrs"));
        assert!(generated.code.contains("_push(`<div${"));
        assert!(generated.code.contains("<span style=\"${"));
        assert!(generated.code.contains("_ssrRenderStyle([\n      {\"color\":\"red\"},\n      _ctx.style,\n      (_ctx.ok) ? null : { display: \"none\" }\n    ])"));
        assert!(!generated.code.contains("_push(\"<span\");"));
    }

    #[test]
    fn generate_vue3_ssr_mir_merges_v_show_style_after_object_bindings() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source:
                r#"<div><span v-bind="extra" :title="title" style="display:flex" :style="style" v-show="ok"/></div>"#
                    .into(),
            file_id: FileId(49),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(generated.code.contains("ssrRenderAttrs as _ssrRenderAttrs"));
        assert!(!generated
            .code
            .contains("guardReactiveProps as _guardReactiveProps"));
        assert!(!generated.code.contains("_guardReactiveProps("));
        assert!(!generated.code.contains("ssrRenderStyle as _ssrRenderStyle"));
        assert!(!generated.code.contains("ssrRenderAttr as _ssrRenderAttr"));
        assert!(!generated.code.contains("style=\\\"display:flex\\\""));
        assert!(generated.code.contains("_push(`<div${"));
        assert!(generated.code.contains("_ssrRenderAttrs(_mergeProps(_ctx.extra, { title: _ctx.title }, {\n      style: [\n        {\"display\":\"flex\"},\n        _ctx.style,\n        (_ctx.ok) ? null : { display: \"none\" }\n      ]\n    }))"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_v_model_input_payloads_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><input v-model="bar"><input type="radio" value="foo" v-model="bar"><input type="checkbox" :true-value="foo" v-model="baz"><input :type="kind" v-model="foo" :value="value"></div>"#.into(),
            file_id: FileId(51),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("ssrRenderAttr as _ssrRenderAttr"));
        assert!(generated
            .code
            .contains("ssrIncludeBooleanAttr as _ssrIncludeBooleanAttr"));
        assert!(generated.code.contains("ssrLooseEqual as _ssrLooseEqual"));
        assert!(generated
            .code
            .contains("ssrRenderDynamicModel as _ssrRenderDynamicModel"));
        assert!(generated
            .code
            .contains("_ssrRenderAttr(\"value\", _ctx.bar)"));
        assert!(generated.code.contains(
            "(_ssrIncludeBooleanAttr(_ssrLooseEqual(_ctx.bar, \"foo\"))) ? \" checked\" : \"\""
        ));
        assert!(generated.code.contains(
            "(_ssrIncludeBooleanAttr(_ssrLooseEqual(_ctx.baz, _ctx.foo))) ? \" checked\" : \"\""
        ));
        assert!(generated
            .code
            .contains("_ssrRenderDynamicModel(_ctx.kind, _ctx.foo, _ctx.value)"));
        assert!(generated
            .code
            .contains("_ssrRenderAttr(\"value\", _ctx.value)"));
        assert!(!generated.code.contains("true-value"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_textarea_and_select_v_model_payloads() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><textarea v-model="text">old</textarea><select v-model="model"><option value="1"></option><option v-for="item in items" :value="item">{{ item }}</option></select></div>"#.into(),
            file_id: FileId(52),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("ssrInterpolate as _ssrInterpolate"));
        assert!(generated
            .code
            .contains("ssrRenderAttrs as _ssrRenderAttrs, ssrInterpolate as _ssrInterpolate"));
        assert!(generated
            .code
            .contains("ssrIncludeBooleanAttr as _ssrIncludeBooleanAttr"));
        assert!(generated
            .code
            .contains("ssrLooseContain as _ssrLooseContain"));
        assert!(generated.code.contains("ssrLooseEqual as _ssrLooseEqual"));
        assert!(generated.code.contains("_push(`<div${"));
        assert!(generated.code.contains("_ssrInterpolate(_ctx.text)"));
        assert!(!generated
            .code
            .contains("_push(_ssrInterpolate(_ctx.text));"));
        assert!(!generated.code.contains("_push(\"old\");"));
        assert!(generated.code.contains(
            "(_ssrIncludeBooleanAttr((Array.isArray(_ctx.model))\n      ? _ssrLooseContain(_ctx.model, \"1\")\n      : _ssrLooseEqual(_ctx.model, \"1\"))) ? \" selected\" : \"\""
        ));
        assert!(generated
            .code
            .contains("_ssrRenderList(_ctx.items, (item) => {"));
        assert!(generated.code.contains(
            "(_ssrIncludeBooleanAttr((Array.isArray(_ctx.model))\n        ? _ssrLooseContain(_ctx.model, item)\n        : _ssrLooseEqual(_ctx.model, item))) ? \" selected\" : \"\""
        ));
        assert!(!generated.code.contains("_ctx.item)"));
        assert!(!generated.code.contains("_ctx.item,"));
    }

    #[test]
    fn generate_vue3_ssr_mir_merges_input_object_v_model_props() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<input id="x" v-bind="obj" v-model="foo" class="y">"#.into(),
            file_id: FileId(53),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(generated
            .code
            .contains("ssrGetDynamicModelProps as _ssrGetDynamicModelProps"));
        assert!(generated.code.contains("let _temp0"));
        assert!(!generated
            .code
            .contains("_push(\"<input id=\\\"x\\\" class=\\\"y\\\"\");"));
        assert!(generated.code.contains(
            "_push(_ssrRenderAttrs((_temp0 = _mergeProps(_mergeProps({ id: \"x\" }, _ctx.obj, { class: \"y\" }), _attrs), _mergeProps(_temp0, _ssrGetDynamicModelProps(_temp0, _ctx.foo)))));"
        ));
    }

    #[test]
    fn generate_vue3_ssr_mir_merges_dynamic_key_input_v_model_props() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<input :[name]="value" v-model="foo">"#.into(),
            file_id: FileId(54),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(!generated.code.contains("normalizeProps as _normalizeProps"));
        assert!(generated
            .code
            .contains("ssrGetDynamicModelProps as _ssrGetDynamicModelProps"));
        assert!(generated
            .code
            .contains("_temp0 = _mergeProps({ [_ctx.name || \"\"]: _ctx.value }, _attrs)"));
        assert!(generated
            .code
            .contains("{ [_ctx.name || \"\"]: _ctx.value }"));
        assert!(generated.code.contains(", _attrs)"));
        assert!(generated
            .code
            .contains("_mergeProps(_temp0, _ssrGetDynamicModelProps(_temp0, _ctx.foo))"));
    }

    #[test]
    fn generate_vue3_ssr_mir_merges_v_show_input_object_v_model_props() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<input v-bind="obj" style="color:red" v-show="ok" v-model="foo">"#.into(),
            file_id: FileId(55),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(generated.code.contains("ssrRenderAttrs as _ssrRenderAttrs"));
        assert!(generated
            .code
            .contains("ssrGetDynamicModelProps as _ssrGetDynamicModelProps"));
        assert!(generated.code.contains("_push(_ssrRenderAttrs((_temp0 = _mergeProps(_ctx.obj, {\n    style: [\n      {\"color\":\"red\"},\n      (_ctx.ok) ? null : { display: \"none\" }\n    ]\n  }, _attrs), _mergeProps(_temp0, _ssrGetDynamicModelProps(_temp0, _ctx.foo)))));"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_content_override_payloads_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source:
                r#"<div><section v-html="raw">old</section><p v-text="msg"/><span>keep</span></div>"#
                    .into(),
            file_id: FileId(57),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("ssrInterpolate as _ssrInterpolate"));
        assert!(generated.code.contains("(_ctx.raw) ?? ''"));
        assert!(generated.code.contains("_ssrInterpolate(_ctx.msg)"));
        assert!(generated.code.contains("<p>${"));
        assert!(generated.code.contains("}</p>"));
        assert!(!generated.code.contains("_push(\"old\");"));
        assert!(generated.code.contains("<span>keep</span>"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_slot_outlet_payload_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<slot :name="active" foo="bar" :baz="baz">fallback {{ msg }}</slot>"#.into(),
            file_id: FileId(44),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("ssrRenderSlot as _ssrRenderSlot"));
        assert!(generated.code.contains("ssrInterpolate as _ssrInterpolate"));
        assert!(generated
            .code
            .contains("_ssrRenderSlot(_ctx.$slots, _ctx.active, {"));
        assert!(generated.code.contains("foo: \"bar\""));
        assert!(generated.code.contains("baz: _ctx.baz"));
        assert!(generated.code.contains("() => {"));
        assert!(generated
            .code
            .contains("_push(`fallback ${_ssrInterpolate(_ctx.msg)}`)"));
        assert!(!generated.code.contains("name: _ctx.active"));
    }

    #[test]
    fn generate_vue3_ssr_mir_keeps_slot_object_props_as_props() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<slot name="default" v-bind="slotProps" v-on="listeners" :foo="value" />"#
                .into(),
            file_id: FileId(47),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(generated.code.contains("toHandlers as _toHandlers"));
        assert!(generated
            .code
            .contains("_ssrRenderSlot(_ctx.$slots, \"default\", _normalizeProps("));
        assert!(generated.code.contains("_mergeProps("));
        assert!(generated.code.contains("_ctx.slotProps"));
        assert!(generated.code.contains("_toHandlers(_ctx.listeners, true)"));
        assert!(generated.code.contains("foo: _ctx.value"));
        assert!(!generated.code.contains("_ssrRenderAttrs(_ctx.slotProps)"));
        assert!(!generated.code.contains("ssrRenderAttrs as _ssrRenderAttrs"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_dynamic_component_vnode_path() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<component :is="view" prop="b" v-bind="attrs"><span>hi</span></component><component is="plain" />"#.into(),
            file_id: FileId(78),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let components = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3SsrMirKind::RenderComponent(component) => Some(component),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(components.len(), 2);
        assert!(components.iter().all(|component| component.dynamic));
        assert!(components.iter().all(|component| component
            .props
            .static_attrs
            .iter()
            .all(|attr| attr.name != "is")));
        assert!(components.iter().all(|component| component
            .props
            .dynamic_bindings
            .iter()
            .all(|binding| binding.name != "is")));

        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("resolveDynamicComponent as _resolveDynamicComponent"));
        assert!(generated.code.contains("createVNode as _createVNode"));
        assert!(generated.code.contains("ssrRenderVNode as _ssrRenderVNode"));
        assert!(!generated
            .code
            .contains("ssrRenderComponent as _ssrRenderComponent"));
        assert!(generated
            .code
            .contains("_ssrRenderVNode(_push, _createVNode(_resolveDynamicComponent(_ctx.view),"));
        assert!(generated
            .code
            .contains("_createVNode(_resolveDynamicComponent(\"plain\"), null, null)"));
        assert!(generated.code.contains("prop: \"b\""));
        assert!(generated.code.contains("_ctx.attrs"));
        assert!(!generated.code.contains("is: _ctx.view"));
        assert!(!generated.code.contains("is: \"plain\""));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_control_flow_and_components_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Comp foo="bar" :baz="baz" v-bind="extra"><p v-if="ok">yes</p><p v-else-if="maybe">maybe</p><p v-else>no</p><li v-for="item in list">{{ item.name }}</li></Comp>"#.into(),
            file_id: FileId(45),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let component = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::RenderComponent(component) => Some(component),
                _ => None,
            })
            .expect("ssr component");
        assert_eq!(component.props.static_attrs.len(), 1);
        assert_eq!(component.props.static_attrs[0].name, "foo");
        assert_eq!(component.props.dynamic_bindings.len(), 1);
        assert_eq!(component.props.dynamic_bindings[0].name, "baz");
        assert_eq!(component.props.object_bindings.len(), 1);
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("ssrRenderComponent as _ssrRenderComponent"));
        assert!(generated
            .code
            .contains("resolveComponent as _resolveComponent"));
        assert!(generated.code.contains("withCtx as _withCtx"));
        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(generated.code.contains("ssrRenderList as _ssrRenderList"));
        assert!(generated
            .code
            .contains("const _component_Comp = _resolveComponent(\"Comp\")"));
        assert!(generated
            .code
            .contains("_push(_ssrRenderComponent(_component_Comp, _mergeProps("));
        assert!(generated
            .code
            .contains("default: _withCtx((_, _push, _parent, _scopeId) => {"));
        assert!(generated.code.contains("_mergeProps({"));
        assert!(generated.code.contains("foo: \"bar\""));
        assert!(generated.code.contains("baz: _ctx.baz"));
        assert!(generated.code.contains("_ctx.extra"));
        assert!(generated.code.contains("if (_ctx.ok) {"));
        assert!(generated.code.contains("} else if (_ctx.maybe) {"));
        assert!(!generated.code.contains("} else {\n        if (_ctx.maybe)"));
        let ok_offset = generated.code.find("if (_ctx.ok)").expect("ok branch");
        let maybe_offset = generated
            .code
            .find("} else if (_ctx.maybe)")
            .expect("maybe branch");
        let yes_offset = generated
            .code
            .find("_push(`<p${_scopeId}>yes</p>`)")
            .expect("yes body");
        let maybe_body_offset = generated
            .code
            .find("_push(`<p${_scopeId}>maybe</p>`)")
            .expect("maybe body");
        let no_offset = generated
            .code
            .find("_push(`<p${_scopeId}>no</p>`)")
            .expect("else body");
        assert!(ok_offset < yes_offset);
        assert!(yes_offset < maybe_offset);
        assert!(maybe_offset < maybe_body_offset);
        assert!(maybe_body_offset < no_offset);
        assert!(generated
            .code
            .contains("_ssrRenderList(_ctx.list, (item) => {"));
        assert!(generated.code.contains("_ssrInterpolate(item.name)"));
        assert!(!generated.code.contains("_ctx.item"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_template_v_if_fragments_and_root_attrs() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<template v-if="foo"><div>hi</div><div>ho</div></template><div v-else/>"#
                .into(),
            file_id: FileId(82),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("if (_ctx.foo) {"));
        assert!(generated
            .code
            .contains("_push(`<!--[--><div>hi</div><div>ho</div><!--]-->`)"));
        assert!(generated.code.contains("} else {"));
        assert!(generated
            .code
            .contains("_push(`<div${_ssrRenderAttrs(_attrs)}></div>`)"));
        assert!(!generated.code.contains("<template"));
    }

    #[test]
    fn generate_vue3_ssr_mir_injects_root_attrs_through_if_branches() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-if="foo"></div><span v-else-if="bar"></span><p v-else></p>"#.into(),
            file_id: FileId(83),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("ssrRenderAttrs as _ssrRenderAttrs"));
        assert!(generated.code.contains("if (_ctx.foo) {"));
        assert!(generated.code.contains("} else if (_ctx.bar) {"));
        assert!(generated
            .code
            .contains("_push(`<div${_ssrRenderAttrs(_attrs)}></div>`)"));
        assert!(generated
            .code
            .contains("_push(`<span${_ssrRenderAttrs(_attrs)}></span>`)"));
        assert!(generated
            .code
            .contains("_push(`<p${_ssrRenderAttrs(_attrs)}></p>`)"));
    }

    #[test]
    fn generate_vue3_ssr_mir_injects_fallthrough_attrs_through_root_comments() {
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            built_in_components: vec!["transition".into()],
            ..Vue3CompilerOptions::default()
        };
        let render = |source: &str, file_id: u32| {
            let source = TemplateSource {
                filename: "foo.vue".into(),
                source: source.into(),
                file_id: FileId(file_id),
                base_offset: 0,
            };
            let ast = Vue3Dialect::base_parse(source, &options);
            let result = lower_vue3_ast_to_ssr_mir(&ast, &options);
            generate_vue3_ssr_mir(&result.mir, &result.js, &options).code
        };

        let commented_root = render(r#"<!--!--><div/>"#, 84);
        assert!(commented_root.contains("ssrRenderAttrs as _ssrRenderAttrs"));
        assert!(commented_root
            .contains(r#"_push(`<!--[--><!--!--><div${_ssrRenderAttrs(_attrs)}></div><!--]-->`)"#));

        let transition_root = render(r#"<!--root--><transition><div/></transition>"#, 85);
        assert!(transition_root.contains(
            r#"_push(`<!--[--><!--root--><div${_ssrRenderAttrs(_attrs)}></div><!--]-->`)"#
        ));

        let dynamic_fragment = render(r#"<div v-if="true"/><div/>"#, 86);
        assert!(!dynamic_fragment.contains("_ssrRenderAttrs(_attrs)"));
        assert!(dynamic_fragment.contains(r#"_push(`<div></div><!--]-->`)"#));
    }

    #[test]
    fn generate_vue3_ssr_mir_injects_css_vars_across_fragments_and_suspense() {
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            ssr_css_vars: Some("{ color }".into()),
            ..Vue3CompilerOptions::default()
        };
        let render = |source: &str, file_id: u32| {
            let source = TemplateSource {
                filename: "foo.vue".into(),
                source: source.into(),
                file_id: FileId(file_id),
                base_offset: 0,
            };
            let ast = Vue3Dialect::base_parse(source, &options);
            let result = lower_vue3_ast_to_ssr_mir(&ast, &options);
            generate_vue3_ssr_mir(&result.mir, &result.js, &options).code
        };

        let basic = render(r#"<div/>"#, 87);
        assert!(basic.contains("const _cssVars = { style: { color: _ctx.color }}"));
        assert!(basic.contains(r#"_ssrRenderAttrs(_mergeProps(_attrs, _cssVars))"#));

        let multiline_options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            ssr_css_vars: Some("{\n  \":--x-color\": (color),\n  \":--x-size\": (size)\n}".into()),
            ..Vue3CompilerOptions::default()
        };
        let multiline_source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div/>"#.into(),
            file_id: FileId(91),
            base_offset: 0,
        };
        let multiline_ast = Vue3Dialect::base_parse(multiline_source, &multiline_options);
        let multiline = lower_vue3_ast_to_ssr_mir(&multiline_ast, &multiline_options);
        let multiline_code =
            generate_vue3_ssr_mir(&multiline.mir, &multiline.js, &multiline_options).code;
        assert!(multiline_code.contains(
            "const _cssVars = { style: {\n  \":--x-color\": (_ctx.color),\n  \":--x-size\": (_ctx.size)\n}}"
        ));

        let fragment = render(r#"<div/><div/>"#, 88);
        assert_eq!(fragment.matches("_ssrRenderAttrs(_cssVars)").count(), 2);
        assert!(!fragment.contains("_mergeProps(_attrs, _cssVars)"));

        let branches = render(
            r#"<div v-if="ok"/><template v-else><div/><div/></template>"#,
            89,
        );
        assert!(branches.contains(r#"_ssrRenderAttrs(_mergeProps(_attrs, _cssVars))"#));
        assert_eq!(branches.matches("_ssrRenderAttrs(_cssVars)").count(), 2);

        let suspense = render(
            r#"<Suspense><div>ok</div><template #fallback><div>fallback</div></template></Suspense>"#,
            90,
        );
        assert!(suspense.contains(r#"_push(`<div${_ssrRenderAttrs(_cssVars)}>fallback</div>`)"#));
        assert!(suspense.contains(r#"_push(`<div${_ssrRenderAttrs(_cssVars)}>ok</div>`)"#));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_component_slot_vnode_asset_fallback() {
        let mut ast = Vue3Dialect::base_parse(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<Comp><img :src="_imports_0"></Comp>"#.into(),
                file_id: FileId(76),
                base_offset: 0,
            },
            &Vue3CompilerOptions::default(),
        );
        if let Some(root_node) = ast.root_node_mut() {
            if let Vue3AstKind::Root(root) = &mut root_node.kind {
                root.imports.push(vuec_ast::Vue3ImportItem {
                    name: "_imports_0".into(),
                    path: "./logo.png".into(),
                });
            }
        }

        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("import { resolveComponent as _resolveComponent, withCtx as _withCtx, createVNode as _createVNode } from \"vue\""));
        assert!(generated
            .code
            .contains("import _imports_0 from './logo.png'"));
        assert!(generated
            .code
            .contains("const _component_Comp = _resolveComponent(\"Comp\")"));
        assert!(generated
            .code
            .contains("_ssrRenderAttr(\"src\", _imports_0)"));
        assert!(generated
            .code
            .contains("_createVNode(\"img\", { src: _imports_0 })"));
        assert!(!generated.code.contains("_ctx._imports_0"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_scope_id_slot_vnode_fallback() {
        let text_source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<foo>foo</foo>"#.into(),
            file_id: FileId(77),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            scope_id: Some("data-v-xxxxxxx".into()),
            native_tags: Some(vec!["span".into(), "div".into()]),
            ..Vue3CompilerOptions::default()
        };
        let text_ast = Vue3Dialect::base_parse(text_source, &options);
        let text_result = lower_vue3_ast_to_ssr_mir(&text_ast, &options);
        let text_generated = generate_vue3_ssr_mir(&text_result.mir, &text_result.js, &options);
        assert!(text_generated
            .code
            .contains("createTextVNode as _createTextVNode"));
        assert!(text_generated
            .code
            .contains("return [\n          _createTextVNode(\"foo\")\n        ]"));
        assert!(!text_generated
            .code
            .contains("return [\n          \"foo\"\n        ]"));

        let nested_source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<foo><span>hello</span><bar><span/></bar></foo>"#.into(),
            file_id: FileId(78),
            base_offset: 0,
        };
        let nested_ast = Vue3Dialect::base_parse(nested_source, &options);
        let nested_result = lower_vue3_ast_to_ssr_mir(&nested_ast, &options);
        let nested_generated =
            generate_vue3_ssr_mir(&nested_result.mir, &nested_result.js, &options);
        assert!(nested_generated
            .code
            .contains("_push(`<span data-v-xxxxxxx${_scopeId}>hello</span>`)"));
        assert!(nested_generated.code.contains("}, _parent, _scopeId)"));
        assert!(nested_generated
            .code
            .contains("_createVNode(\"span\", null, \"hello\")"));
        assert!(nested_generated.code.contains(
            "_createVNode(_component_bar, null, {\n            default: _withCtx(() => ["
        ));
        assert!(nested_generated.code.contains("_createVNode(\"span\")"));
        assert!(!nested_generated.code.contains("\"data-v-xxxxxxx\": \"\""));
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_projects_component_slots_and_directives() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<foo v-xxx:x.y="z"><template v-slot:named v-if="ok">foo</template><template v-for="(key, index) in names" v-slot:[key]="{ msg }">{{ msg + key + index + bar }}</template></foo>"#.into(),
            file_id: FileId(92),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            native_tags: Some(vec!["div".into()]),
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_ssr_mir(&ast, &options);

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let component = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::RenderComponent(component) => Some(component),
                _ => None,
            })
            .expect("ssr component");

        assert_eq!(component.directives.len(), 1);
        assert_eq!(component.directives[0].name, "xxx");
        assert_eq!(component.directives[0].argument.as_deref(), Some("x"));
        assert_eq!(component.directives[0].modifiers, vec!["y"]);
        assert!(component.directives[0].expression.is_some());

        let slots = component.slots.as_ref().expect("component slots");
        assert_eq!(slots.flag, Vue3SlotFlag::Dynamic);
        assert!(slots.dynamic_slots.iter().any(|slot| matches!(
            slot,
            vuec_ast::Vue3DomDynamicSlot::Conditional(slot)
                if slot.condition.is_some()
                    && matches!(slot.slot.name, Vue3DomSlotName::Static(ref name) if name == "named")
                    && slot.slot.key.as_deref() == Some("0")
        )));
        assert!(slots.dynamic_slots.iter().any(|slot| matches!(
            slot,
            vuec_ast::Vue3DomDynamicSlot::For(slot)
                if matches!(slot.slot.name, Vue3DomSlotName::Dynamic(_))
                    && slot.key_alias.is_some()
                    && slot.index_alias.is_none()
                    && slot.slot.params.is_some()
        )));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_component_slots_and_directives() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<foo v-xxx:x.y="z"><template v-slot:named v-if="ok">foo</template><template v-for="(key, index) in names" v-slot:[key]="{ msg }">{{ msg + key + index + bar }}</template></foo>"#.into(),
            file_id: FileId(93),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            scope_id: Some("data-v-test".into()),
            native_tags: Some(vec!["div".into()]),
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_ssr_mir(&ast, &options);
        let generated = generate_vue3_ssr_mir(&result.mir, &result.js, &options);

        assert!(generated
            .code
            .contains("resolveDirective: _resolveDirective"));
        assert!(generated
            .code
            .contains("ssrGetDirectiveProps: _ssrGetDirectiveProps"));
        assert!(generated
            .code
            .contains(r#"_ssrGetDirectiveProps(_ctx, _directive_xxx, _ctx.z, "x", { y: true })"#));
        assert!(generated
            .code
            .contains("_createSlots({ _: 2 /* DYNAMIC */ }, ["));
        assert!(generated
            .code
            .contains("(_ctx.ok)\n      ? {\n          name: \"named\",\n          fn: _withCtx"));
        assert!(generated
            .code
            .contains("_renderList(_ctx.names, (key, index) => {"));
        assert!(generated
            .code
            .contains("_push(`${_ssrInterpolate(msg + key + index + _ctx.bar)}`)"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_transition_vnode_fallback_in_component_slot() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<foo><transition><div v-if="false"/></transition></foo>"#.into(),
            file_id: FileId(94),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            scope_id: Some("data-v-test".into()),
            native_tags: Some(vec!["div".into()]),
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_ssr_mir(&ast, &options);
        let generated = generate_vue3_ssr_mir(&result.mir, &result.js, &options);

        assert!(generated
            .code
            .contains("_push(`<div data-v-test${_scopeId}></div>`)"));
        assert!(generated.code.contains("_createVNode(_Transition, null, {"));
        assert!(generated.code.contains("default: _withCtx(() => ["));
        assert!(generated
            .code
            .contains(r#"_openBlock(), _createBlock("div", { key: 0 })"#));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_v_for_alias_payload_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<li v-for="(item, key, index) in list">{{ key }}:{{ index }}:{{ item.name }}</li>"#.into(),
            file_id: FileId(48),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("ssrRenderList as _ssrRenderList"));
        assert!(generated
            .code
            .contains("_ssrRenderList(_ctx.list, (item, key, index) => {"));
        assert!(generated.code.contains("_ssrInterpolate(key)"));
        assert!(generated.code.contains("_ssrInterpolate(index)"));
        assert!(generated.code.contains("_ssrInterpolate(item.name)"));
        assert!(!generated.code.contains("_ctx.key"));
        assert!(!generated.code.contains("_ctx.index"));
        assert!(!generated.code.contains("_ctx.item"));
    }

    #[test]
    fn generate_vue3_ssr_mir_wraps_v_for_fragments_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-for="row, i in list"><div v-for="j in row">{{ i }},{{ j }}</div></div><template v-for="item in items"><span>{{ item }}</span></template>"#.into(),
            file_id: FileId(85),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("_push(`<!--[-->`)\n  _ssrRenderList(_ctx.list, (row, i) => {"));
        assert!(generated
            .code
            .contains("_push(`<div><!--[-->`)\n    _ssrRenderList(row, (j) => {"));
        assert!(generated.code.contains("_push(`<!--]--></div>`)"));
        assert!(generated
            .code
            .contains("_push(`<!--[-->`)\n  _ssrRenderList(_ctx.items, (item) => {"));
        assert!(generated
            .code
            .contains("_push(`<span>${_ssrInterpolate(item)}</span>`)"));
        assert!(!generated.code.contains("<template"));
        assert!(!generated.code.contains("_ctx.row"));
        assert!(!generated.code.contains("_ctx.j"));
        assert!(!generated.code.contains("_ssrInterpolate(_ctx.item)"));
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_flattens_transition_group_artifacts() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<TransitionGroup tag="ul"><template v-for="item in list"><li>{{ item }}</li></template><span v-if="ok">ok</span><!--x--></TransitionGroup><template v-for="entry in entries"><i>{{ entry }}</i></template>"#.into(),
            file_id: FileId(86),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));

        let for_nodes = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3SsrMirKind::For(for_mir) => Some((
                    result.js.expressions()[for_mir.source.0 as usize]
                        .source
                        .as_str(),
                    for_mir.fragment,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(for_nodes.len(), 2);
        assert!(
            for_nodes
                .iter()
                .any(|(source, fragment)| *source == "list" && !fragment),
            "{for_nodes:?}"
        );
        assert!(for_nodes
            .iter()
            .any(|(source, fragment)| *source == "entries" && *fragment));

        let if_node = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::If {
                    condition: Some(condition),
                    comment,
                } if result.js.expressions()[condition.0 as usize].source == "ok" => Some(comment),
                _ => None,
            })
            .expect("transition-group if");
        assert!(!if_node);

        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("_ssrRenderList(_ctx.list, (item) => {"));
        assert!(generated
            .code
            .contains("_push(`<li>${_ssrInterpolate(item)}</li>`)"));
        assert!(!generated.code.contains("<ul><!--[-->"));
        assert!(!generated.code.contains("<!--x-->"));
        assert!(generated
            .code
            .contains("_ssrRenderList(_ctx.entries, (entry) => {"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_transition_group_public_shapes() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Transition appear><div>foo</div></Transition><TransitionGroup :tag="someTag" class="red"><span>hello</span></TransitionGroup>"#.into(),
            file_id: FileId(87),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            scope_id: Some("data-v-xxxxxxx".into()),
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_ssr_mir(&ast, &options);
        let generated = generate_vue3_ssr_mir(&result.mir, &result.js, &options);

        assert!(
            generated
                .code
                .contains("<template><div data-v-xxxxxxx>foo</div></template>"),
            "{}",
            generated.code
        );
        assert!(generated.code.contains("_ctx.someTag"));
        assert!(generated
            .code
            .contains("_ssrRenderAttrs({ class: \"red\" })"));
        assert!(
            generated
                .code
                .contains("} data-v-xxxxxxx><span data-v-xxxxxxx>hello</span>"),
            "{}",
            generated.code
        );
        assert!(!generated.code.contains("data-vuec-slotted"));
        assert!(!generated.code.contains("<#expr"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_builtin_component_payloads_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r##"<Teleport to="#modal" disabled><div id="x"/></Teleport><Suspense><template #default><Foo/></template><template #fallback>loading</template></Suspense>"##.into(),
            file_id: FileId(49),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("ssrRenderTeleport as _ssrRenderTeleport"));
        assert!(generated
            .code
            .contains("ssrRenderSuspense as _ssrRenderSuspense"));
        assert!(generated
            .code
            .contains("_ssrRenderTeleport(_push, (_push) => {"));
        assert!(generated.code.contains("}, \"#modal\", true, _parent)"));
        assert!(!generated.code.contains("}, \"#modal\", true, _parent);"));
        assert!(generated.code.contains("withCtx as _withCtx"));
        assert!(generated.code.contains("_ssrRenderSuspense(_push, {"));
        assert!(generated.code.contains("default: () => {"));
        assert!(generated.code.contains("fallback: () => {"));
        assert!(generated.code.contains("_: 1 /* STABLE */"));
        assert!(generated.code.contains("_push(`loading`)"));
        assert!(!generated.code.contains("_push(\"loading\");"));
        assert!(!generated.code.contains("});"));
        assert!(!generated.code.contains("_ssrRenderComponent(\"Teleport\""));
        assert!(!generated.code.contains("_ssrRenderComponent(\"Suspense\""));
    }

    #[test]
    fn transform_collects_root_helpers_components_and_directives() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Child v-focus><slot/><Transition/><component :is="view"/><div v-show="ok" :class="klass">{{ msg }}</div></Child>"#.into(),
            file_id: FileId(20),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions::default();
        let mut ast = Vue3Dialect::base_parse(source, &options);
        let mut ctx = TransformContext::default();
        Vue3Dialect::transform(&mut ast, &mut ctx, &options);

        let root = ast
            .root_node()
            .and_then(|node| match &node.kind {
                Vue3AstKind::Root(root) => Some(root),
                _ => None,
            })
            .expect("Vue3 root");

        assert!(root.components.contains("Child"));
        assert_eq!(root.directives.iter().collect::<Vec<_>>(), vec!["focus"]);
        for helper in [
            RuntimeHelper::Vue3OpenBlock,
            RuntimeHelper::Vue3CreateElementBlock,
            RuntimeHelper::Vue3CreateElementVNode,
            RuntimeHelper::Vue3RenderSlot,
            RuntimeHelper::Vue3ResolveComponent,
            RuntimeHelper::Vue3ResolveDynamicComponent,
            RuntimeHelper::Vue3Transition,
            RuntimeHelper::Vue3WithDirectives,
            RuntimeHelper::Vue3ResolveDirective,
            RuntimeHelper::Vue3VShow,
            RuntimeHelper::Vue3NormalizeClass,
            RuntimeHelper::Vue3ToDisplayString,
        ] {
            assert!(
                root.helpers.contains(&helper),
                "missing root helper {helper:?}"
            );
            assert!(
                ctx.helpers.contains(&helper),
                "missing ctx helper {helper:?}"
            );
        }

        let projected = ast.project_public();
        let projected_root = match &projected.kind {
            Vue3AstKind::Root(root) => root,
            _ => panic!("projected root"),
        };
        assert_eq!(projected_root.components, root.components);
        assert_eq!(projected_root.directives, root.directives);
        assert_eq!(projected_root.helpers, root.helpers);
    }

    #[test]
    fn transform_root_collection_keeps_structural_directives_out_of_runtime_assets() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<ul><li v-for="item in list" v-if="item.ok" v-once v-memo="[item.id]">{{ item.name }}</li></ul>"#.into(),
            file_id: FileId(21),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions::default();
        let mut ast = Vue3Dialect::base_parse(source, &options);
        let mut ctx = TransformContext::default();
        Vue3Dialect::transform(&mut ast, &mut ctx, &options);

        let root = ast
            .root_node()
            .and_then(|node| match &node.kind {
                Vue3AstKind::Root(root) => Some(root),
                _ => None,
            })
            .expect("Vue3 root");

        assert!(root.components.is_empty());
        assert!(root.directives.is_empty());
        for helper in [
            RuntimeHelper::Vue3Fragment,
            RuntimeHelper::Vue3RenderList,
            RuntimeHelper::Vue3CreateCommentVNode,
            RuntimeHelper::Vue3WithMemo,
            RuntimeHelper::Vue3IsMemoSame,
        ] {
            assert!(
                root.helpers.contains(&helper),
                "missing root helper {helper:?}"
            );
            assert!(
                ctx.helpers.contains(&helper),
                "missing ctx helper {helper:?}"
            );
        }
        assert!(!root.helpers.contains(&RuntimeHelper::Vue3ResolveDirective));
        assert!(!root.helpers.contains(&RuntimeHelper::Vue3WithDirectives));
    }

    #[test]
    fn base_compile_wraps_multiple_root_children_in_fragment_block() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div/><span/>"#.into(),
                file_id: FileId(92),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("Fragment as _Fragment"));
        assert!(result
            .code
            .contains("return (_openBlock(), _createElementBlock(_Fragment, null, ["));
        assert!(result.code.contains("_createElementVNode(\"div\")"));
        assert!(result.code.contains("_createElementVNode(\"span\")"));
        assert!(result.code.contains("64 /* STABLE_FRAGMENT */"));
    }

    #[test]
    fn generate_consumes_transformed_root_component_state() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><Child/><Transition/><component :is="view"/></div>"#.into(),
            file_id: FileId(22),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions::default();
        let mut ast = Vue3Dialect::base_parse(source, &options);
        let mut ctx = TransformContext::default();
        Vue3Dialect::transform(&mut ast, &mut ctx, &options);

        let result = Vue3Dialect::generate(
            &ast,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
            &TransformContext::default(),
        );

        assert!(result
            .code
            .contains("resolveComponent as _resolveComponent"));
        assert!(result
            .code
            .contains("resolveDynamicComponent as _resolveDynamicComponent"));
        assert!(result.code.contains("Transition as _Transition"));
        assert!(result
            .code
            .contains("const _component_Child = _resolveComponent(\"Child\")"));
        assert!(!result.code.contains("_component_Transition"));
        assert!(result.code.contains("_createVNode(_Transition"));
        assert!(result
            .code
            .contains("_createVNode(_resolveDynamicComponent(_ctx.view)"));
    }

    #[test]
    fn generate_preserves_raw_ast_component_scan_fallback() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<Child/>".into(),
            file_id: FileId(23),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = Vue3Dialect::generate(
            &ast,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
            &TransformContext::default(),
        );

        assert!(result
            .code
            .contains("resolveComponent as _resolveComponent"));
        assert!(result
            .code
            .contains("const _component_Child = _resolveComponent(\"Child\")"));
        assert!(result.code.contains("_createBlock(_component_Child"));
    }

    #[test]
    fn generate_preserves_raw_ast_runtime_directive_scan_fallback() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-focus:foo.bar="value"/>"#.into(),
            file_id: FileId(24),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = Vue3Dialect::generate(
            &ast,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
            &TransformContext::default(),
        );

        assert!(result.code.contains("withDirectives as _withDirectives"));
        assert!(result
            .code
            .contains("resolveDirective as _resolveDirective"));
        assert!(result
            .code
            .contains("const _directive_focus = _resolveDirective(\"focus\")"));
        assert!(result
            .code
            .contains("[_directive_focus, _ctx.value, \"foo\", {"));
        assert!(result.code.contains("512 /* NEED_PATCH */"));
    }

    #[test]
    fn generate_consumes_transformed_root_runtime_directive_state() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-focus:foo.bar="value"><span v-show="ok"/></div>"#.into(),
            file_id: FileId(25),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions::default();
        let mut ast = Vue3Dialect::base_parse(source, &options);
        let mut ctx = TransformContext::default();
        Vue3Dialect::transform(&mut ast, &mut ctx, &options);

        let result = Vue3Dialect::generate(
            &ast,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
            &TransformContext::default(),
        );

        assert!(result.code.contains("withDirectives as _withDirectives"));
        assert!(result
            .code
            .contains("resolveDirective as _resolveDirective"));
        assert!(result.code.contains("vShow as _vShow"));
        assert!(result
            .code
            .contains("const _directive_focus = _resolveDirective(\"focus\")"));
        assert!(result
            .code
            .contains("[_directive_focus, _ctx.value, \"foo\", {"));
        assert!(result.code.contains("bar: true"));
        assert!(result.code.contains("[_vShow, _ctx.ok]"));
        assert!(result.code.contains("512 /* NEED_PATCH */"));
    }

    fn process_expression_test_projection(content: &str, context: Value) -> Value {
        process_expression_projection(&json!({
            "node": {
                "type": 4,
                "content": content,
                "isStatic": false,
                "loc": {
                    "start": { "offset": 0, "line": 1, "column": 1 },
                    "end": { "offset": content.len(), "line": 1, "column": content.len() + 1 },
                    "source": content
                }
            },
            "context": context
        }))
    }

    fn process_expression_test_statement_projection(content: &str, context: Value) -> Value {
        process_expression_projection(&json!({
            "node": {
                "type": 4,
                "content": content,
                "isStatic": false,
                "loc": {
                    "start": { "offset": 0, "line": 1, "column": 1 },
                    "end": { "offset": content.len(), "line": 1, "column": content.len() + 1 },
                    "source": content
                }
            },
            "context": context,
            "asRawStatements": true
        }))
    }

    fn projection_code(value: &Value) -> String {
        match json_str(value, "kind") {
            Some("simple") => json_str(value, "content").unwrap_or("").to_string(),
            Some("compound") => value
                .get("children")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[])
                .iter()
                .map(projection_code)
                .collect::<String>(),
            _ => value.as_str().unwrap_or_default().to_string(),
        }
    }

    #[test]
    fn process_expression_projection_prefixes_simple_identifier() {
        let projection = process_expression_test_projection(
            "foo",
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }),
        );

        assert_eq!(projection["kind"], json!("simple"));
        assert_eq!(projection["content"], json!("_ctx.foo"));
        assert_eq!(projection["constType"], json!(0));
    }

    #[test]
    fn vue3_utils_projection_advances_positions_with_utf16_offsets() {
        let projection = advance_position_with_clone_projection(&json!({
            "pos": { "offset": 2, "line": 3, "column": 4 },
            "source": "a😏\nb",
            "numberOfCharacters": 4,
        }));

        assert_eq!(projection["offset"], json!(6));
        assert_eq!(projection["line"], json!(4));
        assert_eq!(projection["column"], json!(1));
    }

    #[test]
    fn vue3_utils_projection_generates_official_asset_ids() {
        assert_eq!(
            to_valid_asset_id_projection(&json!({
                "name": "test-测试-1",
                "type": "component",
            }))["id"],
            json!("_component_test_2797935797_1")
        );
        assert_eq!(
            to_valid_asset_id_projection(&json!({
                "name": "a😏-b",
                "type": "directive",
            }))["id"],
            json!("_directive_a5535756847_b")
        );
    }

    #[test]
    fn process_expression_projection_prefixes_object_shorthand_value() {
        let projection = process_expression_test_projection(
            "{ foo }",
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }),
        );

        assert_eq!(projection["kind"], json!("compound"));
        assert_eq!(projection["children"][0], json!("{ foo: "));
        assert_eq!(projection["children"][1]["content"], json!("_ctx.foo"));
        assert_eq!(projection["children"][2], json!(" }"));
    }

    #[test]
    fn process_expression_projection_keeps_static_object_literal_simple() {
        let projection = process_expression_test_projection(
            "{ foo: true }",
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }),
        );

        assert_eq!(projection["kind"], json!("setConstType"));
        assert_eq!(projection["constType"], json!(3));
    }

    #[test]
    fn process_expression_projection_preserves_slot_params_as_bindings() {
        let projection = process_expression_projection(&json!({
            "node": {
                "type": 4,
                "content": "{ foo }",
                "isStatic": false,
                "loc": {
                    "start": { "offset": 0, "line": 1, "column": 1 },
                    "end": { "offset": 7, "line": 1, "column": 8 },
                    "source": "{ foo }"
                }
            },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} },
            "asParams": true
        }));

        assert_eq!(projection["kind"], json!("compound"));
        assert_eq!(projection["children"][0], json!("{ "));
        assert_eq!(projection["children"][1]["content"], json!("foo"));
        assert_eq!(projection["children"][2], json!(" }"));
        assert_eq!(projection["identifiers"], json!(["foo"]));
    }

    #[test]
    fn process_expression_projection_keeps_arrow_params_scoped_to_arrow_body() {
        let projection = process_expression_test_projection(
            "{ a: foo => foo, b: foo }",
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }),
        );
        let children = &projection["children"];

        assert_eq!(projection["kind"], json!("compound"));
        assert_eq!(children[0], json!("{ a: "));
        assert_eq!(children[1]["content"], json!("foo"));
        assert_eq!(children[2], json!(" => "));
        assert_eq!(children[3]["content"], json!("foo"));
        assert_eq!(children[4], json!(", b: "));
        assert_eq!(children[5]["content"], json!("_ctx.foo"));
        assert_eq!(children[6], json!(" }"));
    }

    #[test]
    fn process_expression_projection_rewrites_setup_let_assignment_rhs() {
        let projection = process_expression_test_projection(
            "(async () => { x = await bar })()",
            json!({
                "prefixIdentifiers": true,
                "inline": true,
                "identifiers": {},
                "bindingMetadata": {
                    "x": "setup-let",
                    "bar": "setup-const"
                }
            }),
        );

        assert_eq!(projection["kind"], json!("compound"));
        assert_eq!(
            projection["children"][1]["content"],
            json!("_isRef(x) ? x.value = await bar : x")
        );
        assert_eq!(projection["children"][3]["content"], json!("bar"));
        assert_eq!(projection["helpers"], json!(["IS_REF"]));
    }

    #[test]
    fn process_expression_projection_rewrites_inline_assignment_update_and_destructure() {
        let context = json!({
            "prefixIdentifiers": true,
            "inline": true,
            "identifiers": {},
            "bindingMetadata": {
                "count": "setup-ref",
                "maybe": "setup-maybe-ref",
                "lett": "setup-let",
                "val": "setup-const"
            }
        });

        let assignment = process_expression_test_statement_projection(
            "count = 1; maybe = count; lett += count",
            context.clone(),
        );
        let code = projection_code(&assignment);
        assert!(code.contains("count.value = 1"), "{code}");
        assert!(code.contains("maybe.value = count.value"), "{code}");
        assert!(
            code.contains("_isRef(lett) ? lett.value += count.value : lett += count.value"),
            "{code}"
        );
        assert_eq!(assignment["helpers"], json!(["IS_REF"]));

        let update = process_expression_test_statement_projection(
            "count++; --maybe; lett--",
            context.clone(),
        );
        let code = projection_code(&update);
        assert!(code.contains("count.value++"), "{code}");
        assert!(code.contains("--maybe.value"), "{code}");
        assert!(
            code.contains("_isRef(lett) ? lett.value-- : lett--"),
            "{code}"
        );

        let destructure = process_expression_test_statement_projection(
            "({ count } = val); [maybe] = val; ({ lett } = val)",
            context,
        );
        let code = projection_code(&destructure);
        assert!(code.contains("({ count: count.value } = val)"), "{code}");
        assert!(code.contains("[maybe.value] = val"), "{code}");
        assert!(code.contains("({ lett: lett } = val)"), "{code}");
    }

    #[test]
    fn js_ast_extract_identifiers_projects_destructure_patterns() {
        let pattern = json!({
            "type": "ObjectPattern",
            "properties": [{
                "type": "ObjectProperty",
                "key": { "type": "Identifier", "name": "foo", "start": 8, "end": 11 },
                "value": {
                    "type": "AssignmentPattern",
                    "left": { "type": "Identifier", "name": "bar", "start": 15, "end": 18 },
                    "right": { "type": "Identifier", "name": "baz", "start": 21, "end": 24 }
                },
                "computed": false,
                "shorthand": false
            }, {
                "type": "RestElement",
                "argument": { "type": "Identifier", "name": "rest", "start": 29, "end": 33 }
            }]
        });
        let projection = extract_identifiers_projection(&json!({ "node": pattern }));
        let names = projection["identifiers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["bar", "rest"]);
    }

    #[test]
    fn js_ast_function_type_projection_recognizes_babel_function_nodes() {
        for kind in [
            "FunctionDeclaration",
            "FunctionExpression",
            "ArrowFunctionExpression",
            "ObjectMethod",
            "ClassMethod",
            "ClassPrivateMethod",
        ] {
            let projection = is_function_type_projection(&json!({
                "node": { "type": kind }
            }));
            assert_eq!(projection["isFunctionType"], json!(true), "{kind}");
        }

        let projection = is_function_type_projection(&json!({
            "node": { "type": "ObjectProperty" }
        }));
        assert_eq!(projection["isFunctionType"], json!(false));
    }

    #[test]
    fn js_ast_reference_projection_excludes_declarations_and_params() {
        let parent_stack = vec![json!({
            "type": "VariableDeclaration",
            "kind": "const"
        })];
        let parent = json!({
            "type": "VariableDeclarator",
            "id": { "type": "Identifier", "name": "foo" },
            "init": { "type": "Identifier", "name": "bar" }
        });
        let id = json!({ "type": "Identifier", "name": "foo" });
        assert!(!js_ast_is_referenced_identifier(
            &id,
            &parent,
            &parent_stack,
            Some("id")
        ));
        let id = json!({ "type": "Identifier", "name": "bar" });
        assert!(js_ast_is_referenced_identifier(
            &id,
            &parent,
            &parent_stack,
            Some("init")
        ));

        let function = json!({
            "type": "FunctionDeclaration",
            "id": { "type": "Identifier", "name": "test" },
            "params": [{ "type": "Identifier", "name": "foo" }],
            "body": { "type": "BlockStatement", "body": [] }
        });
        let id = json!({ "type": "Identifier", "name": "foo" });
        assert!(!js_ast_is_referenced_identifier(
            &id,
            &function,
            &[],
            Some("params")
        ));
    }

    #[test]
    fn js_ast_reference_projection_excludes_destructured_function_params() {
        let id = json!({ "type": "Identifier", "name": "title" });
        let property = json!({
            "type": "ObjectProperty",
            "key": { "type": "Identifier", "name": "title" },
            "value": id,
            "computed": false,
            "shorthand": true
        });
        let pattern = json!({
            "type": "ObjectPattern",
            "properties": [property]
        });
        let function = json!({
            "type": "ArrowFunctionExpression",
            "params": [pattern],
            "body": { "type": "ArrayExpression", "elements": [] }
        });
        let statement = json!({
            "type": "ExpressionStatement",
            "expression": function
        });
        let parent = &statement["expression"]["params"][0]["properties"][0];
        let parent_stack = vec![
            statement.clone(),
            statement["expression"].clone(),
            statement["expression"]["params"][0].clone(),
            parent.clone(),
        ];

        assert!(!js_ast_is_referenced_identifier(
            &statement["expression"]["params"][0]["properties"][0]["key"],
            parent,
            &parent_stack,
            Some("key")
        ));
        assert!(!js_ast_is_referenced_identifier(
            &statement["expression"]["params"][0]["properties"][0]["value"],
            parent,
            &parent_stack,
            Some("value")
        ));
    }

    #[test]
    fn js_ast_walk_identifiers_respects_function_and_for_locals() {
        let root = json!({
            "type": "Program",
            "body": [{
                "type": "FunctionDeclaration",
                "id": { "type": "Identifier", "name": "test" },
                "params": [{ "type": "Identifier", "name": "foo", "start": 14, "end": 17 }],
                "body": {
                    "type": "BlockStatement",
                    "body": [{
                        "type": "ExpressionStatement",
                        "expression": {
                            "type": "CallExpression",
                            "callee": { "type": "Identifier", "name": "console", "start": 27, "end": 34 },
                            "arguments": [{ "type": "Identifier", "name": "foo", "start": 39, "end": 42 }]
                        }
                    }]
                }
            }, {
                "type": "ForStatement",
                "init": {
                    "type": "VariableDeclaration",
                    "kind": "let",
                    "declarations": [{
                        "type": "VariableDeclarator",
                        "id": { "type": "Identifier", "name": "i", "start": 55, "end": 56 },
                        "init": { "type": "NumericLiteral", "value": 0 }
                    }]
                },
                "test": {
                    "type": "BinaryExpression",
                    "left": { "type": "Identifier", "name": "i", "start": 63, "end": 64 },
                    "right": { "type": "Identifier", "name": "len", "start": 67, "end": 70 }
                },
                "update": {
                    "type": "UpdateExpression",
                    "argument": { "type": "Identifier", "name": "i", "start": 72, "end": 73 },
                    "operator": "++",
                    "prefix": false
                },
                "body": { "type": "BlockStatement", "body": [] }
            }]
        });
        let projection = walk_identifiers_projection(&json!({ "root": root }));
        let names = projection["identifiers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["console", "len"]);
    }

    #[test]
    fn js_ast_destructure_assignment_detects_object_pattern_assignment() {
        let parent = json!({
            "type": "ObjectProperty",
            "key": { "type": "Identifier", "name": "foo" },
            "value": { "type": "Identifier", "name": "foo" },
            "computed": false,
            "shorthand": true
        });
        let stack = vec![
            json!({
                "type": "AssignmentExpression",
                "left": {
                    "type": "ObjectPattern",
                    "properties": [parent.clone()]
                }
            }),
            json!({
                "type": "ObjectPattern",
                "properties": [parent.clone()]
            }),
        ];
        assert!(js_ast_is_in_destructure_assignment(&parent, &stack));
    }

    #[test]
    fn transform_expression_projection_processes_interpolation_content() {
        let projection = transform_expression_projection(&json!({
            "node": {
                "type": 5,
                "content": {
                    "type": 4,
                    "content": "foo",
                    "isStatic": false
                }
            },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }
        }));

        assert_eq!(projection["operations"][0]["path"], json!(["content"]));
        assert_eq!(
            projection["operations"][0]["projection"]["content"],
            json!("_ctx.foo")
        );
    }

    #[test]
    fn transform_expression_projection_processes_slot_params_but_skips_on_handlers() {
        let projection = transform_expression_projection(&json!({
            "node": {
                "type": 1,
                "props": [
                    {
                        "type": 7,
                        "name": "slot",
                        "arg": { "type": 4, "content": "foo", "isStatic": true },
                        "exp": { "type": 4, "content": "{ bar }", "isStatic": false }
                    },
                    {
                        "type": 7,
                        "name": "on",
                        "arg": { "type": 4, "content": "click", "isStatic": true },
                        "exp": { "type": 4, "content": "submit", "isStatic": false }
                    },
                    {
                        "type": 7,
                        "name": "bind",
                        "arg": { "type": 4, "content": "name", "isStatic": false },
                        "exp": { "type": 4, "content": "value", "isStatic": false }
                    }
                ]
            },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }
        }));

        let operations = projection["operations"].as_array().expect("operations");
        assert_eq!(operations.len(), 3);
        assert_eq!(operations[0]["path"], json!(["props", "0", "exp"]));
        assert_eq!(operations[0]["projection"]["identifiers"], json!(["bar"]));
        assert_eq!(operations[1]["path"], json!(["props", "2", "exp"]));
        assert_eq!(operations[2]["path"], json!(["props", "2", "arg"]));
    }

    #[test]
    fn transform_expression_projection_skips_v_memo_key_expression() {
        let projection = transform_expression_projection(&json!({
            "node": {
                "type": 1,
                "props": [
                    { "type": 7, "name": "memo" },
                    {
                        "type": 7,
                        "name": "bind",
                        "arg": { "type": 4, "content": "key", "isStatic": true },
                        "exp": { "type": 4, "content": "item.id", "isStatic": false }
                    }
                ]
            },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }
        }));

        assert_eq!(projection["operations"], json!([]));
    }

    #[test]
    fn transform_once_projection_enters_once_node() {
        let projection = transform_once_projection(&json!({
            "node": {
                "type": 1,
                "props": [{ "type": 7, "name": "once" }]
            },
            "context": {}
        }));

        assert_eq!(projection["kind"], json!("enter"));
        assert_eq!(projection["helper"], json!("SET_BLOCK_TRACKING"));
        assert_eq!(projection["exit"]["cacheCodegen"], json!(true));
        assert_eq!(projection["exit"]["inVOnce"], json!(true));
    }

    #[test]
    fn transform_once_projection_skips_seen_nested_and_ssr_nodes() {
        let node = json!({
            "type": 1,
            "props": [{ "type": 7, "name": "once" }]
        });

        assert_eq!(
            transform_once_projection(&json!({ "node": node, "context": {}, "seen": true }))
                ["kind"],
            json!("noop")
        );
        assert_eq!(
            transform_once_projection(&json!({ "node": node, "context": { "inVOnce": true } }))
                ["kind"],
            json!("noop")
        );
        assert_eq!(
            transform_once_projection(&json!({ "node": node, "context": { "inSSR": true } }))
                ["kind"],
            json!("noop")
        );
    }

    #[test]
    fn transform_memo_projection_enters_plain_element() {
        let projection = transform_memo_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 0,
                "props": [{
                    "type": 7,
                    "name": "memo",
                    "exp": { "type": 4, "content": "[x]", "isStatic": false }
                }]
            },
            "context": { "cachedLength": 3 }
        }));

        assert_eq!(projection["kind"], json!("enter"));
        assert_eq!(projection["exit"]["helper"], json!("WITH_MEMO"));
        assert_eq!(projection["exit"]["convertToBlock"], json!(true));
        assert_eq!(projection["exit"]["cacheIndex"], json!(3));
        assert_eq!(projection["exit"]["exp"]["content"], json!("[x]"));
    }

    #[test]
    fn transform_memo_projection_keeps_component_vnode_shape() {
        let projection = transform_memo_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 1,
                "props": [{
                    "type": 7,
                    "name": "memo",
                    "exp": { "type": 4, "content": "[x]", "isStatic": false }
                }]
            },
            "context": {}
        }));

        assert_eq!(projection["kind"], json!("enter"));
        assert_eq!(projection["exit"]["convertToBlock"], json!(false));
    }

    #[test]
    fn transform_memo_projection_skips_seen_ssr_and_empty_nodes() {
        let node = json!({
            "type": 1,
            "tagType": 0,
            "props": [{
                "type": 7,
                "name": "memo",
                "exp": { "type": 4, "content": "[x]", "isStatic": false }
            }]
        });
        assert_eq!(
            transform_memo_projection(&json!({ "node": node, "context": {}, "seen": true }))
                ["kind"],
            json!("noop")
        );
        assert_eq!(
            transform_memo_projection(&json!({ "node": node, "context": { "inSSR": true } }))
                ["kind"],
            json!("noop")
        );
        assert_eq!(
            transform_memo_projection(&json!({
                "node": { "type": 1, "tagType": 0, "props": [{ "type": 7, "name": "memo" }] },
                "context": {}
            }))["kind"],
            json!("noop")
        );
    }

    #[test]
    fn base_compile_uses_binding_metadata_for_prefixed_interpolations() {
        let mut options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "function".into(),
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("props".into(), "props".into());
        options
            .binding_metadata
            .insert("setup".into(), "setup-maybe-ref".into());
        options
            .binding_metadata
            .insert("literal".into(), "literal-const".into());
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div>{{ props }} {{ setup }} {{ literal }}</div>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(source, options);
        assert!(result.code.contains("$props.props"));
        assert!(result.code.contains("$setup.setup"));
        assert!(result.code.contains("$setup.literal"));
    }

    #[test]
    fn base_compile_source_map_maps_inline_setup_ref_interpolation() {
        let mut options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            inline: true,
            source_map: true,
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("count".into(), "setup-ref".into());
        let source = "<button>{{ count }}</button>";
        let result = base_compile(
            TemplateSource {
                filename: "FooBar.vue".into(),
                source: source.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(result.code.contains("count.value"));
        let generated_offset = result
            .code
            .find("count.value")
            .expect("generated count ref");
        let generated = loc_for_offset(&result.code, generated_offset).expect("generated loc");
        let original = result
            .map
            .expect("source map")
            .original_position(vuec_source::GeneratedPosition::new(
                generated.0,
                generated.1,
            ))
            .expect("source map lookup")
            .expect("original position");
        let expected = loc_for_offset(source, source.find("count").expect("source count"))
            .expect("source loc");
        assert_eq!(original.source, "FooBar.vue");
        assert_eq!((original.line, original.column), expected);
        assert_eq!(original.name.as_deref(), Some("count"));
    }

    #[test]
    fn base_compile_source_map_maps_inline_setup_ref_static_bind_expression() {
        let mut options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            inline: true,
            source_map: true,
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("count".into(), "setup-ref".into());
        let source = r#"<button :id="count"></button>"#;
        let result = base_compile(
            TemplateSource {
                filename: "FooBar.vue".into(),
                source: source.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(result.code.contains("count.value"));
        let generated_offset = result
            .code
            .find("count.value")
            .expect("generated count ref");
        let generated = loc_for_offset(&result.code, generated_offset).expect("generated loc");
        let original = result
            .map
            .expect("source map")
            .original_position(vuec_source::GeneratedPosition::new(
                generated.0,
                generated.1,
            ))
            .expect("source map lookup")
            .expect("original position");
        let expected = loc_for_offset(source, source.find("count").expect("source count"))
            .expect("source loc");
        assert_eq!(original.source, "FooBar.vue");
        assert_eq!((original.line, original.column), expected);
        assert_eq!(original.name.as_deref(), Some("count"));
    }

    #[test]
    fn base_compile_source_map_maps_inline_setup_ref_interpolation_in_sfc_source() {
        let sfc_source = concat!(
            "<script setup>\n",
            "const count = ref(0)\n",
            "</script>\n",
            "<template><button>{{ count }}</button></template>"
        );
        let template_source = "<button>{{ count }}</button>";
        let template_start = sfc_source.find(template_source).expect("template content");
        let mut options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            inline: true,
            source_map: true,
            source_map_source: Some(sfc_source.into()),
            source_map_base_offset: 0,
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("count".into(), "setup-ref".into());
        let result = base_compile(
            TemplateSource {
                filename: "FooBar.vue".into(),
                source: template_source.into(),
                file_id: FileId(0),
                base_offset: template_start,
            },
            options,
        );

        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(result.code.contains("count.value"));
        let generated_offset = result
            .code
            .find("count.value")
            .expect("generated count ref");
        let generated = loc_for_offset(&result.code, generated_offset).expect("generated loc");
        let original = result
            .map
            .expect("source map")
            .original_position(vuec_source::GeneratedPosition::new(
                generated.0,
                generated.1,
            ))
            .expect("source map lookup")
            .expect("original position");
        let expected = loc_for_offset(
            sfc_source,
            sfc_source.find("count }}").expect("source count"),
        )
        .expect("source loc");
        assert_eq!(original.source, "FooBar.vue");
        assert_eq!((original.line, original.column), expected);
        assert_eq!(original.name.as_deref(), Some("count"));
    }

    #[test]
    fn base_compile_prefixes_template_literal_placeholders() {
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            ..Vue3CompilerOptions::default()
        };

        let interpolation = base_compile(
            TemplateSource {
                filename: "literal.vue".into(),
                source: r#"<div>{{ `Hello ${msg}` }}</div>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options.clone(),
        );
        assert!(interpolation.diagnostics.is_empty());
        assert!(interpolation
            .code
            .contains("_toDisplayString(`Hello ${_ctx.msg}`)"));

        let directive = base_compile(
            TemplateSource {
                filename: "literal.vue".into(),
                source: r#"<div :title="`Hello ${msg}`"></div>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options.clone(),
        );
        assert!(directive.diagnostics.is_empty());
        assert!(directive.code.contains("title: `Hello ${_ctx.msg}`"));

        let scoped = base_compile(
            TemplateSource {
                filename: "literal.vue".into(),
                source: r#"<div v-for="item in rows">{{ `${item}:${msg}` }}</div>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );
        assert!(scoped.diagnostics.is_empty());
        assert!(scoped.code.contains("`${item}:${_ctx.msg}`"));
    }

    #[test]
    fn base_compile_accepts_v_for_of_expression_with_v_memo() {
        let result = base_compile(
            TemplateSource {
                filename: "memo.vue".into(),
                source:
                    r#"<span v-for="data of tableData" :key="getId(data)" v-memo="getLetter(data)"></span>"#
                        .into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(result.code.contains("_renderList(_ctx.tableData, (data"));
        assert!(result.code.contains("const _memo = (_ctx.getLetter(data))"));
    }

    #[test]
    fn base_compile_reports_v_for_structural_expression_diagnostics() {
        let missing = base_compile(
            TemplateSource {
                filename: "bad.vue".into(),
                source: r#"<span v-for />"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions::default(),
        );
        assert!(missing
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "31"));

        let malformed = base_compile(
            TemplateSource {
                filename: "bad.vue".into(),
                source: r#"<span v-for="item in" />"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions::default(),
        );
        assert!(malformed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "32"));
        assert!(!malformed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "46"));
    }

    #[test]
    fn base_compile_reports_structural_parser_diagnostics() {
        let missing = base_compile(
            TemplateSource {
                filename: "bad.vue".into(),
                source: "<div><span></div>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions::default(),
        );
        assert!(missing
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "24"
                && diagnostic.message == "Element is missing end tag."));

        let invalid = base_compile(
            TemplateSource {
                filename: "bad.vue".into(),
                source: "</span><div></div>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions::default(),
        );
        assert!(invalid
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "23" && diagnostic.message == "Invalid end tag."));
    }

    #[test]
    fn base_compile_uses_props_namespace_for_inline_component_member_tag() {
        let mut options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "function".into(),
            inline: true,
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("Foo".into(), "props".into());
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<Foo.Bar/>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };

        let result = base_compile(source, options);

        assert!(result
            .code
            .contains("_createBlock(_unref(__props[\"Foo\"]).Bar)"));
        assert!(result.code.contains("unref"));
        assert!(!result.code.contains("_component_Foo46Bar"));
    }

    #[test]
    fn base_compile_uses_inline_setup_imports_for_component_and_directive_assets() {
        let mut options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            inline: true,
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("ChildComp".into(), "setup-const".into());
        options
            .binding_metadata
            .insert("SomeOtherComp".into(), "setup-const".into());
        options
            .binding_metadata
            .insert("vMyDir".into(), "setup-maybe-ref".into());
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div><div v-my-dir/><ChildComp/><some-other-comp/></div>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("[_unref(vMyDir)]"));
        assert!(result.code.contains("_createVNode(ChildComp)"));
        assert!(result.code.contains("_createVNode(SomeOtherComp)"));
        assert!(!result.code.contains("_resolveDirective(\"my-dir\")"));
        assert!(!result.code.contains("_resolveComponent(\"ChildComp\")"));
        assert!(!result
            .code
            .contains("_resolveComponent(\"some-other-comp\")"));
    }

    #[test]
    fn base_compile_caches_root_static_siblings_in_inline_hoist_mode() {
        let mut options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            inline: true,
            hoist_static: true,
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("count".into(), "setup-ref".into());
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: "<div>{{ count }}</div><div>static</div>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains(
            "_cache[0] || (_cache[0] = _createElementVNode(\"div\", null, \"static\", -1 /* CACHED */))"
        ));
    }

    #[test]
    fn base_compile_does_not_cache_dynamic_interpolation_subtrees() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: "<template><div>{{ msg }}</div></template>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "module".into(),
                hoist_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("_toDisplayString(_ctx.msg)"));
        assert!(result.code.contains("1 /* TEXT */"));
        assert!(!result.code.contains("-1 /* CACHED */"));
        assert!(!result.code.contains("_cache[0] || (_cache[0] = ["));
    }

    #[test]
    fn base_compile_wraps_unref_constructor_targets_in_new_expressions() {
        let mut options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            inline: true,
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("Foo".into(), "setup-maybe-ref".into());
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: "<div>{{ new Foo() }}</div><div>{{ new Foo.Bar() }}</div>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("new (_unref(Foo))()"));
        assert!(result.code.contains("new (_unref(Foo)).Bar()"));
    }

    #[test]
    fn base_compile_skips_patch_props_for_inline_setup_const_handlers() {
        let mut options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            inline: true,
            cache_handlers: true,
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("fn".into(), "setup-const".into());
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div @click="fn"/>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("{ onClick: fn }"));
        assert!(!result.code.contains("PROPS"));
        assert!(!result.code.contains("[\"onClick\"]"));
    }

    #[test]
    fn base_compile_reports_directive_expression_parse_errors() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div :bar="a["/>"#.into(),
            file_id: FileId(0),
            base_offset: 0,
        };

        let result = base_compile(
            source,
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );

        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "46");
        assert!(result.diagnostics[0]
            .message
            .contains("Error parsing JavaScript expression: Unexpected token"));
        assert_eq!(
            result.diagnostics[0].span,
            Some(Span::new(FileId(0), 13, 13))
        );
    }

    #[test]
    fn base_compile_rewrites_event_handler_statement_scopes() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div @click=\"() => {\n        for (const x in list) {\n          log(x)\n        }\n        error(x)\n      }\"/>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "function".into(),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result.code.contains("for (const x in _ctx.list)"));
        assert!(result.code.contains("_ctx.log(x)"));
        assert!(result.code.contains("_ctx.error(_ctx.x)"));
    }

    #[test]
    fn base_compile_maps_event_handler_statement_identifiers() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<button @click="throw new Error(`msg`);"></button>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "module".into(),
                source_map: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.diagnostics.is_empty());
        assert!(result.code.contains("throw new Error(`msg`);"));
        let map = result.map.expect("source map");
        assert!(map.names.contains(&"Error".into()));
    }

    #[test]
    fn base_compile_caches_native_event_handlers_with_cache_handlers() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div><button @click="foo"/><button @click="foo($event)"/><button @[event]="run"/><Comp @save="save"/><Comp @submit="submit($event)"/><p v-once @click="once">once</p></div>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                cache_handlers: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("toHandlerKey as _toHandlerKey"));
        assert!(result.code.contains(
            "onClick: _cache[0] || (_cache[0] = (...args) => (_ctx.foo && _ctx.foo(...args)))"
        ));
        assert!(result
            .code
            .contains("onClick: _cache[1] || (_cache[1] = $event => (_ctx.foo($event)))"));
        assert!(result.code.contains(
            "[_toHandlerKey(_ctx.event)]: _cache[2] || (_cache[2] = (...args) => (_ctx.run && _ctx.run(...args)))"
        ));
        assert!(result.code.contains("onSave: _ctx.save"));
        assert!(result
            .code
            .contains("onSubmit: _cache[3] || (_cache[3] = $event => (_ctx.submit($event)))"));
        assert!(result.code.contains("onClick: _ctx.once"));
        assert!(result.code.contains("16 /* FULL_PROPS */"));
        assert!(result.code.contains("8 /* PROPS */, [\"onClick\"]"));
    }

    #[test]
    fn base_compile_merges_static_and_dynamic_native_events() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<input @blur="onBlur" @[validateEvent]="onValidateEvent">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                cache_handlers: true,
                hoist_static: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains(
            r#"import { toHandlerKey as _toHandlerKey, mergeProps as _mergeProps, openBlock as _openBlock, createElementBlock as _createElementBlock } from "vue""#
        ));
        assert!(result.code.contains(
            "onBlur: _cache[0] || (_cache[0] = (...args) => (_ctx.onBlur && _ctx.onBlur(...args)))"
        ));
        assert!(result.code.contains("[_toHandlerKey(_ctx.validateEvent)]: _cache[1] || (_cache[1] = (...args) => (_ctx.onValidateEvent && _ctx.onValidateEvent(...args)))"));
        assert!(result.code.contains("_mergeProps({\n"));
        assert!(result.code.contains("  }, {\n"));
        assert!(result.code.contains("16 /* FULL_PROPS */"));
        assert!(!result.code.contains("\"data-vuec-dom\""));
    }

    #[test]
    fn base_compile_emits_object_v_bind_merge_props() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div><p v-bind="obj"/><section id="x" v-bind="base" :class="cls" :style="style" :foo="bar"/></div>"#
                    .into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                cache_handlers: true,
                hoist_static: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("normalizeProps as _normalizeProps"));
        assert!(result
            .code
            .contains("guardReactiveProps as _guardReactiveProps"));
        assert!(result.code.contains("mergeProps as _mergeProps"));
        assert!(result
            .code
            .contains("_normalizeProps(_guardReactiveProps(_ctx.obj))"));
        assert!(result.code.contains(
            "_mergeProps({ id: \"x\" }, _ctx.base, {\n      class: _ctx.cls,\n      style: _ctx.style,\n      foo: _ctx.bar\n    })"
        ));
        assert!(result.code.contains("16 /* FULL_PROPS */"));
        assert!(result.code.contains("[\"foo\"]"));
        assert!(!result.code.contains("_normalizeClass(_ctx.cls)"));
        assert!(!result.code.contains("_normalizeStyle(_ctx.style)"));
        assert!(!result.code.contains("[\"style\"]"));
    }

    #[test]
    fn base_compile_emits_dom_content_directive_props() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div><section v-html="raw">old</section><p v-text="msg">old</p><span v-text="'hi'" v-bind="after"/></div>"#
                    .into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("toDisplayString as _toDisplayString"));
        assert!(result.code.contains("{ innerHTML: _ctx.raw }"));
        assert!(result
            .code
            .contains("textContent: _toDisplayString(_ctx.msg)"));
        assert!(result
            .code
            .contains("_mergeProps({ textContent: 'hi' }, _ctx.after)"));
        assert!(result.code.contains("8 /* PROPS */, [\"innerHTML\"]"));
        assert!(result.code.contains("8 /* PROPS */, [\"textContent\"]"));
        assert!(!result.code.contains("\"old\""));
        assert!(!result.code.contains("_toDisplayString('hi')"));
    }

    #[test]
    fn base_compile_keeps_component_merge_dynamic_props() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<Comp v-bind="obj" :class="cls"/><Comp v-bind="obj" :style="style"/>"#
                    .into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                cache_handlers: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result
            .code
            .contains("_mergeProps(_ctx.obj, { class: _ctx.cls })"));
        assert!(result
            .code
            .contains("_mergeProps(_ctx.obj, { style: _ctx.style })"));
        assert!(result.code.contains("16 /* FULL_PROPS */, [\"class\"]"));
        assert!(result.code.contains("16 /* FULL_PROPS */, [\"style\"]"));
    }

    #[test]
    fn base_compile_uses_class_and_style_patch_flags_for_native_props() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div :class="cls" :style="style"/><Comp :class="cls" :foo="foo"/>"#
                    .into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("normalizeClass as _normalizeClass"));
        assert!(result.code.contains("normalizeStyle as _normalizeStyle"));
        assert!(result.code.contains("class: _normalizeClass(_ctx.cls)"));
        assert!(result.code.contains("style: _normalizeStyle(_ctx.style)"));
        assert!(result.code.contains("6 /* CLASS, STYLE */"));
        assert!(result.code.contains("8 /* PROPS */, [\"class\", \"foo\"]"));
    }

    #[test]
    fn base_compile_normalizes_static_style_comments_for_native_props() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source:
                    "<div style=\"/* before */ width: 300px; height: 100px/* after */\">{{ render }}</div>"
                        .into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result
            .code
            .contains(r#"style: {"width":"300px","height":"100px"}"#));
        assert!(!result.code.contains("/* before */"));
        assert!(!result.code.contains("/* after */"));
    }

    #[test]
    fn base_compile_emits_object_v_on_to_handlers_for_native_and_component() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div v-on="listeners"/><Comp v-on="listeners"/>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                cache_handlers: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("toHandlers as _toHandlers"));
        assert!(result
            .code
            .contains("\"div\", _toHandlers(_ctx.listeners, true)"));
        assert!(result
            .code
            .contains("_component_Comp, _toHandlers(_ctx.listeners)"));
        assert!(!result.code.contains("on: _ctx.listeners"));
        assert!(!result.code.contains("on: _cache"));
    }

    #[test]
    fn base_compile_preserves_merge_props_order_and_cache_slots() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div :foo="bar" v-bind="obj" v-on="listeners" @click="foo"/>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                cache_handlers: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("mergeProps as _mergeProps"));
        assert!(result.code.contains("toHandlers as _toHandlers"));
        assert!(result.code.contains(
            "_mergeProps({ foo: _ctx.bar }, _ctx.obj, _toHandlers(_ctx.listeners, true), {"
        ));
        assert!(result.code.contains(
            "onClick: _cache[0] || (_cache[0] = (...args) => (_ctx.foo && _ctx.foo(...args)))"
        ));
        assert!(result.code.contains("16 /* FULL_PROPS */, [\"foo\"]"));
        assert!(!result.code.contains("_cache[1]"));
    }

    #[test]
    fn base_compile_normalizes_dynamic_bind_args() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div :class="cls" :[key]="value" @click="foo"/><Comp :[name].camel="value"/>"#
                    .into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                cache_handlers: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("normalizeProps as _normalizeProps"));
        assert!(result.code.contains("camelize as _camelize"));
        assert!(result.code.contains("class: _ctx.cls"));
        assert!(result.code.contains("[_ctx.key || \"\"]: _ctx.value"));
        assert!(result
            .code
            .contains("[_camelize(_ctx.name || \"\")]: _ctx.value"));
        assert!(result.code.contains("onClick: _cache[0] || (_cache[0] ="));
        assert!(result.code.contains("16 /* FULL_PROPS */"));
        assert!(!result.code.contains("_normalizeClass(_ctx.cls)"));
        assert!(!result.code.contains("[\"key\"]"));
        assert!(!result.code.contains("[\"name\"]"));
    }

    #[test]
    fn base_compile_merges_slot_outlet_props() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<slot v-bind="slotProps" v-on="listeners" :foo="value"/><slot :[name]="value" :bar="bar"/>"#
                    .into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("renderSlot as _renderSlot"));
        assert!(result.code.contains("mergeProps as _mergeProps"));
        assert!(result.code.contains("toHandlers as _toHandlers"));
        assert!(result.code.contains("normalizeProps as _normalizeProps"));
        assert!(result.code.contains(
            "_renderSlot(_ctx.$slots, \"default\", _mergeProps(_ctx.slotProps, _toHandlers(_ctx.listeners, true), { foo: _ctx.value }))"
        ));
        assert!(result
            .code
            .contains("_renderSlot(_ctx.$slots, \"default\", _normalizeProps({"));
        assert!(result.code.contains("[_ctx.name || \"\"]: _ctx.value"));
        assert!(result.code.contains("bar: _ctx.bar"));
        assert!(!result
            .code
            .contains("_renderSlot(_ctx.$slots, _ctx.value, _normalizeProps"));
    }

    #[test]
    fn base_compile_wraps_v_once_nodes_with_block_tracking_cache() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div><p v-once @click="foo"><span>hello</span></p></div>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                cache_handlers: true,
                hoist_static: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result
            .code
            .contains("setBlockTracking as _setBlockTracking"));
        assert!(result.code.contains("_cache[0] || ("));
        assert!(result.code.contains("_setBlockTracking(-1, true)"));
        assert!(result
            .code
            .contains("(_cache[0] = _createElementVNode(\"p\", {"));
        assert!(result.code.contains("onClick: _ctx.foo"));
        assert!(result.code.contains(")).cacheIndex = 0"));
        assert!(result.code.contains("_setBlockTracking(1)"));
        assert!(result.code.contains(
            "_cache[1] || (_cache[1] = _createElementVNode(\"span\", null, \"hello\", -1 /* CACHED */))"
        ));
        assert!(!result.code.contains("_cache[0] = (...args)"));
    }

    #[test]
    fn base_compile_keeps_v_once_memo_static_cache_indexes_distinct() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<section v-memo="[x]" v-once><span>hello</span></section>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                cache_handlers: true,
                hoist_static: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("return _cache[1] || ("));
        assert!(result
            .code
            .contains("_withMemo([_ctx.x], () => (_openBlock(), _createElementBlock(\"section\""));
        assert!(result.code.contains(", _cache, 0)"));
        assert!(result.code.contains("(_cache[1] = _withMemo"));
        assert!(result.code.contains(
            "_cache[2] || (_cache[2] = _createElementVNode(\"span\", null, \"hello\", -1 /* CACHED */))"
        ));
        assert!(!result.code.contains("_cache[0] || ("));
    }

    #[test]
    fn base_compile_wraps_v_for_v_once_around_fragment() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div><p v-for="item in list" v-once>{{ item }}</p></div>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                cache_handlers: true,
                hoist_static: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("_cache[0] || ("));
        assert!(result
            .code
            .contains("(_cache[0] = (_openBlock(true), _createElementBlock(_Fragment"));
        assert!(result.code.contains("_renderList(_ctx.list, (item) => {"));
        assert!(result.code.contains(")).cacheIndex = 0"));
    }

    #[test]
    fn base_compile_wraps_v_if_v_once_around_chain() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div><p v-if="ok" v-once>{{ msg }}</p><p v-else>no</p></div>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                cache_handlers: true,
                hoist_static: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("_cache[0] || ("));
        assert!(result.code.contains("(_cache[0] = (_ctx.ok)"));
        assert!(result
            .code
            .contains("? (_openBlock(), _createElementBlock(\"p\", { key: 0 }"));
        assert!(result
            .code
            .contains(": (_openBlock(), _createElementBlock(\"p\", { key: 1 }"));
        assert!(result.code.contains(")).cacheIndex = 0"));
    }

    #[test]
    fn base_compile_keeps_scoped_event_handlers_uncached_with_dynamic_props() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div><button v-for="item in list" @click="select(item)"/></div>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                cache_handlers: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result
            .code
            .contains("onClick: $event => (_ctx.select(item))"));
        assert!(result.code.contains("8 /* PROPS */, [\"onClick\"]"));
        assert!(!result
            .code
            .contains("_cache[0] || (_cache[0] = $event => (_ctx.select(item)))"));
    }

    #[test]
    fn base_compile_marks_vnode_hook_need_patch() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div @vue:updated="foo" />"#.into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                prefix_identifiers: true,
                cache_handlers: true,
                mode: "function".into(),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result.code.contains(
            "onVnodeUpdated: _cache[0] || (_cache[0] = (...args) => (_ctx.foo && _ctx.foo(...args)))"
        ));
        assert!(result.code.contains("512 /* NEED_PATCH */"));
        assert!(!result.code.contains("onVue:updated"));
        assert!(!result.code.contains(r#"["onVnodeUpdated"]"#));
    }

    #[test]
    fn base_compile_shares_cache_handler_slots_with_memo_and_static_cache() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source:
                    r#"<div><button @click="go"/><section v-memo="[x]"><div><div>hello</div><div>hello</div></div></section></div>"#
                        .into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                cache_handlers: true,
                hoist_static: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains(
            "onClick: _cache[0] || (_cache[0] = (...args) => (_ctx.go && _ctx.go(...args)))"
        ));
        assert!(result
            .code
            .contains(r#"_withMemo([_ctx.x], () => (_openBlock(), _createElementBlock("section""#));
        assert!(result.code.contains(", _cache, 1)"));
        assert!(result
            .code
            .contains("_cache[2] || (_cache[2] = [_createElementVNode"));
    }

    #[test]
    fn base_compile_generates_core_integration_directives() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div id="foo" :class="bar.baz">
  {{ world.burn() }}
  <div v-if="ok">yes</div>
  <template v-else>no</template>
  <div v-for="(value, index) in list"><span>{{ value + index }}</span></div>
</div>"#
                .into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                source_map: true,
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result.code.contains("class: _normalizeClass(bar.baz)"));
        assert!(result.code.contains("ok\n        ? (_openBlock()"));
        assert!(result.code.contains("_renderList(list, (value, index) =>"));
        assert!(result
            .code
            .contains("_toDisplayString(value + index), 1 /* TEXT */"));
        let map = result.map.expect("source map");
        assert_eq!(map.sources, vec!["foo.vue"]);
        assert_eq!(
            map.sources_content,
            Some(vec![Some(
                r#"<div id="foo" :class="bar.baz">
  {{ world.burn() }}
  <div v-if="ok">yes</div>
  <template v-else>no</template>
  <div v-for="(value, index) in list"><span>{{ value + index }}</span></div>
</div>"#
                    .into()
            )])
        );
    }

    #[test]
    fn base_compile_keeps_v_for_aliases_local_when_prefixed() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-for="(value, index) in list">{{ value + index }}</div>"#.into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "function".into(),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result
            .code
            .contains("_renderList(_ctx.list, (value, index) =>"));
        assert!(result.code.contains("_toDisplayString(value + index)"));
        assert!(!result.code.contains("_ctx.value + _ctx.index"));
    }

    #[test]
    fn base_compile_wraps_v_memo_nodes_with_runtime_helper() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><div v-memo="[x]"></div></div>"#.into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result.code.contains("withMemo as _withMemo"));
        assert!(result.code.contains(
            r#"_withMemo([_ctx.x], () => (_openBlock(), _createElementBlock("div")), _cache, 0)"#
        ));
    }

    #[test]
    fn base_compile_keeps_static_cache_and_v_memo_cache_indexes_distinct() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: "<div><section v-memo=\"[x]\"><div><div>hello</div><div>hello</div></div></section></div>"
                    .into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "module".into(),
                hoist_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result
            .code
            .contains(r#"_withMemo([_ctx.x], () => (_openBlock(), _createElementBlock("section""#));
        assert!(result.code.contains(", _cache, 0)"));
        assert!(result
            .code
            .contains("_cache[1] || (_cache[1] = [_createElementVNode"));
        assert!(!result
            .code
            .contains("_cache[0] || (_cache[0] = [_createElementVNode"));
    }

    #[test]
    fn base_compile_keeps_static_cache_and_v_for_memo_cache_indexes_distinct() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><div v-for="{ x, y } in list" :key="x" v-memo="[x, y === z]"><span>foobar</span></div></div>"#.into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "module".into(),
                hoist_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result
            .code
            .contains("_renderList(_ctx.list, ({ x, y }, __, ___, _cached) =>"));
        assert!(result
            .code
            .contains("}, _cache, 0), 128 /* KEYED_FRAGMENT */)"));
        assert!(result
            .code
            .contains("_cache[2] || (_cache[2] = [_createElementVNode"));
        assert!(!result
            .code
            .contains("_cache[0] || (_cache[0] = [_createElementVNode"));
    }

    #[test]
    fn base_compile_generates_v_for_memo_cache_path() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><div v-for="{ x, y } in list" :key="x" v-memo="[x, y === z]"><span>foobar</span></div></div>"#.into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result.code.contains("isMemoSame as _isMemoSame"));
        assert!(result
            .code
            .contains("_renderList(_ctx.list, ({ x, y }, __, ___, _cached) =>"));
        assert!(result.code.contains("const _memo = ([x, y === _ctx.z])"));
        assert!(result
            .code
            .contains("_cached.key === x && _isMemoSame(_cached, _memo)"));
        assert!(result.code.contains("_item.memo = _memo"));
        assert!(!result.code.contains("_ctx.x, _ctx.y"));
    }

    #[test]
    fn base_compile_wraps_component_default_slot_with_ctx() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<Child><div/></Child>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                mode: "module".into(),
                scope_id: Some("test".into()),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result
            .code
            .contains("const _component_Child = _resolveComponent(\"Child\")"));
        assert!(result.code.contains("default: _withCtx(() => ["));
        assert!(result.code.contains("_createElementVNode(\"div\")"));
    }

    #[test]
    fn base_compile_stringifies_static_child_tree_when_threshold_matches() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    "<div><div>{}</div></div>",
                    r#"<span class="foo"/>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("createStaticVNode"));
        assert!(result.code.contains("_createStaticVNode(\"<div><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span></div>\", 1)"));
    }

    #[test]
    fn base_compile_stringifies_multiple_adjacent_static_nodes() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!("<div>{}</div>", r#"<span class="foo"/>"#.repeat(5)),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("_createStaticVNode(\"<span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span>\", 5)"));
    }

    #[test]
    fn base_compile_stringifies_multiple_static_chunks_around_dynamic_child() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    "<div>{}{{{{ msg }}}}{}</div>",
                    r#"<span class="foo"></span>"#.repeat(5),
                    r#"<span class="bar"></span>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert_eq!(result.code.matches("_createStaticVNode(").count(), 2);
        assert!(result.code.contains("_createStaticVNode(\"<span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span>\", 5)"));
        assert!(result
            .code
            .contains("_createTextVNode(_toDisplayString(_ctx.msg), 1 /* TEXT */)"));
        assert!(result.code.contains("_createStaticVNode(\"<span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span>\", 5)"));
        assert!(!result.code.contains("class: \"foo\""));
        assert!(!result.code.contains("class: \"bar\""));
    }

    #[test]
    fn base_compile_bails_stringify_static_invalid_p_child_placement() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    "<div><p>{}</p></div>",
                    r#"<span class="inline"></span>"#.repeat(5)
                        + "<span><div class=\"block\"></div></span>"
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(!result.code.contains("_createStaticVNode"));
        assert!(result.code.contains("_cache[0] || (_cache[0] = ["));
        assert!(result.code.contains("_createElementVNode(\"p\""));
        assert!(result
            .code
            .contains("_createElementVNode(\"div\", { class: \"block\" })"));
    }

    #[test]
    fn base_compile_stringifies_static_p_with_phrasing_children() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    "<div><p>{}</p></div>",
                    r#"<span class="inline"></span>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("_createStaticVNode"));
        assert!(result.code.contains("_createStaticVNode(\"<p><span class=\\\"inline\\\"></span><span class=\\\"inline\\\"></span><span class=\\\"inline\\\"></span><span class=\\\"inline\\\"></span><span class=\\\"inline\\\"></span></p>\", 1)"));
    }

    #[test]
    fn base_compile_stringifies_static_html_escaping_and_void_tags() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    "<div>{}{}</div>",
                    r#"<span title="foo>bar">&amp; &lt;</span>"#.repeat(5),
                    r#"<img title="foo>bar"/>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result
            .code
            .contains(r#"<span title=\"foo&gt;bar\">&amp; &lt;</span>"#));
        assert!(result.code.contains(r#"<img title=\"foo&gt;bar\">"#));
        assert!(!result.code.contains("</img>"));
    }

    #[test]
    fn base_compile_stringifies_static_html_with_scope_id() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    "<div>{}</div>",
                    r#"<span class="foo"><i>ok</i></span>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "module".into(),
                hoist_static: true,
                stringify_static: true,
                scope_id: Some("data-v-test".into()),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("_createStaticVNode"));
        assert!(result
            .code
            .contains(r#"<span class=\"foo\" data-v-test><i data-v-test>ok</i></span>"#));
    }

    #[test]
    fn base_compile_stringifies_static_constant_bindings_and_interpolations() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    r#"<div><div :style="{{ color: 'red' }}">{}</div></div>"#,
                    r#"<span :class="[{ foo: true }, { bar: true }]">{{ 1 }} + {{ false }}</span>"#
                        .repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("_createStaticVNode"));
        assert!(result
            .code
            .contains(r#"<div style=\"color:red;\"><span class=\"foo bar\">1 + false</span>"#));
        assert!(!result.code.contains("_normalizeClass"));
    }

    #[test]
    fn base_compile_stringifies_static_asset_import_bindings() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: format!(
                r#"<div><img :src="_imports_0" :srcset="_imports_0 + ', ' + _imports_1 + '#heart 2x'" />{}</div>"#,
                r#"<span class="foo"></span>"#.repeat(5)
            ),
            file_id: FileId(0),
            base_offset: 0,
        };
        let mut ast = Vue3Dialect::base_parse(
            source.clone(),
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );
        if let Some(root_node) = ast.root_node_mut() {
            if let Vue3AstKind::Root(root) = &mut root_node.kind {
                root.imports.push(vuec_ast::Vue3ImportItem {
                    name: "_imports_0".into(),
                    path: "./logo.png".into(),
                });
                root.imports.push(vuec_ast::Vue3ImportItem {
                    name: "_imports_1".into(),
                    path: "./icons.svg".into(),
                });
            }
        }
        let mut ctx = TransformContext::default();
        Vue3Dialect::transform(&mut ast, &mut ctx, &Vue3CompilerOptions::default());
        let result = Vue3Dialect::finish_compile(
            ast,
            source,
            Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
            ctx,
        );

        assert!(result.code.contains("import _imports_0 from './logo.png'"));
        assert!(result.code.contains("import _imports_1 from './icons.svg'"));
        assert!(
            result.code.contains("_createStaticVNode"),
            "{}",
            result.code
        );
        assert!(result
            .code
            .contains("const _hoisted_1 = _imports_0 + ', ' + _imports_1 + '#heart 2x'"));
        assert!(result.code.contains(
            r##"_createStaticVNode("<img src=\"" + _imports_0 + "\" srcset=\"" + _hoisted_1 + "\"><span class=\"foo\"></span>"##
        ));
        assert!(!result.code.contains("_ctx._imports_0"));
        assert!(!result.code.contains("_ctx._imports_1"));
    }

    #[test]
    fn base_compile_stringifies_constant_binding_removals_and_escape() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    "<div>{}{}</div>",
                    r#"<span :title="null" :class="'foo' + '&gt;ar'">{{ '<' }}</span>"#.repeat(5),
                    r#"<button :disabled="false">enable</button>"#.repeat(16)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result
            .code
            .contains(r#"<span class=\"foo&gt;ar\">&lt;</span>"#));
        assert!(result.code.contains(r#"<button>enable</button>"#));
        assert!(!result.code.contains("title="));
        assert!(!result.code.contains("disabled="));
    }

    #[test]
    fn base_compile_stringifies_static_svg_namespace_children() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    r#"<div><svg width="50" height="50" viewBox="0 0 50 50" fill="none" xmlns="http://www.w3.org/2000/svg">{}</svg></div>"#,
                    r##"<rect width="50" height="50" fill="#C4C4C4"></rect>"##.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                dom_namespaces: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("_createStaticVNode"));
        assert!(result.code.contains(r#"<svg width=\"50\" height=\"50\" viewBox=\"0 0 50 50\" fill=\"none\" xmlns=\"http://www.w3.org/2000/svg\">"#));
        assert!(result
            .code
            .contains(r##"<rect width=\"50\" height=\"50\" fill=\"#C4C4C4\"></rect>"##));
    }

    #[test]
    fn base_compile_stringifies_static_mathml_namespace_children() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    r#"<div><math xmlns="http://www.w3.org/1998/Math/MathML">{}</math></div>"#,
                    r#"<ms>1</ms>"#.repeat(20)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                dom_namespaces: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("_createStaticVNode"));
        assert!(result
            .code
            .contains(r#"<math xmlns=\"http://www.w3.org/1998/Math/MathML\"><ms>1</ms>"#));
    }

    #[test]
    fn base_compile_bails_stringify_static_option_constant_value_binding() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    "<div><select>{}</select></div>",
                    r#"<option :value="1"/>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(!result.code.contains("_createStaticVNode"));
        assert!(result.code.contains("_cache[0] || (_cache[0] = ["));
    }

    #[test]
    fn base_compile_keeps_static_cache_array_below_stringify_threshold() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: "<div><div><div>hello</div><div>hello</div></div></div>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(!result.code.contains("_createStaticVNode"));
        assert!(result.code.contains("_cache[0] || (_cache[0] = ["));
        assert!(result.code.contains("-1 /* CACHED */"));
    }

    #[test]
    fn base_compile_wraps_named_component_slots_with_ctx() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Child>
        <template #foo="{ msg }">{{ msg }}</template>
        <template #bar><div/></template>
      </Child>"#
                .into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                mode: "module".into(),
                scope_id: Some("test".into()),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result.code.contains("foo: _withCtx(({ msg }) => ["));
        assert!(result
            .code
            .contains("_createTextVNode(_toDisplayString(msg), 1 /* TEXT */)"));
        assert!(result.code.contains("bar: _withCtx(() => ["));
        assert!(result.code.contains("_: 1 /* STABLE */"));
    }

    #[test]
    fn base_compile_wraps_dynamic_component_slots_with_create_slots() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Child>
        <template #foo v-if="ok"><div/></template>
        <template v-for="i in list" #[i]><div/></template>
      </Child>"#
                .into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                mode: "module".into(),
                scope_id: Some("test".into()),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result
            .code
            .contains("_createSlots({ _: 2 /* DYNAMIC */ }, ["));
        assert!(result.code.contains("name: \"foo\""));
        assert!(result.code.contains("fn: _withCtx(() => ["));
        assert!(result.code.contains("name: i"));
        assert!(result.code.contains(", 1024 /* DYNAMIC_SLOTS */"));
    }

    #[test]
    fn root_codegen_projection_uses_child_for_slot_outlet() {
        let root = json!({
            "children": [{
                "type": 1,
                "tagType": 2,
                "codegenNode": { "type": 14 }
            }]
        });

        assert_eq!(
            root_codegen_projection(&root),
            json!({ "kind": "child", "index": 0 })
        );
    }

    #[test]
    fn root_codegen_projection_uses_single_element_codegen_as_block() {
        let root = json!({
            "children": [{
                "type": 1,
                "tagType": 0,
                "codegenNode": { "type": 13 }
            }]
        });

        assert_eq!(
            root_codegen_projection(&root),
            json!({ "kind": "childCodegen", "index": 0, "asBlock": true })
        );
    }

    #[test]
    fn root_codegen_projection_preserves_non_element_child() {
        let root = json!({ "children": [{ "type": 11, "codegenNode": { "type": 13 } }] });

        assert_eq!(
            root_codegen_projection(&root),
            json!({ "kind": "child", "index": 0 })
        );
    }

    #[test]
    fn root_codegen_projection_marks_single_visible_root_fragment() {
        let root = json!({
            "children": [
                { "type": 3 },
                { "type": 1, "tagType": 0 },
                { "type": 3 }
            ]
        });

        assert_eq!(
            root_codegen_projection(&root),
            json!({ "kind": "fragment", "patchFlag": 2112 })
        );
    }

    #[test]
    fn get_constant_type_projection_handles_static_interpolation_and_props() {
        let interpolation = get_constant_type_projection(&json!({
            "node": {
                "type": 5,
                "content": { "type": 4, "content": "1", "constType": 3 }
            },
            "context": {}
        }));
        assert_eq!(interpolation["constantType"], json!(3));

        let static_props = get_constant_type_projection(&json!({
            "node": {
                "type": 1,
                "tag": "div",
                "tagType": 0,
                "props": [],
                "children": [],
                "codegenNode": {
                    "type": 13,
                    "isBlock": false,
                    "props": {
                        "type": 15,
                        "properties": [{
                            "type": 16,
                            "key": { "type": 4, "content": "id", "isStatic": true },
                            "value": { "type": 4, "content": "foo", "isStatic": true }
                        }]
                    }
                }
            },
            "context": {}
        }));
        assert_eq!(static_props["constantType"], json!(3));
    }

    #[test]
    fn cache_static_projection_caches_static_child_arrays() {
        let projection = cache_static_projection(&json!({
            "root": {
                "children": [{
                    "type": 1,
                    "tag": "div",
                    "tagType": 0,
                    "props": [],
                    "children": [
                        {
                            "type": 1,
                            "tag": "span",
                            "tagType": 0,
                            "props": [],
                            "children": [],
                            "codegenNode": { "type": 13, "isBlock": false }
                        },
                        {
                            "type": 1,
                            "tag": "i",
                            "tagType": 0,
                            "props": [],
                            "children": [],
                            "codegenNode": { "type": 13, "isBlock": false }
                        }
                    ],
                    "codegenNode": {
                        "type": 13,
                        "isBlock": true,
                        "children": [{ "type": 1 }, { "type": 1 }]
                    }
                }]
            },
            "context": {}
        }));

        assert_eq!(
            projection["operations"],
            json!([
                {
                    "kind": "setPatchFlag",
                    "path": ["children", "0", "children", "0", "codegenNode"],
                    "patchFlag": -1
                },
                {
                    "kind": "setPatchFlag",
                    "path": ["children", "0", "children", "1", "codegenNode"],
                    "patchFlag": -1
                },
                {
                    "kind": "cacheChildrenArray",
                    "path": ["children", "0", "codegenNode", "children"],
                    "childrenPath": ["children", "0", "children"],
                    "needArraySpread": true
                }
            ])
        );
    }

    #[test]
    fn cache_static_projection_hoists_props_and_dynamic_props() {
        let projection = cache_static_projection(&json!({
            "root": {
                "children": [{
                    "type": 1,
                    "tag": "div",
                    "tagType": 0,
                    "props": [],
                    "children": [
                        {
                            "type": 1,
                            "tag": "span",
                            "tagType": 0,
                            "props": [],
                            "children": [],
                            "codegenNode": {
                                "type": 13,
                                "patchFlag": 512,
                                "props": {
                                    "type": 15,
                                    "properties": [{
                                        "type": 16,
                                        "key": { "type": 4, "content": "id", "isStatic": true },
                                        "value": { "type": 4, "content": "foo", "isStatic": true }
                                    }]
                                }
                            }
                        },
                        {
                            "type": 1,
                            "tag": "p",
                            "tagType": 0,
                            "props": [],
                            "children": [],
                            "codegenNode": {
                                "type": 13,
                                "patchFlag": 8,
                                "dynamicProps": "[\"foo\"]"
                            }
                        }
                    ],
                    "codegenNode": { "type": 13, "isBlock": true }
                }]
            },
            "context": {}
        }));

        assert_eq!(
            projection["operations"],
            json!([
                {
                    "kind": "hoistProps",
                    "path": ["children", "0", "children", "0", "codegenNode", "props"]
                },
                {
                    "kind": "hoistDynamicProps",
                    "path": ["children", "0", "children", "1", "codegenNode", "dynamicProps"]
                }
            ])
        );
    }

    #[test]
    fn cache_static_projection_caches_dynamic_template_slot_returns() {
        let dynamic_slot = json!({
            "type": 8,
            "children": ["foo + ", { "type": 4, "content": "bar", "constType": 0 }]
        });
        let projection = cache_static_projection(&json!({
            "root": {
                "children": [{
                    "type": 1,
                    "tag": "Comp",
                    "tagType": 1,
                    "props": [],
                    "children": [{
                        "type": 1,
                        "tag": "template",
                        "tagType": 3,
                        "props": [{
                            "type": 7,
                            "name": "slot",
                            "arg": dynamic_slot
                        }],
                        "children": [{
                            "type": 1,
                            "tag": "span",
                            "tagType": 0,
                            "props": [],
                            "children": [],
                            "codegenNode": { "type": 13, "isBlock": false }
                        }]
                    }],
                    "codegenNode": {
                        "type": 13,
                        "children": {
                            "type": 15,
                            "properties": [{
                                "key": dynamic_slot,
                                "value": {
                                    "type": 18,
                                    "returns": [{ "type": 1 }]
                                }
                            }]
                        }
                    }
                }]
            },
            "context": {}
        }));

        assert_eq!(projection["operations"][0]["kind"], json!("setPatchFlag"));
        assert_eq!(
            projection["operations"][1],
            json!({
                "kind": "cacheSlotReturns",
                "ownerPath": ["children", "0"],
                "slot": {
                    "kind": "dynamic",
                    "node": dynamic_slot
                },
                "needArraySpread": true
            })
        );
    }

    #[test]
    fn cache_static_projection_downgrades_static_svg_blocks_except_with_directives() {
        let static_svg = cache_static_projection(&json!({
            "root": {
                "children": [{
                    "type": 1,
                    "tag": "div",
                    "tagType": 0,
                    "props": [],
                    "children": [{
                        "type": 1,
                        "tag": "svg",
                        "tagType": 0,
                        "props": [],
                        "children": [],
                        "codegenNode": { "type": 13, "isBlock": true }
                    }],
                    "codegenNode": {
                        "type": 13,
                        "isBlock": true,
                        "children": [{ "type": 1 }]
                    }
                }]
            },
            "context": {}
        }));
        assert_eq!(
            static_svg["operations"][0],
            json!({
                "kind": "setBlock",
                "path": ["children", "0", "children", "0", "codegenNode"],
                "isBlock": false
            })
        );

        let svg_with_directive = cache_static_projection(&json!({
            "root": {
                "children": [{
                    "type": 1,
                    "tag": "div",
                    "tagType": 0,
                    "props": [],
                    "children": [{
                        "type": 1,
                        "tag": "svg",
                        "tagType": 0,
                        "props": [{ "type": 7, "name": "foo" }],
                        "children": [{
                            "type": 1,
                            "tag": "path",
                            "tagType": 0,
                            "props": [],
                            "children": [],
                            "codegenNode": { "type": 13, "isBlock": false }
                        }],
                        "codegenNode": {
                            "type": 13,
                            "isBlock": true,
                            "children": [{ "type": 1 }]
                        }
                    }],
                    "codegenNode": { "type": 13, "isBlock": true }
                }]
            },
            "context": {}
        }));
        let svg_codegen_path = json!(["children", "0", "children", "0", "codegenNode"]);
        assert!(svg_with_directive["operations"]
            .as_array()
            .expect("operations")
            .iter()
            .all(|operation| operation["path"] != svg_codegen_path));
        assert_eq!(
            svg_with_directive["operations"][1],
            json!({
                "kind": "cacheChildrenArray",
                "path": ["children", "0", "children", "0", "codegenNode", "children"],
                "childrenPath": ["children", "0", "children", "0", "children"],
                "needArraySpread": true
            })
        );
    }

    #[test]
    fn stringify_static_projection_stringifies_cached_adjacent_children() {
        let children = (0..STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT)
            .map(|_| {
                json!({
                    "type": 1,
                    "tag": "span",
                    "tagType": 0,
                    "ns": 0,
                    "props": [{
                        "type": 6,
                        "name": "class",
                        "value": { "content": "foo" }
                    }],
                    "children": [],
                    "codegenNode": { "type": 20, "index": 0 }
                })
            })
            .collect::<Vec<_>>();
        let projection = stringify_static_projection(&json!({
            "children": children,
            "context": {}
        }));
        let expected_html =
            r#"<span class="foo"></span>"#.repeat(STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT);

        assert_eq!(
            projection["operations"],
            json!([{
                "kind": "stringifyCachedChildRange",
                "start": 0,
                "count": STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT,
                "html": quote_string(&expected_html),
                "domNodes": STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT
            }])
        );
    }

    #[test]
    fn stringify_static_projection_stringifies_parent_cached_child_tree() {
        let children = vec![json!({
            "type": 1,
            "tag": "div",
            "tagType": 0,
            "ns": 0,
            "props": [],
            "children": (0..STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT)
                .map(|_| json!({
                    "type": 1,
                    "tag": "span",
                    "tagType": 0,
                    "ns": 0,
                    "props": [{
                        "type": 6,
                        "name": "class",
                        "value": { "content": "foo" }
                    }],
                    "children": []
                }))
                .collect::<Vec<_>>()
        })];
        let projection = stringify_static_projection(&json!({
            "children": children,
            "parent": {
                "type": 1,
                "tagType": 0,
                "codegenNode": {
                    "type": 13,
                    "children": { "type": 20 }
                }
            },
            "context": { "scopeId": "data-v-test" }
        }));
        let expected_html = format!(
            r#"<div data-v-test>{}</div>"#,
            r#"<span class="foo" data-v-test></span>"#
                .repeat(STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT)
        );

        assert_eq!(
            projection["operations"],
            json!([{
                "kind": "stringifyParentCachedRange",
                "start": 0,
                "count": 1,
                "html": quote_string(&expected_html),
                "domNodes": 1
            }])
        );
    }

    #[test]
    fn stringify_static_projection_infers_nested_svg_namespace() {
        let children = vec![json!({
            "type": 1,
            "tag": "svg",
            "tagType": 0,
            "props": [{
                "type": 6,
                "name": "viewBox",
                "value": { "content": "0 0 50 50" }
            }],
            "children": (0..STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT)
                .map(|_| json!({
                    "type": 1,
                    "tag": "rect",
                    "tagType": 0,
                    "props": [{
                        "type": 6,
                        "name": "fill",
                        "value": { "content": "#C4C4C4" }
                    }],
                    "children": []
                }))
                .collect::<Vec<_>>()
        })];
        let projection = stringify_static_projection(&json!({
            "children": children,
            "parent": {
                "type": 1,
                "tagType": 0,
                "codegenNode": {
                    "type": 13,
                    "children": { "type": 20 }
                }
            },
            "context": {}
        }));
        let expected_html = format!(
            r#"<svg viewBox="0 0 50 50">{}</svg>"#,
            r##"<rect fill="#C4C4C4"></rect>"##.repeat(STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT)
        );

        assert_eq!(
            projection["operations"],
            json!([{
                "kind": "stringifyParentCachedRange",
                "start": 0,
                "count": 1,
                "html": quote_string(&expected_html),
                "domNodes": 1
            }])
        );
    }

    #[test]
    fn stringify_static_projection_bails_can_cache_option_values() {
        let children = vec![json!({
            "type": 1,
            "tag": "option",
            "tagType": 0,
            "ns": 0,
            "props": [{
                "type": 7,
                "name": "bind",
                "arg": { "type": 4, "content": "value", "isStatic": true },
                "exp": {
                    "type": 4,
                    "content": "_imports_0",
                    "isStatic": false,
                    "constType": VUE3_CONSTANT_CAN_STRINGIFY
                }
            }],
            "children": [],
            "codegenNode": { "type": 20, "index": 0 }
        })];
        let projection = stringify_static_projection(&json!({
            "children": children,
            "context": {}
        }));

        assert_eq!(projection["operations"], json!([]));
    }

    #[test]
    fn transform_for_projection_preserves_skipped_alias_slots_and_locs() {
        let source = "<span v-for=\"( item,, index ) in items\" />";
        let exp_start = source.find("( item").unwrap();
        let projection = transform_for_projection(&json!({
            "dir": {
                "exp": {
                    "content": "( item,, index ) in items",
                    "loc": {
                        "start": { "offset": exp_start, "line": 1, "column": exp_start + 1 },
                        "end": { "offset": exp_start + "( item,, index ) in items".len(), "line": 1, "column": exp_start + "( item,, index ) in items".len() + 1 },
                        "source": "( item,, index ) in items"
                    }
                },
                "loc": { "source": "v-for=\"( item,, index ) in items\"" }
            },
            "node": { "type": 1, "tagType": 0, "children": [] },
            "context": {}
        }));

        assert_eq!(projection["parseResult"]["value"]["content"], json!("item"));
        assert!(projection["parseResult"]["key"].is_null());
        assert_eq!(
            projection["parseResult"]["index"]["content"],
            json!("index")
        );
        assert_eq!(
            projection["parseResult"]["source"]["content"],
            json!("items")
        );
        assert_eq!(
            projection["parseResult"]["index"]["loc"]["start"]["offset"],
            json!(source.find("index").unwrap())
        );
    }

    #[test]
    fn transform_for_projection_reports_missing_and_malformed_expression() {
        let missing = transform_for_projection(&json!({
            "dir": { "loc": { "source": "v-for" } },
            "node": { "type": 1, "tagType": 0 },
            "context": {}
        }));
        assert_eq!(missing["errors"], json!([{ "code": 31, "loc": "dir" }]));

        let malformed = transform_for_projection(&json!({
            "dir": {
                "exp": {
                    "content": "item in",
                    "loc": { "start": { "offset": 0, "line": 1, "column": 1 }, "source": "item in" }
                },
                "loc": { "source": "v-for=\"item in\"" }
            },
            "node": { "type": 1, "tagType": 0 },
            "context": {}
        }));
        assert_eq!(malformed["errors"], json!([{ "code": 32, "loc": "dir" }]));
    }

    #[test]
    fn transform_for_projection_prefixes_source_and_alias_defaults() {
        let projection = transform_for_projection(&json!({
            "dir": {
                "exp": {
                    "content": "({ foo = bar, baz: [qux = quux] }) in list.concat([foo])",
                    "loc": {
                        "start": { "offset": 0, "line": 1, "column": 1 },
                        "end": { "offset": 58, "line": 1, "column": 59 },
                        "source": "({ foo = bar, baz: [qux = quux] }) in list.concat([foo])"
                    }
                },
                "loc": { "source": "v-for" }
            },
            "node": { "type": 1, "tagType": 0, "children": [] },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }
        }));

        assert_eq!(
            projection["parseResult"]["source"]["kind"],
            json!("compound")
        );
        assert_eq!(
            projection["parseResult"]["source"]["children"][0]["content"],
            json!("_ctx.list")
        );
        assert_eq!(
            projection["parseResult"]["value"]["kind"],
            json!("compound")
        );
        let value = &projection["parseResult"]["value"]["children"];
        assert_eq!(value[1]["content"], json!("foo"));
        assert_eq!(value[3]["content"], json!("_ctx.bar"));
        assert_eq!(value[5]["content"], json!("qux"));
        assert_eq!(value[7]["content"], json!("_ctx.quux"));
        assert_eq!(projection["locals"], json!(["foo", "qux"]));
    }

    #[test]
    fn transform_for_projection_reports_template_child_key_placement() {
        let projection = transform_for_projection(&json!({
            "dir": {
                "exp": {
                    "content": "item in items",
                    "loc": { "start": { "offset": 0, "line": 1, "column": 1 }, "source": "item in items" }
                },
                "loc": { "source": "v-for" }
            },
            "node": {
                "type": 1,
                "tagType": 3,
                "children": [{
                    "type": 1,
                    "tag": "div",
                    "props": [{
                        "type": 7,
                        "name": "bind",
                        "arg": { "type": 4, "content": "key", "isStatic": true },
                        "loc": { "source": ":key=\"item.id\"" }
                    }]
                }]
            },
            "context": {}
        }));
        assert_eq!(
            projection["templateKeyErrors"],
            json!([{ "code": 33, "loc": { "source": ":key=\"item.id\"" } }])
        );
    }

    #[test]
    fn build_slots_projection_tracks_slot_locals_and_dynamic_slots() {
        let projection = build_slots_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 1,
                "props": [{
                    "type": 7,
                    "name": "slot",
                    "exp": {
                        "type": 8,
                        "children": [
                            "{ ",
                            { "type": 4, "content": "foo", "isStatic": false },
                            " }"
                        ],
                        "loc": { "source": "{ foo }" }
                    }
                }],
                "children": [
                    { "type": 5, "content": { "type": 4, "content": "foo", "isStatic": false } }
                ],
                "loc": { "source": "<Comp/>" }
            },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }
        }));

        assert_eq!(
            projection["properties"][0]["key"]["content"],
            json!("default")
        );
        assert_eq!(projection["properties"][0]["indices"], json!([0]));
        assert_eq!(projection["hasDynamicSlots"], json!(false));

        let tracking = track_slot_scopes_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 1,
                "props": [{
                    "type": 7,
                    "name": "slot",
                    "exp": {
                        "type": 8,
                        "children": [
                            "{ ",
                            { "type": 4, "content": "foo", "isStatic": false },
                            " }"
                        ],
                        "loc": { "source": "{ foo }" }
                    }
                }]
            }
        }));
        assert_eq!(tracking["locals"], json!(["foo"]));
    }

    #[test]
    fn build_slots_projection_lowers_if_and_for_dynamic_slots() {
        let if_projection = build_slots_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 1,
                "props": [],
                "children": [{
                    "type": 1,
                    "tag": "template",
                    "tagType": 3,
                    "props": [
                        { "type": 7, "name": "slot", "arg": { "type": 4, "content": "one", "isStatic": true }, "loc": { "source": "#one" } },
                        { "type": 7, "name": "if", "exp": { "type": 4, "content": "_ctx.ok", "isStatic": false }, "loc": { "source": "v-if=\"ok\"" } }
                    ],
                    "children": [{ "type": 2, "content": "hello" }]
                }]
            },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }
        }));
        assert_eq!(
            if_projection["dynamicSlots"][0]["kind"],
            json!("conditional")
        );
        assert_eq!(
            if_projection["dynamicSlots"][0]["test"]["content"],
            json!("_ctx.ok")
        );

        let for_projection = build_slots_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 1,
                "props": [],
                "children": [{
                    "type": 1,
                    "tag": "template",
                    "tagType": 3,
                    "props": [
                        { "type": 7, "name": "slot", "arg": { "type": 4, "content": "name", "isStatic": false }, "loc": { "source": "#[name]" } },
                        {
                            "type": 7,
                            "name": "for",
                            "exp": { "type": 4, "content": "name in list", "loc": { "source": "name in list", "start": { "offset": 0, "line": 1, "column": 1 } } },
                            "forParseResult": {
                                "source": { "type": 4, "content": "_ctx.list", "isStatic": false },
                                "value": { "type": 4, "content": "name", "isStatic": false },
                                "key": null,
                                "index": null
                            }
                        }
                    ],
                    "children": [{ "type": 5, "content": { "type": 4, "content": "name", "isStatic": false } }]
                }]
            },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }
        }));
        assert_eq!(for_projection["dynamicSlots"][0]["kind"], json!("for"));
        assert_eq!(
            for_projection["dynamicSlots"][0]["source"]["content"],
            json!("_ctx.list")
        );
        assert_eq!(
            for_projection["dynamicSlots"][0]["slot"]["name"]["content"],
            json!("name")
        );
    }

    #[test]
    fn transform_slot_outlet_projection_projects_name_props_and_codegen_shape() {
        let named = transform_slot_outlet_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 2,
                "props": [
                    { "type": 6, "name": "name", "value": { "content": "foo" } },
                    { "type": 6, "name": "foo-bar", "value": { "content": "baz" } },
                    {
                        "type": 7,
                        "name": "bind",
                        "arg": { "type": 4, "content": "qux-kebab", "isStatic": true },
                        "exp": { "type": 4, "content": "qux", "isStatic": false }
                    }
                ],
                "children": [{ "type": 2, "content": "fallback" }],
                "loc": { "source": "<slot/>" }
            },
            "context": { "scopeId": "data-v-test", "slotted": false }
        }));

        assert_eq!(named["transform"], json!(true));
        assert_eq!(
            named["process"]["slotName"],
            json!({ "kind": "literal", "value": "\"foo\"" })
        );
        assert_eq!(named["process"]["nonNameProps"], json!([1, 2]));
        assert_eq!(
            named["process"]["mutations"],
            json!([
                { "kind": "setPropName", "index": 1, "name": "fooBar" },
                { "kind": "setDirectiveArgContent", "index": 2, "content": "quxKebab" }
            ])
        );
        assert_eq!(named["codegen"]["expectedLen"], json!(5));
        assert_eq!(named["codegen"]["slots"], json!("$slots"));
    }

    #[test]
    fn transform_slot_outlet_projection_handles_dynamic_and_same_name_slots() {
        let dynamic = transform_slot_outlet_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 2,
                "props": [{
                    "type": 7,
                    "name": "bind",
                    "arg": { "type": 4, "content": "name", "isStatic": true },
                    "exp": { "type": 4, "content": "foo", "isStatic": false }
                }],
                "children": []
            },
            "context": {}
        }));
        assert_eq!(
            dynamic["process"]["slotName"],
            json!({ "kind": "node", "path": "props", "index": 0, "field": "exp" })
        );
        assert_eq!(dynamic["codegen"]["expectedLen"], json!(2));

        let shorthand = transform_slot_outlet_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 2,
                "props": [{
                    "type": 7,
                    "name": "bind",
                    "arg": { "type": 4, "content": "name", "isStatic": true, "loc": { "source": "name" } }
                }],
                "children": []
            },
            "context": { "prefixIdentifiers": true, "bindingMetadata": {} }
        }));
        assert_eq!(
            shorthand["process"]["mutations"][0],
            json!({
                "kind": "setDirectiveExp",
                "index": 0,
                "value": {
                    "kind": "simple",
                    "content": "_ctx.name",
                    "isStatic": false,
                    "constType": 0,
                    "loc": { "source": "name" },
                    "helpers": []
                }
            })
        );
        assert_eq!(
            shorthand["process"]["slotName"],
            json!({ "kind": "node", "path": "props", "index": 0, "field": "exp" })
        );
        assert_eq!(shorthand["codegen"]["slots"], json!("_ctx.$slots"));
    }

    #[test]
    fn transform_on_projection_projects_dynamic_event_key_and_prefixes_handler() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "_ctx.event", "isStatic": false },
                "exp": { "type": 4, "content": "handler", "loc": { "source": "handler" } },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "identifiers": {},
                "bindingMetadata": {}
            }
        }));

        assert_eq!(
            projection["props"][0]["key"],
            json!({
                "kind": "compound",
                "children": [
                    { "kind": "helperString", "helper": "TO_HANDLER_KEY" },
                    { "kind": "node", "path": "dir.arg" },
                    ")"
                ]
            })
        );
        assert_eq!(
            projection["props"][0]["value"]["content"],
            json!("_ctx.handler")
        );
        assert_eq!(projection["props"][0]["dynamicKey"], json!(true));
        assert_eq!(
            projection["props"][0]["ignoreDynamicKeyForNormalize"],
            json!(true)
        );
    }

    #[test]
    fn transform_on_projection_wraps_inline_statements_and_caches_members() {
        let inline = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "click", "isStatic": true },
                "exp": { "type": 4, "content": "foo($event)", "loc": { "source": "foo($event)" } },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": {},
                "bindingMetadata": {}
            }
        }));
        assert_eq!(inline["props"][0]["cache"], json!(true));
        assert_eq!(
            inline["props"][0]["value"]["children"][0],
            json!("$event => (")
        );

        let member = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "click", "isStatic": true },
                "exp": { "type": 4, "content": "foo", "loc": { "source": "foo" } },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": {},
                "bindingMetadata": {}
            }
        }));
        assert_eq!(member["props"][0]["cache"], json!(true));
        assert_eq!(
            member["props"][0]["value"]["children"][1]["content"],
            json!("_ctx.foo && _ctx.foo(...args)")
        );

        let component_member = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "click", "isStatic": true },
                "exp": { "type": 4, "content": "foo", "loc": { "source": "foo" } },
                "modifiers": []
            },
            "node": { "tagType": 1 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": {},
                "bindingMetadata": {}
            }
        }));
        assert_eq!(component_member["props"][0]["cache"], json!(false));
    }

    #[test]
    fn transform_on_projection_rewrites_inline_assignment_bindings() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "click", "isStatic": true },
                "exp": {
                    "type": 4,
                    "content": "maybe = count; --lett",
                    "loc": { "source": "maybe = count; --lett" }
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "inline": true,
                "identifiers": {},
                "bindingMetadata": {
                    "count": "setup-ref",
                    "maybe": "setup-maybe-ref",
                    "lett": "setup-let"
                }
            }
        }));

        let code = projection_code(&projection["props"][0]["value"]);
        assert!(
            code.contains("maybe.value = count.value; _isRef(lett) ? --lett.value : --lett"),
            "{code}"
        );
        assert_eq!(
            projection["props"][0]["value"]["children"][1]["helpers"],
            json!(["IS_REF"])
        );
    }

    #[test]
    fn transform_on_projection_rewrites_function_expression_body_refs() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "click", "isStatic": true },
                "exp": {
                    "type": 4,
                    "content": "async function () { await foo() } ",
                    "loc": { "source": "async function () { await foo() } " }
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": {},
                "bindingMetadata": {}
            }
        }));

        assert_eq!(projection["props"][0]["cache"], json!(true));
        assert_eq!(
            projection["props"][0]["value"]["children"][0],
            json!("async function () { await ")
        );
        assert_eq!(
            projection["props"][0]["value"]["children"][1]["content"],
            json!("_ctx.foo")
        );
        assert_eq!(
            projection["props"][0]["value"]["children"][2],
            json!("() } ")
        );
    }

    #[test]
    fn transform_on_projection_keeps_update_operator_child() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "click", "isStatic": true },
                "exp": {
                    "type": 4,
                    "content": "foo++",
                    "loc": { "source": "foo++" }
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": {},
                "bindingMetadata": {}
            }
        }));

        assert_eq!(projection["props"][0]["cache"], json!(true));
        assert_eq!(
            projection["props"][0]["value"]["children"][0],
            json!("$event => (")
        );
        assert_eq!(
            projection["props"][0]["value"]["children"][1]["children"][0]["content"],
            json!("_ctx.foo")
        );
        assert_eq!(
            projection["props"][0]["value"]["children"][1]["children"][1],
            json!("++")
        );
        assert_eq!(projection["props"][0]["value"]["children"][2], json!(")"));
    }

    #[test]
    fn transform_element_props_projection_keeps_dynamic_handlers_unwrapped_for_normalize() {
        let projection = transform_element_props_projection(&json!({
            "props": [{
                "kind": "directiveProp",
                "dynamicKey": true,
                "ignoreDynamicKeyForNormalize": true,
                "valueConstant": false
            }],
            "context": {},
            "isComponent": false
        }));

        assert_eq!(projection["patchFlag"], json!(16));
        assert_eq!(projection["normalizeProps"], json!(false));
    }

    #[test]
    fn transform_bind_projection_projects_static_and_dynamic_args() {
        let static_projection = transform_bind_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "id", "isStatic": true, "loc": { "source": "id" } },
                "exp": { "type": 4, "content": "id", "isStatic": false, "loc": { "source": "id" } },
                "modifiers": []
            },
            "context": {}
        }));
        assert_eq!(
            static_projection["props"][0]["key"],
            json!({
                "kind": "simple",
                "content": "id",
                "isStatic": true,
                "loc": { "source": "id" }
            })
        );
        assert_eq!(
            static_projection["props"][0]["value"],
            json!({ "kind": "node", "path": "dir.exp" })
        );

        let dynamic_projection = transform_bind_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "id", "isStatic": false, "loc": { "source": "[id]" } },
                "exp": { "type": 4, "content": "value", "isStatic": false },
                "modifiers": []
            },
            "context": {}
        }));
        assert_eq!(
            dynamic_projection["props"][0]["key"]["content"],
            json!("id || \"\"")
        );
        assert_eq!(
            dynamic_projection["props"][0]["key"]["isStatic"],
            json!(false)
        );
    }

    #[test]
    fn transform_bind_projection_applies_camel_and_prefix_modifiers() {
        let static_camel = transform_bind_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "foo-bar", "isStatic": true },
                "exp": { "type": 4, "content": "id", "isStatic": false },
                "modifiers": [{ "content": "camel" }]
            },
            "context": {}
        }));
        assert_eq!(static_camel["props"][0]["key"]["content"], json!("fooBar"));

        let dynamic_camel = transform_bind_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "foo", "isStatic": false },
                "exp": { "type": 4, "content": "id", "isStatic": false },
                "modifiers": [{ "content": "camel" }]
            },
            "context": {}
        }));
        assert_eq!(
            dynamic_camel["props"][0]["key"]["content"],
            json!("_camelize(foo || \"\")")
        );
        assert_eq!(
            dynamic_camel["props"][0]["key"]["helpers"],
            json!(["CAMELIZE"])
        );

        let dynamic_prop = transform_bind_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "fooBar", "isStatic": false },
                "exp": { "type": 4, "content": "id", "isStatic": false },
                "modifiers": [{ "content": "prop" }]
            },
            "context": {}
        }));
        assert_eq!(
            dynamic_prop["props"][0]["key"]["content"],
            json!("`.${fooBar || \"\"}`")
        );

        let ssr_prop = transform_bind_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "fooBar", "isStatic": true },
                "exp": { "type": 4, "content": "id", "isStatic": false },
                "modifiers": [{ "content": "prop" }]
            },
            "context": { "inSSR": true }
        }));
        assert_eq!(ssr_prop["props"][0]["key"]["content"], json!("fooBar"));
    }

    #[test]
    fn transform_bind_projection_handles_compound_args_and_empty_expressions() {
        let compound = transform_bind_projection(&json!({
            "dir": {
                "arg": {
                    "type": 8,
                    "children": [
                        { "type": 4, "content": "_ctx.foo", "isStatic": false },
                        "(",
                        { "type": 4, "content": "_ctx.bar", "isStatic": false },
                        ")"
                    ],
                    "loc": { "source": "foo(bar)" }
                },
                "exp": { "type": 4, "content": "_ctx.id", "isStatic": false },
                "modifiers": [{ "content": "camel" }, { "content": "prop" }]
            },
            "context": {}
        }));
        assert_eq!(compound["props"][0]["key"]["children"][0], json!("'.' + ("));
        assert_eq!(
            compound["props"][0]["key"]["children"][1]["children"][0],
            json!({ "kind": "helperString", "helper": "CAMELIZE" })
        );
        assert_eq!(
            compound["props"][0]["key"]["children"][1]["children"][1]["children"][0],
            json!("(")
        );

        let missing = transform_bind_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "arg", "isStatic": true },
                "exp": { "type": 4, "content": "   ", "isStatic": false },
                "modifiers": [],
                "loc": { "source": "v-bind:arg=\"\"" }
            },
            "context": {}
        }));
        assert_eq!(missing["errors"], json!([{ "code": 34, "loc": "dir" }]));
        assert_eq!(missing["props"][0]["value"]["content"], json!(""));
        assert_eq!(missing["props"][0]["value"]["isStatic"], json!(true));

        let browser_missing = transform_bind_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "arg", "isStatic": true },
                "exp": { "type": 4, "content": "", "isStatic": false },
                "modifiers": []
            },
            "context": { "browser": true }
        }));
        assert_eq!(browser_missing["errors"], json!([]));
        assert_eq!(
            browser_missing["props"][0]["value"],
            json!({ "kind": "undefined" })
        );
    }

    #[test]
    fn transform_v_bind_shorthand_projection_expands_static_same_name_bindings() {
        let projection = transform_v_bind_shorthand_projection(&json!({
            "node": {
                "type": 1,
                "props": [{
                    "type": 7,
                    "name": "bind",
                    "arg": {
                        "type": 4,
                        "content": "foo-bar",
                        "isStatic": true,
                        "loc": { "source": "foo-bar" }
                    }
                }]
            },
            "context": {}
        }));

        assert_eq!(
            projection["operations"][0],
            json!({
                "kind": "setExp",
                "index": 0,
                "exp": {
                    "kind": "simple",
                    "content": "fooBar",
                    "isStatic": false,
                    "loc": { "source": "foo-bar" }
                },
                "errors": []
            })
        );
    }

    #[test]
    fn transform_v_bind_shorthand_projection_reports_dynamic_args_and_browser_empty_exp() {
        let invalid = transform_v_bind_shorthand_projection(&json!({
            "node": {
                "type": 1,
                "props": [{
                    "type": 7,
                    "name": "bind",
                    "arg": { "type": 4, "content": "foo", "isStatic": false, "loc": { "source": "[foo]" } }
                }]
            },
            "context": {}
        }));
        assert_eq!(
            invalid["operations"][0]["errors"],
            json!([{ "code": 53, "loc": "arg" }])
        );
        assert_eq!(invalid["operations"][0]["exp"]["content"], json!(""));
        assert_eq!(invalid["operations"][0]["exp"]["isStatic"], json!(true));

        let browser_empty = transform_v_bind_shorthand_projection(&json!({
            "node": {
                "type": 1,
                "props": [{
                    "type": 7,
                    "name": "bind",
                    "arg": { "type": 4, "content": "name", "isStatic": true, "loc": { "source": "name" } },
                    "exp": { "type": 4, "content": "  ", "isStatic": false }
                }]
            },
            "context": { "browser": true }
        }));
        assert_eq!(
            browser_empty["operations"][0]["exp"]["content"],
            json!("name")
        );

        let invalid_first_char = transform_v_bind_shorthand_projection(&json!({
            "node": {
                "type": 1,
                "props": [{
                    "type": 7,
                    "name": "bind",
                    "arg": { "type": 4, "content": "1bad", "isStatic": true }
                }]
            },
            "context": {}
        }));
        assert_eq!(invalid_first_char["operations"], json!([]));

        let unicode_first_char = transform_v_bind_shorthand_projection(&json!({
            "node": {
                "type": 1,
                "props": [{
                    "type": 7,
                    "name": "bind",
                    "arg": { "type": 4, "content": "éclair", "isStatic": true, "loc": { "source": "éclair" } }
                }]
            },
            "context": {}
        }));
        assert_eq!(
            unicode_first_char["operations"][0]["exp"]["content"],
            json!("éclair")
        );
    }

    #[test]
    fn transform_on_projection_marks_setup_const_handlers_constant() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "keydown", "isStatic": true },
                "exp": { "type": 4, "content": "foo", "loc": { "source": "foo" } },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "bindingMetadata": { "foo": "setup-const" }
            }
        }));

        assert_eq!(
            projection["props"][0]["value"]["content"],
            json!("$setup.foo")
        );
        assert_eq!(projection["props"][0]["value"]["constType"], json!(1));
        assert_eq!(projection["props"][0]["valueConstant"], json!(true));
    }

    #[test]
    fn transform_model_projection_emits_model_value_and_update_props() {
        let projection = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "model",
                    "loc": { "source": "model" }
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {}
        }));

        assert_eq!(
            projection["props"][0]["key"],
            json!({ "kind": "static", "content": "modelValue" })
        );
        assert_eq!(projection["props"][0]["dynamic"], json!(true));
        assert_eq!(
            projection["props"][1]["key"],
            json!({ "kind": "static", "content": "onUpdate:modelValue" })
        );
        assert_eq!(
            projection["props"][1]["value"]["children"][0],
            json!("$event => ((")
        );
    }

    #[test]
    fn transform_model_projection_handles_dynamic_argument() {
        let projection = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "_ctx.model",
                    "loc": { "source": "model" }
                },
                "arg": {
                    "type": 4,
                    "content": "_ctx.value",
                    "isStatic": false
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": { "prefixIdentifiers": true }
        }));

        assert_eq!(
            projection["props"][0]["key"],
            json!({ "kind": "node", "path": "dir.arg" })
        );
        assert_eq!(
            projection["props"][1]["key"],
            json!({
                "kind": "compound",
                "children": ["\"onUpdate:\" + ", { "kind": "node", "path": "dir.arg" }]
            })
        );
    }

    #[test]
    fn transform_model_projection_reports_invalid_expression_errors() {
        let no_expression = transform_model_projection(&json!({
            "dir": { "modifiers": [] },
            "node": { "tagType": 0 },
            "context": {}
        }));
        assert_eq!(no_expression["errors"], json!([41]));

        let malformed = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "a + b",
                    "loc": { "source": "a + b" }
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {}
        }));
        assert_eq!(malformed["errors"], json!([42]));
    }

    #[test]
    fn transform_model_projection_tracks_cache_and_scope_refs() {
        let cached = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "_ctx.foo",
                    "loc": { "source": "foo" }
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": {}
            }
        }));
        assert_eq!(cached["props"][1]["cache"], json!(true));
        assert_eq!(cached["props"][1]["dynamic"], json!(false));

        let scoped = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 8,
                    "loc": { "source": "foo[i]" },
                    "children": [
                        { "type": 4, "content": "_ctx.foo" },
                        "[",
                        { "type": 4, "content": "i" },
                        "]"
                    ]
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": { "i": 1 }
            }
        }));
        assert_eq!(scoped["props"][1]["cache"], json!(false));
        assert_eq!(scoped["props"][1]["dynamic"], json!(true));
    }

    #[test]
    fn transform_model_projection_generates_component_modifiers() {
        let projection = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "foo",
                    "loc": { "source": "foo" }
                },
                "arg": {
                    "type": 4,
                    "content": "bar",
                    "isStatic": true
                },
                "modifiers": [
                    { "content": "trim" },
                    { "content": "bar-baz" }
                ]
            },
            "node": { "tagType": 1 },
            "context": {}
        }));

        assert_eq!(
            projection["props"][2]["key"],
            json!({ "kind": "static", "content": "barModifiers" })
        );
        assert_eq!(
            projection["props"][2]["value"]["content"],
            json!("{ trim: true, \"bar-baz\": true }")
        );
    }

    #[test]
    fn transform_model_projection_marks_static_argument_hydration_event() {
        let projection = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "model",
                    "loc": { "source": "model" }
                },
                "arg": {
                    "type": 4,
                    "content": "foo-value",
                    "isStatic": true
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {}
        }));

        assert_eq!(projection["props"][1]["hydrate"], json!(true));
    }

    #[test]
    fn resolve_component_type_projection_uses_setup_bindings() {
        let projection = resolve_component_type_projection(&json!({
            "node": { "type": 1, "tag": "Example", "tagType": 1, "props": [] },
            "context": {
                "bindingMetadata": { "Example": "setup-maybe-ref" },
                "inline": true
            }
        }));

        assert_eq!(projection["kind"], json!("expression"));
        assert_eq!(projection["content"], json!("_unref(Example)"));
        assert_eq!(projection["helpers"], json!(["UNREF"]));
    }

    #[test]
    fn resolve_component_type_projection_handles_namespaced_props_binding() {
        let projection = resolve_component_type_projection(&json!({
            "node": { "type": 1, "tag": "Foo.Example", "tagType": 1, "props": [] },
            "context": {
                "bindingMetadata": { "Foo": "props" },
                "inline": false
            }
        }));

        assert_eq!(projection["kind"], json!("expression"));
        assert_eq!(
            projection["content"],
            json!("_unref($props[\"Foo\"]).Example")
        );
    }

    #[test]
    fn resolve_component_type_projection_marks_self_reference_asset() {
        let projection = resolve_component_type_projection(&json!({
            "node": { "type": 1, "tag": "Example", "tagType": 1, "props": [] },
            "context": { "selfName": "Example" }
        }));

        assert_eq!(projection["kind"], json!("asset"));
        assert_eq!(projection["component"], json!("Example__self"));
        assert_eq!(projection["assetId"], json!("_component_Example"));
    }

    #[test]
    fn resolve_component_type_projection_handles_dynamic_component_is() {
        let projection = resolve_component_type_projection(&json!({
            "node": {
                "type": 1,
                "tag": "component",
                "tagType": 1,
                "props": [
                    {
                        "type": 7,
                        "name": "bind",
                        "arg": { "type": 4, "content": "is", "isStatic": true },
                        "exp": { "type": 4, "content": "foo", "isStatic": false }
                    }
                ]
            },
            "context": {}
        }));

        assert_eq!(projection["kind"], json!("dynamic"));
        assert_eq!(projection["helper"], json!("RESOLVE_DYNAMIC_COMPONENT"));
        assert_eq!(projection["argument"]["content"], json!("foo"));
    }

    #[test]
    fn resolve_component_type_projection_casts_vue_is_attribute() {
        let projection = resolve_component_type_projection(&json!({
            "node": {
                "type": 1,
                "tag": "div",
                "tagType": 1,
                "props": [
                    {
                        "type": 6,
                        "name": "is",
                        "value": { "content": "vue:foo" }
                    }
                ]
            },
            "context": {}
        }));

        assert_eq!(projection["kind"], json!("asset"));
        assert_eq!(projection["component"], json!("foo"));
        assert_eq!(projection["assetId"], json!("_component_foo"));
    }

    #[test]
    fn base_parse_classifies_lowercase_builtins_and_dynamic_component_as_components() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<teleport/><suspense/><keep-alive/><base-transition/><component/>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let tags = root
            .children
            .iter()
            .map(|id| ast.node(*id).expect("element"))
            .map(|node| match &node.kind {
                Vue3AstKind::Element(element) => (&element.tag, element.tag_type),
                _ => panic!("expected element"),
            })
            .collect::<Vec<_>>();

        assert!(tags
            .iter()
            .all(|(_, tag_type)| *tag_type == Vue3ElementType::Component));
    }

    #[test]
    fn transform_element_props_projection_flags_class_style_and_dynamic_props() {
        let projection = transform_element_props_projection(&json!({
            "props": [
                { "kind": "directiveProp", "name": "class", "valueConstant": false },
                { "kind": "directiveProp", "name": "style", "valueConstant": false },
                { "kind": "directiveProp", "name": "foo", "valueConstant": false }
            ],
            "context": {},
            "isComponent": false
        }));

        assert_eq!(projection["patchFlag"], json!(14));
        assert_eq!(projection["dynamicPropNames"], json!(["foo"]));
        assert_eq!(projection["normalizeClass"], json!(true));
        assert_eq!(projection["normalizeStyle"], json!(true));
    }

    #[test]
    fn transform_element_props_projection_normalizes_style_arrays() {
        let array_literal = transform_element_props_projection(&json!({
            "props": [
                {
                    "kind": "directiveProp",
                    "name": "style",
                    "valueConstant": true,
                    "valueStartsWithArray": true
                }
            ],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(array_literal["normalizeStyle"], json!(true));

        let merged_style = transform_element_props_projection(&json!({
            "props": [
                { "kind": "attribute", "name": "style" },
                {
                    "kind": "directiveProp",
                    "name": "style",
                    "valueConstant": true
                }
            ],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(merged_style["normalizeStyle"], json!(true));
    }

    #[test]
    fn transform_element_props_projection_wraps_object_bind_props() {
        let projection = transform_element_props_projection(&json!({
            "props": [{ "kind": "objectBind" }],
            "context": {},
            "isComponent": false
        }));

        assert_eq!(projection["patchFlag"], json!(16));
        assert_eq!(projection["normalizeProps"], json!(true));
        assert_eq!(projection["guardReactiveProps"], json!(true));
    }

    #[test]
    fn transform_element_props_projection_marks_ref_and_runtime_directives_need_patch() {
        let ref_projection = transform_element_props_projection(&json!({
            "props": [{ "kind": "attribute", "name": "ref" }],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(ref_projection["patchFlag"], json!(512));

        let runtime_projection = transform_element_props_projection(&json!({
            "props": [{ "kind": "runtimeDirective" }],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(runtime_projection["patchFlag"], json!(512));
    }

    #[test]
    fn transform_element_props_projection_marks_ref_for_in_v_for_scope() {
        let static_ref = transform_element_props_projection(&json!({
            "props": [{ "kind": "attribute", "name": "ref" }],
            "context": { "vForDepth": 1 },
            "isComponent": false
        }));
        assert_eq!(static_ref["refForMarker"], json!(true));

        let dynamic_ref = transform_element_props_projection(&json!({
            "props": [
                {
                    "kind": "directiveProp",
                    "name": "ref",
                    "valueConstant": false
                }
            ],
            "context": { "vForDepth": 1 },
            "isComponent": false
        }));
        assert_eq!(dynamic_ref["refForMarker"], json!(true));

        let object_bind = transform_element_props_projection(&json!({
            "props": [{ "kind": "objectBind" }],
            "context": { "vForDepth": 1 },
            "isComponent": false
        }));
        assert_eq!(object_bind["refForMarker"], json!(true));

        let outside_for = transform_element_props_projection(&json!({
            "props": [{ "kind": "attribute", "name": "ref" }],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(outside_for["refForMarker"], json!(false));
    }

    #[test]
    fn transform_element_props_projection_forces_blocks_for_selected_props() {
        let key_projection = transform_element_props_projection(&json!({
            "props": [
                {
                    "kind": "directiveProp",
                    "name": "key",
                    "forceBlock": true
                }
            ],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(key_projection["shouldUseBlock"], json!(true));

        let vnode_hook_projection = transform_element_props_projection(&json!({
            "props": [
                {
                    "kind": "directiveProp",
                    "forceBlock": true
                }
            ],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(vnode_hook_projection["shouldUseBlock"], json!(true));
    }

    #[test]
    fn transform_element_props_projection_projects_inline_template_ref_keys() {
        let projection = transform_element_props_projection(&json!({
            "props": [{ "kind": "attribute", "name": "ref", "value": "input" }],
            "context": {
                "inline": true,
                "bindingMetadata": {
                    "input": "setup-ref"
                }
            },
            "isComponent": false
        }));

        assert_eq!(
            projection["inlineTemplateRefs"],
            json!([{ "content": "input" }])
        );

        let outside_inline = transform_element_props_projection(&json!({
            "props": [{ "kind": "attribute", "name": "ref", "value": "input" }],
            "context": {
                "bindingMetadata": {
                    "input": "setup-ref"
                }
            },
            "isComponent": false
        }));
        assert_eq!(outside_inline["inlineTemplateRefs"], json!([]));
    }

    #[test]
    fn build_directive_args_projection_keeps_runtime_directive_shape() {
        let projection = build_directive_args_projection(&json!({
            "dir": {
                "name": "baz",
                "exp": { "type": 4, "content": "y" },
                "arg": { "type": 4, "content": "arg", "isStatic": false },
                "modifiers": ["mod", "mad"]
            }
        }));

        assert_eq!(
            projection,
            json!({
                "runtime": {
                    "kind": "asset",
                    "name": "baz"
                },
                "includeExp": true,
                "includeArg": true,
                "modifiers": [
                    { "name": "mod" },
                    { "name": "mad" }
                ]
            })
        );
    }

    #[test]
    fn transform_element_children_projection_lowers_builtin_component_children() {
        let suspense = transform_element_children_projection(&json!({
            "tag": "SUSPENSE",
            "children": [
                { "type": 2, "content": "foo" }
            ]
        }));
        assert_eq!(suspense["kind"], json!("slots"));
        assert_eq!(suspense["slots"][0]["name"], json!("default"));
        assert_eq!(suspense["shouldUseBlock"], json!(true));

        let suspense_templates = transform_element_children_projection(&json!({
            "tag": "SUSPENSE",
            "children": [
                {
                    "type": 1,
                    "tag": "template",
                    "props": [
                        {
                            "name": "slot",
                            "arg": { "content": "fallback" }
                        }
                    ]
                }
            ]
        }));
        assert_eq!(suspense_templates["slots"][0]["name"], json!("fallback"));
        assert_eq!(
            suspense_templates["slots"][0]["unwrapTemplate"],
            json!(true)
        );

        let keep_alive = transform_element_children_projection(&json!({
            "tag": "KEEP_ALIVE",
            "children": [
                { "type": 1, "tag": "span" }
            ]
        }));
        assert_eq!(keep_alive["kind"], json!("children"));
        assert_eq!(keep_alive["patchFlag"], json!(1024));
        assert_eq!(keep_alive["shouldUseBlock"], json!(true));
    }

    #[test]
    fn transform_text_projection_merges_and_wraps_text_children() {
        let loc = json!({
            "start": { "offset": 0, "line": 1, "column": 1 },
            "end": { "offset": 0, "line": 1, "column": 1 },
            "source": ""
        });
        let projection = transform_text_projection(&json!({
            "node": {
                "type": 0,
                "children": [
                    { "type": 5, "content": { "type": 4, "content": "foo", "constType": 0 }, "loc": loc },
                    { "type": 2, "content": " bar ", "loc": loc },
                    { "type": 5, "content": { "type": 4, "content": "baz", "constType": 0 }, "loc": loc }
                ]
            },
            "context": {}
        }));

        assert_eq!(projection["operations"][0]["kind"], json!("mergeText"));
        assert_eq!(projection["operations"][0]["start"], json!(0));
        assert_eq!(projection["operations"][0]["end"], json!(2));
        assert_eq!(projection["operations"].as_array().unwrap().len(), 1);

        let wrapped = transform_text_projection(&json!({
            "node": {
                "type": 0,
                "children": [
                    { "type": 1, "tag": "div" },
                    { "type": 2, "content": "hello", "loc": loc },
                    { "type": 1, "tag": "div" }
                ]
            },
            "context": {}
        }));
        assert_eq!(wrapped["operations"][0]["kind"], json!("wrapTextCall"));
        assert_eq!(wrapped["operations"][0]["index"], json!(1));
        assert_eq!(wrapped["operations"][0]["includeContent"], json!(true));
    }

    #[test]
    fn transform_text_projection_honors_compat_and_ssr_boundaries() {
        let loc = json!({
            "start": { "offset": 0, "line": 1, "column": 1 },
            "end": { "offset": 0, "line": 1, "column": 1 },
            "source": ""
        });
        let template = json!({
            "type": 1,
            "tag": "template",
            "tagType": 0,
            "props": [],
            "children": [{ "type": 2, "content": "hello", "loc": loc }]
        });

        assert_eq!(
            transform_text_projection(&json!({ "node": template, "context": { "compat": false } }))
                ["operations"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        let compat_projection =
            transform_text_projection(&json!({ "node": template, "context": { "compat": true } }));
        assert_eq!(
            compat_projection["operations"][0]["kind"],
            json!("wrapTextCall")
        );

        let in_ssr_projection = transform_text_projection(&json!({
            "node": {
                "type": 0,
                "children": [
                    { "type": 1, "tag": "div" },
                    { "type": 5, "content": { "type": 4, "content": "foo", "constType": 0 }, "loc": loc },
                    { "type": 1, "tag": "div" }
                ]
            },
            "context": { "inSSR": true }
        }));
        assert_eq!(
            in_ssr_projection["operations"][0]["patchFlag"],
            json!("1 /* TEXT */")
        );

        let ssr_projection = transform_text_projection(&json!({
            "node": {
                "type": 0,
                "children": [
                    { "type": 1, "tag": "div" },
                    { "type": 5, "content": { "type": 4, "content": "foo", "constType": 0 }, "loc": loc },
                    { "type": 1, "tag": "div" }
                ]
            },
            "context": { "ssr": true, "inSSR": true }
        }));
        assert!(ssr_projection["operations"][0]
            .get("patchFlag")
            .is_none_or(Value::is_null));
    }

    #[test]
    fn transform_element_props_projection_marks_hydration_event_without_props_for_constants() {
        let projection = transform_element_props_projection(&json!({
            "props": [
                {
                    "kind": "directiveProp",
                    "name": "onKeydown",
                    "valueConstant": true
                }
            ],
            "context": {},
            "isComponent": false
        }));

        assert_eq!(projection["patchFlag"], json!(32));
        assert_eq!(projection["dynamicPropNames"], json!([]));
    }

    #[test]
    fn base_parse_decodes_builtin_text_entities() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "&gt;&lt;&amp;&apos;&quot;&nbsp;&foo;".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let text = ast.node(root.children[0]).expect("text");
        assert!(matches!(
            &text.kind,
            Vue3AstKind::Text(value) if value.value == "><&'\"\u{00a0}&foo;"
        ));
        assert_eq!(text.span.source(), Some(Span::new(FileId(0), 0, 36)));
    }

    #[test]
    fn base_parse_decodes_directive_expression_entities_but_keeps_raw_span() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<span :class="'foo' + '&gt;ar'"/>"#.into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let element = ast.node(root.children[0]).expect("element");
        let Vue3AstKind::Element(element) = &element.kind else {
            panic!("expected element");
        };
        let Vue3Prop::Directive(dir) = &element.props[0] else {
            panic!("expected directive");
        };
        assert_eq!(
            dir.exp
                .as_ref()
                .map(Vue3Expression::source_string)
                .as_deref(),
            Some("'foo' + '>ar'")
        );
        assert_eq!(dir.exp_span, Some(Span::new(FileId(0), 14, 30)));
    }

    #[test]
    fn base_parse_preserves_nbsp_as_non_whitespace_default_child() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source:
                "<Comp>\n        \u{00a0}\n        <template #one>foo</template>\n      </Comp>"
                    .into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let comp_id = root.children[0];
        let comp = ast.node(comp_id).expect("component");
        assert!(matches!(
            ast.node(comp.children[0]).map(|node| &node.kind),
            Some(Vue3AstKind::Text(text)) if text.value.contains('\u{00a0}')
        ));
    }

    #[test]
    fn scope_ref_identifier_matching_uses_boundaries() {
        assert!(source_contains_identifier("fn(i)", "i"));
        assert!(!source_contains_identifier("click", "i"));
        assert!(!source_contains_identifier("_ctx.list", "i"));
    }

    #[test]
    fn base_parse_preserves_raw_content_inside_v_pre() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-pre :id="foo"><Comp/>{{ bar }}</div><div :id="foo"><Comp/>{{ bar }}</div>"#.into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let with_pre = ast.node(root.children[0]).expect("v-pre div");
        let Vue3AstKind::Element(with_pre_element) = &with_pre.kind else {
            panic!("expected element");
        };
        assert_eq!(with_pre_element.props.len(), 1);
        assert!(matches!(
            &with_pre_element.props[0],
            Vue3Prop::Attribute(attr) if attr.name == ":id" && attr.value.as_deref() == Some("foo")
        ));
        let raw_component = ast.node(with_pre.children[0]).expect("raw component");
        assert!(matches!(
            &raw_component.kind,
            Vue3AstKind::Element(element)
                if element.tag == "Comp" && element.tag_type == Vue3ElementType::Element
        ));
        let raw_text = ast.node(with_pre.children[1]).expect("raw interpolation");
        assert!(matches!(
            &raw_text.kind,
            Vue3AstKind::Text(text) if text.value == "{{ bar }}"
        ));

        let without_pre = ast.node(root.children[1]).expect("normal div");
        let Vue3AstKind::Element(without_pre_element) = &without_pre.kind else {
            panic!("expected element");
        };
        assert!(matches!(
            &without_pre_element.props[0],
            Vue3Prop::Directive(dir) if dir.name == "bind"
        ));
        let component = ast.node(without_pre.children[0]).expect("component");
        assert!(matches!(
            &component.kind,
            Vue3AstKind::Element(element)
                if element.tag == "Comp" && element.tag_type == Vue3ElementType::Component
        ));
        let interpolation = ast.node(without_pre.children[1]).expect("interpolation");
        assert!(matches!(interpolation.kind, Vue3AstKind::Interpolation(_)));
    }

    #[test]
    fn base_parse_splits_half_open_interpolations_inside_v_pre() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div v-pre><span>{{ number </span><span>}}</span></div>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let div = ast.node(root.children[0]).expect("div");
        let first_span = ast.node(div.children[0]).expect("first span");
        let second_span = ast.node(div.children[1]).expect("second span");

        assert!(matches!(
            &ast.node(first_span.children[0]).expect("first text").kind,
            Vue3AstKind::Text(text) if text.value == "{{ number "
        ));
        assert!(matches!(
            &ast.node(second_span.children[0]).expect("second text").kind,
            Vue3AstKind::Text(text) if text.value == "}}"
        ));
    }

    #[test]
    fn base_parse_preserves_inter_element_whitespace_in_preserve_mode() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div/> \n <div/>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(
            source,
            &Vue3CompilerOptions {
                whitespace: "preserve".into(),
                ..Vue3CompilerOptions::default()
            },
        );
        let root = ast.root_node().expect("root");
        assert_eq!(root.children.len(), 3);
        assert!(matches!(
            &ast.node(root.children[1]).expect("whitespace text").kind,
            Vue3AstKind::Text(text) if text.value == " "
        ));
    }

    #[test]
    fn base_parse_preserves_text_inside_configured_pre_tag() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<pre>\n  foo  bar  </pre><span>\n  foo   bar</span>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(
            source,
            &Vue3CompilerOptions {
                pre_tags: vec!["pre".into()],
                ignore_newline_tags: vec!["pre".into()],
                ..Vue3CompilerOptions::default()
            },
        );
        let root = ast.root_node().expect("root");
        let pre = ast.node(root.children[0]).expect("pre");
        let span = ast.node(root.children[1]).expect("span");
        assert!(matches!(
            &ast.node(pre.children[0]).expect("pre text").kind,
            Vue3AstKind::Text(text) if text.value == "  foo  bar  "
        ));
        assert!(matches!(
            &ast.node(span.children[0]).expect("span text").kind,
            Vue3AstKind::Text(text) if text.value == " foo bar"
        ));
    }

    #[test]
    fn base_parse_extends_open_element_spans_to_eof() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<template><div>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let template = ast.node(root.children[0]).expect("template");
        let div = ast.node(template.children[0]).expect("div");
        assert_eq!(template.span.source(), Some(Span::new(FileId(0), 0, 15)));
        assert_eq!(div.span.source(), Some(Span::new(FileId(0), 10, 15)));
    }

    #[test]
    fn base_parse_recovers_from_incomplete_child_start_tag_at_eof() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<template><div id=abc /".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let template = ast.node(root.children[0]).expect("template");
        assert_eq!(template.span.source(), Some(Span::new(FileId(0), 0, 23)));
        assert_eq!(template.children.len(), 1);
        assert!(matches!(
            &ast.node(template.children[0]).expect("recovered text").kind,
            Vue3AstKind::Text(text) if text.value == "/"
        ));
    }

    #[test]
    fn base_parse_treats_empty_incomplete_end_tag_as_text() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<template></".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let template = ast.node(root.children[0]).expect("template");
        assert_eq!(template.span.source(), Some(Span::new(FileId(0), 0, 12)));
        assert!(matches!(
            &ast.node(template.children[0]).expect("end tag text").kind,
            Vue3AstKind::Text(text) if text.value == "</"
        ));
    }

    #[test]
    fn base_parse_uses_configured_namespace_for_cdata_text() {
        let mut namespaces = BTreeMap::new();
        namespaces.insert("svg".into(), vuec_ast::HtmlNamespace::Svg);
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<template><svg><![CDATA[cdata]]></svg></template>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(
            source,
            &Vue3CompilerOptions {
                namespaces,
                ..Vue3CompilerOptions::default()
            },
        );
        let root = ast.root_node().expect("root");
        let template = ast.node(root.children[0]).expect("template");
        let svg = ast.node(template.children[0]).expect("svg");
        assert!(matches!(
            &svg.kind,
            Vue3AstKind::Element(element) if element.ns == vuec_ast::HtmlNamespace::Svg
        ));
        assert!(matches!(
            &ast.node(svg.children[0]).expect("cdata text").kind,
            Vue3AstKind::Text(text) if text.value == "cdata"
        ));
        assert_eq!(
            ast.node(svg.children[0]).expect("cdata text").span.source(),
            Some(Span::new(FileId(0), 24, 29))
        );
    }

    #[test]
    fn base_parse_drops_cdata_children_in_html_namespace() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<template><![CDATA[cdata]]></template>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let template = ast.node(root.children[0]).expect("template");
        assert!(template.children.is_empty());
    }

    #[test]
    fn base_parse_keeps_non_matching_end_tag_as_text_in_textarea() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<textarea></div></textarea>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let textarea = ast.node(root.children[0]).expect("textarea");
        assert_eq!(textarea.span.source(), Some(Span::new(FileId(0), 0, 27)));
        assert!(matches!(
            &ast.node(textarea.children[0]).expect("raw end tag text").kind,
            Vue3AstKind::Text(text) if text.value == "</div>"
        ));
        assert_eq!(
            ast.node(textarea.children[0])
                .expect("raw end tag text")
                .span
                .source(),
            Some(Span::new(FileId(0), 10, 16))
        );
    }

    #[test]
    fn base_parse_extends_open_span_across_invalid_end_tags() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<template></div></template>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let template = ast.node(root.children[0]).expect("template");
        assert_eq!(template.span.source(), Some(Span::new(FileId(0), 0, 27)));
        assert!(template.children.is_empty());
    }

    #[test]
    fn base_parse_treats_html_textarea_and_style_as_special_text_modes() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<textarea>some<div>text</div>and<!--comment--></textarea><style>&amp;</style>"
                .into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let textarea = ast.node(root.children[0]).expect("textarea");
        let style = ast.node(root.children[1]).expect("style");

        assert_eq!(textarea.children.len(), 1);
        assert!(matches!(
            &ast.node(textarea.children[0]).expect("textarea text").kind,
            Vue3AstKind::Text(text) if text.value == "some<div>text</div>and<!--comment-->"
        ));
        assert_eq!(style.children.len(), 1);
        assert!(matches!(
            &ast.node(style.children[0]).expect("style text").kind,
            Vue3AstKind::Text(text) if text.value == "&amp;"
        ));
    }

    #[test]
    fn base_parse_textarea_decodes_entities_and_supports_interpolation() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<textarea>\n<div>{{ a &lt; b }}</textarea>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(
            source,
            &Vue3CompilerOptions {
                ignore_newline_tags: vec!["textarea".into()],
                ..Vue3CompilerOptions::default()
            },
        );
        let root = ast.root_node().expect("root");
        let textarea = ast.node(root.children[0]).expect("textarea");

        assert_eq!(textarea.children.len(), 2);
        assert!(matches!(
            &ast.node(textarea.children[0]).expect("textarea text").kind,
            Vue3AstKind::Text(text) if text.value == "<div>"
        ));
        assert_eq!(
            ast.node(textarea.children[0])
                .expect("textarea text")
                .span
                .source(),
            Some(Span::new(FileId(0), 10, 16))
        );
        assert!(matches!(
            &ast.node(textarea.children[1]).expect("interpolation").kind,
            Vue3AstKind::Interpolation(interpolation)
                if interpolation.expression.source_string() == "a < b"
        ));
    }

    #[test]
    fn base_parse_decodes_dom_text_and_attribute_entity_compatibility() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div a="&ampersand;" b="&amp;ersand;" c="&amp!">&ampersand;&#x86;</div>"#
                .into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let div = ast.node(root.children[0]).expect("div");
        let Vue3AstKind::Element(element) = &div.kind else {
            panic!("expected element");
        };

        assert!(matches!(
            &element.props[0],
            Vue3Prop::Attribute(attr) if attr.value.as_deref() == Some("&ampersand;")
        ));
        assert!(matches!(
            &element.props[1],
            Vue3Prop::Attribute(attr) if attr.value.as_deref() == Some("&ersand;")
        ));
        assert!(matches!(
            &element.props[2],
            Vue3Prop::Attribute(attr) if attr.value.as_deref() == Some("&!")
        ));
        assert!(matches!(
            &ast.node(div.children[0]).expect("text").kind,
            Vue3AstKind::Text(text) if text.value == "&ersand;\u{2020}"
        ));
    }

    #[test]
    fn base_parse_applies_dom_namespace_rules_without_static_namespace_map() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: concat!(
                "<svg><foreignObject><test/></foreignObject></svg>",
                "<math><mtext><test/></mtext><mtext><malignmark/></mtext></math>",
            )
            .into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(
            source,
            &Vue3CompilerOptions {
                dom_namespaces: true,
                ..Vue3CompilerOptions::default()
            },
        );
        let root = ast.root_node().expect("root");
        let svg = ast.node(root.children[0]).expect("svg");
        let math = ast.node(root.children[1]).expect("math");
        let foreign_object = ast.node(svg.children[0]).expect("foreignObject");
        let svg_test = ast.node(foreign_object.children[0]).expect("svg test");
        let mtext_html = ast.node(math.children[0]).expect("mtext html");
        let mtext_math = ast.node(math.children[1]).expect("mtext math");
        let math_test = ast.node(mtext_html.children[0]).expect("math test");
        let malignmark = ast.node(mtext_math.children[0]).expect("malignmark");

        assert!(matches!(
            &svg.kind,
            Vue3AstKind::Element(element) if element.ns == vuec_ast::HtmlNamespace::Svg
        ));
        assert!(matches!(
            &svg_test.kind,
            Vue3AstKind::Element(element) if element.ns == vuec_ast::HtmlNamespace::Html
        ));
        assert!(matches!(
            &math.kind,
            Vue3AstKind::Element(element) if element.ns == vuec_ast::HtmlNamespace::MathMl
        ));
        assert!(matches!(
            &math_test.kind,
            Vue3AstKind::Element(element) if element.ns == vuec_ast::HtmlNamespace::Html
        ));
        assert!(matches!(
            &malignmark.kind,
            Vue3AstKind::Element(element) if element.ns == vuec_ast::HtmlNamespace::MathMl
        ));
    }

    #[test]
    fn base_parse_uses_root_namespace_for_dom_integration_rules() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<foreignObject><test/></foreignObject><script><g/><g/></script>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(
            source,
            &Vue3CompilerOptions {
                root_namespace: vuec_ast::HtmlNamespace::Svg,
                dom_namespaces: true,
                ..Vue3CompilerOptions::default()
            },
        );
        let root = ast.root_node().expect("root");
        let foreign_object = ast.node(root.children[0]).expect("foreignObject");
        let script = ast.node(root.children[1]).expect("script");
        let test = ast.node(foreign_object.children[0]).expect("test");

        assert!(matches!(
            &foreign_object.kind,
            Vue3AstKind::Element(element) if element.ns == vuec_ast::HtmlNamespace::Svg
        ));
        assert!(matches!(
            &test.kind,
            Vue3AstKind::Element(element) if element.ns == vuec_ast::HtmlNamespace::Html
        ));
        assert_eq!(script.children.len(), 2);
        assert!(script.children.iter().all(|child| {
            matches!(
                ast.node(*child).map(|node| &node.kind),
                Some(Vue3AstKind::Element(_))
            )
        }));
    }
}
