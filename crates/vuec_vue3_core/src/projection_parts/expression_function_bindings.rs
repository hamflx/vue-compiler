#[derive(Clone, Copy, Debug)]
struct ProcessExpressionFunctionScope {
    scope_start: usize,
    max_scope_end: usize,
}

#[derive(Clone, Copy, Debug)]
struct ProcessExpressionNonReferenceRange {
    range_start: usize,
    max_range_end: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ProcessExpressionFunctionBindingIndex {
    bindings: BTreeSet<(usize, usize)>,
    constant_blocked_spans: BTreeSet<(usize, usize)>,
    destructure_assignment_spans: BTreeSet<(usize, usize)>,
    // Ordinary spellings use their source slice; only decoded spellings allocate.
    decoded_identifier_names: BTreeMap<(usize, usize), String>,
    identifier_spans: BTreeSet<(usize, usize)>,
    object_shorthand_spans: BTreeSet<(usize, usize)>,
    static_member_spans: BTreeSet<(usize, usize)>,
    non_reference_keys: BTreeSet<(usize, usize)>,
    non_reference_ranges: Vec<ProcessExpressionNonReferenceRange>,
    scopes: BTreeMap<String, Vec<ProcessExpressionFunctionScope>>,
    parsed: bool,
    ast_required_unavailable: bool,
}

struct ProcessExpressionFunctionBindingCollector<'source> {
    parse_source: &'source str,
    source: &'source str,
    source_start: usize,
    source_end: usize,
    lexical_scopes: Vec<(usize, usize)>,
    var_scopes: Vec<(usize, usize)>,
    // Oxc materializes parentheses and ChainExpression wrappers that Babel does
    // not expose to walkIdentifiers. `None` keeps those wrappers transparent.
    babel_parent_frames: Vec<Option<bool>>,
    bindings: ProcessExpressionFunctionBindingIndex,
}

