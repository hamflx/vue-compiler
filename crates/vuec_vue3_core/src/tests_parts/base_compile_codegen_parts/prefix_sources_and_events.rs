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
    fn base_compile_source_map_maps_each_repeated_interpolation_identifier() {
        let source = "<div>{{ value + value }}</div>";
        let result = base_compile(
            TemplateSource {
                filename: "Repeated.vue".into(),
                source: source.into(),
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

        let generated_offsets = result
            .code
            .match_indices("_ctx.value")
            .map(|(offset, _)| offset)
            .collect::<Vec<_>>();
        let original_offsets = source
            .match_indices("value")
            .map(|(offset, _)| offset)
            .collect::<Vec<_>>();
        assert_eq!(generated_offsets.len(), 2);
        assert_eq!(original_offsets.len(), 2);

        let map = result.map.expect("source map");
        assert_eq!(map.names, vec!["value"]);
        for (generated_offset, original_offset) in
            generated_offsets.into_iter().zip(original_offsets)
        {
            let generated = loc_for_offset(&result.code, generated_offset).expect("generated loc");
            let original = map
                .original_position(vuec_source::GeneratedPosition::new(
                    generated.0,
                    generated.1,
                ))
                .expect("source map lookup")
                .expect("original position");
            let expected = loc_for_offset(source, original_offset).expect("source loc");
            assert_eq!((original.line, original.column), expected);
            assert_eq!(original.name.as_deref(), Some("value"));
        }
    }

    #[test]
    fn base_compile_source_map_advances_across_repeated_interpolations() {
        let source = "<div>{{ value + value }} / {{ value }}</div>";
        let result = base_compile(
            TemplateSource {
                filename: "Repeated.vue".into(),
                source: source.into(),
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

        let generated_offsets = result
            .code
            .match_indices("_ctx.value")
            .map(|(offset, _)| offset)
            .collect::<Vec<_>>();
        let original_offsets = source
            .match_indices("value")
            .map(|(offset, _)| offset)
            .collect::<Vec<_>>();
        assert_eq!(generated_offsets.len(), 3);
        assert_eq!(original_offsets.len(), 3);

        let map = result.map.expect("source map");
        for (generated_offset, original_offset) in
            generated_offsets.into_iter().zip(original_offsets)
        {
            let generated = loc_for_offset(&result.code, generated_offset).expect("generated loc");
            let original = map
                .original_position(vuec_source::GeneratedPosition::new(
                    generated.0,
                    generated.1,
                ))
                .expect("source map lookup")
                .expect("original position");
            let expected = loc_for_offset(source, original_offset).expect("source loc");
            assert_eq!((original.line, original.column), expected);
            assert_eq!(original.name.as_deref(), Some("value"));
        }
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
