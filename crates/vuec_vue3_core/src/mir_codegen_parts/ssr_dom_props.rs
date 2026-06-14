impl<'a> Vue3SsrMirCodegen<'a> {
    fn render_slot_name(&self, name: &Vue3DomSlotName, scope: &RenderScope) -> String {
        match name {
            Vue3DomSlotName::Static(name) => quote_string(name),
            Vue3DomSlotName::Dynamic(name) => self.render_js_expr(*name, scope),
        }
    }

    fn render_slot_props(&self, props: &Vue3DomProps, scope: &RenderScope) -> String {
        let rendered = self
            .render_ordered_props(props, scope)
            .unwrap_or_else(|| "{}".into());
        self.render_normalized_props(props, rendered)
    }

    fn render_ordered_props(&self, props: &Vue3DomProps, scope: &RenderScope) -> Option<String> {
        if props.segments.is_empty() {
            let mut entries = Vec::new();
            for attr in &props.static_attrs {
                entries.push(self.render_static_attr(attr));
            }
            for binding in &props.dynamic_bindings {
                entries.push(self.render_dynamic_binding(binding, scope));
            }
            for event in &props.events {
                entries.push(self.render_event(event, scope));
            }
            return self.render_plain_props(&entries);
        }

        let mut merge_args = Vec::new();
        let mut object_entries = Vec::new();
        for segment in &props.segments {
            match segment {
                Vue3DomPropSegment::StaticAttr(attr) => {
                    object_entries.push(self.render_static_attr(attr));
                }
                Vue3DomPropSegment::DynamicBinding(binding) => {
                    object_entries.push(self.render_dynamic_binding(binding, scope));
                }
                Vue3DomPropSegment::Content(_) => {}
                Vue3DomPropSegment::Model(_) => {}
                Vue3DomPropSegment::Event(event) => {
                    object_entries.push(self.render_event(event, scope));
                }
                Vue3DomPropSegment::ObjectBinding(binding) => {
                    self.push_merge_object_arg(&mut merge_args, &mut object_entries);
                    merge_args.push(self.render_js_expr(binding.value, scope));
                }
                Vue3DomPropSegment::ObjectListeners(listeners) => {
                    self.push_merge_object_arg(&mut merge_args, &mut object_entries);
                    merge_args.push(self.render_object_listeners(listeners, scope));
                }
            }
        }
        self.push_merge_object_arg(&mut merge_args, &mut object_entries);
        if merge_args.is_empty() {
            None
        } else if merge_args.len() == 1 {
            merge_args.into_iter().next()
        } else {
            Some(format!("_mergeProps({})", merge_args.join(", ")))
        }
    }

    fn render_static_attr(&self, attr: &Vue3DomStaticAttr) -> String {
        format!("{}: {}", json_key(&attr.name), quote_string(&attr.value))
    }

    fn render_dynamic_binding(&self, binding: &Vue3DomBinding, scope: &RenderScope) -> String {
        let value = self.render_js_expr(binding.value, scope);
        if binding.dynamic_arg {
            format!(
                "[{}]: {}",
                render_vue3_dom_binding_dynamic_key(
                    binding,
                    binding
                        .dynamic_name
                        .map(|id| self.render_js_expr(id, scope))
                        .unwrap_or_else(|| binding.name.clone()),
                    true,
                ),
                value
            )
        } else if binding.name == "class" {
            format!("class: _normalizeClass({value})")
        } else {
            format!(
                "{}: {}",
                json_key(&render_vue3_dom_binding_static_key(binding, true)),
                value
            )
        }
    }

    fn render_event(&self, event: &Vue3DomEvent, scope: &RenderScope) -> String {
        let handler = self.render_guarded_event_handler(event, scope);
        let key = self.render_event_key(event, scope);
        format!("{key}: {handler}")
    }

    fn render_event_key(&self, event: &Vue3DomEvent, scope: &RenderScope) -> String {
        if event.dynamic_arg {
            let name = event
                .dynamic_name
                .map(|id| self.render_js_expr(id, scope))
                .unwrap_or_else(|| event.name.clone());
            let handler_key = format!("_toHandlerKey({})", name.trim());
            let transformed = self.render_event_click_key(event, handler_key);
            return format!("[{}]", self.render_event_option_key(event, transformed));
        }
        json_key(&self.render_event_option_key(event, event.name.clone()))
    }

    fn render_guarded_event_handler(&self, event: &Vue3DomEvent, scope: &RenderScope) -> String {
        let mut handler = self.render_js_stmt(event.handler, scope);
        if !event.runtime_modifiers.is_empty() {
            handler = format!(
                "_withModifiers({handler}, {})",
                render_string_array(&event.runtime_modifiers)
            );
        }
        if !event.key_modifiers.is_empty() {
            handler = format!(
                "_withKeys({handler}, {})",
                render_string_array(&event.key_modifiers)
            );
        }
        handler
    }

    fn render_event_click_key(&self, event: &Vue3DomEvent, key: String) -> String {
        match event.click_event {
            Some(Vue3DomClickEvent::ContextMenu) => {
                format!("({key}) === \"onClick\" ? \"onContextmenu\" : ({key})")
            }
            Some(Vue3DomClickEvent::MouseUp) => {
                format!("({key}) === \"onClick\" ? \"onMouseup\" : ({key})")
            }
            None => key,
        }
    }

    fn render_event_option_key(&self, event: &Vue3DomEvent, key: String) -> String {
        let postfix = vue3_dom_event_option_postfix(&event.option_modifiers);
        if postfix.is_empty() {
            key
        } else if event.dynamic_arg {
            format!("({key}) + {}", quote_string(&postfix))
        } else {
            format!("{key}{postfix}")
        }
    }

    fn render_object_listeners(
        &self,
        listeners: &Vue3DomObjectListeners,
        scope: &RenderScope,
    ) -> String {
        let handlers = self.render_js_expr(listeners.value, scope);
        if listeners.preserve_case {
            format!("_toHandlers({handlers}, true)")
        } else {
            format!("_toHandlers({handlers})")
        }
    }

    fn push_merge_object_arg(
        &self,
        merge_args: &mut Vec<String>,
        object_entries: &mut Vec<String>,
    ) {
        if let Some(object) = self.render_plain_props(object_entries) {
            merge_args.push(object);
            object_entries.clear();
        }
    }

    fn render_plain_props(&self, entries: &[String]) -> Option<String> {
        if entries.is_empty() {
            None
        } else if entries.len() == 1 {
            Some(format!("{{ {} }}", entries.join(", ")))
        } else {
            Some(render_object(entries))
        }
    }

    fn render_normalized_props(&self, props: &Vue3DomProps, rendered: String) -> String {
        if !props.normalize.normalize_props {
            return rendered;
        }
        let argument = if props.normalize.guard_reactive_props {
            format!("_guardReactiveProps({rendered})")
        } else {
            rendered
        };
        format!("_normalizeProps({argument})")
    }
}
