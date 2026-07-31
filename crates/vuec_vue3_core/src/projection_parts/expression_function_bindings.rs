#[derive(Clone, Copy, Debug)]
struct ProcessExpressionFunctionScope {
    scope_start: usize,
    max_scope_end: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ProcessExpressionFunctionBindingIndex {
    bindings: BTreeSet<(usize, usize)>,
    non_reference_keys: BTreeSet<(usize, usize)>,
    scopes: BTreeMap<String, Vec<ProcessExpressionFunctionScope>>,
}

struct ProcessExpressionFunctionBindingCollector<'source> {
    source: &'source str,
    source_start: usize,
    source_end: usize,
    declaration_scopes: Vec<(usize, usize)>,
    bindings: ProcessExpressionFunctionBindingIndex,
}

impl<'source> ProcessExpressionFunctionBindingCollector<'source> {
    fn new(source: &'source str, source_start: usize, source_end: usize) -> Self {
        Self {
            source,
            source_start,
            source_end,
            declaration_scopes: vec![(0, source_end - source_start)],
            bindings: ProcessExpressionFunctionBindingIndex::default(),
        }
    }

    fn relative_span(&self, span: oxc_span::Span) -> Option<(usize, usize)> {
        let start = span.start as usize;
        let end = span.end as usize;
        if start < self.source_start || end > self.source_end || start > end {
            return None;
        }
        Some((start - self.source_start, end - self.source_start))
    }

    fn relative_identifier_span(&self, span: oxc_span::Span) -> Option<(usize, usize)> {
        let (mut start, end) = self.relative_span(span)?;
        if self
            .source
            .as_bytes()
            .get(self.source_start + start)
            .is_some_and(|byte| *byte == b'#')
        {
            start = start.saturating_add(1);
        }
        (start < end).then_some((start, end))
    }

    fn add_binding(&mut self, span: oxc_span::Span) {
        if let Some(span) = self.relative_identifier_span(span) {
            self.bindings.bindings.insert(span);
        }
    }

    fn add_non_reference_key(&mut self, key: &PropertyKey<'_>, computed: bool) {
        if computed {
            return;
        }
        let span = match key {
            PropertyKey::StaticIdentifier(identifier) => identifier.span,
            PropertyKey::PrivateIdentifier(identifier) => identifier.span,
            _ => return,
        };
        if let Some(span) = self.relative_identifier_span(span) {
            self.bindings.non_reference_keys.insert(span);
        }
    }

    fn add_non_reference_modifier(
        &mut self,
        node_span: oxc_span::Span,
        key: &PropertyKey<'_>,
        modifier: &str,
    ) {
        let key_span = oxc_span::GetSpan::span(key);
        self.add_non_reference_modifier_before(node_span, key_span.start, modifier);
    }

    fn add_non_reference_modifier_before(
        &mut self,
        node_span: oxc_span::Span,
        before: u32,
        modifier: &str,
    ) {
        let Some((node_start, node_end)) = self.relative_span(node_span) else {
            return;
        };
        let before = before as usize;
        if before < self.source_start || before > self.source_start + node_end {
            return;
        }
        let modifier_end = before - self.source_start;
        let Some(prefix) = self.source.get(
            self.source_start + node_start..self.source_start + modifier_end,
        ) else {
            return;
        };
        let mut chars = prefix.char_indices().peekable();
        while let Some((offset, ch)) = chars.next() {
            if ch == '/' && chars.peek().is_some_and(|(_, next)| *next == '/') {
                chars.next();
                for (_, comment) in chars.by_ref() {
                    if matches!(comment, '\n' | '\r') {
                        break;
                    }
                }
                continue;
            }
            if ch == '/' && chars.peek().is_some_and(|(_, next)| *next == '*') {
                chars.next();
                let mut previous = '\0';
                for (_, comment) in chars.by_ref() {
                    if previous == '*' && comment == '/' {
                        break;
                    }
                    previous = comment;
                }
                continue;
            }
            if !is_identifier_start(ch) {
                continue;
            }
            let mut end = offset + ch.len_utf8();
            while let Some(&(next_offset, next)) = chars.peek() {
                if !is_identifier_continue(next) {
                    break;
                }
                chars.next();
                end = next_offset + next.len_utf8();
            }
            if prefix.get(offset..end) == Some(modifier) {
                self.bindings
                    .non_reference_keys
                    .insert((node_start + offset, node_start + end));
                return;
            }
        }
    }