impl<'source> ProcessExpressionFunctionBindingCollector<'source> {
    fn new(
        parse_source: &'source str,
        source: &'source str,
        source_start: usize,
        source_end: usize,
    ) -> Self {
        Self {
            parse_source,
            source,
            source_start,
            source_end,
            lexical_scopes: vec![(0, source_end - source_start)],
            var_scopes: vec![(0, source_end - source_start)],
            babel_parent_frames: Vec::new(),
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

    fn add_identifier_span(&mut self, span: oxc_span::Span, expected_name: &str) {
        let Some(((start, end), identifier)) = self.matching_identifier_span(span) else {
            return;
        };
        self.bindings.identifier_spans.insert((start, end));
        self.add_decoded_identifier_name((start, end), identifier, expected_name);
    }

    fn add_constant_blocked_span(&mut self, span: oxc_span::Span) {
        if let Some(span) = self.relative_identifier_span(span) {
            self.bindings.constant_blocked_spans.insert(span);
        }
    }

    fn add_static_member_span(&mut self, span: oxc_span::Span) {
        if let Some(span) = self.relative_identifier_span(span) {
            self.bindings.static_member_spans.insert(span);
        }
    }

    fn current_babel_parent_blocks_constant(&self) -> bool {
        self.babel_parent_frames
            .iter()
            .rev()
            .find_map(|frame| *frame)
            .unwrap_or(false)
    }

    fn add_non_reference_identifier(&mut self, span: oxc_span::Span, expected_name: &str) {
        let Some((identifier_span, identifier)) = self.matching_identifier_span(span) else {
            return;
        };
        self.bindings.non_reference_keys.insert(identifier_span);
        self.add_decoded_identifier_name(identifier_span, identifier, expected_name);
    }

    fn matching_identifier_span(
        &self,
        span: oxc_span::Span,
    ) -> Option<((usize, usize), &'source str)> {
        let (start, end) = self.relative_identifier_span(span)?;
        let absolute_start = self.source_start.checked_add(start)?;
        let absolute_end = self.source_start.checked_add(end)?;
        let identifier = self.source.get(absolute_start..absolute_end)?;
        let parsed_identifier = self.parse_source.get(absolute_start..absolute_end)?;
        (identifier == parsed_identifier).then_some(((start, end), identifier))
    }

    fn add_decoded_identifier_name(
        &mut self,
        span: (usize, usize),
        identifier: &str,
        expected_name: &str,
    ) {
        if identifier != expected_name {
            self.bindings
                .decoded_identifier_names
                .insert(span, expected_name.to_string());
        }
    }

    fn add_object_shorthand_span(&mut self, span: oxc_span::Span) {
        if let Some(span) = self.relative_identifier_span(span) {
            self.bindings.object_shorthand_spans.insert(span);
        }
    }

    fn add_destructure_assignment_identifier(
        &mut self,
        identifier: &oxc_ast::ast::IdentifierReference<'_>,
    ) {
        if let Some(span) = self.relative_identifier_span(identifier.span) {
            self.bindings.destructure_assignment_spans.insert(span);
        }
    }

    fn add_destructure_array_assignment_target(
        &mut self,
        array: &oxc_ast::ast::ArrayAssignmentTarget<'_>,
    ) {
        for element in array.elements.iter().flatten() {
            self.add_destructure_assignment_maybe_default(element);
        }
        if let Some(rest) = &array.rest {
            self.add_destructure_assignment_target(&rest.target);
        }
    }

    fn add_destructure_object_assignment_target(
        &mut self,
        object: &oxc_ast::ast::ObjectAssignmentTarget<'_>,
    ) {
        for property in &object.properties {
            match property {
                oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(
                    property,
                ) => self.add_destructure_assignment_identifier(&property.binding),
                oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyProperty(
                    property,
                ) => self.add_destructure_assignment_maybe_default(&property.binding),
            }
        }
        if let Some(rest) = &object.rest {
            self.add_destructure_assignment_target(&rest.target);
        }
    }

    fn add_destructure_assignment_target(&mut self, target: &oxc_ast::ast::AssignmentTarget<'_>) {
        match target {
            oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                self.add_destructure_assignment_identifier(identifier);
            }
            oxc_ast::ast::AssignmentTarget::ArrayAssignmentTarget(array) => {
                self.add_destructure_array_assignment_target(array);
            }
            oxc_ast::ast::AssignmentTarget::ObjectAssignmentTarget(object) => {
                self.add_destructure_object_assignment_target(object);
            }
            _ => {}
        }
    }

    fn add_destructure_assignment_maybe_default(
        &mut self,
        target: &oxc_ast::ast::AssignmentTargetMaybeDefault<'_>,
    ) {
        match target {
            oxc_ast::ast::AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(target) => {
                self.add_destructure_assignment_target(&target.binding);
            }
            oxc_ast::ast::AssignmentTargetMaybeDefault::AssignmentTargetIdentifier(identifier) => {
                self.add_destructure_assignment_identifier(identifier);
            }
            oxc_ast::ast::AssignmentTargetMaybeDefault::ArrayAssignmentTarget(array) => {
                self.add_destructure_array_assignment_target(array);
            }
            oxc_ast::ast::AssignmentTargetMaybeDefault::ObjectAssignmentTarget(object) => {
                self.add_destructure_object_assignment_target(object);
            }
            _ => {}
        }
    }

    fn add_non_reference_range(&mut self, span: oxc_span::Span) {
        if let Some((range_start, max_range_end)) = self.relative_span(span) {
            self.bindings
                .non_reference_ranges
                .push(ProcessExpressionNonReferenceRange {
                    range_start,
                    max_range_end,
                });
        }
    }

    fn add_non_reference_between(&mut self, after: u32, before: u32) {
        if after <= before {
            self.add_non_reference_range(oxc_span::Span::new(after, before));
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
                    .lexical_scopes
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

    fn enter_arrow_function(
        &mut self,
        function: &oxc_ast::ast::ArrowFunctionExpression<'_>,
    ) {
        let Some(function_span) = self.relative_span(function.span) else {
            return;
        };
        let scope_start = self
            .relative_span(function.params.span)
            .map_or(function_span.0, |span| span.0);
        let scope_end = self
            .relative_span(function.body.span)
            .map_or(function_span.1, |span| span.1);
        let scope = (scope_start, scope_end);
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
                .lexical_scopes
                .last()
                .copied()
                .unwrap_or((0, self.source_end - self.source_start));
            self.add_scope(identifier.name.as_str(), outer_scope);
        }
    }

    fn enter_variable_declaration(
        &mut self,
        declaration: &oxc_ast::ast::VariableDeclaration<'_>,
    ) {
        if matches!(
            declaration.kind,
            oxc_ast::ast::VariableDeclarationKind::Using
                | oxc_ast::ast::VariableDeclarationKind::AwaitUsing
        ) {
            let before = declaration
                .declarations
                .first()
                .map_or(declaration.span.end, |declarator| declarator.span.start);
            self.add_non_reference_modifier_before(declaration.span, before, "using");
        }
        let scope = if declaration.kind == oxc_ast::ast::VariableDeclarationKind::Var {
            self.var_scopes.last().copied()
        } else {
            self.lexical_scopes.last().copied()
        };
        let Some(scope) = scope else {
            return;
        };
        for declarator in &declaration.declarations {
            self.add_pattern_bindings(&declarator.id, scope);
        }
    }

    fn enter_lexical_scope(&mut self, scope: Option<(usize, usize)>) -> bool {
        let Some(scope) = scope else {
            return false;
        };
        self.lexical_scopes.push(scope);
        true
    }

    fn leave_lexical_scope(&mut self, entered: bool) {
        if entered && self.lexical_scopes.len() > 1 {
            self.lexical_scopes.pop();
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
            .non_reference_ranges
            .sort_unstable_by_key(|range| (range.range_start, range.max_range_end));
        let mut max_range_end = 0usize;
        for range in &mut self.bindings.non_reference_ranges {
            max_range_end = max_range_end.max(range.max_range_end);
            range.max_range_end = max_range_end;
        }
        self.bindings.parsed = true;
        self.bindings
    }
}

fn process_expression_base_contains_optional_chain(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::CallExpression(call) => {
            call.optional || process_expression_base_contains_optional_chain(&call.callee)
        }
        Expression::ComputedMemberExpression(member) => {
            member.optional || process_expression_base_contains_optional_chain(&member.object)
        }
        Expression::StaticMemberExpression(member) => {
            member.optional || process_expression_base_contains_optional_chain(&member.object)
        }
        Expression::PrivateFieldExpression(member) => {
            member.optional || process_expression_base_contains_optional_chain(&member.object)
        }
        Expression::TSNonNullExpression(expression) => {
            process_expression_base_contains_optional_chain(&expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            process_expression_base_contains_optional_chain(&expression.expression)
        }
        // A nested ChainExpression is introduced when parentheses terminate an
        // optional chain, e.g. `(foo?.bar).baz`; Babel treats the outer member
        // as an ordinary MemberExpression.
        _ => false,
    }
}

fn process_expression_babel_parent_frame(kind: oxc_ast::AstKind<'_>) -> Option<bool> {
    match kind {
        // Babel's parser keeps parentheses in `extra` and represents optional
        // chains directly as Optional*Expression nodes.
        oxc_ast::AstKind::ParenthesizedExpression(_)
        | oxc_ast::AstKind::ChainExpression(_) => None,
        oxc_ast::AstKind::CallExpression(call) => Some(
            !(call.optional || process_expression_base_contains_optional_chain(&call.callee)),
        ),
        oxc_ast::AstKind::NewExpression(_) => Some(true),
        oxc_ast::AstKind::ComputedMemberExpression(member) => Some(
            !(member.optional
                || process_expression_base_contains_optional_chain(&member.object)),
        ),
        oxc_ast::AstKind::StaticMemberExpression(member) => Some(
            !(member.optional
                || process_expression_base_contains_optional_chain(&member.object)),
        ),
        oxc_ast::AstKind::PrivateFieldExpression(member) => Some(
            !(member.optional
                || process_expression_base_contains_optional_chain(&member.object)),
        ),
        _ => Some(false),
    }
}

impl<'ast> oxc_ast_visit::Visit<'ast> for ProcessExpressionFunctionBindingCollector<'_> {
    fn enter_node(&mut self, kind: oxc_ast::AstKind<'ast>) {
        self.babel_parent_frames
            .push(process_expression_babel_parent_frame(kind));
        match kind {
            oxc_ast::AstKind::Function(function) => self.enter_function(function),
            oxc_ast::AstKind::ArrowFunctionExpression(function) => {
                self.enter_arrow_function(function);
            }
            oxc_ast::AstKind::Class(class) => self.enter_class(class),
            oxc_ast::AstKind::FunctionBody(body) => {
                if let Some(scope) = self.relative_span(body.span) {
                    self.lexical_scopes.push(scope);
                    self.var_scopes.push(scope);
                }
            }
            oxc_ast::AstKind::BlockStatement(block) => {
                self.enter_lexical_scope(self.relative_span(block.span));
            }
            oxc_ast::AstKind::ForStatement(statement) => {
                self.enter_lexical_scope(self.relative_span(statement.span));
            }
            oxc_ast::AstKind::ForInStatement(statement) => {
                self.enter_lexical_scope(self.relative_span(statement.span));
            }
            oxc_ast::AstKind::ForOfStatement(statement) => {
                self.enter_lexical_scope(self.relative_span(statement.span));
            }
            oxc_ast::AstKind::CatchClause(catch) => {
                if let Some(scope) = self.relative_span(catch.span) {
                    self.lexical_scopes.push(scope);
                    if let Some(param) = &catch.param {
                        self.add_pattern_bindings(&param.pattern, scope);
                    }
                }
            }
            oxc_ast::AstKind::SwitchStatement(switch) => {
                let discriminant = oxc_span::GetSpan::span(&switch.discriminant);
                if let Some(scope) =
                    self.relative_braced_scope_after(switch.span, discriminant.end)
                {
                    self.lexical_scopes.push(scope);
                }
            }
            oxc_ast::AstKind::StaticBlock(block) => {
                if let Some(scope) = self.relative_braced_scope_after(block.span, block.span.start) {
                    self.add_non_reference_modifier_before(
                        block.span,
                        (self.source_start + scope.0) as u32,
                        "static",
                    );
                    self.lexical_scopes.push(scope);
                    self.var_scopes.push(scope);
                }
            }
            oxc_ast::AstKind::VariableDeclaration(declaration) => {
                self.enter_variable_declaration(declaration);
            }
            oxc_ast::AstKind::AssignmentExpression(assignment)
                if matches!(
                    assignment.left,
                    oxc_ast::ast::AssignmentTarget::ArrayAssignmentTarget(_)
                        | oxc_ast::ast::AssignmentTarget::ObjectAssignmentTarget(_)
                ) =>
            {
                self.add_destructure_assignment_target(&assignment.left);
            }
            oxc_ast::AstKind::TSAsExpression(expression) => {
                self.add_non_reference_between(
                    oxc_span::GetSpan::span(&expression.expression).end,
                    oxc_span::GetSpan::span(&expression.type_annotation).start,
                );
            }
            oxc_ast::AstKind::TSSatisfiesExpression(expression) => {
                self.add_non_reference_between(
                    oxc_span::GetSpan::span(&expression.expression).end,
                    oxc_span::GetSpan::span(&expression.type_annotation).start,
                );
            }
            oxc_ast::AstKind::TSTypeAliasDeclaration(declaration) => {
                self.add_non_reference_range(declaration.span);
            }
            oxc_ast::AstKind::TSInterfaceDeclaration(declaration) => {
                self.add_non_reference_range(declaration.span);
            }
            oxc_ast::AstKind::PrivateIdentifier(identifier) => {
                if let Some(span) = self.relative_identifier_span(identifier.span) {
                    self.bindings.non_reference_keys.insert(span);
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
        self.babel_parent_frames.pop();
        if let oxc_ast::AstKind::FunctionBody(body) = kind {
            if self.relative_span(body.span).is_some() {
                if self.var_scopes.len() > 1 {
                    self.var_scopes.pop();
                }
                self.leave_lexical_scope(true);
            }
            return;
        }
        let entered_lexical_scope = match kind {
            oxc_ast::AstKind::BlockStatement(block) => self.relative_span(block.span).is_some(),
            oxc_ast::AstKind::ForStatement(statement) => {
                self.relative_span(statement.span).is_some()
            }
            oxc_ast::AstKind::ForInStatement(statement) => {
                self.relative_span(statement.span).is_some()
            }
            oxc_ast::AstKind::ForOfStatement(statement) => {
                self.relative_span(statement.span).is_some()
            }
            oxc_ast::AstKind::CatchClause(catch) => self.relative_span(catch.span).is_some(),
            oxc_ast::AstKind::SwitchStatement(switch) => {
                let discriminant = oxc_span::GetSpan::span(&switch.discriminant);
                self.relative_braced_scope_after(switch.span, discriminant.end)
                    .is_some()
            }
            oxc_ast::AstKind::StaticBlock(block) => {
                let entered = self
                    .relative_braced_scope_after(block.span, block.span.start)
                    .is_some();
                if entered && self.var_scopes.len() > 1 {
                    self.var_scopes.pop();
                }
                entered
            }
            _ => false,
        };
        self.leave_lexical_scope(entered_lexical_scope);
    }

    fn visit_ts_type(&mut self, ty: &oxc_ast::ast::TSType<'ast>) {
        self.add_non_reference_range(oxc_span::GetSpan::span(ty));
    }

    fn visit_ts_type_annotation(&mut self, ty: &oxc_ast::ast::TSTypeAnnotation<'ast>) {
        self.add_non_reference_range(ty.span);
    }

    fn visit_ts_type_parameter_declaration(
        &mut self,
        parameters: &oxc_ast::ast::TSTypeParameterDeclaration<'ast>,
    ) {
        self.add_non_reference_range(parameters.span);
    }

    fn visit_ts_type_parameter_instantiation(
        &mut self,
        parameters: &oxc_ast::ast::TSTypeParameterInstantiation<'ast>,
    ) {
        self.add_non_reference_range(parameters.span);
    }

    fn visit_binding_identifier(&mut self, identifier: &oxc_ast::ast::BindingIdentifier<'ast>) {
        self.add_identifier_span(identifier.span, identifier.name.as_str());
        oxc_ast_visit::walk::walk_binding_identifier(self, identifier);
    }

    fn visit_identifier_reference(
        &mut self,
        identifier: &oxc_ast::ast::IdentifierReference<'ast>,
    ) {
        if self.current_babel_parent_blocks_constant() {
            self.add_constant_blocked_span(identifier.span);
        }
        self.add_identifier_span(identifier.span, identifier.name.as_str());
        oxc_ast_visit::walk::walk_identifier_reference(self, identifier);
    }

    fn visit_static_member_expression(
        &mut self,
        expression: &oxc_ast::ast::StaticMemberExpression<'ast>,
    ) {
        self.add_static_member_span(expression.property.span);
        if !(expression.optional
            || process_expression_base_contains_optional_chain(&expression.object))
        {
            self.add_constant_blocked_span(expression.property.span);
        }
        self.add_identifier_span(expression.property.span, expression.property.name.as_str());
        oxc_ast_visit::walk::walk_static_member_expression(self, expression);
    }

    fn visit_identifier_name(&mut self, identifier: &oxc_ast::ast::IdentifierName<'ast>) {
        self.add_non_reference_identifier(identifier.span, identifier.name.as_str());
        oxc_ast_visit::walk::walk_identifier_name(self, identifier);
    }

    fn visit_label_identifier(&mut self, identifier: &oxc_ast::ast::LabelIdentifier<'ast>) {
        self.add_non_reference_identifier(identifier.span, identifier.name.as_str());
        oxc_ast_visit::walk::walk_label_identifier(self, identifier);
    }

    fn visit_private_identifier(&mut self, identifier: &oxc_ast::ast::PrivateIdentifier<'ast>) {
        self.add_non_reference_identifier(identifier.span, identifier.name.as_str());
        oxc_ast_visit::walk::walk_private_identifier(self, identifier);
    }

    fn visit_jsx_element_name(&mut self, name: &oxc_ast::ast::JSXElementName<'ast>) {
        self.add_non_reference_range(oxc_span::GetSpan::span(name));
        oxc_ast_visit::walk::walk_jsx_element_name(self, name);
    }

    fn visit_jsx_attribute_name(&mut self, name: &oxc_ast::ast::JSXAttributeName<'ast>) {
        self.add_non_reference_range(oxc_span::GetSpan::span(name));
        oxc_ast_visit::walk::walk_jsx_attribute_name(self, name);
    }

    fn visit_jsx_text(&mut self, text: &oxc_ast::ast::JSXText<'ast>) {
        self.add_non_reference_range(text.span);
        oxc_ast_visit::walk::walk_jsx_text(self, text);
    }

    fn visit_object_property(&mut self, property: &oxc_ast::ast::ObjectProperty<'ast>) {
        if property.shorthand {
            self.add_object_shorthand_span(oxc_span::GetSpan::span(&property.value));
        }
        oxc_ast_visit::walk::walk_object_property(self, property);
    }

    fn visit_assignment_target_property_identifier(
        &mut self,
        property: &oxc_ast::ast::AssignmentTargetPropertyIdentifier<'ast>,
    ) {
        self.add_object_shorthand_span(property.binding.span);
        oxc_ast_visit::walk::walk_assignment_target_property_identifier(self, property);
    }

    fn visit_binding_property(&mut self, property: &oxc_ast::ast::BindingProperty<'ast>) {
        if property.shorthand {
            self.add_object_shorthand_span(oxc_span::GetSpan::span(&property.key));
        }
        oxc_ast_visit::walk::walk_binding_property(self, property);
    }
}

pub(crate) const PROCESS_EXPRESSION_MAX_PIPELINE_TOPIC_RECOVERIES: usize = 64;
pub(crate) const PROCESS_EXPRESSION_MAX_SAFE_AST_BYTES: usize = 4 * 1024;
const PROCESS_EXPRESSION_MAX_SAFE_AST_WORK_UNITS: usize = 512;
pub(crate) const PROCESS_EXPRESSION_AST_LIMIT_MESSAGE: &str =
    "Error parsing JavaScript expression: expression exceeds the safe AST analysis limit.";
// Oxc's visitor is recursive for left-deep expressions. Inputs that require
// exact AST roles fail closed above this bound; other inputs retain the lexer.

pub(crate) fn process_expression_ast_visit_allowed(raw: &str) -> bool {
    if raw.len() > PROCESS_EXPRESSION_MAX_SAFE_AST_BYTES {
        return false;
    }
    let mut work_units = 0usize;
    let mut in_word = false;
    for byte in raw.bytes() {
        let word = byte.is_ascii_alphanumeric()
            || matches!(byte, b'_' | b'$' | b'\\')
            || !byte.is_ascii();
        if word && !in_word {
            work_units = work_units.saturating_add(1);
        }
        in_word = word;
        if matches!(
            byte,
            b'(' | b')'
                | b'['
                | b']'
                | b'{'
                | b'}'
                | b'.'
                | b'?'
                | b'+'
                | b'-'
                | b'*'
                | b'/'
                | b'%'
                | b'<'
                | b'>'
                | b'='
                | b'&'
                | b'|'
                | b'^'
                | b'!'
                | b'~'
        ) {
            work_units = work_units.saturating_add(1);
        }
        if work_units > PROCESS_EXPRESSION_MAX_SAFE_AST_WORK_UNITS {
            return false;
        }
    }
    true
}

pub(crate) fn process_expression_requires_jsx_ast(
    raw: &str,
    source_type: oxc_span::SourceType,
) -> bool {
    source_type.is_jsx() && raw.as_bytes().contains(&b'<')
}

pub(crate) fn process_expression_requires_lossless_ast(
    raw: &str,
    source_type: oxc_span::SourceType,
) -> bool {
    process_expression_requires_jsx_ast(raw, source_type)
        || process_expression_may_have_ast_sensitive_syntax(raw, source_type)
}

fn process_expression_may_have_ast_sensitive_syntax(
    raw: &str,
    source_type: oxc_span::SourceType,
) -> bool {
    // Template and regexp contents stay conservative because distinguishing
    // their embedded expression boundaries requires the AST we are gating.
    let bytes = raw.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' | b'"' => {
                let quote = bytes[index];
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == b'\\' {
                        index = index.saturating_add(2);
                    } else {
                        let current = bytes[index];
                        index += 1;
                        if current == quote {
                            break;
                        }
                    }
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index += 2;
                while index < bytes.len() && !matches!(bytes[index], b'\n' | b'\r') {
                    index += 1;
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index += 2;
                while index + 1 < bytes.len() {
                    if bytes[index] == b'*' && bytes[index + 1] == b'/' {
                        index += 2;
                        break;
                    }
                    index += 1;
                }
            }
            b'/' => return true,
            b'\\' if bytes.get(index + 1) == Some(&b'u') => return true,
            b':' | b'<' if source_type.is_typescript() => return true,
            _ if source_type.is_typescript() => {
                let Some(current) = raw[index..].chars().next() else {
                    break;
                };
                if !is_identifier_start(current) {
                    index += current.len_utf8();
                    continue;
                }
                let start = index;
                index += current.len_utf8();
                while index < raw.len() {
                    let Some(current) = raw[index..].chars().next() else {
                        break;
                    };
                    if !is_identifier_continue(current) {
                        break;
                    }
                    index += current.len_utf8();
                }
                if matches!(&raw[start..index], "as" | "interface" | "satisfies" | "type") {
                    return true;
                }
            }
            _ => {
                let Some(current) = raw[index..].chars().next() else {
                    break;
                };
                index += current.len_utf8();
            }
        }
    }
    false
}

pub(crate) fn process_expression_ast_required_unavailable(
    raw: &str,
    source_type: oxc_span::SourceType,
) -> bool {
    process_expression_requires_lossless_ast(raw, source_type)
        && !process_expression_ast_visit_allowed(raw)
}

enum ProcessExpressionFunctionBindingParse {
    Parsed(Box<ProcessExpressionFunctionBindingIndex>),
    Failed { error_offset: Option<usize> },
}

pub(crate) fn process_expression_function_bindings(
    raw: &str,
    source_type: oxc_span::SourceType,
) -> ProcessExpressionFunctionBindingIndex {
    process_expression_function_bindings_with_mode(raw, source_type, false)
}

pub(crate) fn process_expression_identifier_bindings(
    raw: &str,
    source_type: oxc_span::SourceType,
) -> ProcessExpressionFunctionBindingIndex {
    let needs_lexical_disambiguation = process_expression_requires_lossless_ast(raw, source_type)
        || raw.as_bytes().contains(&b'`')
        || raw.as_bytes().contains(&b'/')
        || !raw.is_ascii()
        || process_expression_may_have_destructure_assignment(raw);
    let needs_babel_parent = raw
        .as_bytes()
        .iter()
        .any(|byte| matches!(byte, b'(' | b'.' | b'[' | b'?'))
        || source_contains_identifier(raw, "new");
    if (needs_lexical_disambiguation || needs_babel_parent)
        && process_expression_ast_visit_allowed(raw)
    {
        process_expression_function_bindings_with_mode(raw, source_type, true)
    } else {
        process_expression_function_bindings(raw, source_type)
    }
}

fn process_expression_function_bindings_with_mode(
    raw: &str,
    source_type: oxc_span::SourceType,
    collect_identifiers: bool,
) -> ProcessExpressionFunctionBindingIndex {
    let ast_required = process_expression_requires_lossless_ast(raw, source_type);
    if process_expression_ast_required_unavailable(raw, source_type) {
        return ProcessExpressionFunctionBindingIndex {
            ast_required_unavailable: true,
            ..ProcessExpressionFunctionBindingIndex::default()
        };
    }
    if !process_expression_ast_visit_allowed(raw) {
        return ProcessExpressionFunctionBindingIndex::default();
    }
    let needs_typescript_parse = source_type.is_typescript()
        && (raw.as_bytes().iter().any(|byte| matches!(byte, b':' | b'<'))
            || ["as", "interface", "satisfies", "type"]
                .iter()
                .any(|keyword| source_contains_identifier(raw, keyword)));
    let needs_binding_parse = collect_identifiers
        || raw.as_bytes().contains(&b'(')
        || ["class", "const", "let", "using", "var"]
            .iter()
            .any(|keyword| source_contains_identifier(raw, keyword))
        || needs_typescript_parse;
    if !needs_binding_parse {
        return ProcessExpressionFunctionBindingIndex::default();
    }

    if let ProcessExpressionFunctionBindingParse::Parsed(bindings) =
        process_expression_parse_function_bindings(raw, raw, source_type)
    {
        return *bindings;
    }
    if raw.contains("|>") {
        let normalized = raw.replace("|>", ", ");
        if let Some(bindings) = process_expression_recover_pipeline_function_bindings(
            normalized,
            raw,
            source_type,
            PROCESS_EXPRESSION_MAX_PIPELINE_TOPIC_RECOVERIES,
        ) {
            return bindings;
        }
    }
    if ast_required {
        ProcessExpressionFunctionBindingIndex {
            ast_required_unavailable: true,
            ..ProcessExpressionFunctionBindingIndex::default()
        }
    } else {
        ProcessExpressionFunctionBindingIndex::default()
    }
}

fn process_expression_parse_function_bindings(
    parse_source: &str,
    original_source: &str,
    source_type: oxc_span::SourceType,
) -> ProcessExpressionFunctionBindingParse {
    debug_assert_eq!(parse_source.len(), original_source.len());

    let store = JsAstStore::new();
    if let Ok(expression) = store.parse_expression(parse_source, source_type) {
        let span = oxc_span::GetSpan::span(&expression);
        let trimmed_start = parse_source.len() - parse_source.trim_start().len();
        let trimmed_end = parse_source.trim_end().len();
        if span.start as usize == trimmed_start && span.end as usize == trimmed_end {
            let mut collector = ProcessExpressionFunctionBindingCollector::new(
                parse_source,
                original_source,
                0,
                original_source.len(),
            );
            oxc_ast_visit::Visit::visit_expression(&mut collector, &expression);
            return ProcessExpressionFunctionBindingParse::Parsed(Box::new(collector.finish()));
        }
    }

    const FUNCTION_BODY_PREFIX: &str = "async function __vuec__($event) {\n";
    let wrapped_parse_source = format!("{FUNCTION_BODY_PREFIX}{parse_source}\n}}\n");
    let wrapped_original_source = format!("{FUNCTION_BODY_PREFIX}{original_source}\n}}\n");
    let parsed = store.parse_program(&wrapped_parse_source, source_type);
    if parsed.panicked || !parsed.errors.is_empty() {
        let error_offset = js_diagnostics_primary_offset(&parsed.errors)
            .and_then(|offset| offset.checked_sub(FUNCTION_BODY_PREFIX.len()))
            .filter(|offset| *offset < parse_source.len());
        return ProcessExpressionFunctionBindingParse::Failed { error_offset };
    }
    let source_start = FUNCTION_BODY_PREFIX.len();
    let source_end = source_start + original_source.len();
    let mut collector = ProcessExpressionFunctionBindingCollector::new(
        &wrapped_parse_source,
        &wrapped_original_source,
        source_start,
        source_end,
    );
    oxc_ast_visit::Visit::visit_program(&mut collector, &parsed.program);
    ProcessExpressionFunctionBindingParse::Parsed(Box::new(collector.finish()))
}

fn process_expression_recover_pipeline_function_bindings(
    mut parse_source: String,
    original_source: &str,
    source_type: oxc_span::SourceType,
    max_topic_recoveries: usize,
) -> Option<ProcessExpressionFunctionBindingIndex> {
    debug_assert_eq!(parse_source.len(), original_source.len());

    for recovered in 0..=max_topic_recoveries {
        let error_offset = match process_expression_parse_function_bindings(
            &parse_source,
            original_source,
            source_type,
        ) {
            ProcessExpressionFunctionBindingParse::Parsed(bindings) => return Some(*bindings),
            ProcessExpressionFunctionBindingParse::Failed { error_offset } => error_offset?,
        };
        if recovered == max_topic_recoveries {
            return None;
        }
        let (start, end, replacement) =
            process_expression_pipeline_topic_recovery(&parse_source, error_offset)?;
        parse_source.replace_range(start..end, replacement);
    }
    None
}

fn process_expression_pipeline_topic_recovery(
    source: &str,
    error_offset: usize,
) -> Option<(usize, usize, &'static str)> {
    const TOPICS: [(&str, &str); 5] = [
        ("@@", "$_"),
        ("^^", "$_"),
        ("%", "$"),
        ("#", "$"),
        ("^", "$"),
    ];

    let previous_token_end =
        process_expression_pipeline_topic_chain_start(source, error_offset)?;
    for (topic, replacement) in TOPICS {
        let starts = [
            Some(error_offset),
            (topic.len() > 1)
                .then(|| error_offset.checked_sub(topic.len() - 1))
                .flatten(),
            previous_token_end.checked_sub(topic.len()),
        ];
        for start in starts.into_iter().flatten() {
            let end = start.checked_add(topic.len())?;
            let error_matches = error_offset >= start
                && (error_offset < end || end == previous_token_end);
            if !error_matches
                || source.get(start..end) != Some(topic)
                || source
                    .get(end..)
                    .and_then(|tail| tail.chars().next())
                    .is_some_and(is_identifier_continue)
            {
                continue;
            }
            return Some((start, end, replacement));
        }
    }
    None
}

fn process_expression_pipeline_topic_chain_start(
    source: &str,
    error_offset: usize,
) -> Option<usize> {
    let mut end = source.get(..error_offset)?.trim_end().len();
    while let Some((last, ch)) = previous_char(source, end) {
        match ch {
            ']' => end = find_matching_backward(source, last, '[', ']')?,
            ')' => end = find_matching_backward(source, last, '(', ')')?,
            ch if is_identifier_continue(ch) => {
                let mut identifier_start = last;
                while let Some((previous, ch)) = previous_char(source, identifier_start) {
                    if !is_identifier_continue(ch) {
                        break;
                    }
                    identifier_start = previous;
                }
                let before_identifier = source.get(..identifier_start)?.trim_end();
                if let Some(before_member) = before_identifier.strip_suffix("?.") {
                    end = before_member.len();
                } else if let Some(before_member) = before_identifier.strip_suffix('.') {
                    end = before_member.len();
                } else {
                    break;
                }
            }
            _ => break,
        }
        end = source.get(..end)?.trim_end().len();
    }
    Some(end)
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
    if bindings.object_shorthand_spans.contains(&(start, end)) {
        return false;
    }
    if bindings.non_reference_keys.contains(&(start, end)) {
        return true;
    }
    let containing = bindings
        .non_reference_ranges
        .partition_point(|range| range.range_start <= start);
    containing > 0 && end <= bindings.non_reference_ranges[containing - 1].max_range_end
}

pub(crate) fn process_expression_function_bindings_parsed(
    bindings: &ProcessExpressionFunctionBindingIndex,
) -> bool {
    bindings.parsed
}

pub(crate) fn process_expression_function_bindings_ast_required_unavailable(
    bindings: &ProcessExpressionFunctionBindingIndex,
) -> bool {
    bindings.ast_required_unavailable
}

pub(crate) fn process_expression_function_decoded_identifier_name(
    bindings: &ProcessExpressionFunctionBindingIndex,
    start: usize,
    end: usize,
) -> Option<&str> {
    bindings
        .decoded_identifier_names
        .get(&(start, end))
        .map(String::as_str)
}

pub(crate) fn process_expression_function_decoded_identifier_at(
    bindings: &ProcessExpressionFunctionBindingIndex,
    start: usize,
) -> Option<(usize, &str)> {
    let (&(candidate_start, end), name) = bindings
        .decoded_identifier_names
        .range((
            std::ops::Bound::Included((start, 0)),
            std::ops::Bound::Included((start, usize::MAX)),
        ))
        .next()?;
    (candidate_start == start).then_some((end, name.as_str()))
}

pub(crate) fn process_expression_function_identifier_spans(
    bindings: &ProcessExpressionFunctionBindingIndex,
) -> &BTreeSet<(usize, usize)> {
    &bindings.identifier_spans
}

pub(crate) fn process_expression_function_constant_blocked_spans(
    bindings: &ProcessExpressionFunctionBindingIndex,
) -> &BTreeSet<(usize, usize)> {
    &bindings.constant_blocked_spans
}

pub(crate) fn process_expression_function_binding_spans(
    bindings: &ProcessExpressionFunctionBindingIndex,
) -> &BTreeSet<(usize, usize)> {
    &bindings.bindings
}

pub(crate) fn process_expression_function_object_shorthand_spans(
    bindings: &ProcessExpressionFunctionBindingIndex,
) -> &BTreeSet<(usize, usize)> {
    &bindings.object_shorthand_spans
}

pub(crate) fn process_expression_function_static_member_spans(
    bindings: &ProcessExpressionFunctionBindingIndex,
) -> &BTreeSet<(usize, usize)> {
    &bindings.static_member_spans
}

pub(crate) fn process_expression_function_destructure_assignment_spans(
    bindings: &ProcessExpressionFunctionBindingIndex,
) -> &BTreeSet<(usize, usize)> {
    &bindings.destructure_assignment_spans
}
