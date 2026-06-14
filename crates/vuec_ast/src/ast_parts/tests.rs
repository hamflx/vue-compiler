#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn documents_roundtrip_through_serde() {
        let doc = Vue2Ast::new(Vue2NodeKind::root(), None);
        let root = doc.root;
        let json = serde_json::to_string(&doc).unwrap();
        let decoded: Vue2Ast = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.root, root);
        assert_eq!(decoded.len(), 1);
    }

    #[test]
    fn document_capacity_can_be_reserved_without_changing_tree_shape() {
        let mut doc = Vue3Ast::with_capacity(Vue3NodeKind::root(), None, 16);
        assert_eq!(doc.root, NodeId(0));
        assert_eq!(doc.len(), 1);
        assert!(doc.node_capacity() >= 16);
        doc.reserve_nodes(32);
        assert!(doc.node_capacity() >= 33);

        let child = doc.push_child(doc.root, Vue3NodeKind::text("hello"), None);
        assert_eq!(child, NodeId(1));
        assert_eq!(doc.validate_tree(), Ok(()));
    }

    #[test]
    fn distinct_kind_spaces_exist() {
        let mut vue3 = Vue3Ast::new(Vue3NodeKind::root(), None);
        let id = vue3.push_child(
            vue3.root,
            Vue3NodeKind::element(
                "div",
                vec![TemplateAttribute {
                    name: "id".into(),
                    value: Some("app".into()),
                }],
                false,
            ),
            None,
        );
        assert!(matches!(
            vue3.node(id).unwrap().kind,
            Vue3NodeKind::Element(_)
        ));
        let mut mir = Vue3DomMir::new(Vue3DomMirKind::Root(Vue3DomRoot::default()), None);
        let _ = mir.push_child(
            mir.root,
            Vue3DomMirKind::TextCall {
                value: MirExpr::String("main".into()),
            },
            None,
        );
        assert_eq!(mir.len(), 2);
        assert_eq!(Mir::Vue3Dom(mir).target(), MirTarget::Vue3Dom);
    }

    #[test]
    fn attach_child_records_parent_and_index() {
        let mut doc = Vue3Ast::new(Vue3NodeKind::root(), None);
        let root = doc.root;
        let child = doc.push_child(root, Vue3NodeKind::text("hello"), None);
        assert_eq!(doc.node(child).and_then(|node| node.parent), Some(root));
        assert_eq!(doc.node(child).map(|node| node.index_in_parent), Some(0));
        assert_eq!(doc.validate_tree(), Ok(()));
    }

    #[test]
    fn reattach_child_refreshes_old_parent_indexes() {
        let mut doc = Vue3Ast::new(Vue3NodeKind::root(), None);
        let old_parent = doc.push_child(
            doc.root,
            Vue3NodeKind::element("section", Vec::new(), false),
            None,
        );
        let first = doc.push_child(old_parent, Vue3NodeKind::text("a"), None);
        let moved = doc.push_child(old_parent, Vue3NodeKind::text("b"), None);
        let third = doc.push_child(old_parent, Vue3NodeKind::text("c"), None);

        doc.attach_child(doc.root, moved);

        assert_eq!(doc.node(first).unwrap().index_in_parent, 0);
        assert_eq!(doc.node(third).unwrap().index_in_parent, 1);
        assert_eq!(doc.node(moved).unwrap().parent, Some(doc.root));
        assert_eq!(doc.validate_tree(), Ok(()));
    }

    #[test]
    fn runtime_helpers_are_orderable() {
        let mut helpers = BTreeSet::new();
        helpers.insert(RuntimeHelper::Vue3OpenBlock);
        helpers.insert(RuntimeHelper::Vue3CreateElementBlock);
        assert_eq!(helpers.len(), 2);
    }

    #[test]
    fn public_projection_is_nested_and_deterministic() {
        let mut doc = Vue3Ast::new(Vue3NodeKind::root(), None);
        let child = doc.push_child(
            doc.root,
            Vue3NodeKind::text("hello"),
            NodeSpan::generated(None, GeneratedReason::Lowering),
        );
        let projected = doc.project_public();
        assert!(matches!(projected.kind, Vue3NodeKind::Root(_)));
        assert_eq!(projected.children.len(), 1);
        assert_eq!(doc.node(child).unwrap().index_in_parent, 0);
        let json = serde_json::to_string(&projected).unwrap();
        assert!(json.contains("Generated"));
    }

    #[test]
    fn set_root_rejects_missing_node_id() {
        let mut doc = Vue3Ast::new(Vue3NodeKind::root(), None);
        let original_root = doc.root;

        assert!(!doc.set_root(NodeId(99)));

        assert_eq!(doc.root, original_root);
        assert_eq!(doc.validate_tree(), Ok(()));
    }

    #[test]
    fn try_project_public_reports_invalid_external_root() {
        let mut doc = Vue3Ast::new(Vue3NodeKind::root(), None);
        doc.root = NodeId(99);

        assert_eq!(
            doc.try_project_public(),
            Err(AstInvariantError::MissingRoot { root: NodeId(99) })
        );
    }

    #[test]
    fn lowering_map_records_explicit_edges() {
        let mut map = LoweringMap::default();
        map.record_ast_to_hir(NodeId(1), NodeId(10));
        map.record_hir_to_mir(NodeId(10), NodeId(20));
        assert_eq!(
            map.hir_for_ast(NodeId(1)).collect::<Vec<_>>(),
            vec![NodeId(10)]
        );
        assert_eq!(
            map.mir_for_hir(NodeId(10)).collect::<Vec<_>>(),
            vec![NodeId(20)]
        );
    }

    #[test]
    fn hir_has_no_runtime_helper_or_codegen_call_variant() {
        let expression = JsExprId(0);
        let hir = HirNodeKind::Interpolation(HirInterpolation {
            expression: HirExpr::Js(expression),
        });
        assert!(matches!(hir, HirNodeKind::Interpolation(_)));
    }

    #[test]
    fn visitor_reports_stable_enter_exit_order() {
        #[derive(Default)]
        struct Recorder {
            events: Vec<String>,
        }

        impl AstVisitor<Vue3NodeKind> for Recorder {
            fn enter_node(
                &mut self,
                _document: &AstDocument<Vue3NodeKind>,
                node: &Node<Vue3NodeKind>,
            ) -> VisitControl {
                self.events.push(format!("enter:{}", node.id.0));
                VisitControl::Continue
            }

            fn exit_node(
                &mut self,
                _document: &AstDocument<Vue3NodeKind>,
                node: &Node<Vue3NodeKind>,
            ) -> VisitControl {
                self.events.push(format!("exit:{}", node.id.0));
                VisitControl::Continue
            }
        }

        let mut doc = Vue3Ast::new(Vue3NodeKind::root(), Span::new(FileId(0), 0, 10));
        let element = doc.push_child(
            doc.root,
            Vue3NodeKind::element("div", Vec::new(), false),
            Span::new(FileId(0), 0, 10),
        );
        doc.push_child(
            element,
            Vue3NodeKind::text("hello"),
            Span::new(FileId(0), 5, 10),
        );

        let mut recorder = Recorder::default();
        assert_eq!(doc.visit(&mut recorder), VisitControl::Continue);
        assert_eq!(
            recorder.events,
            vec!["enter:0", "enter:1", "enter:2", "exit:2", "exit:1", "exit:0"]
        );
    }

    #[test]
    fn mutable_visitor_can_update_payloads_without_changing_tree_shape() {
        struct UppercaseText;

        impl AstVisitorMut<Vue3NodeKind> for UppercaseText {
            fn enter_node_mut(&mut self, node: &mut Node<Vue3NodeKind>) -> VisitControl {
                if let Vue3NodeKind::Text(text) = &mut node.kind {
                    text.value.make_ascii_uppercase();
                }
                VisitControl::Continue
            }
        }

        let mut doc = Vue3Ast::new(Vue3NodeKind::root(), Span::new(FileId(0), 0, 5));
        let text = doc.push_child(
            doc.root,
            Vue3NodeKind::text("hello"),
            Span::new(FileId(0), 0, 5),
        );

        assert_eq!(doc.visit_mut(&mut UppercaseText), VisitControl::Continue);
        assert_eq!(doc.validate_tree(), Ok(()));
        assert!(matches!(
            &doc.node(text).unwrap().kind,
            Vue3NodeKind::Text(value) if value.value == "HELLO"
        ));
    }

    #[test]
    fn snapshot_json_preserves_generated_and_missing_span_reasons() {
        let mut doc = Vue3Ast::new(
            Vue3NodeKind::root(),
            NodeSpan::generated(Some(Span::new(FileId(0), 0, 1)), GeneratedReason::Lowering),
        );
        doc.push_child(
            doc.root,
            Vue3NodeKind::text("fallback"),
            NodeSpan::missing(MissingSpanReason::ParseRecovery),
        );

        let json = doc.snapshot_json().expect("snapshot json");
        assert!(json.contains("Generated"));
        assert!(json.contains("Lowering"));
        assert!(json.contains("Missing"));
        assert!(json.contains("ParseRecovery"));
    }

    #[test]
    fn span_consistency_checks_node_and_nested_spans() {
        let mut doc = Vue3Ast::new(Vue3NodeKind::root(), Span::new(FileId(0), 0, 20));
        doc.push_child(
            doc.root,
            Vue3AstKind::Element(Vue3Element {
                tag: "div".into(),
                tag_type: Vue3ElementType::Element,
                ns: HtmlNamespace::Html,
                props: vec![
                    Vue3Prop::Attribute(Vue3Attribute {
                        name: "id".into(),
                        value: Some("a".into()),
                        span: Some(Span::new(FileId(0), 5, 11)),
                        name_span: Some(Span::new(FileId(0), 5, 7)),
                        value_span: Some(Span::new(FileId(0), 9, 10)),
                        quote: Some(QuoteKind::Double),
                    }),
                    Vue3Prop::Directive(Vue3Directive {
                        name: "bind".into(),
                        raw_name: ":class.mod".into(),
                        arg: Some(Vue3Expression::Raw("class".into())),
                        exp: Some(Vue3Expression::Raw("klass".into())),
                        modifiers: vec!["mod".into()],
                        is_dynamic_arg: false,
                        span: Some(Span::new(FileId(0), 12, 30)),
                        arg_span: Some(Span::new(FileId(0), 13, 18)),
                        exp_span: Some(Span::new(FileId(0), 24, 29)),
                        modifier_spans: vec![NodeSpan::Source(Span::new(FileId(0), 19, 22))],
                    }),
                ],
                self_closing: false,
                codegen_node: None,
                ssr_codegen_node: None,
            }),
            Span::new(FileId(0), 0, 30),
        );

        assert_eq!(doc.validate_span_consistency(), Ok(()));
    }

    #[test]
    fn vue2_ast_schema_keeps_source_ranges_and_filter_structure_out_of_children() {
        let mut element = Vue2Element::new("li");
        element.if_exp = Some(JsExprId(0));
        element.if_span = Some(Span::new(FileId(0), 1, 10));
        element.for_exp = Some(JsExprId(1));
        element.for_span = Some(Span::new(FileId(0), 11, 25));
        element.alias = Some(JsPatternId(0));
        element.key = Some(JsExprId(2));
        element.key_span = Some(Span::new(FileId(0), 26, 35));
        element.attrs_list.push(Vue2Attribute {
            name: ":title".into(),
            value: "title".into(),
            span: Some(Span::new(FileId(0), 36, 50)),
            dynamic: true,
        });

        let mut doc = Vue2Ast::new(
            Vue2AstKind::Root(Vue2Root::default()),
            Span::new(FileId(0), 0, 60),
        );
        let element_id = doc.push_child(
            doc.root,
            Vue2AstKind::Element(element),
            Span::new(FileId(0), 0, 60),
        );
        let text_id = doc.push_child(
            element_id,
            Vue2AstKind::ExpressionText(Vue2ExpressionText {
                raw: "msg | cap".into(),
                expr: None,
                filter_expr: Some(Vue2FilterExpr {
                    raw: "msg | cap".into(),
                    base: JsExprId(3),
                    filters: vec![Vue2FilterCall {
                        name: "cap".into(),
                        args: Vec::new(),
                    }],
                }),
            }),
            Span::new(FileId(0), 40, 52),
        );

        assert_eq!(doc.validate_span_consistency(), Ok(()));
        assert_eq!(doc.node(element_id).unwrap().children, vec![text_id]);
        let Vue2AstKind::Element(projected) = &doc.node(element_id).unwrap().kind else {
            panic!("expected element");
        };
        assert_eq!(projected.for_span, Some(Span::new(FileId(0), 11, 25)));
        assert_eq!(
            projected.attrs_list[0].span,
            Some(Span::new(FileId(0), 36, 50))
        );
    }
}