    fn relative_braced_scope_after(
        &self,
        node_span: oxc_span::Span,
        after: u32,
    ) -> Option<(usize, usize)> {
        let (_, node_end) = self.relative_span(node_span)?;
        let after = after as usize;
        if after < self.source_start || after > self.source_start + node_end {
            return None;
        }
        let search_start = after - self.source_start;
        let source = self
            .source
            .get(self.source_start + search_start..self.source_start + node_end)?;
        let open = source.find('{')?;
        Some((search_start + open, node_end))
    }

    fn add_scope(&mut self, name: &str, scope: (usize, usize)) {
        if scope.0 > scope.1 {
            return;
        }
        self.bindings
            .scopes
            .entry(name.to_string())
            .or_default()
            .push(ProcessExpressionFunctionScope {
                scope_start: scope.0,
                max_scope_end: scope.1,
            });
    }

    fn add_pattern_bindings(&mut self, pattern: &BindingPattern<'_>, scope: (usize, usize)) {
        match pattern {
            BindingPattern::BindingIdentifier(identifier) => {
                self.add_binding(identifier.span);
                self.add_scope(identifier.name.as_str(), scope);
            }
            BindingPattern::ObjectPattern(object) => {
                for property in &object.properties {
                    self.add_pattern_bindings(&property.value, scope);
                }
                if let Some(rest) = &object.rest {
                    self.add_pattern_bindings(&rest.argument, scope);
                }
            }
            BindingPattern::ArrayPattern(array) => {
                for element in array.elements.iter().flatten() {
                    self.add_pattern_bindings(element, scope);
                }
                if let Some(rest) = &array.rest {
                    self.add_pattern_bindings(&rest.argument, scope);
                }
            }
            BindingPattern::AssignmentPattern(assignment) => {
                self.add_pattern_bindings(&assignment.left, scope);
            }
        }
    }

    fn enter_function(&mut self, function: &oxc_ast::ast::Function<'_>) {
        let Some(function_span) = self.relative_span(function.span) else {
            return;
        };
        let scope_start = self
            .relative_span(function.params.span)
            .map_or(function_span.0, |span| span.0);
        let scope_end = function
            .body
            .as_ref()
            .and_then(|body| self.relative_span(body.span))
            .map_or(function_span.1, |span| span.1);
        let scope = (scope_start, scope_end);
        self.add_scope("arguments", scope);

        if let Some(identifier) = &function.id {
            self.add_binding(identifier.span);
            self.add_scope(identifier.name.as_str(), scope);
            if function.r#type == oxc_ast::ast::FunctionType::FunctionDeclaration {
                let outer_scope = self
                    .declaration_scopes
                    .last()
                    .copied()
                    .unwrap_or((0, self.source_end - self.source_start));
                self.add_scope(identifier.name.as_str(), outer_scope);
            }
        }
        for param in &function.params.items {
            self.add_pattern_bindings(&param.pattern, scope);
        }
        if let Some(rest) = &function.params.rest {
            self.add_pattern_bindings(&rest.rest.argument, scope);
        }
    }

    fn enter_class(&mut self, class: &oxc_ast::ast::Class<'_>) {
        let Some(class_scope) = self.relative_span(class.span) else {
            return;
        };
        let Some(identifier) = &class.id else {
            return;
        };
        let own_scope_start = self
            .relative_identifier_span(identifier.span)
            .map_or(class_scope.0, |span| span.0);
        self.add_binding(identifier.span);
        self.add_scope(identifier.name.as_str(), (own_scope_start, class_scope.1));
        if class.r#type == oxc_ast::ast::ClassType::ClassDeclaration {
            let outer_scope = self
                .declaration_scopes
                .last()
                .copied()
                .unwrap_or((0, self.source_end - self.source_start));
            self.add_scope(identifier.name.as_str(), outer_scope);
        }
    }

    fn class_element_modifier_span(
        &self,
        span: oxc_span::Span,
        decorators: &[oxc_ast::ast::Decorator<'_>],
    ) -> oxc_span::Span {
        oxc_span::Span::new(
            decorators
                .last()
                .map_or(span.start, |decorator| decorator.span.end),
            span.end,
        )
    }

    fn finish(mut self) -> ProcessExpressionFunctionBindingIndex {
        for scopes in self.bindings.scopes.values_mut() {
            scopes.sort_unstable_by_key(|scope| (scope.scope_start, scope.max_scope_end));
            let mut max_scope_end = 0usize;
            for scope in scopes {
                max_scope_end = max_scope_end.max(scope.max_scope_end);
                scope.max_scope_end = max_scope_end;
            }
        }
        self.bindings
    }
}

