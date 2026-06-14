#[cfg(test)]
mod tests {
    use super::*;
    use vuec_source::{FileId, Span};

    #[test]
    fn parses_expression() {
        let store = JsAstStore::new();
        let expr = store
            .parse_expression("foo + bar", SourceType::script())
            .expect("expression");
        assert!(matches!(expr, Expression::BinaryExpression(_)));
    }

    #[test]
    fn parses_program() {
        let store = JsAstStore::new();
        let ret = store.parse_program("let x = 1 + 2;", SourceType::script());
        assert!(ret.errors.is_empty());
        assert!(!ret.program.body.is_empty());
    }

    #[test]
    fn validates_complete_expression_source() {
        let store = JsAstStore::new();
        assert!(store
            .validate_expression("{ foo: bar }", SourceType::script())
            .is_ok());
        assert!(store
            .validate_expression("a----", SourceType::script())
            .is_err());
        assert!(store
            .validate_expression("foo(", SourceType::script())
            .is_err());
        assert!(store
            .validate_expression("foo(); bar()", SourceType::script())
            .is_err());
        assert!(store
            .validate_function_body("foo(); bar()", SourceType::script())
            .is_ok());
    }

    #[test]
    fn parses_v_for_shape() {
        let store = JsAstStore::new();
        let parsed = store
            .parse_for_expression("(item, index) in list", SourceType::script())
            .expect("v-for");
        assert_eq!(parsed.aliases, "(item, index)");
        assert_eq!(parsed.iterable, "list");
        assert_eq!(parsed.items, vec!["(item, index)"]);
    }

    #[test]
    fn parses_v_for_aliases_without_splitting_literal_commas() {
        let store = JsAstStore::new();
        let parsed = store
            .parse_for_expression(
                "item, label = 'a,b', matcher = /x,y/g in rows",
                SourceType::script(),
            )
            .expect("v-for");

        assert_eq!(parsed.aliases, "item, label = 'a,b', matcher = /x,y/g");
        assert_eq!(
            parsed.items,
            vec!["item", "label = 'a,b'", "matcher = /x,y/g"]
        );
    }

    #[test]
    fn registers_and_parses_expression_by_id() {
        let mut store = JsAstStore::new();
        let id = store.register_expr(
            "foo + bar",
            Span::new(FileId(0), 10, 19),
            SourceType::script(),
        );
        let entry = store.expr_entry(id).expect("entry");
        assert_eq!(entry.source, "foo + bar");
        assert_eq!(entry.span, Span::new(FileId(0), 10, 19));
        assert_eq!(entry.mode, JsParseMode::Expression);
        assert_eq!(entry.source_type, JsSourceType::Script);

        let expr = store.parse_expr(id).expect("registered expression");
        assert!(matches!(expr, Expression::BinaryExpression(_)));
    }

    #[test]
    fn repeated_js_sources_are_interned_without_changing_serialized_shape() {
        let mut store = JsAstStore::new();
        let first = store.register_expr(
            "item.count",
            Span::new(FileId(0), 0, 10),
            SourceType::script(),
        );
        let second = store.register_stmt(
            "item.count",
            Span::new(FileId(0), 20, 30),
            SourceType::script(),
        );
        let distinct =
            store.register_pattern("item", Span::new(FileId(0), 40, 44), SourceType::script());

        let first_entry = store.expr_entry(first).unwrap();
        let second_entry = store.stmt_entry(second).unwrap();
        let distinct_entry = store.pattern_entry(distinct).unwrap();
        assert!(store.interned_source_ptr_eq(first_entry, second_entry));
        assert!(!store.interned_source_ptr_eq(first_entry, distinct_entry));
        assert_eq!(
            store.string_interner_stats(),
            JsStringInternerStats {
                hits: 1,
                misses: 2,
                entries: 2,
            }
        );

        let serialized = serde_json::to_value(first_entry).unwrap();
        assert_eq!(serialized["source"], "item.count");
    }

