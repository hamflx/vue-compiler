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