impl<'ast> oxc_ast_visit::Visit<'ast> for ProcessExpressionFunctionBindingCollector<'_> {
    fn enter_node(&mut self, kind: oxc_ast::AstKind<'ast>) {
        match kind {
            oxc_ast::AstKind::Function(function) => self.enter_function(function),
            oxc_ast::AstKind::Class(class) => self.enter_class(class),
            oxc_ast::AstKind::FunctionBody(body) => {
                if let Some(scope) = self.relative_span(body.span) {
                    self.declaration_scopes.push(scope);
                }
            }
            oxc_ast::AstKind::BlockStatement(block) => {
                if let Some(scope) = self.relative_span(block.span) {
                    self.declaration_scopes.push(scope);
                }
            }
            oxc_ast::AstKind::SwitchStatement(switch) => {
                let discriminant = oxc_span::GetSpan::span(&switch.discriminant);
                if let Some(scope) =
                    self.relative_braced_scope_after(switch.span, discriminant.end)
                {
                    self.declaration_scopes.push(scope);
                }
            }
            oxc_ast::AstKind::StaticBlock(block) => {
                if let Some(scope) = self.relative_braced_scope_after(block.span, block.span.start) {
                    self.add_non_reference_modifier_before(
                        block.span,
                        (self.source_start + scope.0) as u32,
                        "static",
                    );
                    self.declaration_scopes.push(scope);
                }
            }
            oxc_ast::AstKind::PropertyDefinition(property) => {
                self.add_non_reference_key(&property.key, property.computed);
                if property.r#static {
                    let modifier_span =
                        self.class_element_modifier_span(property.span, &property.decorators);
                    self.add_non_reference_modifier(modifier_span, &property.key, "static");
                }
            }
            oxc_ast::AstKind::AccessorProperty(property) => {
                self.add_non_reference_key(&property.key, property.computed);
                let modifier_span =
                    self.class_element_modifier_span(property.span, &property.decorators);
                self.add_non_reference_modifier(modifier_span, &property.key, "accessor");
                if property.r#static {
                    self.add_non_reference_modifier(modifier_span, &property.key, "static");
                }
            }
            oxc_ast::AstKind::ObjectProperty(property)
                if property.method || property.kind != PropertyKind::Init =>
            {
                self.add_non_reference_key(&property.key, property.computed);
                match property.kind {
                    PropertyKind::Get => {
                        self.add_non_reference_modifier(property.span, &property.key, "get");
                    }
                    PropertyKind::Set => {
                        self.add_non_reference_modifier(property.span, &property.key, "set");
                    }
                    PropertyKind::Init => {}
                }
            }
            oxc_ast::AstKind::MethodDefinition(method) => {
                self.add_non_reference_key(&method.key, method.computed);
                let modifier_span =
                    self.class_element_modifier_span(method.span, &method.decorators);
                if method.r#static {
                    self.add_non_reference_modifier(modifier_span, &method.key, "static");
                }
                match method.kind {
                    oxc_ast::ast::MethodDefinitionKind::Get => {
                        self.add_non_reference_modifier(modifier_span, &method.key, "get");
                    }
                    oxc_ast::ast::MethodDefinitionKind::Set => {
                        self.add_non_reference_modifier(modifier_span, &method.key, "set");
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn leave_node(&mut self, kind: oxc_ast::AstKind<'ast>) {
        let entered_declaration_scope = match kind {
            oxc_ast::AstKind::FunctionBody(body) => self.relative_span(body.span).is_some(),
            oxc_ast::AstKind::BlockStatement(block) => self.relative_span(block.span).is_some(),
            oxc_ast::AstKind::SwitchStatement(switch) => {
                let discriminant = oxc_span::GetSpan::span(&switch.discriminant);
                self.relative_braced_scope_after(switch.span, discriminant.end)
                    .is_some()
            }
            oxc_ast::AstKind::StaticBlock(block) => self
                .relative_braced_scope_after(block.span, block.span.start)
                .is_some(),
            _ => false,
        };
        if entered_declaration_scope && self.declaration_scopes.len() > 1 {
            self.declaration_scopes.pop();
        }
    }
}

pub(crate) fn process_expression_function_bindings(
    raw: &str,
    source_type: oxc_span::SourceType,
) -> ProcessExpressionFunctionBindingIndex {
    if !raw.as_bytes().contains(&b'(') && !source_contains_identifier(raw, "class") {
        return ProcessExpressionFunctionBindingIndex::default();
    }

    if let Some(bindings) = process_expression_parse_function_bindings(raw, raw, source_type) {
        return bindings;
    }
    if raw.contains("|>") {
        let normalized = raw.replace("|>", ", ");
        if let Some(bindings) =
            process_expression_parse_function_bindings(&normalized, raw, source_type)
        {
            return bindings;
        }
    }
    ProcessExpressionFunctionBindingIndex::default()
}

fn process_expression_parse_function_bindings(
    parse_source: &str,
    original_source: &str,
    source_type: oxc_span::SourceType,
) -> Option<ProcessExpressionFunctionBindingIndex> {
    debug_assert_eq!(parse_source.len(), original_source.len());

    let store = JsAstStore::new();
    if let Ok(expression) = store.parse_expression(parse_source, source_type) {
        let span = oxc_span::GetSpan::span(&expression);
        let trimmed_start = parse_source.len() - parse_source.trim_start().len();
        let trimmed_end = parse_source.trim_end().len();
        if span.start as usize == trimmed_start && span.end as usize == trimmed_end {
            let mut collector = ProcessExpressionFunctionBindingCollector::new(
                original_source,
                0,
                original_source.len(),
            );
            oxc_ast_visit::Visit::visit_expression(&mut collector, &expression);
            return Some(collector.finish());
        }
    }

    const FUNCTION_BODY_PREFIX: &str = "async function __vuec__($event) {\n";
    let wrapped_parse_source = format!("{FUNCTION_BODY_PREFIX}{parse_source}\n}}\n");
    let wrapped_original_source = format!("{FUNCTION_BODY_PREFIX}{original_source}\n}}\n");
    let parsed = store.parse_program(&wrapped_parse_source, source_type);
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    let source_start = FUNCTION_BODY_PREFIX.len();
    let source_end = source_start + original_source.len();
    let mut collector = ProcessExpressionFunctionBindingCollector::new(
        &wrapped_original_source,
        source_start,
        source_end,
    );
    oxc_ast_visit::Visit::visit_program(&mut collector, &parsed.program);
    Some(collector.finish())
}

pub(crate) fn process_expression_is_function_binding(
    bindings: &ProcessExpressionFunctionBindingIndex,
    ident: &str,
    start: usize,
    end: usize,
) -> bool {
    if bindings.bindings.contains(&(start, end)) {
        return true;
    }
    let Some(scopes) = bindings.scopes.get(ident) else {
        return false;
    };
    let containing = scopes.partition_point(|scope| scope.scope_start <= start);
    containing > 0 && end <= scopes[containing - 1].max_scope_end
}

pub(crate) fn process_expression_is_function_non_reference_key(
    bindings: &ProcessExpressionFunctionBindingIndex,
    start: usize,
    end: usize,
) -> bool {
    bindings.non_reference_keys.contains(&(start, end))
}
