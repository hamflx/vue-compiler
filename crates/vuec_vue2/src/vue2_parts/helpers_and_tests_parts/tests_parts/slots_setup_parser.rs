    #[test]
    fn generates_vue2_scoped_slots_like_official_codegen() {
        let default_template = compile(
            r#"<foo><template slot-scope="bar">{{ bar }}</template></foo>"#,
            options(),
        );
        assert_eq!(
            default_template.render,
            r#"with(this){return _c('foo',{scopedSlots:_u([{key:"default",fn:function(bar){return [_v(_s(bar))]}}])})}"#
        );

        let default_element = compile(
            r#"<foo><div slot-scope="bar">{{ bar }}</div></foo>"#,
            options(),
        );
        assert_eq!(
            default_element.render,
            r#"with(this){return _c('foo',{scopedSlots:_u([{key:"default",fn:function(bar){return _c('div',{},[_v(_s(bar))])}}])})}"#
        );

        let dynamic_slot = compile(
            r#"<foo><template :slot="foo" slot-scope="bar">{{ bar }}</template></foo>"#,
            options(),
        );
        assert_eq!(
            dynamic_slot.render,
            r#"with(this){return _c('foo',{scopedSlots:_u([{key:foo,fn:function(bar){return [_v(_s(bar))]}}],null,true)})}"#
        );

        let legacy_if = compile(
            "<foo><template v-if=\"\nshow\n\" slot-scope=\"bar\">{{ bar }}</template></foo>",
            options(),
        );
        assert_eq!(
            legacy_if.render,
            "with(this){return _c('foo',{scopedSlots:_u([{key:\"default\",fn:function(bar){return (\nshow\n)?[_v(_s(bar))]:undefined}}],null,true)})}"
        );

        let new_syntax_if = compile(
            r#"<foo><template v-if="show" #default="bar">{{ bar }}</template></foo>"#,
            options(),
        );
        assert_eq!(
            new_syntax_if.render,
            r#"with(this){return _c('foo',{scopedSlots:_u([(show)?{key:"default",fn:function(bar){return [_v(_s(bar))]}}:null],null,true)})}"#
        );

        let new_syntax_if_else = compile(
            r#"<foo><template #trigger v-if="isPublic"><button>a</button></template><template #trigger v-else><button>b</button></template></foo>"#,
            options(),
        );
        assert_eq!(
            new_syntax_if_else.render,
            r#"with(this){return _c('foo',{scopedSlots:_u([(isPublic)?{key:"trigger",fn:function(){return [_c('button',[_v("a")])]},proxy:true}:{key:"trigger",fn:function(){return [_c('button',[_v("b")])]},proxy:true}],null,true)})}"#
        );

        let new_syntax_if_else_if = compile(
            r#"<foo><template #trigger v-if="a"><button>a</button></template><template #trigger v-else-if="b"><button>b</button></template><template #trigger v-else><button>c</button></template></foo>"#,
            options(),
        );
        assert_eq!(
            new_syntax_if_else_if.render,
            r#"with(this){return _c('foo',{scopedSlots:_u([(a)?{key:"trigger",fn:function(){return [_c('button',[_v("a")])]},proxy:true}:(b)?{key:"trigger",fn:function(){return [_c('button',[_v("b")])]},proxy:true}:{key:"trigger",fn:function(){return [_c('button',[_v("c")])]},proxy:true}],null,true)})}"#
        );

        let new_syntax_if_else_different_names = compile(
            r#"<foo><template #a v-if="ok"><span>a</span></template><template #b v-else><span>b</span></template></foo>"#,
            options(),
        );
        assert_eq!(
            new_syntax_if_else_different_names.render,
            r#"with(this){return _c('foo',{scopedSlots:_u([(ok)?{key:"a",fn:function(){return [_c('span',[_v("a")])]},proxy:true}:{key:"b",fn:function(){return [_c('span',[_v("b")])]},proxy:true}],null,true)})}"#
        );

        let new_syntax_else_without_slot_binding = compile(
            r#"<foo><template #a v-if="ok"><span>a</span></template><template v-else><span>b</span></template></foo>"#,
            options(),
        );
        assert_eq!(
            new_syntax_else_without_slot_binding.render,
            r#"with(this){return _c('foo',{scopedSlots:_u([(ok)?{key:"a",fn:function(){return [_c('span',[_v("a")])]},proxy:true}:{key:"default",fn:function(undefined){return [_c('span',[_v("b")])]}}],null,true)})}"#
        );

        let legacy_if_else = compile(
            r#"<foo><template slot="trigger" slot-scope="s" v-if="isPublic"><button>a</button></template><template slot="trigger" slot-scope="s" v-else><button>b</button></template></foo>"#,
            options(),
        );
        assert_eq!(
            legacy_if_else.render,
            r#"with(this){return _c('foo',{scopedSlots:_u([{key:"trigger",fn:function(s){return (isPublic)?[_c('button',[_v("a")])]:undefined}}],null,true)})}"#
        );

        let parent_if_key = compile(
            r#"<div v-if="ok"><foo><template #default="s"><span>{{ s.x }}</span></template></foo></div>"#,
            options(),
        );
        assert_eq!(
            parent_if_key.render,
            r#"with(this){return (ok)?_c('div',[_c('foo',{scopedSlots:_u([{key:"default",fn:function(s){return [_c('span',[_v(_s(s.x))])]}}],null,false,2164623802)})],1):_e()}"#
        );

        let if_chain_slot_fallback = compile(
            r#"<div v-if="loading"></div><div v-else-if="empty"><slot name="empty"><foo><template #icon><span/></template></foo></slot></div>"#,
            options(),
        );
        assert_eq!(
            if_chain_slot_fallback.render,
            r#"with(this){return (loading)?_c('div'):(empty)?_c('div',[_t("empty",function(){return [_c('foo',{scopedSlots:_u([{key:"icon",fn:function(){return [_c('span')]},proxy:true}])})]})],2):_e()}"#
        );

        let parent_for_force_update = compile(
            r#"<div v-for="item in items"><foo><template #default="s"><span>{{ item }}{{ s.x }}</span></template></foo></div>"#,
            options(),
        );
        assert_eq!(
            parent_for_force_update.render,
            r#"with(this){return _l((items),function(item){return _c('div',[_c('foo',{scopedSlots:_u([{key:"default",fn:function(s){return [_c('span',[_v(_s(item)+_s(s.x))])]}}],null,true)})],1)})}"#
        );

        let slot_for_force_update = compile(
            r#"<foo><template v-for="item in items" #default="s"><span>{{ item }}{{ s.x }}</span></template></foo>"#,
            options(),
        );
        assert_eq!(
            slot_for_force_update.render,
            r#"with(this){return _c('foo',{scopedSlots:_u([_l((items),function(item){return {key:"default",fn:function(s){return [_c('span',[_v(_s(item)+_s(s.x))])]}}})],null,true)})}"#
        );

        let contains_slot_child_force_update = compile(
            r#"<foo><template #default="s"><slot></slot></template></foo>"#,
            options(),
        );
        assert_eq!(
            contains_slot_child_force_update.render,
            r#"with(this){return _c('foo',{scopedSlots:_u([{key:"default",fn:function(s){return [_t("default")]}}],null,true)})}"#
        );

        let nested_scoped_slot_force_update_stays_inner = compile(
            r#"<van-field><template v-slot:button><ui-page><template v-slot:header><div v-if="multiple"><slot></slot></div>single<div v-else></div></template></ui-page></template></van-field>"#,
            options(),
        );
        assert_eq!(
            nested_scoped_slot_force_update_stays_inner.render,
            r#"with(this){return _c('van-field',{scopedSlots:_u([{key:"button",fn:function(){return [_c('ui-page',{scopedSlots:_u([{key:"header",fn:function(){return [(multiple)?_c('div',[_t("default")],2):_c('div')]},proxy:true}],null,true)})]},proxy:true}])})}"#
        );
    }

    #[test]
    fn generates_vue2_slot_outlet_props_like_official_codegen() {
        let no_fallback = compile(
            r#"<div><slot :has-refinements="canRefine" :refine="state.refine" /></div>"#,
            options(),
        );
        assert_eq!(
            no_fallback.render,
            r#"with(this){return _c('div',[_t("default",null,{"hasRefinements":canRefine,"refine":state.refine})],2)}"#
        );

        let fallback = compile(
            r#"<div><slot foo="bar" :has-refinements="canRefine"><span>x</span></slot></div>"#,
            options(),
        );
        assert_eq!(
            fallback.render,
            r#"with(this){return _c('div',[_t("default",function(){return [_c('span',[_v("x")])]},{"foo":"bar","hasRefinements":canRefine})],2)}"#
        );

        let dynamic_prop = compile(r#"<div><slot v-bind:[foo]="bar | baz" /></div>"#, options());
        assert_eq!(
            dynamic_prop.render,
            r#"with(this){return _c('div',[_t("default",null,_d({},[foo,_f("baz")(bar)]))],2)}"#
        );

        let bind_object = compile(r#"<div><slot v-bind="slotProps" /></div>"#, options());
        assert_eq!(
            bind_object.render,
            r#"with(this){return _c('div',[_t("default",null,null,slotProps)],2)}"#
        );
    }

    #[test]
    fn generates_vue2_inline_template_like_official_codegen() {
        let single = compile(
            r#"<my-component inline-template><p><span>hello world</span></p></my-component>"#,
            options(),
        );
        assert_eq!(
            single.render,
            r#"with(this){return _c('my-component',{inlineTemplate:{render:function(){with(this){return _m(0)}},staticRenderFns:[function(){with(this){return _c('p',[_c('span',[_v("hello world")])])}}]}})}"#
        );

        let multiple = compile(
            r#"<my-component inline-template><hr><hr></my-component>"#,
            options(),
        );
        assert_eq!(
            multiple.render,
            r#"with(this){return _c('my-component',{inlineTemplate:{render:function(){with(this){return _c('hr')}},staticRenderFns:[]}})}"#
        );
        assert!(multiple
            .errors
            .iter()
            .any(|error| error.msg
                == "Inline-template components must have exactly one child element."));

        let empty = compile(
            r#"<my-component inline-template></my-component>"#,
            options(),
        );
        assert_eq!(empty.render, r#"with(this){return _c('my-component',{})}"#);
    }

    #[test]
    fn generates_vue27_setup_binding_component_tags_like_official_codegen() {
        let mut parsed = compile("<div><Foo/><foo-bar></foo-bar></div>", options())
            .element_ast
            .unwrap();
        optimize(&mut parsed, &options());
        let generated = generate(
            Some(&parsed),
            &Vue2CompileOptions {
                bindings: BTreeMap::from([
                    ("Foo".into(), "setup-const".into()),
                    ("FooBar".into(), "setup-const".into()),
                ]),
                ..options()
            },
        );
        assert_eq!(
            generated.render,
            r#"with(this){return _c('div',[_c(Foo),_c(FooBar)],1)}"#
        );
    }

    #[test]
    fn vue27_setup_bindings_do_not_resolve_native_tags() {
        let mut parsed = compile("<div><form>{{ n }}</form></div>", options())
            .element_ast
            .unwrap();
        optimize(&mut parsed, &options());
        let generated = generate(
            Some(&parsed),
            &Vue2CompileOptions {
                bindings: BTreeMap::from([("form".into(), "setup-const".into())]),
                ..options()
            },
        );
        assert_eq!(
            generated.render,
            r#"with(this){return _c('div',[_c('form',[_v(_s(n))])])}"#
        );
    }

    #[test]
    fn generates_vue2_v_pre_template_like_official_codegen() {
        let result = compile(
            r#"<div v-pre><template><p>{{msg}}</p></template></div>"#,
            options(),
        );
        assert_eq!(result.render, r#"with(this){return _m(0)}"#);
        assert_eq!(
            result.static_render_fns,
            vec![
                r#"with(this){return _c('div',{pre:true},[_c('template',[_c('p',[_v("{{msg}}")])])],2)}"#
                    .to_string()
            ]
        );

        let inherited_data = compile(
            r#"<div v-pre><p id="x"><img alt="a"></p><span>{{msg}}</span></div>"#,
            options(),
        );
        assert_eq!(inherited_data.render, r#"with(this){return _m(0)}"#);
        assert_eq!(
            inherited_data.static_render_fns,
            vec![
                r#"with(this){return _c('div',{pre:true},[_c('p',{pre:true,attrs:{"id":"x"}},[_c('img',{pre:true,attrs:{"alt":"a"}})]),_c('span',[_v("{{msg}}")])])}"#
                    .to_string()
            ]
        );

        let plain_child = compile(r#"<div v-pre><p></p></div>"#, options());
        assert_eq!(plain_child.render, r#"with(this){return _m(0)}"#);
        assert_eq!(
            plain_child.static_render_fns,
            vec![r#"with(this){return _c('div',{pre:true},[_c('p')])}"#.to_string()]
        );

        let component_child = compile(r#"<div v-pre><my-widget></my-widget></div>"#, options());
        assert_eq!(
            component_child.render,
            r#"with(this){return _c('div',{pre:true},[_c('my-widget',{pre:true})],1)}"#
        );
        assert!(component_child.static_render_fns.is_empty());
    }

    #[test]
    fn parses_vue2_raw_text_elements_like_official_parser() {
        let textarea = compile(
            "<textarea>\n        <p>Test 1</p>\n        test2\n      </textarea>",
            options(),
        );
        let textarea = textarea.element_ast.unwrap();
        assert_eq!(textarea.tag, "textarea");
        assert_eq!(textarea.children.len(), 1);
        match &textarea.children[0] {
            Vue2Node::Text(text) => {
                assert_eq!(text.text, "        <p>Test 1</p>\n        test2\n      ");
                assert!(text.expression.is_none());
            }
            Vue2Node::Element(_) => panic!("textarea content must stay raw text"),
        }

        let script = compile(
            r#"<script type="x/template">&gt;<foo>&lt;</script>"#,
            options(),
        );
        let script = script.element_ast.unwrap();
        assert_eq!(script.tag, "script");
        assert_eq!(script.children.len(), 1);
        match &script.children[0] {
            Vue2Node::Text(text) => assert_eq!(text.text, "&gt;<foo>&lt;"),
            Vue2Node::Element(_) => panic!("script template content must stay raw text"),
        }
    }

    #[test]
    fn decodes_vue2_text_entities_like_official_parser() {
        let numeric = compile("<span>&#10004;&#x2714;</span>", options());
        assert_eq!(
            numeric.render,
            r#"with(this){return _c('span',[_v("✔✔")])}"#
        );

        let text_mode = compile("<span>&ampersand;&Eacute;&#x80;&#0;</span>", options());
        assert_eq!(
            text_mode.render,
            r#"with(this){return _c('span',[_v("&ersand;É€�")])}"#
        );

        let named = compile(
            "<span>&larr;&uarr;&rarr;&darr;&mdash;&ndash;&copy;&reg;&trade;&plus;&times;&lsaquo;&rsaquo;</span>",
            options(),
        );
        assert_eq!(
            named.render,
            r#"with(this){return _c('span',[_v("←↑→↓—–©®™+×‹›")])}"#
        );

        let textarea = compile("<textarea>&#10004;</textarea>", options());
        assert_eq!(
            textarea.render,
            r#"with(this){return _c('textarea',[_v("✔")])}"#
        );

        let script = compile(
            r#"<script type="x/template">&gt;<foo>&lt;&#10004;</script>"#,
            options(),
        )
        .element_ast
        .unwrap();
        match &script.children[0] {
            Vue2Node::Text(text) => assert_eq!(text.text, "&gt;<foo>&lt;&#10004;"),
            Vue2Node::Element(_) => panic!("script template content must stay raw text"),
        }
    }

    #[test]
    fn decodes_vue2_attr_entities_like_official_parser() {
        let no_optimize_options = || Vue2CompileOptions {
            optimize: false,
            ..options()
        };
        let attrs = compile(
            r#"<div title="&quot; &amp; &lt; &gt; &#39; &apos; &copy; &rarr; &#34; &#x22; &#x27;"/>"#,
            no_optimize_options(),
        );
        assert_eq!(
            attrs.render,
            r#"with(this){return _c('div',{attrs:{"title":"\" & < > ' &apos; &copy; &rarr; &#34; &#x22; &#x27;"}})}"#
        );

        let one_pass = compile(r#"<div title="&amp;quot;"/>"#, no_optimize_options());
        assert_eq!(
            one_pass.render,
            r#"with(this){return _c('div',{attrs:{"title":"&quot;"}})}"#
        );

        let non_semicolon = compile(
            r#"<div title="&quot &amp &lt &gt &#39"/>"#,
            no_optimize_options(),
        );
        assert_eq!(
            non_semicolon.render,
            r#"with(this){return _c('div',{attrs:{"title":"&quot &amp &lt &gt &#39"}})}"#
        );

        let default_newlines = compile(
            r#"<div title="a&#10;b&#9;c"><a href="x&#10;y&#9;z"/></div>"#,
            no_optimize_options(),
        );
        assert_eq!(
            default_newlines.render,
            r#"with(this){return _c('div',{attrs:{"title":"a&#10;b&#9;c"}},[_c('a',{attrs:{"href":"x&#10;y&#9;z"}})])}"#
        );

        let decode_all_but_href = compile(
            r#"<div title="a&#10;b&#9;c"><a href="x&#10;y&#9;z"/></div>"#,
            Vue2CompileOptions {
                should_decode_newlines: true,
                ..no_optimize_options()
            },
        );
        assert_eq!(
            decode_all_but_href.render,
            "with(this){return _c('div',{attrs:{\"title\":\"a\\nb\\tc\"}},[_c('a',{attrs:{\"href\":\"x&#10;y&#9;z\"}})])}"
        );

        let decode_href = compile(
            r#"<div title="a&#10;b&#9;c"><a href="x&#10;y&#9;z"/></div>"#,
            Vue2CompileOptions {
                should_decode_newlines_for_href: true,
                ..no_optimize_options()
            },
        );
        assert_eq!(
            decode_href.render,
            "with(this){return _c('div',{attrs:{\"title\":\"a&#10;b&#9;c\"}},[_c('a',{attrs:{\"href\":\"x\\ny\\tz\"}})])}"
        );

        let dynamic = compile(
            r#"<div :title="'&quot; &amp; &#39;'"/>"#,
            no_optimize_options(),
        );
        assert_eq!(
            dynamic.render,
            r#"with(this){return _c('div',{attrs:{"title":'" & ''}})}"#
        );

        let dynamic_component = compile(
            r#"<Comp :empty-text="'No properties found. Click &quot;Add property&quot; to create one.'"/>"#,
            no_optimize_options(),
        );
        assert_eq!(
            dynamic_component.render,
            r#"with(this){return _c('Comp',{attrs:{"empty-text":'No properties found. Click "Add property" to create one.'}})}"#
        );
    }

    #[test]
    fn parses_vue2_pre_children_as_normal_elements_with_preserved_whitespace() {
        let result = compile(
            "<pre><code>  \n<span>hi</span>\n  </code><span> </span></pre>",
            options(),
        );
        let root = result.element_ast.unwrap();
        assert_eq!(root.tag, "pre");
        assert_eq!(root.children.len(), 2);
        let code = match &root.children[0] {
            Vue2Node::Element(element) => element,
            Vue2Node::Text(_) => panic!("expected code child element"),
        };
        assert_eq!(code.children.len(), 3);
        match &code.children[0] {
            Vue2Node::Text(text) => assert_eq!(text.text, "  \n"),
            Vue2Node::Element(_) => panic!("expected preserved pre whitespace"),
        }
        match &code.children[2] {
            Vue2Node::Text(text) => assert_eq!(text.text, "\n  "),
            Vue2Node::Element(_) => panic!("expected preserved pre whitespace"),
        }
    }

    #[test]
    fn parses_vue2_condensed_whitespace_like_official_parser() {
        let mut options = options();
        options.whitespace = Some("condense".into());
        options.preserve_whitespace = false;
        let result = compile(
            "<p>\n  Welcome to <b>Vue.js</b>    <i>world</i>  \n  <span>.\n  Have fun!\n</span></p>",
            options.clone(),
        );
        let root = result.element_ast.unwrap();
        assert_eq!(root.children.len(), 5);
        match &root.children[2] {
            Vue2Node::Text(text) => assert_eq!(text.text, " "),
            Vue2Node::Element(_) => panic!("expected condensed inline space"),
        }

        let nbsp = compile("<span>&nbsp;</span>", options);
        let root = nbsp.element_ast.unwrap();
        assert_eq!(root.children.len(), 1);
        match &root.children[0] {
            Vue2Node::Text(text) => assert_eq!(text.text, "\u{00a0}"),
            Vue2Node::Element(_) => panic!("expected non-breaking space text"),
        }
    }

    #[test]
    fn compiles_at_default_template_nesting_depth_limit() {
        let template = nested_div_template(DEFAULT_MAX_TEMPLATE_NESTING_DEPTH);
        let result = compile(&template, options());

        assert!(result.errors.is_empty(), "{:#?}", result.diagnostics);
        assert_eq!(
            nested_element_depth(result.element_ast.as_ref().expect("root element")),
            DEFAULT_MAX_TEMPLATE_NESTING_DEPTH
        );
        assert!(!result.render.is_empty());
    }

    #[test]
    fn rejects_template_beyond_default_nesting_depth_without_panicking() {
        let template = nested_div_template(DEFAULT_MAX_TEMPLATE_NESTING_DEPTH + 1_024);
        let result = std::panic::catch_unwind(|| compile(&template, options()))
            .expect("overly nested template must not panic");

        assert_eq!(result.errors.len(), 1, "{:#?}", result.diagnostics);
        assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
        assert!(result.diagnostics[0].starts_with(&format!(
            "[{TEMPLATE_DEPTH_LIMIT_ERROR_CODE}]"
        )));
        assert!(!result.diagnostics[0].contains("E_VUE2_UNCLOSED_TAG"));
        assert_eq!(
            nested_element_depth(result.element_ast.as_ref().expect("bounded partial tree")),
            DEFAULT_MAX_TEMPLATE_NESTING_DEPTH
        );
        assert!(!result.render.is_empty());
    }

    fn nested_div_template(depth: usize) -> String {
        let mut template = String::with_capacity(depth * ("<div>".len() + "</div>".len()) + 4);
        for _ in 0..depth {
            template.push_str("<div>");
        }
        template.push_str("leaf");
        for _ in 0..depth {
            template.push_str("</div>");
        }
        template
    }

    fn nested_element_depth(root: &Vue2Element) -> usize {
        let mut depth = 1;
        let mut element = root;
        while let Some(child) = element.children.iter().find_map(|child| match child {
            Vue2Node::Element(child) => Some(child.as_ref()),
            Vue2Node::Text(_) => None,
        }) {
            depth += 1;
            element = child;
        }
        depth
    }
