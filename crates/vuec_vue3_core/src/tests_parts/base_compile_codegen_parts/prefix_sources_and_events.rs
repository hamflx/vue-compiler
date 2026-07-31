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
    fn base_compile_source_map_excludes_non_identifier_text_from_names() {
        let source =
            r#"<div>{{ 'foo' + /hidden(?<capture>name)/.test(value) + `raw ${other}` /* ignored */ }}</div>"#;
        let result = base_compile(
            TemplateSource {
                filename: "Literals.vue".into(),
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

        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(result.code.contains(
            r#"'foo' + /hidden(?<capture>name)/.test(_ctx.value) + `raw ${_ctx.other}` /* ignored */"#
        ));
        assert_eq!(
            result.map.expect("source map").names,
            vec!["test", "value", "other"]
        );
    }

    #[test]
    fn base_compile_source_map_keeps_object_and_member_identifiers() {
        let source =
            r#"<div>{{ ({ plain: value, [dynamic]: object.member }).plain }}</div>"#;
        let result = base_compile(
            TemplateSource {
                filename: "Members.vue".into(),
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

        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(
            result.map.expect("source map").names,
            vec!["plain", "value", "dynamic", "object", "member"]
        );
    }

    #[test]
    fn base_compile_source_map_leaves_literal_only_names_empty() {
        let result = base_compile(
            TemplateSource {
                filename: "LiteralOnly.vue".into(),
                source: r#"<div>{{ "only text" }}</div>"#.into(),
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

        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(result.map.expect("source map").names.is_empty());
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
    fn js_like_rewrite_preserves_regular_expression_literals() {
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            ..Vue3CompilerOptions::default()
        };

        assert_eq!(
            rewrite_js_like_expression(
                r#"/foo\/[a-z/\\]+(?<capture>bar)/giu.test(value)"#,
                &options,
            ),
            r#"/foo\/[a-z/\\]+(?<capture>bar)/giu.test(_ctx.value)"#,
        );
        assert_eq!(
            rewrite_js_like_expression(
                r#"if (/foo/.test(value)) { const local = value; run(local) }"#,
                &options,
            ),
            r#"if (/foo/.test(_ctx.value)) { const local = _ctx.value; _ctx.run(local) }"#,
        );
        assert_eq!(
            rewrite_js_like_expression(r#"return /foo/.test(value)"#, &options),
            r#"return /foo/.test(_ctx.value)"#,
        );
        assert_eq!(
            rewrite_js_like_expression(r#"`match: ${/[}]/.test(value)}`"#, &options),
            r#"`match: ${/[}]/.test(_ctx.value)}`"#,
        );
        assert_eq!(
            rewrite_js_like_expression(
                r#"/hidden/.test(value) + `raw ${other}`"#,
                &options,
            ),
            r#"/hidden/.test(_ctx.value) + `raw ${_ctx.other}`"#,
        );
        assert_eq!(
            rewrite_js_like_expression("/* (value) => */ value + outside", &options),
            "/* (value) => */ _ctx.value + _ctx.outside",
        );

        let result = base_compile(
            TemplateSource {
                filename: "regexp.vue".into(),
                source: r#"<div>{{ /foo/.test(value) }}</div>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(result
            .code
            .contains("_toDisplayString(/foo/.test(_ctx.value))"));

    }

    #[test]
    fn js_like_rewrite_uses_parser_to_distinguish_division_from_regex() {
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            ..Vue3CompilerOptions::default()
        };

        assert_eq!(
            rewrite_js_like_expression(
                "total / divisor + /foo/.test(value) / scale",
                &options,
            ),
            "_ctx.total / _ctx.divisor + /foo/.test(_ctx.value) / _ctx.scale",
        );
    }

    #[test]
    fn js_like_rewrite_supports_jsx_expression_plugins() {
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            expression_plugins: vec!["jsx".into()],
            ..Vue3CompilerOptions::default()
        };

        assert_eq!(
            rewrite_js_like_expression(
                r#"() => <div title="raw">hello {msg}<span>{other}</span></div>"#,
                &options,
            ),
            r#"() => <div title="raw">hello {_ctx.msg}<span>{_ctx.other}</span></div>"#,
        );
        assert_eq!(
            rewrite_js_like_expression(
                "(item) => <><Foo.Bar {...props}>{item}</Foo.Bar><Comp value={outside} /></>",
                &options,
            ),
            "(item) => <><Foo.Bar {..._ctx.props}>{item}</Foo.Bar><Comp value={_ctx.outside} /></>",
        );

        let mut tsx = options.clone();
        tsx.expression_plugins.push("typescript".into());
        assert_eq!(
            rewrite_js_like_expression(
                "(item: Item) => <Comp>{item as Item}{outside}</Comp>",
                &tsx,
            ),
            "(item: Item) => <Comp>{item as Item}{_ctx.outside}</Comp>",
        );

        let result = base_compile(
            TemplateSource {
                filename: "jsx.vue".into(),
                source: "<div>{{ () => <span>{msg}</span> }}</div>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options.clone(),
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(
            result.code.contains("() => <span>{_ctx.msg}</span>"),
            "{}",
            result.code,
        );

        let malformed = base_compile(
            TemplateSource {
                filename: "malformed-jsx.vue".into(),
                source: "<div>{{ () => <span>{msg} }}</div>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );
        assert!(!malformed.diagnostics.is_empty());
    }

    #[test]
    fn js_like_rewrite_rejects_jsx_above_the_safe_ast_limit() {
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            expression_plugins: vec!["jsx".into()],
            ..Vue3CompilerOptions::default()
        };
        let exact_jsx = format!(
            "<div>{}{{msg}}</div>",
            "x".repeat(PROCESS_EXPRESSION_MAX_SAFE_AST_BYTES - "<div>{msg}</div>".len()),
        );
        assert_eq!(exact_jsx.len(), PROCESS_EXPRESSION_MAX_SAFE_AST_BYTES);
        assert_eq!(
            rewrite_js_like_expression(&exact_jsx, &options),
            exact_jsx.replace("{msg}", "{_ctx.msg}"),
        );

        let long_jsx = format!("<div>{}{{msg}}</div>", "plain text ".repeat(450));
        assert!(long_jsx.len() > PROCESS_EXPRESSION_MAX_SAFE_AST_BYTES);
        assert_eq!(
            rewrite_js_like_expression(&long_jsx, &options),
            long_jsx,
        );

        let deep_jsx = format!(
            "{}{{outside}}{}",
            "<Box>".repeat(700),
            "</Box>".repeat(700),
        );
        assert!(deep_jsx.len() > PROCESS_EXPRESSION_MAX_SAFE_AST_BYTES);
        assert_eq!(
            rewrite_js_like_expression(&deep_jsx, &options),
            deep_jsx,
        );

        let mut compile_options = options;
        compile_options.source_map = true;
        let result = base_compile(
            TemplateSource {
                filename: "long-jsx.vue".into(),
                source: format!("<section>{{{{ {long_jsx} }}}}</section>"),
                file_id: FileId(0),
                base_offset: 0,
            },
            compile_options,
        );
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "46"
                && diagnostic.message == PROCESS_EXPRESSION_AST_LIMIT_MESSAGE));
        assert!(!result.code.contains("<_ctx.div"), "{}", result.code);
        assert!(!result.code.contains("_ctx.plain"), "{}", result.code);
    }

    #[test]
    fn js_like_rewrite_supports_ecmascript_unicode_identifiers() {
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            ..Vue3CompilerOptions::default()
        };

        assert_eq!(
            rewrite_js_like_expression("用户 + count", &options),
            "_ctx.用户 + _ctx.count",
        );
        assert_eq!(
            rewrite_js_like_expression("(参数) => 参数 + 外部", &options),
            "(参数) => 参数 + _ctx.外部",
        );
        assert_eq!(
            rewrite_js_like_expression("参数 => 参数 + 外部", &options),
            "参数 => 参数 + _ctx.外部",
        );
        assert_eq!(
            rewrite_js_like_expression("const 局部 = 来源; 局部 + 外部", &options),
            "const 局部 = _ctx.来源; 局部 + _ctx.外部",
        );

        let combining = "变量\u{0301}";
        assert_eq!(
            rewrite_js_like_expression(&format!("{combining} + 外部"), &options),
            format!("_ctx.{combining} + _ctx.外部"),
        );

        let mut inline = options.clone();
        inline.inline = true;
        inline
            .binding_metadata
            .insert("引用".into(), "setup-ref".into());
        assert_eq!(
            rewrite_js_like_expression("引用 + 外部", &inline),
            "引用.value + _ctx.外部",
        );

        let result = base_compile(
            TemplateSource {
                filename: "unicode.vue".into(),
                source: "<div>{{ 用户 + count }}</div>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(
            result.code.contains("_toDisplayString(_ctx.用户 + _ctx.count)"),
            "{}",
            result.code,
        );
    }

    #[test]
    fn js_like_rewrite_ignores_regex_punctuation_when_rewriting_assignments() {
        let mut options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            inline: true,
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("count".into(), "setup-ref".into());

        assert_eq!(
            rewrite_js_like_expression(r#"count = /foo[,;}]/.test(value)"#, &options),
            r#"count.value = /foo[,;}]/.test(_ctx.value)"#,
        );
        assert_eq!(
            rewrite_js_like_expression("count /= divisor", &options),
            "count.value /= _ctx.divisor",
        );
        assert_eq!(
            rewrite_js_like_expression(r#"/lead/.test(value); count = next"#, &options),
            r#"/lead/.test(_ctx.value); count.value = _ctx.next"#,
        );
    }

    #[test]
    fn js_like_rewrite_cursor_lookup_handles_unicode_byte_boundaries() {
        let chars = "aé🙂z".char_indices().collect::<Vec<_>>();

        assert_eq!(js_like_char_index_at_or_after(&chars, 0, 0), 0);
        assert_eq!(js_like_char_index_at_or_after(&chars, 0, 1), 1);
        assert_eq!(js_like_char_index_at_or_after(&chars, 0, 2), 2);
        assert_eq!(js_like_char_index_at_or_after(&chars, 1, 7), 3);
        assert_eq!(js_like_char_index_at_or_after(&chars, 3, usize::MAX), 4);
    }

    #[test]
    fn js_like_rewrite_advances_through_dense_update_ranges() {
        const UPDATES: usize = 4_096;
        let expression = vec!["count++"; UPDATES].join("; ");
        let mut options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            inline: true,
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("count".into(), "setup-ref".into());

        let rewritten = rewrite_js_like_expression(&expression, &options);

        assert_eq!(rewritten.matches("count.value++").count(), UPDATES);
        assert_eq!(rewritten.matches("; ").count(), UPDATES - 1);
    }

    #[test]
    fn arrow_binding_index_preserves_outer_same_name_scopes() {
        let expression = "x => ((x => x)(value) + x), x";
        let bindings = process_expression_arrow_bindings(expression);
        let spans = expression
            .match_indices('x')
            .map(|(start, value)| (start, start + value.len()))
            .collect::<Vec<_>>();

        assert_eq!(spans.len(), 5);
        assert!(process_expression_is_arrow_param(
            &bindings, spans[0].0, spans[0].1
        ));
        assert!(process_expression_is_arrow_param(
            &bindings, spans[1].0, spans[1].1
        ));
        assert!(process_expression_is_arrow_local(
            &bindings, "x", spans[2].0, spans[2].1
        ));
        assert!(process_expression_is_arrow_local(
            &bindings, "x", spans[3].0, spans[3].1
        ));
        assert!(!process_expression_is_arrow_local(
            &bindings, "x", spans[4].0, spans[4].1
        ));
    }

    #[test]
    fn js_like_rewrite_indexes_dense_arrow_bindings() {
        const ARROWS: usize = 4_096;
        let expression = format!("[{}]", vec!["x => x"; ARROWS].join(", "));
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            ..Vue3CompilerOptions::default()
        };

        assert_eq!(rewrite_js_like_expression(&expression, &options), expression);
    }

    #[test]
    fn expression_scans_stream_multibyte_quoted_and_parameter_content() {
        let expression = r#"(first = 'é\'hidden', { value: alias }) => first + alias"#;
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            ..Vue3CompilerOptions::default()
        };
        let arrow_offsets = process_expression_arrow_offsets(expression);
        assert_eq!(arrow_offsets.len(), 1);
        assert_eq!(
            &expression[arrow_offsets[0]..arrow_offsets[0] + 2],
            "=>"
        );
        let param_range = process_expression_arrow_param_range(expression, arrow_offsets[0])
            .expect("arrow parameter range");

        let binding_names = process_expression_param_binding_spans(expression, param_range)
            .into_iter()
            .map(|(start, end)| &expression[start..end])
            .collect::<Vec<_>>();
        assert_eq!(binding_names, ["first", "alias"]);

        let param_names = process_expression_param_identifier_spans(
            expression,
            param_range,
            &options,
        )
        .into_iter()
        .map(|span| &expression[span.start..span.end])
        .collect::<Vec<_>>();
        assert_eq!(param_names, ["first", "alias"]);

        let identifiers = process_expression_identifier_spans(expression, &options, &[]);
        assert!(identifiers
            .iter()
            .all(|identifier| identifier.content != "_ctx.hidden"));
        assert_eq!(rewrite_js_like_expression(expression, &options), expression);
    }

    #[test]
    fn js_like_rewrite_indexes_function_and_method_bindings() {
        let expression = r#"[function named(first = fallback(call(arg)), { value: alias }, ...rest) {
 return named(first) + alias + rest.length + outside
}, { method(value = seed) { return value + outside }, *generator(item) { yield item + outside } }, class { method(value) { return value + outside } }]"#;
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            ..Vue3CompilerOptions::default()
        };

        assert_eq!(
            rewrite_js_like_expression(expression, &options),
            r#"[function named(first = _ctx.fallback(_ctx.call(_ctx.arg)), { value: alias }, ...rest) {
 return named(first) + alias + rest.length + _ctx.outside
}, { method(value = _ctx.seed) { return value + _ctx.outside }, *generator(item) { yield item + _ctx.outside } }, class { method(value) { return value + _ctx.outside } }]"#,
        );
        assert_eq!(
            rewrite_js_like_expression(
                "({ [method](value) { return value }, method(value) { return value } })",
                &options,
            ),
            "({ [_ctx.method](value) { return value }, method(value) { return value } })",
        );
        assert_eq!(
            rewrite_js_like_expression(
                "({ get value() { return outside }, set value(input) { outside = input } }); class { #method(value) { return value + outside } static method(value) { return value + outside } }",
                &options,
            ),
            "({ get value() { return _ctx.outside }, set value(input) { _ctx.outside = input } }); class { #method(value) { return value + _ctx.outside } static method(value) { return value + _ctx.outside } }",
        );
        assert_eq!(
            rewrite_js_like_expression(
                "class { @dec(get) get /* get */ value() { return outside } }",
                &options,
            ),
            "class { @_ctx.dec(_ctx.get) get /* get */ value() { return _ctx.outside } }",
        );
        assert_eq!(
            rewrite_js_like_expression(
                "class Box { #value = outside; read() { return this.#value + outside } write() { this.#value = outside } }",
                &options,
            ),
            "class Box { #value = _ctx.outside; read() { return this.#value + _ctx.outside } write() { this.#value = _ctx.outside } }",
        );
    }

    #[test]
    fn function_binding_index_preserves_nested_same_name_boundaries() {
        let expression = "(function value(value) { return (function value(value) { return value })(value) + value })(source) + value";
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            ..Vue3CompilerOptions::default()
        };

        assert_eq!(
            rewrite_js_like_expression(expression, &options),
            "(function value(value) { return (function value(value) { return value })(value) + value })(_ctx.source) + _ctx.value",
        );
        assert_eq!(
            rewrite_js_like_expression(
                "(function(value) { return arguments[0] + value + outside })(input) + arguments",
                &options,
            ),
            "(function(value) { return arguments[0] + value + _ctx.outside })(_ctx.input) + _ctx.arguments",
        );
    }

    #[test]
    fn function_declarations_respect_parameter_and_block_boundaries() {
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            ..Vue3CompilerOptions::default()
        };

        assert_eq!(
            rewrite_js_like_expression(
                "function outer(value = inner) { function inner() {}; return inner(value) }; outer(input)",
                &options,
            ),
            "function outer(value = _ctx.inner) { function inner() {}; return inner(value) }; outer(_ctx.input)",
        );
        assert_eq!(
            rewrite_js_like_expression(
                "'use strict'; { function scoped() {} scoped() } scoped()",
                &options,
            ),
            "'use strict'; { function scoped() {} scoped() } _ctx.scoped()",
        );
        assert_eq!(
            rewrite_js_like_expression(
                "switch (scoped) { case 0: function scoped() {} scoped() } scoped()",
                &options,
            ),
            "switch (_ctx.scoped) { case 0: function scoped() {} scoped() } _ctx.scoped()",
        );
        assert_eq!(
            rewrite_js_like_expression(
                "(class { static { function scoped() {} var local = outside; scoped(local) } }); scoped() + local",
                &options,
            ),
            "(class { static { function scoped() {} var local = _ctx.outside; scoped(local) } }); _ctx.scoped() + _ctx.local",
        );
        assert_eq!(
            rewrite_js_like_expression(
                "class Box { field = outside; static value = outside; static {} } Box",
                &options,
            ),
            "class Box { field = _ctx.outside; static value = _ctx.outside; static {} } Box",
        );
        assert_eq!(
            rewrite_js_like_expression("(class Box { field = Box }); Box", &options),
            "(class Box { field = Box }); _ctx.Box",
        );
        assert_eq!(
            rewrite_js_like_expression(
                "function run() { use(hoisted); var hoisted = source; for (let item of items) { use(item) } return hoisted }; hoisted + item",
                &options,
            ),
            "function run() { _ctx.use(hoisted); var hoisted = _ctx.source; for (let item of _ctx.items) { _ctx.use(item) } return hoisted }; _ctx.hoisted + _ctx.item",
        );
        assert_eq!(
            rewrite_js_like_expression(
                "for (let item of items) use(item); item",
                &options,
            ),
            "for (let item of _ctx.items) _ctx.use(item); _ctx.item",
        );
        assert_eq!(
            rewrite_js_like_expression(
                r#"function run(input) { return `${input}:${hoisted}:${outside}`; var hoisted = source }"#,
                &options,
            ),
            r#"function run(input) { return `${input}:${hoisted}:${_ctx.outside}`; var hoisted = _ctx.source }"#,
        );
        assert_eq!(
            rewrite_js_like_expression(
                r#"`items: ${items.map(item => `${item}:${outside}`)}`"#,
                &options,
            ),
            r#"`items: ${_ctx.items.map(item => `${item}:${_ctx.outside}`)}`"#,
        );
    }

    #[test]
    fn expression_rewrite_preserves_outer_bindings_in_assignment_rhs() {
        let mut options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            inline: true,
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("target".into(), "setup-let".into());

        assert_eq!(
            rewrite_js_like_expression(
                r#"function run(input) { target = `${input}:${hoisted}:${outside}`; var hoisted = source }"#,
                &options,
            ),
            r#"function run(input) { _isRef(target) ? target.value = `${input}:${hoisted}:${_ctx.outside}` : target = `${input}:${hoisted}:${_ctx.outside}`; var hoisted = _ctx.source }"#,
        );
    }

    #[test]
    fn expression_rewrite_distinguishes_using_declarations_from_identifiers() {
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            ..Vue3CompilerOptions::default()
        };

        assert_eq!(
            rewrite_js_like_expression(
                "using resource = source; resource; using + source",
                &options,
            ),
            "using resource = _ctx.source; resource; _ctx.using + _ctx.source",
        );
        assert_eq!(
            rewrite_js_like_expression("await using resource = source; resource", &options),
            "await using resource = _ctx.source; resource",
        );
    }

    #[test]
    fn expression_rewrite_recovers_hack_pipeline_topic_bindings() {
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            ..Vue3CompilerOptions::default()
        };

        for topic in ["%", "#", "^", "@@", "^^"] {
            let source = format!(
                "function named(value) {{ const local = value; return local |> {topic} + outside }}"
            );
            assert_eq!(
                rewrite_js_like_expression(&source, &options),
                format!(
                    "function named(value) {{ const local = value; return local |> {topic} + _ctx.outside }}"
                ),
                "topic {topic}",
            );

            let source = format!(
                "function named(value, key, arg) {{ return value |> {topic}.field + {topic}[key] + {topic}(arg) + outside }}"
            );
            assert_eq!(
                rewrite_js_like_expression(&source, &options),
                format!(
                    "function named(value, key, arg) {{ return value |> {topic}.field + {topic}[key] + {topic}(arg) + _ctx.outside }}"
                ),
                "topic operations {topic}",
            );
        }

        assert_eq!(
            rewrite_js_like_expression(
                "function named(value, mod) { return value |> (% % mod) + outside }",
                &options,
            ),
            "function named(value, mod) { return value |> (% % mod) + _ctx.outside }",
        );
        assert_eq!(
            rewrite_js_like_expression(
                "function named(value, mask) { return value |> (^ ^ mask) + outside }",
                &options,
            ),
            "function named(value, mask) { return value |> (^ ^ mask) + _ctx.outside }",
        );
        assert_eq!(
            rewrite_js_like_expression(
                "class Box { #value; method(input) { const local = this.#value; return input |> # + local + outside } }",
                &options,
            ),
            "class Box { #value; method(input) { const local = this.#value; return input |> # + local + _ctx.outside } }",
        );
        assert_eq!(
            rewrite_js_like_expression(
                "function named(value, pair, wrap) { const local = value; return value |> pair(%, %, local) |> wrap(%, outside) }",
                &options,
            ),
            "function named(value, pair, wrap) { const local = value; return value |> pair(%, %, local) |> wrap(%, _ctx.outside) }",
        );
        assert_eq!(
            rewrite_js_like_expression(
                r#"function named(value) { const raw = "% # ^ @@ ^^"; const pattern = /[%#^]/; /* % # ^ @@ ^^ */ return value |> % + outside }"#,
                &options,
            ),
            r#"function named(value) { const raw = "% # ^ @@ ^^"; const pattern = /[%#^]/; /* % # ^ @@ ^^ */ return value |> % + _ctx.outside }"#,
        );
        assert_eq!(
            rewrite_js_like_expression(
                "function named(value) { const note = '偏移'; return value |> ^^ + outside }",
                &options,
            ),
            "function named(value) { const note = '偏移'; return value |> ^^ + _ctx.outside }",
        );
    }

    #[test]
    fn pipeline_topic_binding_recovery_is_bounded() {
        fn pipeline_with_topics(topics: usize) -> String {
            format!(
                "function named(value) {{ return value |> tuple({}) }}",
                vec!["%"; topics].join(", ")
            )
        }

        let exact = pipeline_with_topics(PROCESS_EXPRESSION_MAX_PIPELINE_TOPIC_RECOVERIES);
        let exact_bindings =
            process_expression_function_bindings(&exact, oxc_span::SourceType::mjs());
        assert!(process_expression_function_bindings_parsed(
            &exact_bindings
        ));

        let overflow =
            pipeline_with_topics(PROCESS_EXPRESSION_MAX_PIPELINE_TOPIC_RECOVERIES + 1);
        let overflow_bindings =
            process_expression_function_bindings(&overflow, oxc_span::SourceType::mjs());
        assert!(!process_expression_function_bindings_parsed(
            &overflow_bindings
        ));

        let unrelated_error =
            "function named(value) { return value |> % + ; const local = value }";
        let unrelated_bindings = process_expression_function_bindings(
            unrelated_error,
            oxc_span::SourceType::mjs(),
        );
        assert!(!process_expression_function_bindings_parsed(
            &unrelated_bindings
        ));
    }

    #[test]
    fn expression_rewrite_skips_typescript_type_positions() {
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            is_ts: true,
            ..Vue3CompilerOptions::default()
        };

        assert_eq!(
            rewrite_js_like_expression(
                "(value: External, other: NS.Type = fallback): Result<External> => factory<Generic>(value as Cast, other satisfies Shape)",
                &options,
            ),
            "(value: External, other: NS.Type = _ctx.fallback): Result<External> => _ctx.factory<Generic>(value as Cast, other satisfies Shape)",
        );
        assert_eq!(
            rewrite_js_like_expression(
                "(<Cast>value).field + (source as NS.Type) + factory<Generic>(arg)",
                &options,
            ),
            "(<Cast>_ctx.value).field + (_ctx.source as NS.Type) + _ctx.factory<Generic>(_ctx.arg)",
        );
        assert_eq!(
            rewrite_js_like_expression(
                "function run<T extends Base>(value: T): Result<T> { const local: T = value; return local + T + outside }",
                &options,
            ),
            "function run<T extends Base>(value: T): Result<T> { const local: T = value; return local + _ctx.T + _ctx.outside }",
        );
        assert_eq!(
            rewrite_js_like_expression(
                "type Local = External; interface Shape { field: External }; const value: Local = source; value + Local",
                &options,
            ),
            "type Local = External; interface Shape { field: External }; const value: Local = _ctx.source; value + _ctx.Local",
        );
        assert_eq!(
            rewrite_js_like_expression("value as External", &options),
            "_ctx.value as External",
        );
        assert_eq!(
            rewrite_js_like_expression("value satisfies Shape", &options),
            "_ctx.value satisfies Shape",
        );
        assert_eq!(
            rewrite_js_like_expression("as + value", &options),
            "_ctx.as + _ctx.value",
        );
    }

    #[test]
    fn expression_rewrite_indexes_dense_flat_identifiers() {
        const IDENTIFIERS: usize = 4_096;
        let expression = vec!["value"; IDENTIFIERS].join(" + ");
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            ..Vue3CompilerOptions::default()
        };

        let spans = process_expression_identifier_spans(&expression, &options, &[]);
        assert_eq!(spans.len(), IDENTIFIERS);
        assert!(spans.iter().all(|span| span.content == "_ctx.value"));

        let rewritten = rewrite_js_like_expression(&expression, &options);
        assert_eq!(rewritten.matches("_ctx.value").count(), IDENTIFIERS);
        assert_eq!(rewritten.matches(" + ").count(), IDENTIFIERS - 1);

        let mut inline = options;
        inline.inline = true;
        inline
            .binding_metadata
            .insert("value".into(), "setup-let".into());
        let spans = process_expression_identifier_spans(&expression, &inline, &[]);
        assert_eq!(spans.len(), IDENTIFIERS);
        assert!(spans.iter().all(|span| span.content == "_unref(value)"));
        let rewritten = rewrite_js_like_expression(&expression, &inline);
        assert_eq!(rewritten.matches("_unref(value)").count(), IDENTIFIERS);
    }

    #[test]
    fn js_like_rewrite_indexes_destructure_assignment_roles() {
        let mut options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            inline: true,
            ..Vue3CompilerOptions::default()
        };
        for (name, kind) in [
            ("maybe", "setup-maybe-ref"),
            ("key", "setup-let"),
            ("target", "setup-let"),
            ("rest", "setup-ref"),
        ] {
            options.binding_metadata.insert(name.into(), kind.into());
        }

        assert_eq!(
            rewrite_js_like_expression(
                "({ nested: { maybe = fallback }, [key]: target, ...rest } = source)",
                &options,
            ),
            "({ nested: { maybe: maybe.value = _ctx.fallback }, [_unref(key)]: target, ...rest.value } = _ctx.source)",
        );
        assert_eq!(
            rewrite_js_like_expression("[maybe = fallback, ...rest] = source", &options),
            "[maybe.value = _ctx.fallback, ...rest.value] = _ctx.source",
        );
        assert_eq!(
            rewrite_js_like_expression("{ ...items, values: [...others] }", &options),
            "{ ..._ctx.items, values: [..._ctx.others] }",
        );
    }

    #[test]
    fn expression_rewrite_indexes_dense_sibling_lexical_scopes() {
        const SCOPES: usize = 2_048;
        let expression = format!(
            "{}; value",
            vec!["{ let value = source; use(value) }"; SCOPES].join("; ")
        );
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            ..Vue3CompilerOptions::default()
        };

        let rewritten = rewrite_js_like_expression(&expression, &options);
        assert_eq!(rewritten.matches("let value").count(), SCOPES);
        assert_eq!(rewritten.matches("_ctx.source").count(), SCOPES);
        assert_eq!(rewritten.matches("_ctx.use(value)").count(), SCOPES);
        assert_eq!(rewritten.matches("_ctx.value").count(), 1);
        assert!(rewritten.ends_with("; _ctx.value"));
    }

    #[test]
    fn expression_rewrite_indexes_dense_single_scope_declarations() {
        const DECLARATIONS: usize = 2_048;
        let declarations = (0..DECLARATIONS)
            .map(|index| format!("local{index} = source"))
            .collect::<Vec<_>>()
            .join(", ");
        let locals = (0..DECLARATIONS)
            .map(|index| format!("local{index}"))
            .collect::<Vec<_>>()
            .join(" + ");
        let expression = format!(
            "let {declarations}; {locals}; {}",
            vec!["outside"; DECLARATIONS].join(" + ")
        );
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            ..Vue3CompilerOptions::default()
        };

        let rewritten = rewrite_js_like_expression(&expression, &options);
        assert_eq!(rewritten.matches("_ctx.source").count(), DECLARATIONS);
        assert_eq!(rewritten.matches("_ctx.outside").count(), DECLARATIONS);
        assert!(!rewritten.contains("_ctx.local"));
    }

    #[test]
    fn arrow_body_end_index_matches_scalar_scan_boundaries() {
        for expression in [
            "a => b => c => value",
            "a => (b => b)(value), tail",
            "a => { return { nested: true } }, tail",
            r#"a => 'comma, and \' quote' + value, tail"#,
            "a => ({ value: [one, two] }), tail",
            "a => value), tail",
            "a => value; tail",
        ] {
            let body_starts = process_expression_arrow_offsets(expression)
                .into_iter()
                .map(|arrow| skip_ws_forward(expression, arrow + 2))
                .collect::<Vec<_>>();
            let indexed = process_expression_arrow_body_ends(expression, &body_starts);
            let scalar = body_starts
                .iter()
                .map(|start| process_expression_arrow_body_end(expression, *start))
                .collect::<Vec<_>>();

            assert_eq!(indexed, scalar, "body ends differ for {expression:?}");
        }
    }

    #[test]
    fn js_like_rewrite_indexes_deeply_nested_arrow_bodies() {
        const ARROWS: usize = 4_096;
        let expression = format!("{}x", "x => ".repeat(ARROWS));
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            ..Vue3CompilerOptions::default()
        };

        assert_eq!(rewrite_js_like_expression(&expression, &options), expression);
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
        assert!(!map.names.contains(&"__vuec__".into()));
        assert!(!map.names.contains(&"$event".into()));
        assert!(!map.names.contains(&"msg".into()));
    }

    #[test]
    fn base_compile_source_map_excludes_event_handler_literal_text() {
        let result = base_compile(
            TemplateSource {
                filename: "Event.vue".into(),
                source: r#"<button @click="throw /hidden/.test(value) && run('fake');"></button>"#
                    .into(),
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

        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(
            result.map.expect("source map").names,
            vec!["test", "value", "run"]
        );
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