    #[test]
    fn clear_drops_registered_sources_and_restarts_ids() {
        let mut store = JsAstStore::new();
        let old = store.register_expr("old", Span::new(FileId(0), 0, 3), SourceType::script());
        assert_eq!(old, JsExprId(0));
        assert_eq!(store.string_interner_stats().entries, 1);
        assert!(store
            .parse_expression("old + value", SourceType::script())
            .is_ok());

        store.clear();

        assert!(store.expr_entry(old).is_none());
        assert_eq!(
            store.string_interner_stats(),
            JsStringInternerStats::default()
        );
        let new = store.register_expr("new", Span::new(FileId(0), 4, 7), SourceType::script());
        assert_eq!(new, JsExprId(0));
        assert_eq!(store.expr_entry(new).unwrap().source, "new");
    }

    #[test]
    fn registers_statements_patterns_and_programs_by_id() {
        let mut store = JsAstStore::new();
        let stmt_id =
            store.register_stmt("foo();", Span::new(FileId(0), 0, 6), SourceType::script());
        let pattern_id = store.register_pattern(
            "{ item, index }",
            Span::new(FileId(0), 7, 22),
            SourceType::script(),
        );
        let program_id = store.register_program(
            "export const x = 1;",
            Span::new(FileId(0), 23, 42),
            JsParseMode::ScriptModule,
            SourceType::mjs(),
        );

        let stmt = store
            .parse_single_stmt(stmt_id)
            .expect("registered statement");
        assert!(matches!(stmt, Statement::ExpressionStatement(_)));

        let pattern = store.parse_pattern(pattern_id).expect("registered pattern");
        assert_eq!(pattern.items, vec!["{ item, index }"]);

        let program = store
            .parse_registered_program(program_id)
            .expect("registered program");
        assert!(program.errors.is_empty());
        assert_eq!(
            store.program_entry(program_id).unwrap().source_type,
            JsSourceType::Module
        );
    }

    #[test]
    fn parses_statement_lists_params_for_script_modes_by_id() {
        let mut store = JsAstStore::new();
        let stmt_id = store.register_stmt(
            "foo(); bar();",
            Span::new(FileId(0), 0, 12),
            SourceType::script(),
        );
        let params_id = store.register_pattern(
            "item, i",
            Span::new(FileId(0), 13, 20),
            SourceType::script(),
        );
        let for_id = store.register_for_expression(
            "(item, i) in list",
            Span::new(FileId(0), 21, 38),
            SourceType::script(),
        );
        let classic_id = store.register_program(
            "var x = 1;",
            Span::new(FileId(0), 39, 49),
            JsParseMode::ScriptClassic,
            SourceType::script(),
        );
        let ts_id = store.register_program(
            "const x: number = 1;",
            Span::new(FileId(0), 50, 70),
            JsParseMode::TypeScript,
            SourceType::ts(),
        );

        let statements = store.parse_stmt(stmt_id).expect("statement list");
        assert_eq!(statements.program.body.len(), 2);
        assert!(store.parse_single_stmt(stmt_id).is_err());

        let params = store.parse_pattern(params_id).expect("params");
        assert_eq!(params.items, vec!["item", "i"]);

        let parsed_for = match store
            .parse_mode(
                "(item, i) in list",
                JsParseMode::ForExpression,
                SourceType::script(),
            )
            .expect("v-for mode")
        {
            JsParseResult::ForExpression(parsed) => parsed,
            _ => panic!("expected v-for result"),
        };
        assert_eq!(parsed_for.iterable, "list");
        assert!(store.parse_expr(for_id).is_ok());

        assert!(store
            .parse_registered_program(classic_id)
            .expect("classic script")
            .errors
            .is_empty());
        assert!(store
            .parse_registered_program(ts_id)
            .expect("typescript")
            .errors
            .is_empty());
    }

    #[test]
    fn parses_params_without_splitting_literal_commas() {
        let store = JsAstStore::new();
        let parsed = store
            .parse_params("first = ',', second = /a,b/g, third = `x,y`, fourth")
            .expect("params");

        assert_eq!(
            parsed.items,
            vec!["first = ','", "second = /a,b/g", "third = `x,y`", "fourth"]
        );
    }

