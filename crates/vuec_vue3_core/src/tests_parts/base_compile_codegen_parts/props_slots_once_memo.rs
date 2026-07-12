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
    fn base_compile_gives_v_if_precedence_over_v_for_on_the_same_element() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div><span v-if="ok" v-for="x in xs">{{ x }}</span><span v-else v-for="y in ys">{{ y }}</span></div>"#.into(),
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
        let condition = result.code.find("_ctx.ok").expect("v-if condition");
        let first_loop = result
            .code
            .find("_renderList(_ctx.xs")
            .expect("v-for loop");
        assert!(condition < first_loop);
        assert!(result
            .code
            .contains("_Fragment, { key: 0 }, _renderList(_ctx.xs, (x) =>"));
        assert!(result
            .code
            .contains("_Fragment, { key: 1 }, _renderList(_ctx.ys, (y) =>"));
        assert!(result.code.contains("_toDisplayString(x)"));
        assert!(result.code.contains("_toDisplayString(y)"));
        assert!(!result.code.contains("_toDisplayString(_ctx.x)"));
        assert!(!result.code.contains("_toDisplayString(_ctx.y)"));

        let scoped_condition = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<li v-if="item.ok" v-for="item in items">{{ item.name }}</li>"#
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
        let condition = scoped_condition
            .code
            .find("_ctx.item.ok")
            .expect("outer v-if condition");
        let loop_start = scoped_condition
            .code
            .find("_renderList(_ctx.items, (item) =>")
            .expect("inner v-for loop");
        assert!(condition < loop_start);
        assert!(scoped_condition
            .code
            .contains("_toDisplayString(item.name)"));
        assert!(!scoped_condition
            .code
            .contains("_toDisplayString(_ctx.item.name)"));

        let template = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<template v-if="show" v-for="item in items"><i>{{ item }}</i></template>"#
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
        assert!(template.diagnostics.is_empty(), "{:?}", template.diagnostics);
        assert!(template
            .code
            .contains("_Fragment, { key: 0 }, _renderList(_ctx.items, (item) =>"));
        assert!(template.code.contains("_toDisplayString(item)"));
        assert!(!template.code.contains("_toDisplayString(_ctx.item)"));

        let memo = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<li v-if="ok" v-for="item in items" :key="item.id" v-memo="[item.id]">{{ item.name }}</li>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(memo
            .code
            .contains("_Fragment, { key: 0 }, _renderList(_ctx.items"));
        assert!(memo.code.contains("_cached.key === item.id"));
        assert!(memo.code.contains("const _memo = ([item.id])"));

        let once_memo = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<li v-if="ok" v-for="item in items" :key="item.id" v-memo="[item.id]" v-once>{{ item.name }}</li>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(once_memo.code.contains("_cache[2] || ("));
        assert!(once_memo.code.contains("_renderList(_ctx.items"));
        assert!(once_memo.code.contains("}, _cache, 0)"));
        assert!(once_memo.code.contains(")).cacheIndex = 2"));
        assert!(!once_memo.code.contains("_cache[1] || ("));
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
