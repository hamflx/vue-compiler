    #[test]
    fn vue27_sfc_asset_url_transform_rewrites_attrs_and_srcset() {
        let mut compile_options = options();
        compile_options.sfc_asset_url_transform = Some(Vue2SfcAssetUrlTransformOptions::default());
        let result = compile(
            r#"<div><img src="./logo.png" srcset="./logo.png 2x, @/icon.svg#heart 3x"><svg><use href="~@svg/file.svg#fragment"/></svg></div>"#,
            compile_options,
        );
        let code = format!("{}{}", result.render, result.static_render_fns.join(""));

        assert!(code.contains(r#""src":require("./logo.png")"#));
        assert!(code.contains(
            r##""srcset":require("./logo.png") + " 2x, " + require("@/icon.svg") + "#heart" + " 3x""##
        ));
        assert!(code.contains(r##""href":require("@svg/file.svg") + "#fragment""##));
    }

    #[test]
    fn vue27_sfc_asset_url_transform_honors_base_and_include_absolute() {
        let mut base_options = options();
        base_options.sfc_asset_url_transform = Some(Vue2SfcAssetUrlTransformOptions {
            base: Some("/base/".into()),
            ..Vue2SfcAssetUrlTransformOptions::default()
        });
        let base = compile(
            r#"<div><img src="./logo.png" srcset="./logo.png 2x, @/logo.png 3x"><img src="@/alias.png"></div>"#,
            base_options,
        );
        let base_code = format!("{}{}", base.render, base.static_render_fns.join(""));
        assert!(base_code.contains(r#""src":"/base/logo.png""#));
        assert!(base_code
            .contains(r#""srcset":"/base/logo.png" + " 2x, " + require("@/logo.png") + " 3x""#));
        assert!(base_code.contains(r#""src":require("@/alias.png")"#));

        let mut absolute_options = options();
        absolute_options.sfc_asset_url_transform = Some(Vue2SfcAssetUrlTransformOptions {
            include_absolute: true,
            ..Vue2SfcAssetUrlTransformOptions::default()
        });
        let absolute = compile(r#"<img src="/logo.png">"#, absolute_options);
        let absolute_code = format!("{}{}", absolute.render, absolute.static_render_fns.join(""));
        assert!(absolute_code.contains(r#""src":require("/logo.png")"#));
    }

    #[test]
    fn warns_for_vue2_duplicate_raw_attrs_and_invalid_dynamic_args() {
        let duplicate = compile(r#"<p class="one" class="two"></p>"#, options());
        assert!(duplicate
            .errors
            .iter()
            .any(|error| error.msg.contains("duplicate attribute")));

        for template in [
            r#"<div v-bind:['foo' + bar]="baz"/>"#,
            r#"<div :['foo' + bar]="baz"/>"#,
            r#"<div @['foo' + bar]="baz"/>"#,
            r#"<foo #['foo' + bar]="baz"/>"#,
            r#"<div :['foo' + bar].some.mod="baz"/>"#,
        ] {
            let result = compile(template, options());
            assert!(
                result
                    .errors
                    .iter()
                    .any(|error| error.msg.contains("Invalid dynamic argument expression")),
                "{template}"
            );
        }
    }

    #[test]
    fn tips_for_vue2_component_v_for_without_key_like_official_codegen() {
        let result = compile(
            r#"<div><el-dropdown-item v-for="item in handle">{{ item.label }}</el-dropdown-item><span v-for="item in handle">{{ item }}</span><slot v-for="item in handle"></slot><template v-for="item in handle"><foo/></template></div>"#,
            options(),
        );

        assert_eq!(result.tips.len(), 1);
        assert_eq!(
            result.tips[0].msg,
            r#"<el-dropdown-item v-for="item in handle">: component lists rendered with v-for should have explicit keys. See https://v2.vuejs.org/v2/guide/list.html#key for more info."#
        );
        assert!(result.tips[0].tip);
        assert_eq!(result.tips[0].start, Some(23));
        assert_eq!(result.tips[0].end, Some(45));
        assert!(result.errors.is_empty());

        let leading_whitespace = compile(
            "\n<div><el-dropdown-item v-for=\"item in handle\">{{ item.label }}</el-dropdown-item></div>\n",
            options(),
        );
        assert_eq!(leading_whitespace.tips.len(), 1);
        assert_eq!(leading_whitespace.tips[0].start, Some(24));
        assert_eq!(leading_whitespace.tips[0].end, Some(46));

        let keyed = compile(
            r#"<div><el-dropdown-item v-for="item in handle" :key="item.value">{{ item.label }}</el-dropdown-item></div>"#,
            options(),
        );
        assert!(keyed.tips.is_empty());
    }

    #[test]
    fn collects_vue2_source_ranges_like_official_compiler() {
        let text_root = compile("hello", options());
        assert_eq!(text_root.errors.len(), 1);
        assert_eq!(text_root.errors[0].start, Some(0));
        assert_eq!(text_root.errors[0].end, None);

        let invalid_expr = compile(r#"<div v-if="a----">{{ b++++ }}</div>"#, options());
        assert_eq!(invalid_expr.errors.len(), 2);
        assert!(invalid_expr.errors[0]
            .msg
            .contains(r#"Raw expression: v-if="a----""#));
        assert_eq!(invalid_expr.errors[0].start, Some(5));
        assert_eq!(invalid_expr.errors[0].end, Some(17));
        assert!(invalid_expr.errors[1]
            .msg
            .contains("Raw expression: {{ b++++ }}"));
        assert_eq!(invalid_expr.errors[1].start, Some(18));
        assert_eq!(invalid_expr.errors[1].end, Some(29));

        let unclosed = compile("<div><span></div>", options());
        assert_eq!(unclosed.errors.len(), 1);
        assert_eq!(unclosed.errors[0].start, Some(5));
        assert_eq!(unclosed.errors[0].end, Some(11));

        let multiple_roots = compile("<div></div><span></span><p></p>", options());
        assert_eq!(multiple_roots.errors.len(), 1);
        assert_eq!(multiple_roots.errors[0].start, Some(11));
        assert_eq!(multiple_roots.errors[0].end, None);

        let slot_key = compile(r#"<div><slot v-bind:key="key" /></div>"#, options());
        assert_eq!(slot_key.errors.len(), 1);
        assert_eq!(slot_key.errors[0].start, Some(11));
        assert_eq!(slot_key.errors[0].end, Some(27));
    }

    #[test]
    fn validates_vue2_template_expressions_with_parser() {
        for (template, expected) in [
            (r#"<div>{{ foo( }}</div>"#, "Raw expression: {{ foo( }}"),
            (r#"<div :id="foo("></div>"#, r#"Raw expression: id="foo(""#),
            (
                r#"<div v-show="ok &&"></div>"#,
                r#"Raw expression: v-show="ok &&""#,
            ),
            (
                r#"<button @click="foo( }"></button>"#,
                r#"Raw expression: @click="foo( }""#,
            ),
            (
                r#"<div v-for="item in list("></div>"#,
                r#"Raw expression: v-for="list(""#,
            ),
            (
                r#"<div v-for="(item, invalid-) in list"></div>"#,
                r#"Raw expression: v-for alias="item,invalid-""#,
            ),
            (
                r#"<input v-model="foo()">"#,
                r#"Raw expression: v-model assignment="foo()""#,
            ),
            (
                r#"<div>{{ msg | append(foo() }}</div>"#,
                "Raw expression: {{ msg | append(foo() }}",
            ),
        ] {
            let result = compile(template, options());
            assert!(
                result
                    .errors
                    .iter()
                    .any(|error| error.msg.contains(expected)),
                "{template}: {:#?}",
                result.errors
            );
        }

        for template in [
            r#"<input @input="onInput1();onInput2()">"#,
            r#"<div :payload="{ a: 1, b: foo ? bar : baz }"></div>"#,
            r#"<div>{{ msg | append(',', `x,y`, /a,b/g, count) }}</div>"#,
        ] {
            let result = compile(template, options());
            assert!(result.errors.is_empty(), "{template}: {:#?}", result.errors);
        }
    }

    #[test]
    fn optimizer_marks_static_roots() {
        let result = compile("<h1 id=\"x\"><span>hello</span></h1>", options());
        let root = result.element_ast.as_ref().unwrap();
        assert!(root.static_node);
        assert!(root.static_root);
        assert!(result.render.contains("_m(0)"));
    }

    #[test]
    fn optimizer_honors_platform_reserved_tag_options() {
        let mut parsed = compile("<h1 id=\"x\">hello</h1>", options())
            .element_ast
            .unwrap();
        let mut optimizer_options = options();
        optimizer_options.reserved_tags = Some(Vec::new());
        optimizer_options.use_default_reserved_tags = false;
        optimize(&mut parsed, &optimizer_options);
        assert!(!parsed.static_node);
    }

    #[test]
    fn generates_vue2_svg_reserved_tags_like_official_codegen() {
        let mut compile_options = options();
        compile_options.optimize = false;

        let symbol = compile(
            "<svg><symbol><path></path></symbol></svg>",
            compile_options.clone(),
        );
        assert_eq!(
            symbol.render,
            r#"with(this){return _c('svg',[_c('symbol',[_c('path')])])}"#
        );

        let clip_path = compile(
            "<svg><clipPath><rect></rect></clipPath></svg>",
            compile_options.clone(),
        );
        assert_eq!(
            clip_path.render,
            r#"with(this){return _c('svg',[_c('clipPath',[_c('rect')])])}"#
        );

        let linear_gradient = compile(
            "<svg><linearGradient><stop></stop></linearGradient></svg>",
            compile_options,
        );
        assert_eq!(
            linear_gradient.render,
            r#"with(this){return _c('svg',[_c('linearGradient',[_c('stop')],1)],1)}"#
        );

        let optimized_slot_fallback = compile(
            r#"<div v-if="state"><slot><svg style="display:none"><symbol><path></path></symbol></svg></slot></div>"#,
            options(),
        );
        assert_eq!(
            optimized_slot_fallback.render,
            r#"with(this){return (state)?_c('div',[_t("default",function(){return [_c('svg',{staticStyle:{"display":"none"}},[_c('symbol',[_c('path')])])]})],2):_e()}"#
        );
        assert!(optimized_slot_fallback.static_render_fns.is_empty());
    }

    #[test]
    fn parser_honors_platform_namespace_options() {
        let mut parse_options = options();
        parse_options.tag_namespaces = BTreeMap::new();
        parse_options.use_default_tag_namespaces = false;
        let root = compile("<svg><text>hello</text></svg>", parse_options)
            .element_ast
            .unwrap();
        assert_eq!(root.ns, None);
    }

    #[test]
    fn code_frame_matches_vue2_shape() {
        let source = "<div>\n  <span key=\"one\"></span>\n</div>";
        let start = source.find("key").unwrap();
        let frame = generate_code_frame(source, start, start + 9);
        assert!(frame.contains("2  |    <span key=\"one\"></span>"));
        assert!(frame.contains("^"));

        let multiline = "<div attr=\"some\n  multiline\nattr\n\">\n</div>";
        let multiline_start = multiline.find("attr=").unwrap();
        let multiline_end = multiline.find("\">").unwrap() + 1;
        assert_eq!(
            generate_code_frame(multiline, multiline_start, multiline_end),
            "1  |  <div attr=\"some\n   |       ^^^^^^^^^^\n2  |    multiline\n   |  ^^^^^^^^^^^\n3  |  attr\n   |  ^^^^\n4  |  \">\n   |  ^"
        );
    }