    #[test]
    fn parse_mode_checks_program_errors() {
        let store = JsAstStore::new();
        assert!(store
            .parse_mode("const =", JsParseMode::Statements, SourceType::script())
            .is_err());
    }

    #[test]
    fn maps_template_local_oxc_spans_to_absolute_source_spans() {
        let mapper = TemplateJsSource::new(FileId(7), 42, 20);
        assert_eq!(mapper.full_span(), Span::new(FileId(7), 42, 62));
        assert_eq!(mapper.span(3, 9), Some(Span::new(FileId(7), 45, 51)));
        assert_eq!(
            mapper.oxc_span(oxc_span::Span::new(4, 8)),
            Some(Span::new(FileId(7), 46, 50))
        );
        assert_eq!(mapper.point(20), Some(Span::new(FileId(7), 62, 62)));
        assert_eq!(mapper.span(3, 21), None);
    }

    #[test]
    fn maps_parse_errors_to_vue3_diagnostics_with_source_span() {
        let store = JsAstStore::new();
        let err = store
            .parse_expression("(a[)", SourceType::script())
            .expect_err("parse error");
        let diagnostic =
            err.to_vue3_invalid_expression_diagnostic("a[", Some(Span::new(FileId(3), 100, 102)));
        assert_eq!(diagnostic.code, "46");
        assert!(diagnostic
            .message
            .contains("Error parsing JavaScript expression"));
        assert_eq!(diagnostic.span, Some(Span::new(FileId(3), 102, 102)));
    }

    #[test]
    fn parses_vue2_filters_before_validating_base_expression() {
        let store = JsAstStore::new();
        let parsed = store
            .parse_vue2_filter_expression(
                "message | capitalize | append('!')",
                SourceType::script(),
            )
            .expect("filter chain");

        assert_eq!(parsed.base, "message");
        assert_eq!(parsed.filters.len(), 2);
        assert_eq!(parsed.filters[0].name, "capitalize");
        assert_eq!(parsed.filters[1].name, "append");
        assert_eq!(parsed.filters[1].args, vec!["'!'"]);
        assert_eq!(
            rewrite_vue2_filter_expression("message | capitalize | append('!')"),
            "_f(\"append\")(_f(\"capitalize\")(message),'!')"
        );

        let logical_or = parse_vue2_filter_expression("a || b | c");
        assert_eq!(logical_or.base, "a || b");
        assert_eq!(logical_or.filters[0].name, "c");

        assert_eq!(
            rewrite_vue2_filter_expression("message | append('!', count + 1)"),
            "_f(\"append\")(message,'!', count + 1)"
        );
    }

    #[test]
    fn parses_vue2_filter_args_without_splitting_literal_commas() {
        let store = JsAstStore::new();
        let parsed = store
            .parse_vue2_filter_expression(
                "message | append(',', `x,y`, /a,b/g, count)",
                SourceType::script(),
            )
            .expect("filter chain");

        assert_eq!(parsed.filters.len(), 1);
        assert_eq!(
            parsed.filters[0].args,
            vec!["','", "`x,y`", "/a,b/g", "count"]
        );
        assert_eq!(
            rewrite_vue2_filter_expression("message | append(',', `x,y`, /a,b/g, count)"),
            "_f(\"append\")(message,',', `x,y`, /a,b/g, count)"
        );
    }

    #[test]
    fn rejects_invalid_vue2_filter_arguments() {
        let store = JsAstStore::new();
        assert!(store
            .parse_vue2_filter_expression("message | append(foo()", SourceType::script())
            .is_err());
        assert!(store
            .parse_vue2_filter_expression("message | append(ok, nope }", SourceType::script())
            .is_err());
    }

    #[test]
    fn prefixes_expression_identifiers_with_locals_and_property_keys() {
        let rewritten = prefix_expression_identifiers(
            "{ foo, bar: baz, nested: local + Math.max(count, item.value) }",
            |ident: &str| Some(format!("_ctx.{ident}")),
            &["local".into()],
        );
        assert_eq!(
            rewritten,
            "{ foo: _ctx.foo, bar: _ctx.baz, nested: local + Math.max(_ctx.count, _ctx.item.value) }"
        );
    }
}
