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
