    #[test]
    fn scheduler_runs_passes() {
        let mut scheduler = PassScheduler::new();
        scheduler.push(CountPass::default());
        let mut nodes = vec![1, 2, 3];
        let mut ctx = TransformContext::default();
        scheduler.run(&mut nodes, &mut ctx);
        assert_eq!(ctx.helpers.len(), 1);
        assert!(ctx.helpers.contains(&RuntimeHelper::Vue3OpenBlock));
    }

    #[test]
    fn scheduler_orders_passes_stably() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut scheduler = PassScheduler::new();
        scheduler.push(NamedPass {
            name: "late",
            order: PassOrder::LATE,
            events: events.clone(),
        });
        scheduler.push(NamedPass {
            name: "early",
            order: PassOrder::EARLY,
            events: events.clone(),
        });
        scheduler.push(NamedPass {
            name: "default-a",
            order: PassOrder::DEFAULT,
            events: events.clone(),
        });
        scheduler.push(NamedPass {
            name: "default-b",
            order: PassOrder::DEFAULT,
            events: events.clone(),
        });

        assert_eq!(
            scheduler.pass_names(),
            vec!["early", "default-a", "default-b", "late"]
        );
        let mut nodes = vec![0usize];
        scheduler.run(&mut nodes, &mut TransformContext::default());
        assert_eq!(
            events.borrow().as_slice(),
            ["early", "default-a", "default-b", "late"]
        );
    }

    #[test]
    fn transform_context_tracks_scope_stack_helpers_and_diagnostics() {
        let mut ctx = TransformContext::default();
        assert!(ctx.add_helper(RuntimeHelper::Vue3OpenBlock));
        assert!(ctx.has_helper(RuntimeHelper::Vue3OpenBlock));
        ctx.enter_scope(ScopeKind::Root);
        ctx.enter_scope(ScopeKind::VFor);
        assert_eq!(ctx.scope_depth, 2);
        assert!(ctx.add_scope_binding("item"));
        assert!(ctx.is_binding_in_scope("item"));
        assert_eq!(ctx.current_scope().unwrap().kind, ScopeKind::VFor);
        assert_eq!(ctx.exit_scope().unwrap().kind, ScopeKind::VFor);
        assert_eq!(ctx.scope_depth, 1);
        assert!(!ctx.is_binding_in_scope("item"));
    }

    #[derive(Default)]
    struct RecordDocumentPass {
        name: &'static str,
        order: PassOrder,
        events: Rc<RefCell<Vec<String>>>,
    }

    impl DocumentPass<usize> for RecordDocumentPass {
        fn name(&self) -> &'static str {
            self.name
        }

        fn order(&self) -> PassOrder {
            self.order
        }

        fn enter(
            &mut self,
            _doc: &mut AstDocument<usize>,
            node: NodeId,
            _ctx: &mut TransformContext,
        ) {
            self.events
                .borrow_mut()
                .push(format!("{}:enter:{}", self.name, node.0));
        }

        fn exit(
            &mut self,
            _doc: &mut AstDocument<usize>,
            node: NodeId,
            _ctx: &mut TransformContext,
        ) {
            self.events
                .borrow_mut()
                .push(format!("{}:exit:{}", self.name, node.0));
        }
    }

    #[test]
    fn document_walk_is_depth_first() {
        let mut doc = vuec_ast::AstDocument::new(0usize, None);
        let root = doc.root;
        let child = doc.push_child(root, 1usize, None);
        let _grandchild = doc.push_child(child, 2usize, None);
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut pass = RecordDocumentPass {
            name: "document",
            order: PassOrder::DEFAULT,
            events: events.clone(),
        };
        let mut ctx = TransformContext::default();
        walk_document(&mut doc, &mut pass, &mut ctx);
        assert_eq!(
            events.borrow().as_slice(),
            [
                "document:enter:0",
                "document:enter:1",
                "document:enter:2",
                "document:exit:2",
                "document:exit:1",
                "document:exit:0"
            ]
        );
    }

    #[test]
    fn document_scheduler_orders_passes() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut doc = AstDocument::new(0usize, None);
        let mut scheduler = DocumentPassScheduler::new();
        scheduler.push(RecordDocumentPass {
            name: "late",
            order: PassOrder::LATE,
            events: events.clone(),
        });
        scheduler.push(RecordDocumentPass {
            name: "early",
            order: PassOrder::EARLY,
            events: events.clone(),
        });

        assert_eq!(scheduler.pass_names(), vec!["early", "late"]);
        scheduler.run(&mut doc, &mut TransformContext::default());
        assert_eq!(
            events.borrow().as_slice(),
            [
                "early:enter:0",
                "early:exit:0",
                "late:enter:0",
                "late:exit:0"
            ]
        );
    }

    struct Vue2Recorder {
        name: &'static str,
        order: PassOrder,
        events: Rc<RefCell<Vec<String>>>,
    }

    impl Vue2Module for Vue2Recorder {
        fn name(&self) -> &'static str {
            self.name
        }

        fn order(&self) -> PassOrder {
            self.order
        }

        fn pre_transform_node(
            &mut self,
            _doc: &mut Vue2Ast,
            node: NodeId,
            _ctx: &mut TransformContext,
        ) -> VisitControl {
            self.events
                .borrow_mut()
                .push(format!("{}:pre:{}", self.name, node.0));
            VisitControl::Continue
        }

        fn transform_node(
            &mut self,
            _doc: &mut Vue2Ast,
            node: NodeId,
            _ctx: &mut TransformContext,
        ) -> VisitControl {
            self.events
                .borrow_mut()
                .push(format!("{}:transform:{}", self.name, node.0));
            VisitControl::Continue
        }

        fn post_transform_node(
            &mut self,
            _doc: &mut Vue2Ast,
            node: NodeId,
            _ctx: &mut TransformContext,
        ) -> VisitControl {
            self.events
                .borrow_mut()
                .push(format!("{}:post:{}", self.name, node.0));
            VisitControl::Continue
        }

        fn gen_data(
            &mut self,
            element: &Vue2Element,
            _ctx: &mut TransformContext,
        ) -> Option<String> {
            Some(format!("{}:{}", self.name, element.tag))
        }
    }

    #[test]
    fn vue2_module_scheduler_replicates_pre_transform_post_order() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut doc = Vue2Ast::new(Vue2AstKind::root(), None);
        let child = doc.push_child(doc.root, Vue2AstKind::element("div"), None);
        doc.push_child(child, Vue2AstKind::text("hello"), None);
        let mut scheduler = Vue2ModuleScheduler::new();
        scheduler.push(Vue2Recorder {
            name: "module",
            order: PassOrder::DEFAULT,
            events: events.clone(),
        });

        assert_eq!(
            scheduler.run(&mut doc, &mut TransformContext::default()),
            VisitControl::Continue
        );
        assert_eq!(
            events.borrow().as_slice(),
            [
                "module:pre:0",
                "module:pre:1",
                "module:pre:2",
                "module:transform:2",
                "module:post:2",
                "module:transform:1",
                "module:post:1",
                "module:transform:0",
                "module:post:0"
            ]
        );
        assert_eq!(
            scheduler.gen_data_for_node(&doc, child, &mut TransformContext::default()),
            vec!["module:div".to_string()]
        );
    }

    struct Vue3Recorder {
        name: &'static str,
        order: PassOrder,
        events: Rc<RefCell<Vec<String>>>,
    }

    impl Vue3NodeTransform for Vue3Recorder {
        fn name(&self) -> &'static str {
            self.name
        }

        fn order(&self) -> PassOrder {
            self.order
        }

        fn enter(
            &mut self,
            _doc: &mut Vue3Ast,
            node: NodeId,
            _ctx: &mut TransformContext,
        ) -> Vue3NodeTransformResult {
            self.events
                .borrow_mut()
                .push(format!("{}:enter:{}", self.name, node.0));
            Vue3NodeTransformResult::with_exit()
        }

        fn exit(&mut self, _doc: &mut Vue3Ast, node: NodeId, _ctx: &mut TransformContext) {
            self.events
                .borrow_mut()
                .push(format!("{}:exit:{}", self.name, node.0));
        }
    }

    #[test]
    fn vue3_node_scheduler_uses_depth_first_lifo_exit_order() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut doc = Vue3Ast::new(Vue3AstKind::root(), None);
        let child = doc.push_child(doc.root, Vue3AstKind::text("hello"), None);
        doc.push_child(child, Vue3AstKind::comment("x"), None);
        let mut scheduler = Vue3NodeTransformScheduler::new();
        scheduler.push(Vue3Recorder {
            name: "a",
            order: PassOrder::DEFAULT,
            events: events.clone(),
        });
        scheduler.push(Vue3Recorder {
            name: "b",
            order: PassOrder::DEFAULT,
            events: events.clone(),
        });

        assert_eq!(
            scheduler.run(&mut doc, &mut TransformContext::default()),
            VisitControl::Continue
        );
        assert_eq!(
            events.borrow().as_slice(),
            [
                "a:enter:0",
                "b:enter:0",
                "a:enter:1",
                "b:enter:1",
                "a:enter:2",
                "b:enter:2",
                "b:exit:2",
                "a:exit:2",
                "b:exit:1",
                "a:exit:1",
                "b:exit:0",
                "a:exit:0"
            ]
        );
    }

    struct DirectiveTransform {
        name: &'static str,
        directive_name: &'static str,
        order: PassOrder,
        outcome: Vue3DirectiveTransformOutcome,
    }

    impl Vue3DirectiveTransform for DirectiveTransform {
        fn name(&self) -> &'static str {
            self.name
        }

        fn directive_name(&self) -> &'static str {
            self.directive_name
        }

        fn order(&self) -> PassOrder {
            self.order
        }

        fn transform(
            &mut self,
            _doc: &mut Vue3Ast,
            _node: NodeId,
            _prop_index: usize,
            _directive: &Vue3Directive,
            _ctx: &mut TransformContext,
        ) -> Vue3DirectiveTransformOutcome {
            self.outcome.clone()
        }
    }

    #[test]
    fn vue3_directive_registry_can_extend_or_replace_default_behavior() {
        let directive = Vue3Prop::Directive(Vue3Directive {
            name: "on".into(),
            raw_name: "@click".into(),
            arg: Some(Vue3Expression::Raw("click".into())),
            exp: Some(Vue3Expression::Raw("submit".into())),
            modifiers: Vec::new(),
            is_dynamic_arg: false,
            span: None,
            arg_span: None,
            exp_span: None,
            modifier_spans: Vec::new(),
        });
        let mut doc = Vue3Ast::new(Vue3AstKind::root(), None);
        let element = doc.push_child(
            doc.root,
            Vue3AstKind::Element(Vue3Element {
                tag: "button".into(),
                tag_type: Vue3ElementType::Element,
                ns: vuec_ast::HtmlNamespace::Html,
                props: vec![directive],
                self_closing: false,
                codegen_node: None,
                ssr_codegen_node: None,
            }),
            None,
        );
        let replacement = Vue3Prop::Attribute(Vue3Attribute {
            name: "data-replaced".into(),
            value: Some("yes".into()),
            span: None,
            name_span: None,
            value_span: None,
            quote: Some(QuoteKind::Double),
        });
        let extension = Vue3Prop::Attribute(Vue3Attribute {
            name: "data-extra".into(),
            value: None,
            span: None,
            name_span: None,
            value_span: None,
            quote: None,
        });
        let mut output = Vue3DirectiveTransformOutput::with_prop(replacement.clone());
        output.add_helper(RuntimeHelper::Vue3WithModifiers);
        let mut registry = Vue3DirectiveTransformRegistry::new();
        registry.push(DirectiveTransform {
            name: "replace-on",
            directive_name: "on",
            order: PassOrder::DEFAULT,
            outcome: Vue3DirectiveTransformOutcome::Replace(output),
        });
        registry.push(DirectiveTransform {
            name: "extend-on",
            directive_name: "on",
            order: PassOrder::LATE,
            outcome: Vue3DirectiveTransformOutcome::Extend(
                Vue3DirectiveTransformOutput::with_prop(extension.clone()),
            ),
        });

        let mut ctx = TransformContext::default();
        let resolution = registry.resolve(&mut doc, element, 0, &mut ctx);
        assert!(!resolution.use_default);
        assert_eq!(resolution.props, vec![replacement, extension]);
        assert!(ctx.has_helper(RuntimeHelper::Vue3WithModifiers));
    }
