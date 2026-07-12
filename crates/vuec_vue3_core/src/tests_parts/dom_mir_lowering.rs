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
        let options = Vue3CompilerOptions {
            built_in_components: vec!["transition".into()],
            ..Vue3CompilerOptions::default()
        };
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
