struct Vue3InlineModuleSource<'a> {
    filename: &'a str,
    source: &'a str,
    source_type: oxc_span::SourceType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Vue3TypeResolutionRequest {
    mode: Vue3TypeResolutionMode,
    explicit_mode: bool,
}

impl Vue3TypeResolutionRequest {
    fn inferred(mode: Vue3TypeResolutionMode) -> Self {
        Self {
            mode,
            explicit_mode: false,
        }
    }

    fn explicit(mode: Vue3TypeResolutionMode) -> Self {
        Self {
            mode,
            explicit_mode: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Vue3ModuleDependency {
    Module {
        source: String,
        request: Vue3TypeResolutionRequest,
    },
    ReferencePath(String),
    ReferenceTypes {
        source: String,
        request: Vue3TypeResolutionRequest,
    },
}

impl Vue3ModuleDependency {
    fn module(source: &str, resolution_mode: Vue3TypeResolutionMode) -> Self {
        Self::Module {
            source: source.to_string(),
            request: Vue3TypeResolutionRequest::inferred(resolution_mode),
        }
    }

    fn module_with_request(source: &str, request: Vue3TypeResolutionRequest) -> Self {
        Self::Module {
            source: source.to_string(),
            request,
        }
    }

    fn source(&self) -> &str {
        match self {
            Self::Module { source, .. } | Self::ReferencePath(source) => source,
            Self::ReferenceTypes { source, .. } => source,
        }
    }

    fn is_global_program_reference(&self) -> bool {
        !matches!(self, Self::Module { .. })
    }
}

struct Vue3ModuleDependencyCollector<'budget> {
    dependencies: BTreeSet<Vue3ModuleDependency>,
    collect_commonjs_requires: bool,
    static_resolution_mode: Vue3TypeResolutionMode,
    dynamic_resolution_mode: Vue3TypeResolutionMode,
    namespace_budget: &'budget mut Vue3NamespaceProjectionBudget,
}

impl Vue3ModuleDependencyCollector<'_> {
    fn insert(&mut self, dependency: Vue3ModuleDependency) {
        if self.dependencies.contains(&dependency) {
            return;
        }
        if self.namespace_budget.reserve(
            dependency
                .source()
                .len()
                .saturating_add(std::mem::size_of::<Vue3ModuleDependency>())
                .saturating_add(1),
        ) {
            self.dependencies.insert(dependency);
        }
    }

    fn insert_module(&mut self, source: &str, resolution_mode: Vue3TypeResolutionMode) {
        self.insert(Vue3ModuleDependency::module(source, resolution_mode));
    }

    fn insert_module_request(&mut self, source: &str, request: Vue3TypeResolutionRequest) {
        self.insert(Vue3ModuleDependency::module_with_request(source, request));
    }
}

fn vue3_resolution_mode_from_value(value: &str) -> Option<Vue3TypeResolutionMode> {
    match value {
        "import" => Some(Vue3TypeResolutionMode::Import),
        "require" => Some(Vue3TypeResolutionMode::Require),
        _ => None,
    }
}

fn vue3_static_resolution_mode(source_type: oxc_span::SourceType) -> Vue3TypeResolutionMode {
    if source_type.is_commonjs() {
        Vue3TypeResolutionMode::Require
    } else {
        Vue3TypeResolutionMode::Import
    }
}

fn vue3_resolution_mode_from_with_clause(
    with_clause: Option<&WithClause<'_>>,
) -> Option<Vue3TypeResolutionMode> {
    let entries = &with_clause?.with_entries;
    let [attribute] = entries.as_slice() else {
        return None;
    };
    let ImportAttributeKey::StringLiteral(key) = &attribute.key else {
        return None;
    };
    if key.value != "resolution-mode" {
        return None;
    }
    vue3_resolution_mode_from_value(attribute.value.value.as_str())
}

fn vue3_declaration_resolution_request(
    kind: ImportOrExportKind,
    with_clause: Option<&WithClause<'_>>,
    default: Vue3TypeResolutionMode,
) -> Vue3TypeResolutionRequest {
    if kind == ImportOrExportKind::Type {
        if let Some(mode) = vue3_resolution_mode_from_with_clause(with_clause) {
            return Vue3TypeResolutionRequest::explicit(mode);
        }
    }
    Vue3TypeResolutionRequest::inferred(default)
}

fn vue3_single_plain_object_property<'a>(
    object: &'a ObjectExpression<'a>,
) -> Option<&'a ObjectProperty<'a>> {
    let [property] = object.properties.as_slice() else {
        return None;
    };
    let ObjectPropertyKind::ObjectProperty(property) = property else {
        return None;
    };
    if property.kind != PropertyKind::Init
        || property.method
        || property.shorthand
        || property.computed
    {
        return None;
    }
    Some(property)
}

fn vue3_resolution_mode_from_ts_import_type_options(
    options: Option<&ObjectExpression<'_>>,
) -> Option<Vue3TypeResolutionMode> {
    let wrapper = vue3_single_plain_object_property(options?)?;
    let PropertyKey::StaticIdentifier(wrapper_key) = &wrapper.key else {
        return None;
    };
    if !matches!(wrapper_key.name.as_str(), "with" | "assert") {
        return None;
    }
    let Expression::ObjectExpression(attributes) = &wrapper.value else {
        return None;
    };
    let attribute = vue3_single_plain_object_property(attributes)?;
    let PropertyKey::StringLiteral(key) = &attribute.key else {
        return None;
    };
    if key.value != "resolution-mode" {
        return None;
    }
    match &attribute.value {
        Expression::StringLiteral(value) => {
            vue3_resolution_mode_from_value(value.value.as_str())
        }
        Expression::TemplateLiteral(value) => value
            .single_quasi()
            .and_then(|value| vue3_resolution_mode_from_value(value.as_str())),
        _ => None,
    }
}

fn vue3_ts_import_type_resolution_request(
    import_type: &TSImportType<'_>,
    default: Vue3TypeResolutionMode,
) -> Vue3TypeResolutionRequest {
    vue3_resolution_mode_from_ts_import_type_options(import_type.options.as_deref())
        .map(Vue3TypeResolutionRequest::explicit)
        .unwrap_or_else(|| Vue3TypeResolutionRequest::inferred(default))
}

fn resolve_vue3_type_import_for_request(
    filename: &str,
    source: &str,
    request: Vue3TypeResolutionRequest,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if request.explicit_mode {
        resolve_vue3_type_import_with_explicit_mode(
            filename,
            source,
            request.mode,
            type_resolver,
        )
    } else {
        resolve_vue3_type_import_with_mode(filename, source, request.mode, type_resolver)
    }
}

fn vue3_is_commonjs_require_call(call: &CallExpression<'_>) -> bool {
    matches!(
        &call.callee,
        Expression::Identifier(identifier)
            if identifier.name == "require" && identifier.span.start == call.span.start
    ) && call.arguments.len() == 1
}

fn vue3_static_commonjs_require_source<'a>(call: &'a CallExpression<'_>) -> Option<&'a str> {
    if !vue3_is_commonjs_require_call(call) {
        return None;
    }
    match call.arguments.first()? {
        Argument::StringLiteral(literal) => Some(literal.value.as_str()),
        Argument::TemplateLiteral(literal)
            if literal.expressions.is_empty() && literal.quasis.len() == 1 =>
        {
            literal
                .quasis
                .first()?
                .value
                .cooked
                .as_ref()
                .map(|value| value.as_str())
        }
        _ => None,
    }
}

fn vue3_expression_is_commonjs_exports_root(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::Identifier(identifier) => identifier.name == "exports",
        Expression::StaticMemberExpression(member) => {
            matches!(&member.object, Expression::Identifier(identifier) if identifier.name == "module")
                && member.property.name == "exports"
        }
        Expression::ComputedMemberExpression(member) => {
            matches!(&member.object, Expression::Identifier(identifier) if identifier.name == "module")
                && vue3_expression_is_exports_property_name(&member.expression)
        }
        _ => false,
    }
}

fn vue3_expression_is_static_property_name(expression: &Expression<'_>) -> bool {
    matches!(
        expression,
        Expression::StringLiteral(_) | Expression::NumericLiteral(_)
    ) || expression.is_no_substitution_template()
}

fn vue3_expression_is_exports_property_name(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::StringLiteral(property) => property.value == "exports",
        Expression::TemplateLiteral(property) => property
            .single_quasi()
            .is_some_and(|property| property == "exports"),
        _ => false,
    }
}

fn vue3_expression_is_commonjs_exports_chain(expression: &Expression<'_>) -> bool {
    if vue3_expression_is_commonjs_exports_root(expression) {
        return true;
    }
    match expression {
        Expression::StaticMemberExpression(member) => {
            vue3_expression_is_commonjs_exports_chain(&member.object)
        }
        Expression::ComputedMemberExpression(member)
            if vue3_expression_is_static_property_name(&member.expression) =>
        {
            vue3_expression_is_commonjs_exports_chain(&member.object)
        }
        _ => false,
    }
}

fn vue3_rightmost_assigned_expression<'a>(mut expression: &'a Expression<'a>) -> &'a Expression<'a> {
    while let Expression::AssignmentExpression(assignment) = expression {
        if !assignment.operator.is_assign() {
            break;
        }
        expression = &assignment.right;
    }
    expression
}

fn vue3_assignment_is_commonjs_export(assignment: &AssignmentExpression<'_>) -> bool {
    if !assignment.operator.is_assign()
        || vue3_rightmost_assigned_expression(&assignment.right).is_void_0()
    {
        return false;
    }
    match &assignment.left {
        AssignmentTarget::StaticMemberExpression(member) => {
            vue3_expression_is_commonjs_exports_chain(&member.object)
                || matches!(
                    &member.object,
                    Expression::Identifier(identifier)
                        if identifier.name == "module" && member.property.name == "exports"
                )
        }
        AssignmentTarget::ComputedMemberExpression(member) => {
            vue3_expression_is_static_property_name(&member.expression)
                && (vue3_expression_is_commonjs_exports_chain(&member.object)
                    || matches!(&member.object, Expression::Identifier(identifier) if identifier.name == "module")
                        && vue3_expression_is_exports_property_name(&member.expression))
        }
        _ => false,
    }
}

fn vue3_is_commonjs_define_property_call(call: &CallExpression<'_>) -> bool {
    if call.arguments.len() != 3 {
        return false;
    }
    let Expression::StaticMemberExpression(callee) = &call.callee else {
        return false;
    };
    if !matches!(&callee.object, Expression::Identifier(identifier) if identifier.name == "Object")
        || callee.property.name != "defineProperty"
    {
        return false;
    }
    let Some(target) = call
        .arguments
        .first()
        .and_then(Argument::as_expression)
    else {
        return false;
    };
    let Some(property) = call.arguments.get(1).and_then(Argument::as_expression) else {
        return false;
    };
    vue3_expression_is_commonjs_exports_root(target)
        && vue3_expression_is_static_property_name(property)
}

#[derive(Default)]
struct Vue3CommonJsModuleDetector {
    found: bool,
}

impl<'a> oxc_ast_visit::Visit<'a> for Vue3CommonJsModuleDetector {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if vue3_is_commonjs_require_call(call) || vue3_is_commonjs_define_property_call(call) {
            self.found = true;
            return;
        }
        oxc_ast_visit::walk::walk_call_expression(self, call);
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if vue3_assignment_is_commonjs_export(assignment) {
            self.found = true;
            return;
        }
        oxc_ast_visit::walk::walk_assignment_expression(self, assignment);
    }
}

fn vue3_javascript_statements_have_commonjs_module_indicator(
    statements: &[Statement<'_>],
    source_type: oxc_span::SourceType,
) -> bool {
    if !source_type.is_javascript() {
        return false;
    }
    let mut detector = Vue3CommonJsModuleDetector::default();
    for statement in statements {
        oxc_ast_visit::Visit::visit_statement(&mut detector, statement);
        if detector.found {
            break;
        }
    }
    detector.found
}

impl<'a> oxc_ast_visit::Visit<'a> for Vue3ModuleDependencyCollector<'_> {
    fn visit_import_expression(&mut self, expression: &ImportExpression<'a>) {
        if let Expression::StringLiteral(source) = &expression.source {
            self.insert_module(source.value.as_str(), self.dynamic_resolution_mode);
        }
        oxc_ast_visit::walk::walk_import_expression(self, expression);
    }

    fn visit_ts_import_type(&mut self, import: &TSImportType<'a>) {
        self.insert_module_request(
            import.source.value.as_str(),
            vue3_ts_import_type_resolution_request(import, self.static_resolution_mode),
        );
        oxc_ast_visit::walk::walk_ts_import_type(self, import);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if self.collect_commonjs_requires {
            if let Some(source) = vue3_static_commonjs_require_source(call) {
                self.insert_module(source, Vue3TypeResolutionMode::Require);
            }
        }
        oxc_ast_visit::walk::walk_call_expression(self, call);
    }

    fn visit_ts_external_module_reference(
        &mut self,
        reference: &TSExternalModuleReference<'a>,
    ) {
        self.insert_module(
            reference.expression.value.as_str(),
            Vue3TypeResolutionMode::Require,
        );
    }
}

enum Vue3TripleSlashReference<'a> {
    Path(&'a str),
    Types(&'a str, Option<Vue3TypeResolutionMode>),
    Unsupported,
}

fn vue3_triple_slash_reference(comment: &str) -> Option<Vue3TripleSlashReference<'_>> {
    let input = comment.strip_prefix('/')?.trim_start();
    let tag = input.get(.."<reference".len())?;
    if !tag.eq_ignore_ascii_case("<reference") {
        return None;
    }
    let input = &input[tag.len()..];
    if !input.chars().next().is_some_and(char::is_whitespace) {
        return None;
    }
    let mut input = input.trim_end().strip_suffix("/>")?;
    let mut path = None;
    let mut types = None;
    let mut has_lib = false;
    let mut no_default_lib = false;
    let mut resolution_mode = None;
    while !input.trim_start().is_empty() {
        input = input.trim_start();
        let name_end = input
            .find(|character: char| character.is_whitespace() || character == '=')
            .unwrap_or(input.len());
        if name_end == 0 {
            return None;
        }
        let name = &input[..name_end];
        input = input[name_end..].trim_start().strip_prefix('=')?.trim_start();
        let quote = input.chars().next()?;
        if !matches!(quote, '\'' | '"') {
            return None;
        }
        input = &input[quote.len_utf8()..];
        let value_end = input.find(quote)?;
        let value = &input[..value_end];
        input = &input[value_end + quote.len_utf8()..];
        if name.eq_ignore_ascii_case("path") && path.is_none() {
            path = Some(value);
        } else if name.eq_ignore_ascii_case("types") && types.is_none() {
            types = Some(value);
        } else if name.eq_ignore_ascii_case("lib") {
            has_lib = true;
        } else if name.eq_ignore_ascii_case("no-default-lib")
            && value.eq_ignore_ascii_case("true")
        {
            no_default_lib = true;
        } else if name.eq_ignore_ascii_case("resolution-mode") && resolution_mode.is_none() {
            resolution_mode = Some(value);
        }
    }
    if no_default_lib {
        None
    } else if let Some(types) = types {
        let resolution_mode = match resolution_mode {
            Some("import") => Some(Vue3TypeResolutionMode::Import),
            Some("require") => Some(Vue3TypeResolutionMode::Require),
            Some("") | None => None,
            Some(_) => return Some(Vue3TripleSlashReference::Unsupported),
        };
        Some(Vue3TripleSlashReference::Types(types, resolution_mode))
    } else if has_lib {
        None
    } else if let Some(path) = path {
        Some(Vue3TripleSlashReference::Path(path))
    } else if resolution_mode.is_some() {
        Some(Vue3TripleSlashReference::Unsupported)
    } else {
        None
    }
}

#[cfg(test)]
fn vue3_module_dependencies_from_source(
    source: &str,
    source_type: oxc_span::SourceType,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<(bool, BTreeSet<Vue3ModuleDependency>)> {
    vue3_module_dependencies_from_source_with_modes(
        source,
        source_type,
        vue3_static_resolution_mode(source_type),
        Vue3TypeResolutionMode::Import,
        namespace_budget,
    )
}

fn vue3_module_dependencies_from_source_with_modes(
    source: &str,
    source_type: oxc_span::SourceType,
    static_resolution_mode: Vue3TypeResolutionMode,
    dynamic_resolution_mode: Vue3TypeResolutionMode,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<(bool, BTreeSet<Vue3ModuleDependency>)> {
    if !namespace_budget.reserve(source.len().saturating_add(1)) {
        return None;
    }
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    if !namespace_budget.reserve(parsed.program.body.len()) {
        return None;
    }
    let has_global_augmentation = parsed.program.body.iter().any(|statement| {
        matches!(statement, Statement::TSGlobalDeclaration(_))
            || matches!(
                statement,
                Statement::TSModuleDeclaration(declaration)
                    if vue3_ts_module_declaration_is_global(declaration)
            )
            || matches!(
                statement,
                Statement::ExportNamedDeclaration(export)
                    if matches!(
                        export.declaration.as_ref(),
                        Some(Declaration::TSModuleDeclaration(declaration))
                            if vue3_ts_module_declaration_is_global(declaration)
                    )
            )
    });
    let mut collector = Vue3ModuleDependencyCollector {
        dependencies: BTreeSet::new(),
        collect_commonjs_requires: source_type.is_javascript(),
        static_resolution_mode,
        dynamic_resolution_mode,
        namespace_budget,
    };
    let first_syntax_start = parsed
        .program
        .directives
        .first()
        .map(|directive| directive.span.start as usize)
        .into_iter()
        .chain(
            parsed
                .program
                .body
                .first()
                .map(|statement| statement.span().start as usize),
        )
        .min()
        .unwrap_or(source.len());
    for comment in &parsed.program.comments {
        if !comment.is_line() || comment.span.end as usize > first_syntax_start {
            continue;
        }
        let comment_start = comment.span.start as usize;
        let line_start = source
            .get(..comment_start)?
            .as_bytes()
            .iter()
            .rposition(|byte| matches!(byte, b'\r' | b'\n'))
            .map_or(0, |index| index + 1);
        let line_prefix = source.get(line_start..comment_start)?;
        let line_prefix = if line_start == 0 {
            line_prefix.strip_prefix('\u{feff}').unwrap_or(line_prefix)
        } else {
            line_prefix
        };
        if !line_prefix.chars().all(char::is_whitespace) {
            continue;
        }
        let span = comment.content_span();
        let comment_source = source.get(span.start as usize..span.end as usize)?;
        match vue3_triple_slash_reference(comment_source) {
            Some(Vue3TripleSlashReference::Path(path)) => {
                collector.insert(Vue3ModuleDependency::ReferencePath(path.to_string()));
            }
            Some(Vue3TripleSlashReference::Types(types, resolution_mode)) => {
                collector.insert(Vue3ModuleDependency::ReferenceTypes {
                    source: types.to_string(),
                    request: resolution_mode
                        .map(Vue3TypeResolutionRequest::explicit)
                        .unwrap_or_else(|| {
                            Vue3TypeResolutionRequest::inferred(collector.static_resolution_mode)
                        }),
                });
            }
            Some(Vue3TripleSlashReference::Unsupported) => return None,
            None => {}
        }
    }
    for statement in &parsed.program.body {
        match statement {
            Statement::ImportDeclaration(import) => {
                collector.insert_module_request(
                    import.source.value.as_str(),
                    vue3_declaration_resolution_request(
                        import.import_kind,
                        import.with_clause.as_deref(),
                        collector.static_resolution_mode,
                    ),
                );
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(source) = &export.source {
                    collector.insert_module_request(
                        source.value.as_str(),
                        vue3_declaration_resolution_request(
                            export.export_kind,
                            export.with_clause.as_deref(),
                            collector.static_resolution_mode,
                        ),
                    );
                }
            }
            Statement::ExportAllDeclaration(export) => {
                collector.insert_module_request(
                    export.source.value.as_str(),
                    vue3_declaration_resolution_request(
                        export.export_kind,
                        export.with_clause.as_deref(),
                        collector.static_resolution_mode,
                    ),
                );
            }
            _ => {}
        }
        oxc_ast_visit::Visit::visit_statement(&mut collector, statement);
    }
    if collector.namespace_budget.is_exhausted() {
        None
    } else {
        Some((has_global_augmentation, collector.dependencies))
    }
}

fn enqueue_vue3_module_dependencies(
    importer: &str,
    dependencies: BTreeSet<Vue3ModuleDependency>,
    depth: usize,
    pending: &mut VecDeque<(String, Vue3ModuleDependency, usize)>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    let work = dependencies.iter().fold(0usize, |work, dependency| {
        work
            .saturating_add(importer.len())
            .saturating_add(dependency.source().len())
            .saturating_add(std::mem::size_of::<(
                String,
                Vue3ModuleDependency,
                usize,
            )>())
            .saturating_add(1)
    });
    if !namespace_budget.reserve(work) {
        return None;
    }
    pending.extend(
        dependencies
            .into_iter()
            .map(|dependency| (importer.to_string(), dependency, depth)),
    );
    Some(())
}

fn vue3_module_path_can_contain_typescript(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "vue"]
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

fn vue3_reachable_global_augmentation_files(
    project_filename: &str,
    initial_global_files: &[Vue3GlobalTypeFile],
    inline_module_sources: &[Vue3InlineModuleSource<'_>],
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vec<PathBuf>> {
    let type_resolver = Vue3TypeResolverContext {
        typescript_version: type_resolver.typescript_version.clone(),
        module_resolution: type_resolver.module_resolution,
        module: type_resolver.module,
        resolve_package_json_exports: type_resolver.resolve_package_json_exports,
        resolve_package_json_imports: type_resolver.resolve_package_json_imports,
        active_package_json_features: type_resolver.active_package_json_features,
        module_suffixes: type_resolver.module_suffixes.clone(),
        external_type_session: Vue3ExternalTypeLoadSession::with_limits(
            type_resolver.external_type_session.limits(),
        ),
    };
    let mut namespace_budget = Vue3NamespaceProjectionBudget::default();
    let mut seen = BTreeSet::new();
    let mut pending = VecDeque::new();
    for file in initial_global_files {
        let identity = vue3_external_type_source_semantic_identity(&file.path, &file.source);
        if seen.contains(&identity) {
            continue;
        }
        if !namespace_budget.reserve(identity.work()) {
            return None;
        }
        seen.insert(identity);
        let (_, dependencies) = vue3_module_dependencies_from_source_with_modes(
            &file.source.source,
            file.source.source_type,
            file.source.resolution_mode,
            file.source.dynamic_resolution_mode,
            &mut namespace_budget,
        )?;
        enqueue_vue3_module_dependencies(
            &normalize_path_string(&file.path),
            dependencies,
            1,
            &mut pending,
            &mut namespace_budget,
        )?;
    }
    for root in inline_module_sources {
        let (static_resolution_mode, dynamic_resolution_mode) =
            vue3_inline_type_resolution_modes(root.source_type, &type_resolver);
        let (_, dependencies) = vue3_module_dependencies_from_source_with_modes(
            root.source,
            root.source_type,
            static_resolution_mode,
            dynamic_resolution_mode,
            &mut namespace_budget,
        )?;
        enqueue_vue3_module_dependencies(
            root.filename,
            dependencies,
            1,
            &mut pending,
            &mut namespace_budget,
        )?;
    }

    let max_import_files = type_resolver
        .external_type_session
        .limits()
        .max_import_files
        .min(VUE3_EXTERNAL_TYPE_MAX_IMPORT_FILES);
    let mut scanned_import_files = 0usize;
    let mut additional_global_paths =
        BTreeMap::<Vue3ExternalTypeSemanticIdentity, PathBuf>::new();
    while let Some((importer, dependency, depth)) = pending.pop_front() {
        if depth > VUE3_EXTERNAL_TYPE_MAX_ACTIVE_FILES {
            return None;
        }
        let is_global_program_reference = dependency.is_global_program_reference();
        let failure_epoch = type_resolver.external_type_session.failure_epoch();
        let resolved = match &dependency {
            Vue3ModuleDependency::Module {
                source,
                request,
            } => resolve_vue3_type_import_for_request(
                &importer,
                source,
                *request,
                &type_resolver,
            ),
            Vue3ModuleDependency::ReferencePath(reference) => {
                resolve_vue3_type_reference_path(&importer, reference, &type_resolver)
            }
            Vue3ModuleDependency::ReferenceTypes {
                source: reference,
                request,
            } => {
                let has_implicit_mode = matches!(
                        type_resolver.module_resolution,
                        Vue3TypeModuleResolutionKind::Node16
                            | Vue3TypeModuleResolutionKind::NodeNext
                    ) && !vue3_path_has_vue_extension(Path::new(&importer));
                let resolution_mode = (request.explicit_mode || has_implicit_mode)
                    .then_some(request.mode);
                resolve_vue3_type_reference_directive_with_mode(
                    project_filename,
                    &importer,
                    reference,
                    resolution_mode,
                    &type_resolver,
                )
            }
        };
        let Some(resolved) = resolved else {
            if is_global_program_reference
                || type_resolver.external_type_session.failure_epoch() != failure_epoch
            {
                return None;
            }
            continue;
        };
        let identity = vue3_external_type_path_identity(&resolved);
        if is_global_program_reference
            && identity == vue3_external_type_path_identity(Path::new(&importer))
        {
            return None;
        }
        let normalized = normalize_path_string(&resolved);
        if !namespace_budget.reserve(
            normalized
                .len()
                .saturating_add(std::mem::size_of::<PathBuf>())
                .saturating_add(1),
        ) {
            return None;
        }
        if !vue3_module_path_can_contain_typescript(&resolved) {
            if is_global_program_reference {
                return None;
            }
            continue;
        }
        let format = vue3_external_type_format_with_resolver(&resolved, &type_resolver)?;
        let semantic_identity = vue3_external_type_semantic_identity(&resolved, format);
        if is_global_program_reference
            && !additional_global_paths.contains_key(&semantic_identity)
        {
            if !namespace_budget.reserve(
                normalized
                    .len()
                    .saturating_add(semantic_identity.work())
                    .saturating_add(std::mem::size_of::<PathBuf>())
                    .saturating_add(1),
            ) {
                return None;
            }
            additional_global_paths.insert(semantic_identity.clone(), resolved.clone());
        }
        if seen.contains(&semantic_identity) {
            continue;
        }
        if scanned_import_files >= max_import_files {
            return None;
        }
        if !namespace_budget.reserve(semantic_identity.work()) {
            return None;
        }
        seen.insert(semantic_identity.clone());
        scanned_import_files = scanned_import_files.saturating_add(1);
        let source = vue3_external_type_source_from_path(&resolved, &type_resolver)?;
        let (has_global_augmentation, dependencies) =
            vue3_module_dependencies_from_source_with_modes(
            &source.source,
            source.source_type,
            source.resolution_mode,
            source.dynamic_resolution_mode,
            &mut namespace_budget,
        )?;
        if has_global_augmentation
            && !additional_global_paths.contains_key(&semantic_identity)
        {
            if !namespace_budget.reserve(
                normalized
                    .len()
                    .saturating_add(semantic_identity.work())
                    .saturating_add(std::mem::size_of::<PathBuf>())
                    .saturating_add(1),
            ) {
                return None;
            }
            additional_global_paths.insert(semantic_identity, resolved.clone());
        }
        enqueue_vue3_module_dependencies(
            &normalized,
            dependencies,
            depth.saturating_add(1),
            &mut pending,
            &mut namespace_budget,
        )?;
    }
    Some(additional_global_paths.into_values().collect())
}

pub(crate) fn extend_vue3_type_context_from_external_imports(
    filename: &str,
    source: &str,
    source_type: oxc_span::SourceType,
    context: &mut Vue27TypeContext,
    type_resolver: &Vue3TypeResolverContext,
) -> bool {
    let mut seen = BTreeSet::new();
    let mut namespace_budget = Vue3NamespaceProjectionBudget::default();
    extend_vue3_type_context_from_external_imports_with_seen(
        filename,
        source,
        source_type,
        context,
        &mut seen,
        type_resolver,
        &mut namespace_budget,
    )
}

pub(crate) fn extend_vue3_type_context_from_external_imports_with_seen(
    filename: &str,
    source: &str,
    source_type: oxc_span::SourceType,
    context: &mut Vue27TypeContext,
    seen: &mut BTreeSet<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    let (static_resolution_mode, _) =
        vue3_inline_type_resolution_modes(source_type, type_resolver);
    extend_vue3_type_context_from_external_imports_with_seen_and_mode(
        filename,
        source,
        source_type,
        static_resolution_mode,
        context,
        seen,
        type_resolver,
        namespace_budget,
    )
}

fn extend_vue3_type_context_from_external_imports_with_seen_and_mode(
    filename: &str,
    source: &str,
    source_type: oxc_span::SourceType,
    static_resolution_mode: Vue3TypeResolutionMode,
    context: &mut Vue27TypeContext,
    seen: &mut BTreeSet<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return false;
    }
    if !namespace_budget.reserve(vue3_external_type_context_cache_cost(context)) {
        return false;
    }
    let mut working_context = context.clone();
    for statement in &parsed.program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let import_source = import.source.value.as_str();
        let Some(specifiers) = &import.specifiers else {
            continue;
        };
        let request = vue3_declaration_resolution_request(
            import.import_kind,
            import.with_clause.as_deref(),
            static_resolution_mode,
        );
        let Some(resolved) =
            resolve_vue3_type_import_for_request(filename, import_source, request, type_resolver)
        else {
            if !clear_vue3_failed_import_bindings(
                &mut working_context,
                specifiers,
                Some(import_source),
                namespace_budget,
            ) {
                return false;
            }
            continue;
        };
        let Some(imported_context) =
            vue3_external_type_context_from_path(&resolved, &mut *seen, type_resolver)
        else {
            if !clear_vue3_failed_import_bindings(
                &mut working_context,
                specifiers,
                None,
                namespace_budget,
            ) {
                return false;
            }
            continue;
        };
        let normalized = normalize_path_string(&resolved);
        for specifier in specifiers {
            let local = import_specifier_local_name(specifier);
            let imported = import_specifier_imported_name(specifier).unwrap_or("default");
            if imported == "*" {
                if !insert_vue3_external_namespace_types(
                    &mut working_context,
                    &imported_context,
                    local,
                    &normalized,
                    namespace_budget,
                ) {
                    return false;
                }
                continue;
            }
            if !insert_vue3_external_type_alias_and_namespace_members(
                &mut working_context,
                &imported_context,
                imported,
                local,
                &normalized,
                namespace_budget,
            ) {
                return false;
            }
        }
    }
    *context = working_context;
    true
}

fn clear_vue3_failed_import_bindings(
    context: &mut Vue27TypeContext,
    specifiers: &[ImportDeclarationSpecifier<'_>],
    unresolved_source: Option<&str>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    for specifier in specifiers {
        let local = import_specifier_local_name(specifier);
        if !reserve_vue3_external_import_binding_clear(context, local, namespace_budget) {
            return false;
        }
        if let Some(source) = unresolved_source {
            let metadata_work = local
                .len()
                .saturating_add(source.len())
                .saturating_add(64);
            if !namespace_budget.reserve(metadata_work) {
                return false;
            }
        }
    }
    for specifier in specifiers {
        let local = import_specifier_local_name(specifier);
        clear_vue3_external_import_binding(context, local);
        if let Some(source) = unresolved_source {
            context
                .unresolved_import_sources
                .insert(local.to_string(), source.to_string());
        }
    }
    true
}

fn vue3_external_type_context_from_source_inner(
    source: &str,
    filename: &str,
    source_type: oxc_span::SourceType,
    static_resolution_mode: Vue3TypeResolutionMode,
    seen: &mut BTreeSet<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue27TypeContext {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue27TypeContext::default();
    }
    let mut analysis = Vue3ScriptSetupAnalysis {
        type_filename: Some(filename.to_string()),
        type_resolution_mode: static_resolution_mode,
        type_seen: seen.clone(),
        type_resolver: type_resolver.clone(),
        ..Vue3ScriptSetupAnalysis::default()
    };
    let mut seed_context = Vue27TypeContext::default();
    let mut namespace_budget = Vue3NamespaceProjectionBudget::default();
    if !extend_vue3_type_context_from_external_imports_with_seen_and_mode(
        filename,
        source,
        source_type,
        static_resolution_mode,
        &mut seed_context,
        seen,
        type_resolver,
        &mut namespace_budget,
    ) {
        return Vue27TypeContext::default();
    }
    analysis.declared_types = seed_context.declared_types;
    analysis.define_model_declared_types = seed_context.define_model_declared_types;
    analysis.type_query_declared_types = seed_context.type_query_declared_types;
    analysis.define_model_type_query_declared_types =
        seed_context.define_model_type_query_declared_types;
    analysis.keyof_type_query_declared_types = seed_context.keyof_type_query_declared_types;
    analysis.props_type_declarations = seed_context.props_type_declarations;
    analysis.keyof_runtime_type_declarations = seed_context.keyof_runtime_type_declarations;
    analysis.tuple_runtime_type_declarations = seed_context.tuple_runtime_type_declarations;
    analysis.define_model_tuple_runtime_type_declarations =
        seed_context.define_model_tuple_runtime_type_declarations;
    analysis.array_element_runtime_type_declarations =
        seed_context.array_element_runtime_type_declarations;
    analysis.define_model_array_element_runtime_type_declarations =
        seed_context.define_model_array_element_runtime_type_declarations;
    analysis.parameter_tuple_runtime_type_declarations =
        seed_context.parameter_tuple_runtime_type_declarations;
    analysis.define_model_parameter_tuple_runtime_type_declarations =
        seed_context.define_model_parameter_tuple_runtime_type_declarations;
    analysis.constructor_parameter_tuple_runtime_type_declarations =
        seed_context.constructor_parameter_tuple_runtime_type_declarations;
    analysis.define_model_constructor_parameter_tuple_runtime_type_declarations =
        seed_context.define_model_constructor_parameter_tuple_runtime_type_declarations;
    analysis.return_type_runtime_type_declarations =
        seed_context.return_type_runtime_type_declarations;
    analysis.define_model_return_type_runtime_type_declarations =
        seed_context.define_model_return_type_runtime_type_declarations;
    analysis.props_options_type_declarations = seed_context.props_options_type_declarations;
    analysis.return_type_props_options_declarations =
        seed_context.return_type_props_options_declarations;
    analysis.generic_type_aliases = seed_context.generic_type_aliases;
    analysis.string_literal_type_declarations = seed_context.string_literal_type_declarations;
    analysis.ordered_string_literal_type_declarations =
        seed_context.ordered_string_literal_type_declarations;
    analysis.emits_type_declarations = seed_context.emits_type_declarations;
    analysis.type_sources = seed_context.type_sources;
    analysis.type_direct_deps = seed_context.type_direct_deps;
    analysis.type_deps = seed_context.type_deps;
    analysis.unresolved_import_sources = seed_context.unresolved_import_sources;
    analysis.silent_unresolved_type_names = seed_context.silent_unresolved_type_names;
    collect_vue3_declared_types_from_statements_with_namespace_budget(
        source,
        &parsed.program.body,
        source_type.is_typescript_definition(),
        0,
        &mut analysis,
        &mut namespace_budget,
    );
    if namespace_budget.is_exhausted() || analysis.type_dependency_work_exhausted {
        return Vue27TypeContext::default();
    }
    collect_vue3_declared_type_deps_from_statements(&parsed.program.body, &mut analysis);
    if analysis.type_dependency_work_exhausted {
        return Vue27TypeContext::default();
    }
    project_vue3_default_type_exports(source, &parsed.program.body, &mut analysis);
    finalize_vue3_local_generic_alias_scopes(&mut analysis);
    seed_vue3_external_type_deps(filename, &mut analysis);
    let Some(re_exported) = project_vue3_type_re_exports(
        filename,
        &parsed.program.body,
        static_resolution_mode,
        &mut analysis,
        seen,
        type_resolver,
        &mut namespace_budget,
    ) else {
        return Vue27TypeContext::default();
    };
    if project_vue3_exported_type_specifiers_with_budget(
        &parsed.program.body,
        &mut analysis,
        &mut namespace_budget,
    )
    .is_none()
    {
        return Vue27TypeContext::default();
    }
    let Some(mut exported) =
        vue3_exported_type_names_with_budget(&parsed.program.body, &mut namespace_budget)
    else {
        return Vue27TypeContext::default();
    };
    let Some(namespace_specifier_names) =
        project_vue3_exported_namespace_specifiers_with_budget(
            &parsed.program.body,
            source_type.is_typescript_definition(),
            &mut analysis,
            &mut namespace_budget,
        )
    else {
        return Vue27TypeContext::default();
    };
    exported.extend(namespace_specifier_names);
    exported.extend(re_exported);
    analysis
        .declared_types
        .retain(|name, _| exported.contains(name));
    analysis
        .define_model_declared_types
        .retain(|name, _| exported.contains(name));
    analysis
        .type_query_declared_types
        .retain(|name, _| exported.contains(name));
    analysis
        .define_model_type_query_declared_types
        .retain(|name, _| exported.contains(name));
    analysis
        .keyof_type_query_declared_types
        .retain(|name, _| exported.contains(name));
    analysis
        .props_type_declarations
        .retain(|name, _| exported.contains(name));
    analysis
        .keyof_runtime_type_declarations
        .retain(|name, _| exported.contains(name));
    analysis
        .tuple_runtime_type_declarations
        .retain(|name, _| exported.contains(name));
    analysis
        .define_model_tuple_runtime_type_declarations
        .retain(|name, _| exported.contains(name));
    analysis
        .array_element_runtime_type_declarations
        .retain(|name, _| exported.contains(name));
    analysis
        .define_model_array_element_runtime_type_declarations
        .retain(|name, _| exported.contains(name));
    analysis
        .parameter_tuple_runtime_type_declarations
        .retain(|name, _| exported.contains(name));
    analysis
        .define_model_parameter_tuple_runtime_type_declarations
        .retain(|name, _| exported.contains(name));
    analysis
        .constructor_parameter_tuple_runtime_type_declarations
        .retain(|name, _| exported.contains(name));
    analysis
        .define_model_constructor_parameter_tuple_runtime_type_declarations
        .retain(|name, _| exported.contains(name));
    analysis
        .return_type_runtime_type_declarations
        .retain(|name, _| exported.contains(name));
    analysis
        .define_model_return_type_runtime_type_declarations
        .retain(|name, _| exported.contains(name));
    analysis
        .props_options_type_declarations
        .retain(|name, _| exported.contains(name));
    analysis
        .return_type_props_options_declarations
        .retain(|name, _| exported.contains(name));
    analysis
        .generic_type_aliases
        .retain(|name, _| exported.contains(name));
    analysis
        .string_literal_type_declarations
        .retain(|name, _| exported.contains(name));
    analysis
        .ordered_string_literal_type_declarations
        .retain(|name, _| exported.contains(name));
    analysis
        .emits_type_declarations
        .retain(|name, _| exported.contains(name));
    analysis
        .type_sources
        .retain(|name, _| exported.contains(name));
    analysis
        .type_direct_deps
        .retain(|name, _| exported.contains(name));
    analysis.type_deps.retain(|name, _| exported.contains(name));
    analysis
        .unresolved_import_sources
        .retain(|name, _| exported.contains(name));
    analysis
        .silent_unresolved_type_names
        .retain(|name| exported.contains(name));
    Vue27TypeContext {
        declared_types: analysis.declared_types,
        define_model_declared_types: analysis.define_model_declared_types,
        type_query_declared_types: analysis.type_query_declared_types,
        define_model_type_query_declared_types: analysis.define_model_type_query_declared_types,
        keyof_type_query_declared_types: analysis.keyof_type_query_declared_types,
        props_type_declarations: analysis.props_type_declarations,
        keyof_runtime_type_declarations: analysis.keyof_runtime_type_declarations,
        tuple_runtime_type_declarations: analysis.tuple_runtime_type_declarations,
        define_model_tuple_runtime_type_declarations: analysis
            .define_model_tuple_runtime_type_declarations,
        array_element_runtime_type_declarations: analysis.array_element_runtime_type_declarations,
        define_model_array_element_runtime_type_declarations: analysis
            .define_model_array_element_runtime_type_declarations,
        parameter_tuple_runtime_type_declarations: analysis
            .parameter_tuple_runtime_type_declarations,
        define_model_parameter_tuple_runtime_type_declarations: analysis
            .define_model_parameter_tuple_runtime_type_declarations,
        constructor_parameter_tuple_runtime_type_declarations: analysis
            .constructor_parameter_tuple_runtime_type_declarations,
        define_model_constructor_parameter_tuple_runtime_type_declarations: analysis
            .define_model_constructor_parameter_tuple_runtime_type_declarations,
        return_type_runtime_type_declarations: analysis.return_type_runtime_type_declarations,
        define_model_return_type_runtime_type_declarations: analysis
            .define_model_return_type_runtime_type_declarations,
        props_options_type_declarations: analysis.props_options_type_declarations,
        return_type_props_options_declarations: analysis.return_type_props_options_declarations,
        generic_type_aliases: analysis.generic_type_aliases,
        string_literal_type_declarations: analysis.string_literal_type_declarations,
        ordered_string_literal_type_declarations: analysis.ordered_string_literal_type_declarations,
        emits_type_declarations: analysis.emits_type_declarations,
        type_sources: analysis.type_sources,
        type_direct_deps: analysis.type_direct_deps,
        type_deps: analysis.type_deps,
        unresolved_import_sources: analysis.unresolved_import_sources,
        silent_unresolved_type_names: analysis.silent_unresolved_type_names,
    }
}

pub(crate) fn seed_vue3_external_type_deps(filename: &str, analysis: &mut Vue3ScriptSetupAnalysis) {
    let dependency = normalize_path_string(Path::new(filename));
    let names = analysis
        .declared_types
        .keys()
        .chain(analysis.define_model_declared_types.keys())
        .chain(analysis.type_query_declared_types.keys())
        .chain(analysis.define_model_type_query_declared_types.keys())
        .chain(analysis.keyof_type_query_declared_types.keys())
        .chain(analysis.props_type_declarations.keys())
        .chain(analysis.keyof_runtime_type_declarations.keys())
        .chain(analysis.tuple_runtime_type_declarations.keys())
        .chain(analysis.define_model_tuple_runtime_type_declarations.keys())
        .chain(analysis.array_element_runtime_type_declarations.keys())
        .chain(
            analysis
                .define_model_array_element_runtime_type_declarations
                .keys(),
        )
        .chain(analysis.parameter_tuple_runtime_type_declarations.keys())
        .chain(
            analysis
                .define_model_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(
            analysis
                .constructor_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(
            analysis
                .define_model_constructor_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(analysis.return_type_runtime_type_declarations.keys())
        .chain(
            analysis
                .define_model_return_type_runtime_type_declarations
                .keys(),
        )
        .chain(analysis.props_options_type_declarations.keys())
        .chain(analysis.return_type_props_options_declarations.keys())
        .chain(analysis.generic_type_aliases.keys())
        .chain(analysis.string_literal_type_declarations.keys())
        .chain(analysis.ordered_string_literal_type_declarations.keys())
        .chain(analysis.emits_type_declarations.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for name in names {
        analysis
            .type_sources
            .insert(name.clone(), dependency.clone());
        analysis.type_direct_deps.entry(name.clone()).or_default();
        analysis
            .type_deps
            .entry(name)
            .or_default()
            .insert(dependency.clone());
    }
}

pub(crate) fn project_vue3_type_re_exports(
    filename: &str,
    statements: &[Statement<'_>],
    static_resolution_mode: Vue3TypeResolutionMode,
    analysis: &mut Vue3ScriptSetupAnalysis,
    seen: &mut BTreeSet<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let mut exported_names = BTreeSet::new();
    for statement in statements {
        match statement {
            Statement::ExportNamedDeclaration(declaration) => {
                let Some(source) = declaration.source.as_ref() else {
                    continue;
                };
                let import_source = source.value.as_str();
                let request = vue3_declaration_resolution_request(
                    declaration.export_kind,
                    declaration.with_clause.as_deref(),
                    static_resolution_mode,
                );
                let Some(resolved_external) = vue3_external_type_context_from_source(
                    filename,
                    import_source,
                    request,
                    seen,
                    type_resolver,
                ) else {
                    continue;
                };
                for specifier in &declaration.specifiers {
                    let Some(imported) = module_export_name(specifier.local()) else {
                        continue;
                    };
                    let Some(exported) = module_export_name(specifier.exported()) else {
                        continue;
                    };
                    let names = insert_vue3_re_exported_type_alias_and_namespace_members(
                        analysis,
                        &resolved_external.context,
                        imported,
                        exported,
                        &resolved_external.dependency,
                        namespace_budget,
                    )?;
                    exported_names.extend(names);
                }
            }
            Statement::ExportAllDeclaration(declaration) => {
                let import_source = declaration.source.value.as_str();
                let request = vue3_declaration_resolution_request(
                    declaration.export_kind,
                    declaration.with_clause.as_deref(),
                    static_resolution_mode,
                );
                let Some(resolved_external) = vue3_external_type_context_from_source(
                    filename,
                    import_source,
                    request,
                    seen,
                    type_resolver,
                ) else {
                    continue;
                };
                    let names = project_vue3_export_all_type_context(
                        analysis,
                        &resolved_external.context,
                        &resolved_external.dependency,
                        namespace_budget,
                    )?;
                    exported_names.extend(names);
            }
            _ => {}
        }
    }
    Some(exported_names)
}

pub(crate) struct Vue3ResolvedExternalTypeContext {
    pub(crate) dependency: String,
    pub(crate) context: std::sync::Arc<Vue27TypeContext>,
}

fn vue3_external_type_context_from_source(
    filename: &str,
    source: &str,
    request: Vue3TypeResolutionRequest,
    seen: &mut BTreeSet<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue3ResolvedExternalTypeContext> {
    let resolved = resolve_vue3_type_import_for_request(filename, source, request, type_resolver)?;
    let dependency = normalize_path_string(&resolved);
    let context = vue3_external_type_context_from_path(&resolved, seen, type_resolver)?;
    Some(Vue3ResolvedExternalTypeContext {
        dependency,
        context,
    })
}

pub(crate) fn vue3_external_type_context_from_path(
    path: &Path,
    seen: &mut BTreeSet<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<std::sync::Arc<Vue27TypeContext>> {
    let identity = vue3_external_type_path_identity(path);
    if seen.len() >= VUE3_EXTERNAL_TYPE_MAX_ACTIVE_FILES || seen.contains(&identity) {
        type_resolver
            .external_type_session
            .record_context_failure();
        return None;
    }
    let cache_key = vue3_external_type_context_cache_key(path, type_resolver);
    let mut owner = match type_resolver
        .external_type_session
        .begin_context_load(&cache_key)
    {
        Vue3ExternalTypeContextLoad::Ready(context) => return Some(context),
        Vue3ExternalTypeContextLoad::Wait(waiter) => return waiter.wait(),
        Vue3ExternalTypeContextLoad::Failed => return None,
        Vue3ExternalTypeContextLoad::Start(owner) => owner,
    };
    seen.insert(identity.clone());
    let Some(source) = vue3_external_type_source_from_path(path, type_resolver) else {
        seen.remove(&identity);
        return owner.complete(None);
    };
    if !owner.reserve_build_weight(source.source.len()) {
        seen.remove(&identity);
        return None;
    }
    let normalized = normalize_path_string(path);
    let context = vue3_external_type_context_from_source_inner(
        &source.source,
        &normalized,
        source.source_type,
        source.resolution_mode,
        seen,
        type_resolver,
    );
    seen.remove(&identity);
    owner.complete(Some(context))
}

pub(crate) struct Vue3ResolvedImportType {
    pub(crate) name: String,
    pub(crate) dependency: String,
    pub(crate) context: std::sync::Arc<Vue27TypeContext>,
}

pub(crate) fn vue3_resolve_import_type(
    import_type: &TSImportType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue3ResolvedImportType> {
    let source = import_type.source.value.as_str();
    let name = vue3_import_type_qualifier_key(import_type.qualifier.as_ref()?);
    let filename = analysis.type_filename.as_deref()?;
    let request = vue3_ts_import_type_resolution_request(
        import_type,
        analysis.type_resolution_mode,
    );
    let resolved = resolve_vue3_type_import_for_request(
        filename,
        source,
        request,
        &analysis.type_resolver,
    )?;
    let dependency = normalize_path_string(&resolved);
    let mut seen = analysis.type_seen.clone();
    let context =
        vue3_external_type_context_from_path(&resolved, &mut seen, &analysis.type_resolver)?;
    Some(Vue3ResolvedImportType {
        name,
        dependency,
        context,
    })
}

fn vue3_exported_type_names_with_budget(
    statements: &[Statement<'_>],
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    for statement in statements {
        match statement {
            Statement::ExportDefaultDeclaration(declaration)
                if vue3_default_export_may_be_type(declaration) =>
            {
                insert_vue3_declared_type_name_with_budget(
                    &mut names,
                    "default",
                    namespace_budget,
                )?;
            }
            Statement::ExportNamedDeclaration(declaration) => {
                if let Some(declaration) = &declaration.declaration {
                    match declaration {
                        Declaration::TSInterfaceDeclaration(declaration) => {
                            insert_vue3_declared_type_name_with_budget(
                                &mut names,
                                declaration.id.name.as_str(),
                                namespace_budget,
                            )?;
                        }
                        Declaration::TSTypeAliasDeclaration(declaration) => {
                            insert_vue3_declared_type_name_with_budget(
                                &mut names,
                                declaration.id.name.as_str(),
                                namespace_budget,
                            )?;
                        }
                        Declaration::TSEnumDeclaration(declaration) => {
                            insert_vue3_declared_type_name_with_budget(
                                &mut names,
                                declaration.id.name.as_str(),
                                namespace_budget,
                            )?;
                        }
                        Declaration::FunctionDeclaration(function)
                            if vue3_function_has_return_projection(function) =>
                        {
                            if let Some(id) = &function.id {
                                insert_vue3_declared_type_name_with_budget(
                                    &mut names,
                                    id.name.as_str(),
                                    namespace_budget,
                                )?;
                            }
                        }
                        Declaration::VariableDeclaration(declaration) if declaration.declare => {
                            for declarator in &declaration.declarations {
                                if let Some(name) = first_pattern_binding_name(&declarator.id) {
                                    insert_vue3_declared_type_name_with_budget(
                                        &mut names,
                                        name,
                                        namespace_budget,
                                    )?;
                                }
                            }
                        }
                        Declaration::VariableDeclaration(declaration) => {
                            for declarator in &declaration.declarations {
                                if vue3_variable_declarator_has_type_projection(declarator) {
                                    if let Some(name) = first_pattern_binding_name(&declarator.id) {
                                        insert_vue3_declared_type_name_with_budget(
                                            &mut names,
                                            name,
                                            namespace_budget,
                                        )?;
                                    }
                                }
                            }
                        }
                        Declaration::ClassDeclaration(declaration) => {
                            if let Some(id) = &declaration.id {
                                insert_vue3_declared_type_name_with_budget(
                                    &mut names,
                                    id.name.as_str(),
                                    namespace_budget,
                                )?;
                            }
                        }
                        Declaration::TSModuleDeclaration(declaration) => {
                            names.extend(vue3_namespace_exported_type_names_with_budget(
                                declaration,
                                namespace_budget,
                            )?);
                        }
                        _ => {}
                    }
                }
                if declaration.source.is_none() {
                    for specifier in &declaration.specifiers {
                        if let Some(exported) = module_export_name(specifier.exported()) {
                            insert_vue3_declared_type_name_with_budget(
                                &mut names,
                                exported,
                                namespace_budget,
                            )?;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Some(names)
}

fn project_vue3_exported_namespace_specifiers_with_budget(
    statements: &[Statement<'_>],
    ambient: bool,
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let mut namespace_members = BTreeMap::<String, BTreeSet<String>>::new();
    let mut exported_names = BTreeSet::new();
    for statement in statements {
        let Some(declaration) = vue3_namespace_declaration_from_statement(statement) else {
            continue;
        };
        let Some(namespace) = vue3_ts_module_declaration_name_ref(declaration) else {
            continue;
        };
        let names = vue3_namespace_visible_type_names_with_budget(
            declaration,
            ambient,
            namespace_budget,
        )?;
        if matches!(statement, Statement::ExportNamedDeclaration(_)) {
            for name in &names {
                insert_vue3_declared_type_name_with_budget(
                    &mut exported_names,
                    name,
                    namespace_budget,
                )?;
            }
        }
        if !namespace_members.contains_key(namespace) {
            if !namespace_budget.reserve(namespace.len().saturating_add(1)) {
                return None;
            }
            namespace_members.insert(namespace.to_string(), BTreeSet::new());
        }
        namespace_members
            .get_mut(namespace)
            .expect("namespace entry was inserted")
            .extend(names);
    }

    let mut aliases = BTreeSet::new();
    for statement in statements {
        let Statement::ExportNamedDeclaration(export) = statement else {
            continue;
        };
        if export.source.is_some() {
            continue;
        }
        for specifier in &export.specifiers {
            let Some(local) = module_export_name(specifier.local()) else {
                continue;
            };
            if !namespace_members.contains_key(local) {
                continue;
            }
            let Some(exported) = module_export_name(specifier.exported()) else {
                continue;
            };
            if !namespace_budget.reserve(
                local
                    .len()
                    .saturating_add(exported.len())
                    .saturating_add(1),
            ) {
                return None;
            }
            aliases.insert((local.to_string(), exported.to_string()));
        }
    }

    let mut projections = BTreeSet::new();
    for (local, exported) in aliases {
        let Some(source_names) = namespace_members.get(&local) else {
            continue;
        };
        for source_name in source_names {
            let Some(member_name) = source_name
                .strip_prefix(&local)
                .and_then(|suffix| suffix.strip_prefix('.'))
            else {
                continue;
            };
            let target_name = if local == exported {
                if !namespace_budget.reserve(source_name.len().saturating_add(1)) {
                    return None;
                }
                source_name.clone()
            } else {
                reserve_vue3_qualified_namespace_name(
                    &exported,
                    member_name,
                    namespace_budget,
                )?
            };
            if source_name != &target_name {
                if !namespace_budget.reserve(
                    source_name
                        .len()
                        .saturating_add(target_name.len())
                        .saturating_add(2),
                ) {
                    return None;
                }
                projections.insert((source_name.clone(), target_name.clone()));
            }
            exported_names.insert(target_name);
        }
    }

    let mut source_projection = Vue3ScriptSetupAnalysis::default();
    if !namespace_budget.reserve(
        projections
            .len()
            .saturating_mul(std::mem::size_of::<&String>()),
    ) {
        return None;
    }
    let source_names = projections
        .iter()
        .map(|(source_name, _)| source_name)
        .collect::<BTreeSet<_>>();
    for source_name in source_names {
        sync_vue3_namespace_type_projection(
            &mut source_projection,
            analysis,
            source_name,
            source_name,
            namespace_budget,
        )?;
    }
    for (source_name, target_name) in projections {
        sync_vue3_namespace_type_projection(
            analysis,
            &source_projection,
            &source_name,
            &target_name,
            namespace_budget,
        )?;
    }
    Some(exported_names)
}

pub(crate) fn vue3_default_export_may_be_type(declaration: &ExportDefaultDeclaration<'_>) -> bool {
    match &declaration.declaration {
        ExportDefaultDeclarationKind::TSInterfaceDeclaration(_)
        | ExportDefaultDeclarationKind::ClassDeclaration(_)
        | ExportDefaultDeclarationKind::Identifier(_) => true,
        ExportDefaultDeclarationKind::ObjectExpression(object) => {
            vue3_static_runtime_props_options_object_is_projectable(object)
        }
        ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
            vue3_function_has_return_projection(function)
        }
        declaration => {
            vue3_default_export_function_value_has_return_projection(declaration)
                || vue3_default_export_static_runtime_props_options_is_projectable(declaration)
        }
    }
}

pub(crate) fn project_vue3_default_type_exports(
    source: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for statement in statements {
        let Statement::ExportDefaultDeclaration(declaration) = statement else {
            continue;
        };
        match &declaration.declaration {
            ExportDefaultDeclarationKind::TSInterfaceDeclaration(declaration) => {
                let name = declaration.id.name.to_string();
                let deps = collect_vue3_interface_type_deps(declaration, analysis);
                register_vue3_interface_declaration(source, declaration, analysis);
                insert_vue3_declared_type_deps(analysis, &name, deps);
                insert_vue3_local_type_alias(analysis, &name, "default");
            }
            ExportDefaultDeclarationKind::Identifier(identifier) => {
                insert_vue3_local_type_alias(analysis, identifier.name.as_str(), "default");
            }
            ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                if let Some(id) = &function.id {
                    let name = id.name.as_str();
                    register_vue3_function_return_projection(source, name, function, analysis);
                    if let Some(return_type) = function.return_type.as_ref() {
                        let deps =
                            collect_vue3_type_argument_deps(&return_type.type_annotation, analysis);
                        insert_vue3_declared_type_deps(analysis, name, deps);
                    }
                    insert_vue3_local_type_alias(analysis, name, "default");
                } else {
                    register_vue3_function_return_projection(source, "default", function, analysis);
                    if let Some(return_type) = function.return_type.as_ref() {
                        let deps =
                            collect_vue3_type_argument_deps(&return_type.type_annotation, analysis);
                        insert_vue3_declared_type_deps(analysis, "default", deps);
                    }
                }
            }
            declaration
                if vue3_default_export_function_value_has_return_projection(declaration) =>
            {
                if let Some(expression) = declaration.as_expression() {
                    register_vue3_function_value_expression_return_projection(
                        source, "default", expression, analysis,
                    );
                    if let Some(return_type) = vue3_function_value_return_type(expression) {
                        let deps = collect_vue3_type_argument_deps(return_type, analysis);
                        insert_vue3_declared_type_deps(analysis, "default", deps);
                    }
                }
            }
            declaration
                if vue3_default_export_static_runtime_props_options_is_projectable(declaration) =>
            {
                register_vue3_default_static_runtime_props_options(source, declaration, analysis);
            }
            ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                if let Some(id) = &class.id {
                    register_vue3_class_type_name(analysis, id.name.as_str());
                    insert_vue3_local_type_alias(analysis, id.name.as_str(), "default");
                } else {
                    register_vue3_class_type_name(analysis, "default");
                }
            }
            _ => {}
        }
    }
}

fn project_vue3_exported_type_specifiers_with_budget(
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    let mut projections = BTreeSet::new();
    for statement in statements {
        let Statement::ExportNamedDeclaration(declaration) = statement else {
            continue;
        };
        if declaration.source.is_some() {
            continue;
        }
        for specifier in &declaration.specifiers {
            let Some(local) = module_export_name(specifier.local()) else {
                continue;
            };
            let Some(exported) = module_export_name(specifier.exported()) else {
                continue;
            };
            if local == exported {
                continue;
            }
            if !has_vue3_type_alias_projection(analysis, local) {
                continue;
            }
            if !namespace_budget.reserve(
                local
                    .len()
                    .saturating_add(exported.len())
                    .saturating_add(1),
            ) {
                return None;
            }
            projections.insert((local.to_string(), exported.to_string()));
        }
    }

    let mut source_projection = Vue3ScriptSetupAnalysis::default();
    if !namespace_budget.reserve(
        projections
            .len()
            .saturating_mul(std::mem::size_of::<&String>()),
    ) {
        return None;
    }
    let source_names = projections
        .iter()
        .map(|(source_name, _)| source_name)
        .collect::<BTreeSet<_>>();
    for source_name in source_names {
        sync_vue3_namespace_type_projection(
            &mut source_projection,
            analysis,
            source_name,
            source_name,
            namespace_budget,
        )?;
    }
    for (source_name, target_name) in projections {
        sync_vue3_namespace_type_projection(
            analysis,
            &source_projection,
            &source_name,
            &target_name,
            namespace_budget,
        )?;
    }
    Some(())
}

const VUE3_MAX_NAMESPACE_PROJECTION_DEPTH: usize = 64;
const VUE3_MAX_NAMESPACE_PROJECTION_WORK: usize = 16 * 1024 * 1024;

pub(crate) struct Vue3NamespaceProjectionBudget {
    remaining_work: usize,
    exhausted: bool,
}

impl Default for Vue3NamespaceProjectionBudget {
    fn default() -> Self {
        Self {
            remaining_work: VUE3_MAX_NAMESPACE_PROJECTION_WORK,
            exhausted: false,
        }
    }
}

impl Vue3NamespaceProjectionBudget {
    fn reserve(&mut self, work: usize) -> bool {
        let Some(remaining_work) = self.remaining_work.checked_sub(work) else {
            self.remaining_work = 0;
            self.exhausted = true;
            return false;
        };
        self.remaining_work = remaining_work;
        true
    }

    pub(crate) fn is_exhausted(&self) -> bool {
        self.exhausted
    }
}

fn sync_vue3_namespace_type_projection(
    target: &mut Vue3ScriptSetupAnalysis,
    source: &Vue3ScriptSetupAnalysis,
    source_name: &str,
    target_name: &str,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<bool> {
    if !namespace_budget.reserve(vue3_type_alias_projection_work(
        source,
        source_name,
        target_name,
    )) {
        return None;
    }
    Some(sync_vue3_type_alias_from_analysis(
        target,
        source,
        source_name,
        target_name,
    ))
}

pub(crate) fn project_vue3_namespace_declaration(
    source: &str,
    declaration: &TSModuleDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let Some(namespace) = vue3_ts_module_declaration_name(declaration) else {
        return;
    };
    let mut namespace_budget = Vue3NamespaceProjectionBudget::default();
    if !validate_vue3_namespace_declaration_structure(
        declaration,
        1,
        &mut namespace_budget,
    ) {
        return;
    }
    let Some(public_names) = vue3_namespace_visible_type_names_with_budget(
        declaration,
        false,
        &mut namespace_budget,
    ) else {
        return;
    };
    let mergeable_scan_work = public_names.iter().fold(0usize, |work, name| {
        work.saturating_add(name.len()).saturating_add(1)
    });
    if !namespace_budget.reserve(mergeable_scan_work.saturating_mul(3)) {
        return;
    }
    let mergeable_names = vue3_namespace_visible_mergeable_names(
        declaration,
        false,
        Vue3NamespaceMergeKind::Interface,
    )
    .into_iter()
    .chain(vue3_namespace_visible_mergeable_names(
        declaration,
        false,
        Vue3NamespaceMergeKind::Enum,
    ))
    .chain(vue3_namespace_visible_mergeable_names(
        declaration,
        false,
        Vue3NamespaceMergeKind::Class,
    ))
    .collect::<BTreeSet<_>>();
    let mut projection = Vue3ScriptSetupAnalysis::default();
    project_vue3_namespace_declaration_with_prefix(
        source,
        declaration,
        &namespace,
        declaration.declare,
        1,
        &mergeable_names,
        analysis,
        &mut projection,
        &mut namespace_budget,
    );
    if namespace_budget.exhausted {
        return;
    }
    let final_projection_work = public_names.iter().fold(0usize, |work, name| {
        work.saturating_add(vue3_type_alias_projection_work(
            &projection,
            name,
            name,
        ))
    });
    if !namespace_budget.reserve(final_projection_work) {
        return;
    }
    for name in public_names {
        sync_vue3_type_alias_from_analysis(analysis, &projection, &name, &name);
    }
}

pub(crate) fn project_vue3_namespace_groups_from_statements_with_budget(
    source: &str,
    statements: &[Statement<'_>],
    ambient: bool,
    namespace_depth: usize,
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    project_vue3_namespace_groups_from_statement_groups_with_budget(
        source,
        &[statements],
        ambient,
        namespace_depth,
        analysis,
        namespace_budget,
    )
}

pub(crate) fn project_vue3_namespace_groups_from_statement_groups_with_budget(
    source: &str,
    statement_groups: &[&[Statement<'_>]],
    ambient: bool,
    namespace_depth: usize,
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    if !namespace_budget.reserve(vue3_type_analysis_clone_work(analysis)) {
        return false;
    }
    let mut working_analysis = analysis.clone();
    let Some(changed) =
        converge_vue3_namespace_groups_from_statement_groups_in_place_with_budget(
            source,
            statement_groups,
            ambient,
            namespace_depth,
            &mut working_analysis,
            namespace_budget,
        )
    else {
        namespace_budget.exhausted = true;
        return false;
    };
    *analysis = working_analysis;
    changed
}

fn converge_vue3_namespace_groups_from_statement_groups_in_place_with_budget(
    source: &str,
    statement_groups: &[&[Statement<'_>]],
    ambient: bool,
    namespace_depth: usize,
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<bool> {
    for statements in statement_groups {
        if !validate_vue3_namespace_structure(statements, namespace_depth, namespace_budget) {
            return None;
        }
    }
    let namespace_steps = statement_groups
        .iter()
        .fold(0usize, |steps, statements| {
            steps.saturating_add(count_vue3_namespace_projection_steps(statements))
        });
    let refresh_steps = statement_groups
        .iter()
        .fold(0usize, |count, statements| {
            count.saturating_add(count_vue3_refreshable_type_declarations_in_statements(
                statements,
            ))
        });
    if namespace_steps == 0 && refresh_steps == 0 {
        return Some(false);
    }
    for statements in statement_groups {
        if !seed_vue3_namespace_public_type_names(
            statements,
            ambient,
            analysis,
            namespace_budget,
        ) {
            return None;
        }
    }
    let limit = namespace_steps
        .saturating_add(refresh_steps)
        .saturating_add(1);
    let mut converged = false;
    let mut any_changed = false;
    for _ in 0..limit {
        let statement_count = statement_groups
            .iter()
            .fold(1usize, |count, statements| {
                count.saturating_add(statements.len())
            });
        let outer_work = statement_count.saturating_mul(statement_count);
        if !namespace_budget.reserve(outer_work) {
            return None;
        }
        let mut changed = project_vue3_namespace_groups_from_statement_groups_once(
            source,
            statement_groups,
            ambient,
            namespace_depth,
            analysis,
            namespace_budget,
        );
        if namespace_budget.exhausted {
            return None;
        }
        changed |= refresh_vue3_declared_type_declarations_from_statement_groups_once(
            source,
            statement_groups,
            analysis,
        );
        changed |= collect_vue3_declared_type_deps_from_statement_groups(
            statement_groups,
            analysis,
        );
        if analysis.type_dependency_work_exhausted {
            namespace_budget.exhausted = true;
            return None;
        }
        any_changed |= changed;
        if !changed {
            converged = true;
            break;
        }
    }
    if converged {
        Some(any_changed)
    } else {
        None
    }
}

fn project_vue3_namespace_groups_from_statement_groups_once(
    source: &str,
    statement_groups: &[&[Statement<'_>]],
    ambient: bool,
    namespace_depth: usize,
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    let mut groups = BTreeMap::<String, Vec<&TSModuleDeclaration<'_>>>::new();
    for statements in statement_groups {
        for statement in *statements {
            let declaration = match statement {
                Statement::TSModuleDeclaration(declaration)
                    if !vue3_ts_module_declaration_is_global(declaration) =>
                {
                    Some(declaration)
                }
                Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
                    Some(Declaration::TSModuleDeclaration(declaration))
                        if !vue3_ts_module_declaration_is_global(declaration) =>
                    {
                        Some(declaration)
                    }
                    _ => None,
                },
                _ => None,
            };
            let Some(declaration) = declaration else {
                continue;
            };
            let Some(namespace) = vue3_ts_module_declaration_name(declaration) else {
                continue;
            };
            groups.entry(namespace).or_default().push(declaration);
        }
    }

    let mut changed = false;
    for (namespace, declarations) in groups {
        let mut projections = Vec::with_capacity(declarations.len());
        let mut contribution_indexes = BTreeMap::<String, Vec<usize>>::new();
        for declaration in declarations {
            let Some(declaration_public_names) = vue3_namespace_visible_type_names_with_budget(
                declaration,
                ambient,
                namespace_budget,
            ) else {
                return changed;
            };
            let mergeable_scan_work =
                declaration_public_names
                    .iter()
                    .fold(0usize, |work, name| {
                        work.saturating_add(name.len()).saturating_add(1)
                    });
            if !namespace_budget.reserve(mergeable_scan_work.saturating_mul(3)) {
                return changed;
            }
            let interface_names = vue3_namespace_visible_mergeable_names(
                declaration,
                ambient,
                Vue3NamespaceMergeKind::Interface,
            );
            let enum_names = vue3_namespace_visible_mergeable_names(
                declaration,
                ambient,
                Vue3NamespaceMergeKind::Enum,
            );
            let class_names = vue3_namespace_visible_mergeable_names(
                declaration,
                ambient,
                Vue3NamespaceMergeKind::Class,
            );
            let mergeable_names = interface_names
                .iter()
                .chain(&enum_names)
                .chain(&class_names)
                .cloned()
                .collect::<BTreeSet<_>>();
            let mut projection = Vue3ScriptSetupAnalysis::default();
            project_vue3_namespace_declaration_with_prefix(
                source,
                declaration,
                &namespace,
                ambient || declaration.declare,
                namespace_depth.saturating_add(1),
                &mergeable_names,
                analysis,
                &mut projection,
                namespace_budget,
            );
            let projection_index = projections.len();
            for name in &declaration_public_names {
                contribution_indexes
                    .entry(name.clone())
                    .or_default()
                    .push(projection_index);
            }
            projections.push(Vue3NamespaceBlockProjection {
                interface_names,
                enum_names,
                class_names,
                analysis: projection,
            });
            if namespace_budget.exhausted {
                return changed;
            }
        }

        for (name, indexes) in contribution_indexes {
            let contributors = indexes
                .iter()
                .map(|index| &projections[*index])
                .collect::<Vec<_>>();
            let interface_contributors = contributors
                .iter()
                .copied()
                .filter(|projection| projection.interface_names.contains(&name))
                .collect::<Vec<_>>();
            let enum_contributors = contributors
                .iter()
                .copied()
                .filter(|projection| projection.enum_names.contains(&name))
                .collect::<Vec<_>>();
            let class_contributors = contributors
                .iter()
                .copied()
                .filter(|projection| projection.class_names.contains(&name))
                .collect::<Vec<_>>();
            let merges_interfaces = interface_contributors.len() > 1
                && interface_contributors.len() == contributors.len();
            let merges_class_and_interfaces = !interface_contributors.is_empty()
                && class_contributors.len() == 1
                && interface_contributors.len().saturating_add(class_contributors.len())
                    == contributors.len();
            if merges_interfaces || merges_class_and_interfaces {
                for contributor in &contributors {
                    if !namespace_budget.reserve(vue3_type_alias_projection_work(
                        &contributor.analysis,
                        &name,
                        &name,
                    )) {
                        return changed;
                    }
                }
                let merged = merge_vue3_namespace_declaration_projections(&contributors, &name);
                let Some(sync_changed) = sync_vue3_namespace_type_projection(
                    analysis,
                    &merged,
                    &name,
                    &name,
                    namespace_budget,
                ) else {
                    return changed;
                };
                changed |= sync_changed;
            } else if enum_contributors.len() > 1
                && enum_contributors.len() == contributors.len()
            {
                for contributor in &enum_contributors {
                    if !namespace_budget.reserve(vue3_type_alias_projection_work(
                        &contributor.analysis,
                        &name,
                        &name,
                    )) {
                        return changed;
                    }
                }
                let merged =
                    merge_vue3_namespace_declaration_projections(&enum_contributors, &name);
                let Some(sync_changed) = sync_vue3_namespace_type_projection(
                    analysis,
                    &merged,
                    &name,
                    &name,
                    namespace_budget,
                ) else {
                    return changed;
                };
                changed |= sync_changed;
                changed |= analysis.local_ts_enum_type_names.insert(name.clone());
            } else if let Some(projection) = contributors.last() {
                let Some(sync_changed) = sync_vue3_namespace_type_projection(
                    analysis,
                    &projection.analysis,
                    &name,
                    &name,
                    namespace_budget,
                ) else {
                    return changed;
                };
                changed |= sync_changed;
            }
        }
    }
    changed
}

struct Vue3NamespaceBlockProjection {
    interface_names: BTreeSet<String>,
    enum_names: BTreeSet<String>,
    class_names: BTreeSet<String>,
    analysis: Vue3ScriptSetupAnalysis,
}

fn project_vue3_namespace_declaration_with_prefix(
    source: &str,
    declaration: &TSModuleDeclaration<'_>,
    prefix: &str,
    ambient: bool,
    namespace_depth: usize,
    mergeable_names: &BTreeSet<String>,
    analysis: &Vue3ScriptSetupAnalysis,
    projection: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    if namespace_depth > VUE3_MAX_NAMESPACE_PROJECTION_DEPTH {
        namespace_budget.exhausted = true;
        return false;
    }
    let Some(body) = declaration.body.as_ref() else {
        return false;
    };
    match body {
        TSModuleDeclarationBody::TSModuleBlock(block) => {
            let statement_count = block.body.len().saturating_add(1);
            let block_work = statement_count
                .saturating_mul(statement_count)
                .saturating_mul(2);
            if !namespace_budget.reserve(block_work) {
                return false;
            }
            let Some(referenced_names) =
                vue3_namespace_referenced_names(&block.body, namespace_budget)
            else {
                return false;
            };
            let Some(mut namespace_analysis) = vue3_namespace_child_analysis(
                analysis,
                &referenced_names,
                namespace_budget,
            ) else {
                return false;
            };
            for local_name in &referenced_names {
                let public_name = format!("{prefix}.{local_name}");
                if !has_vue3_type_alias_projection(analysis, &public_name) {
                    continue;
                }
                if sync_vue3_namespace_type_projection(
                    &mut namespace_analysis,
                    analysis,
                    &public_name,
                    local_name,
                    namespace_budget,
                )
                .is_none()
                {
                    return false;
                }
                if analysis.local_ts_enum_type_names.contains(&public_name) {
                    namespace_analysis
                        .local_ts_enum_type_names
                        .insert(local_name.clone());
                }
            }
            if !seed_vue3_namespace_type_names(
                prefix,
                &block.body,
                &mut namespace_analysis,
                namespace_budget,
            ) {
                return false;
            }
            collect_vue3_declared_types_from_statements_with_namespace_budget(
                source,
                &block.body,
                ambient,
                namespace_depth,
                &mut namespace_analysis,
                namespace_budget,
            );
            if namespace_budget.exhausted {
                return false;
            }
            collect_vue3_declared_type_deps_from_statements(&block.body, &mut namespace_analysis);
            if namespace_analysis.type_dependency_work_exhausted {
                namespace_budget.exhausted = true;
                return false;
            }
            if !namespace_budget.reserve(vue3_local_generic_scope_capture_work(
                &namespace_analysis,
            )) {
                return false;
            }
            finalize_vue3_local_generic_alias_scopes(&mut namespace_analysis);
            let local_mergeable_names = mergeable_names
                .iter()
                .filter_map(|name| name.strip_prefix(&format!("{prefix}.")))
                .filter(|name| !name.contains('.'))
                .map(str::to_string)
                .collect::<BTreeSet<_>>();
            let excluded_interfaces = local_mergeable_names
                .iter()
                .filter(|name| !namespace_analysis.local_ts_enum_type_names.contains(*name))
                .cloned()
                .collect::<BTreeSet<_>>();
            let mut local_mergeable_projection = Vue3ScriptSetupAnalysis::default();
            for name in &local_mergeable_names {
                if sync_vue3_namespace_type_projection(
                    &mut local_mergeable_projection,
                    &namespace_analysis,
                    name,
                    name,
                    namespace_budget,
                )
                .is_none()
                {
                    return false;
                }
            }
            for name in &local_mergeable_names {
                let public_name = format!("{prefix}.{name}");
                if !has_vue3_type_alias_projection(analysis, &public_name) {
                    continue;
                }
                if sync_vue3_namespace_type_projection(
                    &mut namespace_analysis,
                    analysis,
                    &public_name,
                    name,
                    namespace_budget,
                )
                .is_none()
                {
                    return false;
                }
                if analysis.local_ts_enum_type_names.contains(&public_name) {
                    namespace_analysis.local_ts_enum_type_names.insert(name.clone());
                }
            }
            refresh_vue3_declared_type_declarations_excluding_interfaces(
                source,
                &block.body,
                &excluded_interfaces,
                &mut namespace_analysis,
            );
            collect_vue3_declared_type_deps_from_statement_groups_excluding_names(
                &[&block.body],
                &local_mergeable_names,
                &mut namespace_analysis,
            );
            if namespace_analysis.type_dependency_work_exhausted {
                namespace_budget.exhausted = true;
                return false;
            }
            if !namespace_budget.reserve(vue3_local_generic_scope_capture_work(
                &namespace_analysis,
            )) {
                return false;
            }
            finalize_vue3_local_generic_alias_scopes(&mut namespace_analysis);
            let names = if ambient {
                let Some(names) = vue3_declared_type_names_from_statements_with_budget(
                    &block.body,
                    namespace_budget,
                ) else {
                    return false;
                };
                names
            } else {
                let Some(names) =
                    vue3_exported_type_names_with_budget(&block.body, namespace_budget)
                else {
                    return false;
                };
                names
            };
            let mut changed = false;
            for name in names {
                let prefixed = format!("{prefix}.{name}");
                let source_analysis = if local_mergeable_names.contains(&name) {
                    &local_mergeable_projection
                } else {
                    &namespace_analysis
                };
                let Some(sync_changed) = sync_vue3_namespace_type_projection(
                    projection,
                    source_analysis,
                    &name,
                    &prefixed,
                    namespace_budget,
                ) else {
                    return false;
                };
                changed |= sync_changed;
            }
            changed
        }
        TSModuleDeclarationBody::TSModuleDeclaration(nested) => {
            let Some(name) = vue3_ts_module_declaration_name(nested) else {
                return false;
            };
            let prefix = format!("{prefix}.{name}");
            project_vue3_namespace_declaration_with_prefix(
                source,
                nested,
                &prefix,
                ambient || nested.declare,
                namespace_depth.saturating_add(1),
                mergeable_names,
                analysis,
                projection,
                namespace_budget,
            )
        }
    }
}

fn vue3_namespace_child_analysis(
    analysis: &Vue3ScriptSetupAnalysis,
    referenced_names: &BTreeSet<String>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<Vue3ScriptSetupAnalysis> {
    let mut child = Vue3ScriptSetupAnalysis {
        type_filename: analysis.type_filename.clone(),
        type_resolution_mode: analysis.type_resolution_mode,
        type_seen: analysis.type_seen.clone(),
        type_resolver: analysis.type_resolver.clone(),
        ..Vue3ScriptSetupAnalysis::default()
    };
    let captures_generic_aliases = referenced_names
        .iter()
        .any(|name| analysis.generic_type_aliases.contains_key(name));
    let captured_aliases = if captures_generic_aliases {
        if !namespace_budget.reserve(vue3_generic_alias_capture_work(
            analysis,
            referenced_names,
        )) {
            return None;
        }
        captured_vue3_generic_aliases_for_child_scope(analysis, referenced_names)
    } else {
        BTreeMap::new()
    };
    for name in referenced_names {
        sync_vue3_namespace_type_projection(
            &mut child,
            analysis,
            name,
            name,
            namespace_budget,
        )?;
    }
    child.generic_type_aliases.extend(captured_aliases);
    Some(child)
}

fn vue3_namespace_referenced_names(
    statements: &[Statement<'_>],
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let mut collector = Vue3NamespaceReferenceCollector {
        names: BTreeSet::new(),
        namespace_path: Vec::new(),
        namespace_budget,
    };
    for statement in statements {
        oxc_ast_visit::Visit::visit_statement(&mut collector, statement);
        if collector.namespace_budget.exhausted {
            return None;
        }
    }
    Some(collector.names)
}

struct Vue3NamespaceReferenceCollector<'budget> {
    names: BTreeSet<String>,
    namespace_path: Vec<String>,
    namespace_budget: &'budget mut Vue3NamespaceProjectionBudget,
}

impl<'a> oxc_ast_visit::Visit<'a> for Vue3NamespaceReferenceCollector<'_> {
    fn visit_identifier_reference(
        &mut self,
        identifier: &oxc_ast::ast::IdentifierReference<'a>,
    ) {
        if self.namespace_budget.exhausted {
            return;
        }
        self.insert_name(identifier.name.as_str());
        if self.namespace_budget.exhausted {
            return;
        }
        oxc_ast_visit::walk::walk_identifier_reference(self, identifier);
    }

    fn visit_ts_type_reference(&mut self, reference: &TSTypeReference<'a>) {
        if self.namespace_budget.exhausted {
            return;
        }
        if let Some(name) = vue3_ts_type_name_key(&reference.type_name) {
            self.insert_name(&name);
        }
        if self.namespace_budget.exhausted {
            return;
        }
        oxc_ast_visit::walk::walk_ts_type_reference(self, reference);
    }

    fn visit_ts_type_query(&mut self, query: &TSTypeQuery<'a>) {
        if self.namespace_budget.exhausted {
            return;
        }
        if let Some(name) = vue3_type_query_name_key(query) {
            self.insert_name(&name);
        }
        if self.namespace_budget.exhausted {
            return;
        }
        oxc_ast_visit::walk::walk_ts_type_query(self, query);
    }

    fn visit_ts_interface_heritage(&mut self, heritage: &TSInterfaceHeritage<'a>) {
        if self.namespace_budget.exhausted {
            return;
        }
        if let Some(name) = vue3_interface_heritage_name(heritage) {
            self.insert_name(&name);
        }
        if self.namespace_budget.exhausted {
            return;
        }
        oxc_ast_visit::walk::walk_ts_interface_heritage(self, heritage);
    }

    fn visit_ts_module_declaration(&mut self, declaration: &TSModuleDeclaration<'a>) {
        if self.namespace_budget.exhausted {
            return;
        }
        let Some(name) = vue3_ts_module_declaration_name(declaration) else {
            oxc_ast_visit::walk::walk_ts_module_declaration(self, declaration);
            return;
        };
        self.namespace_path.push(name);
        oxc_ast_visit::walk::walk_ts_module_declaration(self, declaration);
        self.namespace_path.pop();
    }
}

impl Vue3NamespaceReferenceCollector<'_> {
    fn insert_name(&mut self, name: &str) {
        if !self.namespace_budget.reserve(name.len().saturating_add(1)) {
            return;
        }
        self.names.insert(name.to_string());
        for length in 1..=self.namespace_path.len() {
            let path_length = self.namespace_path[..length]
                .iter()
                .fold(0usize, |size, segment| size.saturating_add(segment.len()))
                .saturating_add(length.saturating_sub(1));
            let qualified_length = path_length.saturating_add(name.len()).saturating_add(1);
            if !self
                .namespace_budget
                .reserve(qualified_length.saturating_add(1))
            {
                return;
            }
            let mut qualified = String::with_capacity(qualified_length);
            for segment in &self.namespace_path[..length] {
                if !qualified.is_empty() {
                    qualified.push('.');
                }
                qualified.push_str(segment);
            }
            qualified.push('.');
            qualified.push_str(name);
            self.names.insert(qualified);
        }
    }
}

fn merge_vue3_namespace_declaration_projections(
    projections: &[&Vue3NamespaceBlockProjection],
    name: &str,
) -> Vue3ScriptSetupAnalysis {
    let Some(last) = projections.last() else {
        return Vue3ScriptSetupAnalysis::default();
    };
    let mut merged = Vue3ScriptSetupAnalysis::default();
    sync_vue3_type_alias_from_analysis(&mut merged, &last.analysis, name, name);

    macro_rules! merge_vector_entry {
        ($field:ident) => {{
            let mut found = false;
            let mut values = Vec::new();
            let mut seen = BTreeSet::new();
            for projection in projections {
                if let Some(source_values) = projection.analysis.$field.get(name) {
                    found = true;
                    for value in source_values {
                        if seen.insert(value) {
                            values.push(value.clone());
                        }
                    }
                }
            }
            if found {
                merged.$field.insert(name.to_string(), values);
            } else {
                merged.$field.remove(name);
            }
        }};
    }

    macro_rules! merge_set_entry {
        ($field:ident) => {{
            let mut found = false;
            let mut values = BTreeSet::new();
            for projection in projections {
                if let Some(source_values) = projection.analysis.$field.get(name) {
                    found = true;
                    values.extend(source_values.iter().cloned());
                }
            }
            if found {
                merged.$field.insert(name.to_string(), values);
            } else {
                merged.$field.remove(name);
            }
        }};
    }

    macro_rules! merge_members_entry {
        ($field:ident) => {{
            let mut values = projections
                .iter()
                .filter_map(|projection| projection.analysis.$field.get(name).cloned())
                .collect::<Vec<_>>();
            if values.is_empty() {
                merged.$field.remove(name);
            } else {
                let mut source_parts = Vec::new();
                let mut seen_sources = BTreeSet::new();
                for value in &values {
                    if seen_sources.insert(value.source.as_str()) {
                        source_parts.push(value.source.clone());
                    }
                }
                let interface_heritage =
                    vue3_take_and_merge_interface_heritage_evidence(&mut values);
                let (members, errors) = vue3_merge_props_type_members(values, false);
                merged.$field.insert(
                    name.to_string(),
                    Vue27TypeMembers {
                        source: source_parts.join("\n"),
                        members,
                        errors,
                        interface_heritage,
                    },
                );
            }
        }};
    }

    merge_vector_entry!(declared_types);
    merge_vector_entry!(define_model_declared_types);
    merge_vector_entry!(type_query_declared_types);
    merge_vector_entry!(define_model_type_query_declared_types);
    merge_vector_entry!(keyof_type_query_declared_types);
    merge_members_entry!(props_type_declarations);
    merge_vector_entry!(keyof_runtime_type_declarations);
    merge_vector_entry!(tuple_runtime_type_declarations);
    merge_vector_entry!(define_model_tuple_runtime_type_declarations);
    merge_vector_entry!(array_element_runtime_type_declarations);
    merge_vector_entry!(define_model_array_element_runtime_type_declarations);
    merge_vector_entry!(parameter_tuple_runtime_type_declarations);
    merge_vector_entry!(define_model_parameter_tuple_runtime_type_declarations);
    merge_vector_entry!(constructor_parameter_tuple_runtime_type_declarations);
    merge_vector_entry!(define_model_constructor_parameter_tuple_runtime_type_declarations);
    merge_vector_entry!(return_type_runtime_type_declarations);
    merge_vector_entry!(define_model_return_type_runtime_type_declarations);
    merge_members_entry!(props_options_type_declarations);
    merge_members_entry!(return_type_props_options_declarations);
    let aliases = projections
        .iter()
        .filter_map(|projection| projection.analysis.generic_type_aliases.get(name))
        .collect::<Vec<_>>();
    let interface_contributor_count = projections
        .iter()
        .filter(|projection| projection.interface_names.contains(name))
        .count();
    if !aliases.is_empty() && aliases.len() == interface_contributor_count {
        let first = aliases[0];
        if first.kind == Vue3GenericTypeAliasKind::Interface
            && aliases
                .iter()
                .all(|alias| alias.kind == first.kind && alias.params == first.params)
        {
            let mut alias = aliases[aliases.len() - 1].clone();
            let mut fragments = Vec::new();
            for contributor in aliases {
                if contributor.interface_fragments.is_empty() {
                    fragments.push(Vue3GenericInterfaceFragment {
                        source: contributor.source.clone(),
                        scope: contributor.scope.clone(),
                    });
                } else {
                    fragments.extend(contributor.interface_fragments.iter().cloned());
                }
            }
            alias.source.clear();
            alias.interface_fragments = fragments;
            merged.generic_type_aliases.insert(name.to_string(), alias);
        }
    }
    merge_set_entry!(string_literal_type_declarations);
    merge_vector_entry!(ordered_string_literal_type_declarations);
    merge_vector_entry!(type_direct_deps);
    merge_set_entry!(type_deps);

    let emits = projections
        .iter()
        .filter_map(|projection| projection.analysis.emits_type_declarations.get(name))
        .collect::<Vec<_>>();
    if emits.is_empty() {
        merged.emits_type_declarations.remove(name);
    } else {
        let mut source_parts = Vec::new();
        let mut seen_sources = BTreeSet::new();
        let mut events = Vec::new();
        let mut seen_events = BTreeSet::new();
        let mut syntax = Vue3EmitsTypeSyntax::default();
        let mut call_count = 0usize;
        for emit in emits {
            if seen_sources.insert(emit.source.as_str()) {
                source_parts.push(emit.source.clone());
            }
            for event in &emit.events {
                if seen_events.insert(event.as_str()) {
                    events.push(event.clone());
                }
            }
            syntax.has_call_signature |= emit.syntax.has_call_signature;
            syntax.has_property |= emit.syntax.has_property;
            call_count = call_count.saturating_add(emit.call_count);
        }
        merged.emits_type_declarations.insert(
            name.to_string(),
            Vue27EmitsType {
                source: source_parts.join("\n"),
                events,
                syntax,
                call_count,
            },
        );
    }

    if projections
        .iter()
        .any(|projection| projection.analysis.silent_unresolved_type_names.contains(name))
    {
        merged.silent_unresolved_type_names.insert(name.to_string());
    } else {
        merged.silent_unresolved_type_names.remove(name);
    }
    merged
}

fn validate_vue3_namespace_structure(
    statements: &[Statement<'_>],
    parent_depth: usize,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    let roots = statements
        .iter()
        .filter_map(vue3_namespace_declaration_from_statement)
        .map(|declaration| (declaration, parent_depth.saturating_add(1)))
        .collect::<Vec<_>>();
    validate_vue3_namespace_declarations(roots, namespace_budget)
}

fn validate_vue3_namespace_declaration_structure(
    declaration: &TSModuleDeclaration<'_>,
    depth: usize,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    validate_vue3_namespace_declarations(vec![(declaration, depth)], namespace_budget)
}

fn validate_vue3_namespace_declarations<'a>(
    mut pending: Vec<(&'a TSModuleDeclaration<'a>, usize)>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    while let Some((declaration, depth)) = pending.pop() {
        if depth > VUE3_MAX_NAMESPACE_PROJECTION_DEPTH || !namespace_budget.reserve(1) {
            namespace_budget.exhausted = true;
            return false;
        }
        match declaration.body.as_ref() {
            Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                pending.push((nested, depth.saturating_add(1)));
            }
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                pending.extend(
                    block
                        .body
                        .iter()
                        .filter_map(vue3_namespace_declaration_from_statement)
                        .map(|nested| (nested, depth.saturating_add(1))),
                );
            }
            None => {}
        }
    }
    true
}

fn count_vue3_namespace_projection_steps(statements: &[Statement<'_>]) -> usize {
    let mut pending = statements
        .iter()
        .filter_map(vue3_namespace_declaration_from_statement)
        .collect::<Vec<_>>();
    let mut count = 0usize;
    while let Some(declaration) = pending.pop() {
        count = count.saturating_add(1);
        match declaration.body.as_ref() {
            Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                pending.push(nested);
            }
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                for statement in &block.body {
                    if let Some(nested) = vue3_namespace_declaration_from_statement(statement) {
                        pending.push(nested);
                    } else {
                        count = count.saturating_add(
                            vue3_namespace_projection_statement_step_bound(statement),
                        );
                    }
                }
            }
            None => {}
        }
    }
    count
}

fn vue3_namespace_projection_statement_step_bound(statement: &Statement<'_>) -> usize {
    match statement {
        Statement::VariableDeclaration(declaration) => declaration.declarations.len(),
        Statement::ExportNamedDeclaration(export) => export.declaration.as_ref().map_or(0, |decl| {
            if let Declaration::VariableDeclaration(declaration) = decl {
                declaration.declarations.len()
            } else {
                1
            }
        }),
        Statement::TSInterfaceDeclaration(_)
        | Statement::TSTypeAliasDeclaration(_)
        | Statement::TSEnumDeclaration(_)
        | Statement::FunctionDeclaration(_)
        | Statement::ClassDeclaration(_) => 1,
        _ => 0,
    }
}

pub(crate) fn seed_vue3_namespace_public_type_names(
    statements: &[Statement<'_>],
    ambient: bool,
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    for statement in statements {
        let Some(declaration) = vue3_namespace_declaration_from_statement(statement) else {
            continue;
        };
        let Some(names) =
            vue3_namespace_visible_type_names_with_budget(declaration, ambient, namespace_budget)
        else {
            return false;
        };
        let seed_work = names.iter().fold(0usize, |work, name| {
            work.saturating_add(name.len()).saturating_add(1)
        });
        if !namespace_budget.reserve(seed_work.saturating_mul(2)) {
            return false;
        }
        seed_vue3_qualified_type_names(
            names,
            analysis,
        );
    }
    true
}

fn vue3_namespace_declaration_from_statement<'a>(
    statement: &'a Statement<'a>,
) -> Option<&'a TSModuleDeclaration<'a>> {
    match statement {
        Statement::TSModuleDeclaration(declaration)
            if !vue3_ts_module_declaration_is_global(declaration) =>
        {
            Some(declaration)
        }
        Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
            Some(Declaration::TSModuleDeclaration(declaration))
                if !vue3_ts_module_declaration_is_global(declaration) =>
            {
                Some(declaration)
            }
            _ => None,
        },
        _ => None,
    }
}

fn vue3_namespace_visible_type_names_with_budget(
    declaration: &TSModuleDeclaration<'_>,
    ambient: bool,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    if ambient || declaration.declare {
        vue3_namespace_declared_type_names_with_budget(declaration, namespace_budget)
    } else {
        vue3_namespace_exported_type_names_with_budget(declaration, namespace_budget)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Vue3NamespaceMergeKind {
    Interface,
    Enum,
    Class,
}

fn vue3_namespace_visible_mergeable_names(
    declaration: &TSModuleDeclaration<'_>,
    ambient: bool,
    kind: Vue3NamespaceMergeKind,
) -> BTreeSet<String> {
    let Some(namespace) = vue3_ts_module_declaration_name(declaration) else {
        return BTreeSet::new();
    };
    vue3_namespace_visible_mergeable_names_with_prefix(
        declaration,
        &namespace,
        ambient || declaration.declare,
        kind,
    )
}

fn vue3_namespace_visible_mergeable_names_with_prefix(
    declaration: &TSModuleDeclaration<'_>,
    prefix: &str,
    ambient: bool,
    kind: Vue3NamespaceMergeKind,
) -> BTreeSet<String> {
    let Some(body) = declaration.body.as_ref() else {
        return BTreeSet::new();
    };
    match body {
        TSModuleDeclarationBody::TSModuleBlock(block) => vue3_visible_mergeable_names_from_statements(
            &block.body,
            prefix,
            ambient,
            kind,
        ),
        TSModuleDeclarationBody::TSModuleDeclaration(nested) => {
            let Some(name) = vue3_ts_module_declaration_name(nested) else {
                return BTreeSet::new();
            };
            let prefix = format!("{prefix}.{name}");
            vue3_namespace_visible_mergeable_names_with_prefix(
                nested,
                &prefix,
                ambient || nested.declare,
                kind,
            )
        }
    }
}

fn vue3_visible_mergeable_names_from_statements(
    statements: &[Statement<'_>],
    prefix: &str,
    ambient: bool,
    kind: Vue3NamespaceMergeKind,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for statement in statements {
        match statement {
            Statement::TSInterfaceDeclaration(declaration)
                if ambient && kind == Vue3NamespaceMergeKind::Interface =>
            {
                names.insert(format!("{prefix}.{}", declaration.id.name));
            }
            Statement::TSEnumDeclaration(declaration)
                if ambient && kind == Vue3NamespaceMergeKind::Enum =>
            {
                names.insert(format!("{prefix}.{}", declaration.id.name));
            }
            Statement::ClassDeclaration(declaration)
                if ambient && kind == Vue3NamespaceMergeKind::Class =>
            {
                if let Some(id) = &declaration.id {
                    names.insert(format!("{prefix}.{}", id.name));
                }
            }
            Statement::TSModuleDeclaration(declaration) if ambient => {
                let Some(name) = vue3_ts_module_declaration_name(declaration) else {
                    continue;
                };
                names.extend(vue3_namespace_visible_mergeable_names_with_prefix(
                    declaration,
                    &format!("{prefix}.{name}"),
                    true,
                    kind,
                ));
            }
            Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
                Some(Declaration::TSInterfaceDeclaration(declaration))
                    if kind == Vue3NamespaceMergeKind::Interface =>
                {
                    names.insert(format!("{prefix}.{}", declaration.id.name));
                }
                Some(Declaration::TSEnumDeclaration(declaration))
                    if kind == Vue3NamespaceMergeKind::Enum =>
                {
                    names.insert(format!("{prefix}.{}", declaration.id.name));
                }
                Some(Declaration::ClassDeclaration(declaration))
                    if kind == Vue3NamespaceMergeKind::Class =>
                {
                    if let Some(id) = &declaration.id {
                        names.insert(format!("{prefix}.{}", id.name));
                    }
                }
                Some(Declaration::TSModuleDeclaration(declaration)) => {
                    let Some(name) = vue3_ts_module_declaration_name(declaration) else {
                        continue;
                    };
                    names.extend(vue3_namespace_visible_mergeable_names_with_prefix(
                        declaration,
                        &format!("{prefix}.{name}"),
                        ambient || declaration.declare,
                        kind,
                    ));
                }
                _ => {}
            },
            _ => {}
        }
    }
    names
}

pub(crate) fn seed_vue3_namespace_type_names(
    prefix: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    let Some(names) =
        vue3_declared_type_names_from_statements_with_budget(statements, namespace_budget)
    else {
        return false;
    };
    for name in names {
        let Some(prefixed) =
            reserve_vue3_qualified_namespace_name(prefix, &name, namespace_budget)
        else {
            return false;
        };
        let key_work = name
            .len()
            .saturating_add(1)
            .saturating_add(prefixed.len())
            .saturating_add(1)
            .saturating_mul(2);
        if !namespace_budget.reserve(key_work) {
            return false;
        }
        for candidate in [name, prefixed] {
            analysis
                .declared_types
                .entry(candidate.clone())
                .or_insert_with(|| vec!["Object".into()]);
            analysis
                .define_model_declared_types
                .entry(candidate)
                .or_insert_with(|| vec!["Object".into()]);
        }
    }
    true
}

pub(crate) fn seed_vue3_qualified_type_names(
    names: BTreeSet<String>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for name in names {
        analysis
            .declared_types
            .entry(name.clone())
            .or_insert_with(|| vec!["Object".into()]);
        analysis
            .define_model_declared_types
            .entry(name)
            .or_insert_with(|| vec!["Object".into()]);
    }
}

pub(crate) fn vue3_ts_module_declaration_name(
    declaration: &TSModuleDeclaration<'_>,
) -> Option<String> {
    vue3_ts_module_declaration_name_ref(declaration).map(str::to_string)
}

pub(crate) fn vue3_ts_module_declaration_name_ref<'a>(
    declaration: &'a TSModuleDeclaration<'_>,
) -> Option<&'a str> {
    match &declaration.id {
        TSModuleDeclarationName::Identifier(identifier) => Some(identifier.name.as_str()),
        TSModuleDeclarationName::StringLiteral(_) => None,
    }
}

pub(crate) fn vue3_ts_module_declaration_is_global(declaration: &TSModuleDeclaration<'_>) -> bool {
    vue3_ts_module_declaration_name_ref(declaration) == Some("global")
}

pub(crate) fn vue3_ts_module_declaration_block_body<'a>(
    declaration: &'a TSModuleDeclaration<'a>,
) -> Option<&'a [Statement<'a>]> {
    match declaration.body.as_ref()? {
        TSModuleDeclarationBody::TSModuleBlock(block) => Some(&block.body),
        TSModuleDeclarationBody::TSModuleDeclaration(_) => None,
    }
}

fn vue3_namespace_exported_type_names_with_budget(
    declaration: &TSModuleDeclaration<'_>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let Some(namespace) = vue3_ts_module_declaration_name_ref(declaration) else {
        return Some(BTreeSet::new());
    };
    vue3_namespace_exported_type_names_with_prefix_and_budget(
        declaration,
        namespace,
        namespace_budget,
    )
}

pub(crate) fn vue3_namespace_declared_type_names_with_budget(
    declaration: &TSModuleDeclaration<'_>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let Some(namespace) = vue3_ts_module_declaration_name_ref(declaration) else {
        return Some(BTreeSet::new());
    };
    vue3_namespace_declared_type_names_with_prefix_and_budget(
        declaration,
        namespace,
        namespace_budget,
    )
}

fn vue3_namespace_declared_type_names_with_prefix_and_budget(
    declaration: &TSModuleDeclaration<'_>,
    prefix: &str,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    if !namespace_budget.reserve(prefix.len().saturating_add(1)) {
        return None;
    }
    let mut names = BTreeSet::new();
    let mut pending = vec![(declaration, prefix.to_string(), 1usize)];
    while let Some((declaration, prefix, depth)) = pending.pop() {
        if depth > VUE3_MAX_NAMESPACE_PROJECTION_DEPTH {
            continue;
        }
        match declaration.body.as_ref() {
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                for statement in &block.body {
                    if let Some(nested) = vue3_namespace_declaration_from_statement(statement) {
                        if let Some(name) = vue3_ts_module_declaration_name_ref(nested) {
                            let prefix = reserve_vue3_qualified_namespace_name(
                                &prefix,
                                name,
                                namespace_budget,
                            )?;
                            pending.push((
                                nested,
                                prefix,
                                depth.saturating_add(1),
                            ));
                        }
                        continue;
                    }
                    for name in vue3_declared_type_names_from_statement_with_budget(
                        statement,
                        namespace_budget,
                    )? {
                        names.insert(reserve_vue3_qualified_namespace_name(
                            &prefix,
                            &name,
                            namespace_budget,
                        )?);
                    }
                }
            }
            Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                if let Some(name) = vue3_ts_module_declaration_name_ref(nested) {
                    let prefix = reserve_vue3_qualified_namespace_name(
                        &prefix,
                        name,
                        namespace_budget,
                    )?;
                    pending.push((
                        nested,
                        prefix,
                        depth.saturating_add(1),
                    ));
                }
            }
            None => {}
        }
    }
    Some(names)
}

fn vue3_namespace_exported_type_names_with_prefix_and_budget(
    declaration: &TSModuleDeclaration<'_>,
    prefix: &str,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    if !namespace_budget.reserve(prefix.len().saturating_add(1)) {
        return None;
    }
    let mut names = BTreeSet::new();
    let mut pending = vec![(declaration, prefix.to_string(), 1usize)];
    while let Some((declaration, prefix, depth)) = pending.pop() {
        if depth > VUE3_MAX_NAMESPACE_PROJECTION_DEPTH {
            continue;
        }
        match declaration.body.as_ref() {
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                for statement in &block.body {
                    let exported_nested = match statement {
                        Statement::ExportNamedDeclaration(export) => export
                            .declaration
                            .as_ref()
                            .and_then(|declaration| match declaration {
                                Declaration::TSModuleDeclaration(declaration) => {
                                    Some(declaration)
                                }
                                _ => None,
                            }),
                        _ => None,
                    };
                    if let Some(nested) = exported_nested {
                        if let Some(name) = vue3_ts_module_declaration_name_ref(nested) {
                            let prefix = reserve_vue3_qualified_namespace_name(
                                &prefix,
                                name,
                                namespace_budget,
                            )?;
                            pending.push((
                                nested,
                                prefix,
                                depth.saturating_add(1),
                            ));
                        }
                        continue;
                    }
                    for name in vue3_exported_type_names_with_budget(
                        std::slice::from_ref(statement),
                        namespace_budget,
                    )? {
                        names.insert(reserve_vue3_qualified_namespace_name(
                            &prefix,
                            &name,
                            namespace_budget,
                        )?);
                    }
                }
            }
            Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                if let Some(name) = vue3_ts_module_declaration_name_ref(nested) {
                    let prefix = reserve_vue3_qualified_namespace_name(
                        &prefix,
                        name,
                        namespace_budget,
                    )?;
                    pending.push((
                        nested,
                        prefix,
                        depth.saturating_add(1),
                    ));
                }
            }
            None => {}
        }
    }
    Some(names)
}

fn reserve_vue3_qualified_namespace_name(
    prefix: &str,
    name: &str,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<String> {
    let length = prefix.len().saturating_add(name.len()).saturating_add(1);
    if !namespace_budget.reserve(length.saturating_add(1)) {
        return None;
    }
    let mut qualified = String::with_capacity(length);
    qualified.push_str(prefix);
    qualified.push('.');
    qualified.push_str(name);
    Some(qualified)
}

#[cfg(test)]
mod namespace_projection_budget_tests {
    use super::*;

    fn budget(limit: usize) -> Vue3NamespaceProjectionBudget {
        Vue3NamespaceProjectionBudget {
            remaining_work: limit,
            exhausted: false,
        }
    }

    fn type_resolver_with_limits(
        limits: Vue3ExternalTypeLoadLimits,
    ) -> Vue3TypeResolverContext {
        Vue3TypeResolverContext {
            external_type_session: Vue3ExternalTypeLoadSession::with_limits(limits),
            ..Vue3TypeResolverContext::default()
        }
    }

    fn javascript_has_commonjs_module_indicator(source: &str) -> bool {
        let allocator = oxc_allocator::Allocator::default();
        let source_type = oxc_span::SourceType::unambiguous();
        let parsed = oxc_parser::Parser::new(&allocator, source, source_type).parse();
        assert!(!parsed.panicked, "parser panicked for {source:?}");
        assert!(parsed.errors.is_empty(), "parse failed for {source:?}");
        vue3_javascript_statements_have_commonjs_module_indicator(
            &parsed.program.body,
            source_type,
        )
    }

    #[test]
    fn module_dependency_scan_covers_typescript_module_edges_and_exact_budget() {
        let source = r#"
/// <reference path="./reference-path" />
/// <REFERENCE TYPES='./reference-types' />
/// <reference lib="esnext" path="./ignored-lib" />
import './side-effect'
export { Named } from './named'
export * from './all'
type Imported = import('./import-type').Value
const load = () => import('./dynamic')
import Required = require('./import-equals')
declare global { interface Augmented { value: string } }
"#;
        let expected = BTreeSet::from([
            Vue3ModuleDependency::module("./all", Vue3TypeResolutionMode::Import),
            Vue3ModuleDependency::module("./dynamic", Vue3TypeResolutionMode::Import),
            Vue3ModuleDependency::module("./import-equals", Vue3TypeResolutionMode::Require),
            Vue3ModuleDependency::module("./import-type", Vue3TypeResolutionMode::Import),
            Vue3ModuleDependency::module("./named", Vue3TypeResolutionMode::Import),
            Vue3ModuleDependency::module("./side-effect", Vue3TypeResolutionMode::Import),
            Vue3ModuleDependency::ReferencePath("./reference-path".to_string()),
            Vue3ModuleDependency::ReferenceTypes {
                source: "./reference-types".to_string(),
                request: Vue3TypeResolutionRequest::inferred(Vue3TypeResolutionMode::Import),
            },
        ]);
        let mut measured = budget(usize::MAX);
        let (has_augmentation, dependencies) = vue3_module_dependencies_from_source(
            source,
            oxc_span::SourceType::ts(),
            &mut measured,
        )
        .expect("measure dependency scan");
        assert!(has_augmentation);
        assert_eq!(dependencies, expected);
        let (_, require_dynamic_dependencies) = vue3_module_dependencies_from_source_with_modes(
            source,
            oxc_span::SourceType::ts(),
            Vue3TypeResolutionMode::Import,
            Vue3TypeResolutionMode::Require,
            &mut budget(usize::MAX),
        )
        .expect("scan transformed dynamic import");
        assert!(require_dynamic_dependencies.contains(&Vue3ModuleDependency::module(
            "./dynamic",
            Vue3TypeResolutionMode::Require,
        )));
        assert!(!require_dynamic_dependencies.contains(&Vue3ModuleDependency::module(
            "./dynamic",
            Vue3TypeResolutionMode::Import,
        )));
        let required = usize::MAX - measured.remaining_work;
        assert!(required > source.len());

        let mut exact = budget(required);
        assert_eq!(
            vue3_module_dependencies_from_source(
                source,
                oxc_span::SourceType::ts(),
                &mut exact,
            ),
            Some((true, expected)),
        );
        assert_eq!(exact.remaining_work, 0);
        assert!(!exact.exhausted);

        let mut short = budget(required - 1);
        assert!(
            vue3_module_dependencies_from_source(
                source,
                oxc_span::SourceType::ts(),
                &mut short,
            )
            .is_none()
        );
        assert_eq!(short.remaining_work, 0);
        assert!(short.exhausted);
    }

    #[test]
    fn javascript_require_scan_is_static_javascript_only_and_exactly_bounded() {
        let source = r#"
import './shared'
require('./shared')
require('./root')
function nested(require) { require(`./shadowed`) }
require.resolve('./resolve')
object.require('./member')
require('./' + name)
require(`./${name}`)
require()
require('./two', options)
require(...sources)
(require)('./parenthesized')
"#;
        let expected = BTreeSet::from([
            Vue3ModuleDependency::module("./root", Vue3TypeResolutionMode::Require),
            Vue3ModuleDependency::module("./shadowed", Vue3TypeResolutionMode::Require),
            Vue3ModuleDependency::module("./shared", Vue3TypeResolutionMode::Import),
            Vue3ModuleDependency::module("./shared", Vue3TypeResolutionMode::Require),
        ]);
        let mut measured = budget(usize::MAX);
        let dependencies = vue3_module_dependencies_from_source(
            source,
            oxc_span::SourceType::unambiguous(),
            &mut measured,
        )
        .expect("measure JavaScript dependency scan")
        .1;
        assert_eq!(dependencies, expected);
        let required = usize::MAX - measured.remaining_work;

        let mut exact = budget(required);
        assert_eq!(
            vue3_module_dependencies_from_source(
                source,
                oxc_span::SourceType::unambiguous(),
                &mut exact,
            ),
            Some((false, expected)),
        );
        assert_eq!(exact.remaining_work, 0);
        assert!(!exact.exhausted);

        let mut short = budget(required - 1);
        assert!(
            vue3_module_dependencies_from_source(
                source,
                oxc_span::SourceType::unambiguous(),
                &mut short,
            )
            .is_none()
        );
        assert_eq!(short.remaining_work, 0);
        assert!(short.exhausted);

        let typescript = r#"
require('./ignored')
function nested() { require('./also-ignored') }
import Required = require('./required')
"#;
        assert_eq!(
            vue3_module_dependencies_from_source(
                typescript,
                oxc_span::SourceType::ts(),
                &mut budget(usize::MAX),
            )
            .map(|(_, dependencies)| dependencies),
            Some(BTreeSet::from([Vue3ModuleDependency::module(
                "./required",
                Vue3TypeResolutionMode::Require,
            )])),
        );

        let commonjs = r#"
import './static'
export * from './exported'
type Imported = import('./import-type').Value
const load = () => import('./dynamic')
"#;
        assert_eq!(
            vue3_module_dependencies_from_source(
                commonjs,
                oxc_span::SourceType::from_path("types.cts").expect("CommonJS source type"),
                &mut budget(usize::MAX),
            )
            .map(|(_, dependencies)| dependencies),
            Some(BTreeSet::from([
                Vue3ModuleDependency::module("./dynamic", Vue3TypeResolutionMode::Import),
                Vue3ModuleDependency::module("./exported", Vue3TypeResolutionMode::Require),
                Vue3ModuleDependency::module("./import-type", Vue3TypeResolutionMode::Require),
                Vue3ModuleDependency::module("./static", Vue3TypeResolutionMode::Require),
            ])),
        );
    }

    #[test]
    fn resolution_mode_attributes_override_only_type_only_edges_and_are_bounded() {
        let source = r#"
import type { ImportRequired } from './import-required' with { "resolution-mode": "require" }
export type { ExportRequired } from './export-required' with { "resolution-mode": "require" }
export type * from './export-all-required' with { "resolution-mode": "require" }
type ImportedRequired = import('./type-required', { with: { "resolution-mode": "require" } }).Value
type AssertedRequired = import('./assert-required', { assert: { "resolution-mode": "require" } }).Value
type TemplateRequired = import('./template-required', { with: { "resolution-mode": `require` } }).Value
import { type SpecifierOnly } from './specifier-only' with { "resolution-mode": "require" }
export { type ExportSpecifierOnly } from './export-specifier-only' with { "resolution-mode": "require" }
import './runtime-attribute' with { "resolution-mode": "require" }
const dynamic = import('./dynamic', { with: { "resolution-mode": "require" } })
import type { Extra } from './extra-attribute' with { "resolution-mode": "require", type: "json" }
import type { Invalid } from './invalid-value' with { "resolution-mode": "Require" }
"#;
        let expected = BTreeSet::from([
            Vue3ModuleDependency::module_with_request(
                "./assert-required",
                Vue3TypeResolutionRequest::explicit(Vue3TypeResolutionMode::Require),
            ),
            Vue3ModuleDependency::module("./dynamic", Vue3TypeResolutionMode::Import),
            Vue3ModuleDependency::module_with_request(
                "./export-all-required",
                Vue3TypeResolutionRequest::explicit(Vue3TypeResolutionMode::Require),
            ),
            Vue3ModuleDependency::module_with_request(
                "./export-required",
                Vue3TypeResolutionRequest::explicit(Vue3TypeResolutionMode::Require),
            ),
            Vue3ModuleDependency::module(
                "./export-specifier-only",
                Vue3TypeResolutionMode::Import,
            ),
            Vue3ModuleDependency::module("./extra-attribute", Vue3TypeResolutionMode::Import),
            Vue3ModuleDependency::module_with_request(
                "./import-required",
                Vue3TypeResolutionRequest::explicit(Vue3TypeResolutionMode::Require),
            ),
            Vue3ModuleDependency::module("./invalid-value", Vue3TypeResolutionMode::Import),
            Vue3ModuleDependency::module("./runtime-attribute", Vue3TypeResolutionMode::Import),
            Vue3ModuleDependency::module("./specifier-only", Vue3TypeResolutionMode::Import),
            Vue3ModuleDependency::module_with_request(
                "./template-required",
                Vue3TypeResolutionRequest::explicit(Vue3TypeResolutionMode::Require),
            ),
            Vue3ModuleDependency::module_with_request(
                "./type-required",
                Vue3TypeResolutionRequest::explicit(Vue3TypeResolutionMode::Require),
            ),
        ]);
        let mut measured = budget(usize::MAX);
        let dependencies = vue3_module_dependencies_from_source(
            source,
            oxc_span::SourceType::ts(),
            &mut measured,
        )
        .expect("measure resolution-mode attribute scan")
        .1;
        assert_eq!(dependencies, expected);
        let required = usize::MAX - measured.remaining_work;

        let mut exact = budget(required);
        assert_eq!(
            vue3_module_dependencies_from_source(
                source,
                oxc_span::SourceType::ts(),
                &mut exact,
            ),
            Some((false, expected)),
        );
        assert_eq!(exact.remaining_work, 0);
        assert!(!exact.exhausted);

        let mut short = budget(required - 1);
        assert!(
            vue3_module_dependencies_from_source(
                source,
                oxc_span::SourceType::ts(),
                &mut short,
            )
            .is_none()
        );
        assert_eq!(short.remaining_work, 0);
        assert!(short.exhausted);

        let commonjs_source = r#"
import type { ImportMode } from './import-mode' with { "resolution-mode": "import" }
export type { ExportMode } from './export-mode' with { "resolution-mode": "import" }
export type * from './export-all-mode' with { "resolution-mode": "import" }
type ImportedMode = import('./type-mode', { with: { "resolution-mode": "import" } }).Value
import { type SpecifierOnly } from './specifier-only' with { "resolution-mode": "import" }
const dynamic = import('./dynamic', { with: { "resolution-mode": "require" } })
"#;
        assert_eq!(
            vue3_module_dependencies_from_source(
                commonjs_source,
                oxc_span::SourceType::from_path("types.cts").expect("CommonJS source type"),
                &mut budget(usize::MAX),
            )
            .map(|(_, dependencies)| dependencies),
            Some(BTreeSet::from([
                Vue3ModuleDependency::module("./dynamic", Vue3TypeResolutionMode::Import),
                Vue3ModuleDependency::module_with_request(
                    "./export-all-mode",
                    Vue3TypeResolutionRequest::explicit(Vue3TypeResolutionMode::Import),
                ),
                Vue3ModuleDependency::module_with_request(
                    "./export-mode",
                    Vue3TypeResolutionRequest::explicit(Vue3TypeResolutionMode::Import),
                ),
                Vue3ModuleDependency::module_with_request(
                    "./import-mode",
                    Vue3TypeResolutionRequest::explicit(Vue3TypeResolutionMode::Import),
                ),
                Vue3ModuleDependency::module("./specifier-only", Vue3TypeResolutionMode::Require),
                Vue3ModuleDependency::module_with_request(
                    "./type-mode",
                    Vue3TypeResolutionRequest::explicit(Vue3TypeResolutionMode::Import),
                ),
            ])),
        );
    }

    #[test]
    fn commonjs_module_indicator_matches_static_export_assignment_rules() {
        for source in [
            "exports.api.value = 1",
            "module.exports.api.value = 1",
            "module[`exports`].api[0] = 1",
            "Object.defineProperty(exports, 'value', {})",
            "Object.defineProperty(module['exports'], 0, {})",
            "Object.defineProperty(exports, `value`, {})",
        ] {
            assert!(
                javascript_has_commonjs_module_indicator(source),
                "expected CommonJS indicator for {source:?}"
            );
        }

        for source in [
            "module.exports.value += 1",
            "module.exports ??= value",
            "exports[key] = value",
            "exports.api[key].value = value",
            "module.exports.value = void 0",
            "module.exports.value = alias = void 0",
            "Object.defineProperty(exports)",
            "Object.defineProperty(exports, key, {})",
            "Object.defineProperty(exports.api, 'value', {})",
            "Object['defineProperty'](exports, 'value', {})",
            "Object.defineProperty(exports, 'value')",
            "Object.defineProperty(exports, 'value', {}, extra)",
        ] {
            assert!(
                !javascript_has_commonjs_module_indicator(source),
                "unexpected CommonJS indicator for {source:?}"
            );
        }
    }

    #[test]
    fn external_source_types_accept_case_insensitive_javascript_extensions() {
        let javascript = vue3_type_source_type("TYPES.JS");
        assert!(javascript.is_javascript());
        assert!(javascript.is_unambiguous());

        let commonjs = vue3_type_source_type("TYPES.CJS");
        assert!(commonjs.is_javascript());
        assert!(commonjs.is_commonjs());

        let declaration = vue3_type_source_type("TYPES.D.CTS");
        assert!(declaration.is_typescript_definition());
        assert!(declaration.is_commonjs());
    }

    #[test]
    fn module_resolution_modes_select_require_and_keep_same_source_edges() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir.path().join("node_modules").join("conditional-module");
        std::fs::create_dir_all(&package).expect("create package directory");
        std::fs::write(
            package.join("package.json"),
            r#"{
                "exports": {
                    ".": {
                        "types": {
                            "import": "./import.d.mts",
                            "require": "./require.d.cts"
                        }
                    }
                }
            }"#,
        )
        .expect("write package manifest");
        let import_entry = package.join("import.d.mts");
        let require_entry = package.join("require.d.cts");
        std::fs::write(
            &import_entry,
            "declare global { interface ImportCondition {} } export {}",
        )
        .expect("write import entry");
        std::fs::write(
            &require_entry,
            "declare global { interface RequireCondition {} } export {}",
        )
        .expect("write require entry");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let resolver = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..Vue3TypeResolverContext::default()
        };

        let require_root = [Vue3InlineModuleSource {
            filename: &filename,
            source: "import Required = require('conditional-module')",
            source_type: oxc_span::SourceType::ts(),
        }];
        assert_eq!(
            vue3_reachable_global_augmentation_files(
                &filename,
                &[],
                &require_root,
                &resolver,
            ),
            Some(vec![require_entry.clone()]),
        );

        let attribute_require_root = [Vue3InlineModuleSource {
            filename: &filename,
            source: r#"import type { Required } from 'conditional-module' with { "resolution-mode": "require" }"#,
            source_type: oxc_span::SourceType::ts(),
        }];
        assert_eq!(
            vue3_reachable_global_augmentation_files(
                &filename,
                &[],
                &attribute_require_root,
                &resolver,
            ),
            Some(vec![require_entry.clone()]),
        );

        let attribute_import_root = [Vue3InlineModuleSource {
            filename: &filename,
            source: r#"import type { Imported } from 'conditional-module' with { "resolution-mode": "import" }"#,
            source_type: oxc_span::SourceType::from_path("root.cts")
                .expect("CommonJS source type"),
        }];
        assert_eq!(
            vue3_reachable_global_augmentation_files(
                &filename,
                &[],
                &attribute_import_root,
                &resolver,
            ),
            Some(vec![import_entry.clone()]),
        );

        for (version, expected) in [
            ((5, 0, 0), &import_entry),
            ((5, 2, 2), &import_entry),
            ((5, 3, 0), &require_entry),
        ] {
            let bundler = Vue3TypeResolverContext {
                typescript_version: version.into(),
                module_resolution: Vue3TypeModuleResolutionKind::Bundler,
                ..Vue3TypeResolverContext::default()
            };
            for source in [
                r#"import type { Required } from 'conditional-module' assert { "resolution-mode": "require" }"#,
                r#"type Required = import('conditional-module', { assert: { "resolution-mode": "require" } }).Required"#,
            ] {
                let roots = [Vue3InlineModuleSource {
                    filename: &filename,
                    source,
                    source_type: oxc_span::SourceType::ts(),
                }];
                assert_eq!(
                    vue3_reachable_global_augmentation_files(
                        &filename,
                        &[],
                        &roots,
                        &bundler,
                    ),
                    Some(vec![expected.clone()]),
                    "TypeScript {version:?} source {source:?}",
                );
            }
        }

        let dual_root = [Vue3InlineModuleSource {
            filename: &filename,
            source: "import Required = require('conditional-module')\nvoid import('conditional-module')",
            source_type: oxc_span::SourceType::ts(),
        }];
        let exact = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..type_resolver_with_limits(Vue3ExternalTypeLoadLimits {
                max_import_files: 2,
                ..Vue3ExternalTypeLoadLimits::default()
            })
        };
        assert_eq!(
            vue3_reachable_global_augmentation_files(&filename, &[], &dual_root, &exact),
            Some(vec![import_entry, require_entry]),
        );

        let short = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..type_resolver_with_limits(Vue3ExternalTypeLoadLimits {
                max_import_files: 1,
                ..Vue3ExternalTypeLoadLimits::default()
            })
        };
        assert!(
            vue3_reachable_global_augmentation_files(&filename, &[], &dual_root, &short).is_none()
        );
    }

    #[test]
    fn commonjs_entries_derive_require_mode_for_internal_static_imports() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let outer = node_modules.join("conditional-outer");
        let child = node_modules.join("conditional-child");
        std::fs::create_dir_all(&outer).expect("create outer package");
        std::fs::create_dir_all(&child).expect("create child package");
        std::fs::write(
            outer.join("package.json"),
            r#"{"exports":{".":{"types":{"import":"./import.d.mts","require":"./require.d.cts"}}}}"#,
        )
        .expect("write outer package manifest");
        std::fs::write(outer.join("import.d.mts"), "export {}")
            .expect("write outer import entry");
        std::fs::write(
            outer.join("require.d.cts"),
            "import 'conditional-child'",
        )
        .expect("write outer require entry");
        std::fs::write(
            child.join("package.json"),
            r#"{"exports":{".":{"types":{"import":"./import.d.mts","require":"./require.d.cts"}}}}"#,
        )
        .expect("write child package manifest");
        std::fs::write(
            child.join("import.d.mts"),
            "declare global { interface WrongChildCondition {} } export {}",
        )
        .expect("write child import entry");
        let child_require = child.join("require.d.cts");
        std::fs::write(
            &child_require,
            "declare global { interface RequiredChildCondition {} } export {}",
        )
        .expect("write child require entry");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let roots = [Vue3InlineModuleSource {
            filename: &filename,
            source: "import Required = require('conditional-outer')",
            source_type: oxc_span::SourceType::ts(),
        }];
        let resolver = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..Vue3TypeResolverContext::default()
        };

        assert_eq!(
            vue3_reachable_global_augmentation_files(
                &filename,
                &[],
                &roots,
                &resolver,
            ),
            Some(vec![child_require]),
        );
    }

    #[test]
    fn package_module_type_drives_unqualified_reference_type_conditions() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let conditional = node_modules.join("conditional-reference");
        let module_bridge = node_modules.join("module-reference-bridge");
        let commonjs_bridge = node_modules.join("commonjs-reference-bridge");
        for package in [&conditional, &module_bridge, &commonjs_bridge] {
            std::fs::create_dir_all(package).expect("create package directory");
        }
        std::fs::write(
            conditional.join("package.json"),
            r#"{
                "types":"./legacy.d.ts",
                "exports":{".":{"types":{"import":"./import.d.mts","require":"./require.d.cts"}}}
            }"#,
        )
        .expect("write conditional package manifest");
        let import_entry = conditional.join("import.d.mts");
        let require_entry = conditional.join("require.d.cts");
        let legacy_entry = conditional.join("legacy.d.ts");
        std::fs::write(
            &import_entry,
            "declare global { interface ImportReferenceGlobal {} } export {}",
        )
        .expect("write import reference entry");
        std::fs::write(
            &require_entry,
            "declare global { interface RequireReferenceGlobal {} } export {}",
        )
        .expect("write require reference entry");
        std::fs::write(
            &legacy_entry,
            "declare global { interface LegacyReferenceGlobal {} } export {}",
        )
        .expect("write legacy reference entry");
        for (bridge, module_type) in [
            (&module_bridge, "module"),
            (&commonjs_bridge, "commonjs"),
        ] {
            std::fs::write(
                bridge.join("package.json"),
                format!(r#"{{"type":"{module_type}","types":"index.d.ts"}}"#),
            )
            .expect("write bridge package manifest");
            std::fs::write(
                bridge.join("index.d.ts"),
                "/// <reference types=\"conditional-reference\" />\nexport {}",
            )
            .expect("write bridge declaration");
        }
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let roots = [Vue3InlineModuleSource {
            filename: &filename,
            source: "import 'module-reference-bridge'; import 'commonjs-reference-bridge'",
            source_type: oxc_span::SourceType::ts(),
        }];
        let node_next_5_2 = Vue3TypeResolverContext {
            typescript_version: (5, 2, 2).into(),
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            resolve_package_json_exports: Some(false),
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            vue3_reachable_global_augmentation_files(
                &filename,
                &[],
                &roots,
                &node_next_5_2,
            ),
            Some(vec![legacy_entry]),
        );

        let node_next_5_3 = Vue3TypeResolverContext {
            typescript_version: (5, 3, 0).into(),
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            resolve_package_json_exports: Some(false),
            ..Vue3TypeResolverContext::default()
        };
        let augmentations = vue3_reachable_global_augmentation_files(
            &filename,
            &[],
            &roots,
            &node_next_5_3,
        )
        .expect("resolve inherited reference modes")
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(
            augmentations,
            BTreeSet::from([import_entry.clone(), require_entry])
        );

        let bundler = Vue3TypeResolverContext {
            typescript_version: (5, 3, 0).into(),
            module_resolution: Vue3TypeModuleResolutionKind::Bundler,
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            vue3_reachable_global_augmentation_files(&filename, &[], &roots, &bundler),
            Some(vec![import_entry]),
        );
    }

    #[test]
    fn initial_global_references_are_deduplicated_before_import_io() {
        let dir = tempfile::tempdir().expect("temp dir");
        let first_path = dir.path().join("first.d.mts");
        let second_path = dir.path().join("second.d.mts");
        std::fs::write(
            &first_path,
            "/// <reference path=\"./second.d.mts\" />\ninterface FirstGlobal {}",
        )
        .expect("write first global source");
        std::fs::write(&second_path, "interface SecondGlobal {}")
            .expect("write second global source");
        let resolver = type_resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_import_files: 0,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        let initial = [&first_path, &second_path]
            .into_iter()
            .map(|path| Vue3GlobalTypeFile {
                path: path.clone(),
                source: vue3_external_global_type_source_from_path(path, &resolver)
                    .expect("load initial global source"),
            })
            .collect::<Vec<_>>();
        let project = dir.path().join("Comp.vue");

        assert_eq!(
            vue3_reachable_global_augmentation_files(
                &project.to_string_lossy(),
                &initial,
                &[],
                &resolver,
            ),
            Some(vec![second_path])
        );
    }

    #[test]
    fn triple_slash_references_are_limited_to_leading_line_comments() {
        let source = r#"
/* ordinary leading comment */
/// <reference path="./leading" />
//// <reference path="./four-slashes" />
//   / <reference path="./spaced-third-slash" />
/* / <reference path="./block" /> */
/* same line */ /// <reference path="./after-block" />
"use strict";
/// <reference path="./after-directive" />
export {}
/// <reference types="after-statement" />
"#;
        let mut namespace_budget = budget(usize::MAX);
        let (_, dependencies) = vue3_module_dependencies_from_source(
            source,
            oxc_span::SourceType::ts(),
            &mut namespace_budget,
        )
        .expect("scan triple-slash directives");

        assert_eq!(
            dependencies,
            BTreeSet::from([Vue3ModuleDependency::ReferencePath(
                "./leading".to_string(),
            )]),
        );
    }

    #[test]
    fn triple_slash_references_accept_bom_and_keep_first_duplicate_attribute() {
        assert!(matches!(
            vue3_triple_slash_reference(
                r#"/ <reference path="./first" path="./second" />"#,
            ),
            Some(Vue3TripleSlashReference::Path("./first"))
        ));
        assert!(matches!(
            vue3_triple_slash_reference(
                r#"/ <reference types="first" types="second" />"#,
            ),
            Some(Vue3TripleSlashReference::Types("first", None))
        ));

        let source = "\u{feff}/// <reference path=\"./bom\" />\ninterface Global {}";
        let mut namespace_budget = budget(usize::MAX);
        let (_, dependencies) = vue3_module_dependencies_from_source(
            source,
            oxc_span::SourceType::ts(),
            &mut namespace_budget,
        )
        .expect("scan BOM-prefixed directive");
        assert_eq!(
            dependencies,
            BTreeSet::from([Vue3ModuleDependency::ReferencePath("./bom".to_string())]),
        );
    }

    #[test]
    fn triple_slash_resolution_modes_are_typed_and_invalid_values_fail_closed() {
        let source = r#"/// <reference types="mode-specific" resolution-mode="require" resolution-mode="import" />
/// <reference types="legacy" resolution-mode="" />
/// <reference path="./path" resolution-mode="invalid" />
export {}"#;
        let mut namespace_budget = budget(usize::MAX);

        assert_eq!(
            vue3_module_dependencies_from_source(
                source,
                oxc_span::SourceType::ts(),
                &mut namespace_budget,
            )
            .map(|(_, dependencies)| dependencies),
            Some(BTreeSet::from([
                Vue3ModuleDependency::ReferencePath("./path".to_string()),
                Vue3ModuleDependency::ReferenceTypes {
                    source: "legacy".to_string(),
                    request: Vue3TypeResolutionRequest::inferred(Vue3TypeResolutionMode::Import),
                },
                Vue3ModuleDependency::ReferenceTypes {
                    source: "mode-specific".to_string(),
                    request: Vue3TypeResolutionRequest::explicit(Vue3TypeResolutionMode::Require),
                },
            ])),
        );
        assert!(!namespace_budget.exhausted);

        for source in [
            r#"/// <reference types="mode-specific" resolution-mode="Require" />"#,
            r#"/// <reference resolution-mode="require" />"#,
        ] {
            assert!(vue3_module_dependencies_from_source(
                source,
                oxc_span::SourceType::ts(),
                &mut budget(usize::MAX),
            )
            .is_none());
        }
    }

    #[test]
    fn triple_slash_resolution_modes_reach_both_conditional_type_entries() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir.path().join("node_modules").join("conditional-types");
        std::fs::create_dir_all(&package).expect("create package directory");
        std::fs::write(
            package.join("package.json"),
            r#"{
                "exports": {
                    ".": {
                        "types": {
                            "import": "./import.d.mts",
                            "require": "./require.d.cts"
                        }
                    }
                }
            }"#,
        )
        .expect("write package manifest");
        let import_entry = package.join("import.d.mts");
        let require_entry = package.join("require.d.cts");
        std::fs::write(
            &import_entry,
            "declare global { interface ImportGlobal {} } export {}",
        )
        .expect("write import declaration");
        std::fs::write(
            &require_entry,
            "declare global { interface RequireGlobal {} } export {}",
        )
        .expect("write require declaration");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let roots = [Vue3InlineModuleSource {
            filename: &filename,
            source: r#"/// <reference types="conditional-types" resolution-mode="import" />
/// <reference types="conditional-types" resolution-mode="require" />"#,
            source_type: oxc_span::SourceType::ts(),
        }];
        let resolver = Vue3TypeResolverContext {
            typescript_version: (5, 3, 0).into(),
            module_resolution: Vue3TypeModuleResolutionKind::Node10,
            ..Vue3TypeResolverContext::default()
        };

        assert_eq!(
            vue3_reachable_global_augmentation_files(
                &filename,
                &[],
                &roots,
                &resolver,
            ),
            Some(vec![import_entry, require_entry]),
        );
    }

    #[test]
    fn inline_vue_implicit_type_references_preserve_absent_resolution_mode() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir.path().join("node_modules").join("conditional-types");
        std::fs::create_dir_all(&package).expect("create package directory");
        std::fs::write(
            package.join("package.json"),
            r#"{
                "types": "./legacy.d.ts",
                "exports": {
                    ".": {
                        "types": {
                            "require": "./modern.d.cts"
                        }
                    }
                }
            }"#,
        )
        .expect("write package manifest");
        let legacy = package.join("legacy.d.ts");
        let modern = package.join("modern.d.cts");
        std::fs::write(&legacy, "interface LegacyGlobal {}")
            .expect("write legacy declaration");
        std::fs::write(&modern, "interface ModernGlobal {}")
            .expect("write modern declaration");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let resolver = Vue3TypeResolverContext {
            typescript_version: (5, 9, 3).into(),
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            module: Some(Vue3TypeModuleKind::NodeNext),
            resolve_package_json_exports: Some(false),
            ..Vue3TypeResolverContext::default()
        };

        for (source, expected) in [
            (r#"/// <reference types="conditional-types" />"#, legacy),
            (
                r#"/// <reference types="conditional-types" resolution-mode="require" />"#,
                modern,
            ),
        ] {
            let roots = [Vue3InlineModuleSource {
                filename: &filename,
                source,
                source_type: oxc_span::SourceType::ts(),
            }];
            assert_eq!(
                vue3_reachable_global_augmentation_files(
                    &filename,
                    &[],
                    &roots,
                    &resolver,
                ),
                Some(vec![expected]),
            );
        }
    }

    #[test]
    fn reference_paths_promote_global_program_files_after_module_deduplication() {
        let dir = tempfile::tempdir().expect("temp dir");
        let referenced = dir.path().join("referenced.d.ts");
        std::fs::write(
            &referenced,
            "interface ReferencedProps { value: string }",
        )
        .expect("write referenced global file");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let resolver = Vue3TypeResolverContext::default();
        let roots = [Vue3InlineModuleSource {
            filename: &filename,
            source: r#"/// <reference path="./referenced.d.ts" />
import './referenced.d.ts'"#,
            source_type: oxc_span::SourceType::ts(),
        }];

        assert_eq!(
            vue3_reachable_global_augmentation_files(&filename, &[], &roots, &resolver),
            Some(vec![referenced]),
        );
    }

    #[test]
    fn missing_and_self_referential_program_references_fail_closed() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("Comp.vue");
        std::fs::write(&filename, "").expect("write root file identity");
        std::fs::write(dir.path().join("unsupported.css"), "body {}").expect("write asset");
        let filename = filename.to_string_lossy().to_string();
        let resolver = Vue3TypeResolverContext::default();
        for source in [
            r#"/// <reference path="./missing.d.ts" />"#,
            r#"/// <reference path="./Comp.vue" />"#,
            r#"/// <reference path="./unsupported.css" />"#,
            r#"/// <reference types="missing-package" />"#,
        ] {
            let roots = [Vue3InlineModuleSource {
                filename: &filename,
                source,
                source_type: oxc_span::SourceType::ts(),
            }];
            assert!(vue3_reachable_global_augmentation_files(
                &filename,
                &[],
                &roots,
                &resolver,
            )
            .is_none());
        }
    }

    #[test]
    fn reference_path_resolution_uses_typescript_extensions_without_index_fallback() {
        let dir = tempfile::tempdir().expect("temp dir");
        let referenced = dir.path().join("referenced.ts");
        let javascript = dir.path().join("referenced.js");
        let hidden = dir.path().join(".hidden.ts");
        let modern = dir.path().join("modern.mts");
        let index_dir = dir.path().join("directory");
        std::fs::create_dir_all(&index_dir).expect("create reference directory");
        std::fs::write(&referenced, "interface Referenced {}")
            .expect("write extensionless reference target");
        std::fs::write(&javascript, "export const value = 1")
            .expect("write disallowed JavaScript reference target");
        std::fs::write(&hidden, "interface HiddenReference {}")
            .expect("write hidden TypeScript target");
        std::fs::write(&modern, "export interface Modern {}")
            .expect("write modern reference target");
        std::fs::write(index_dir.join("index.d.ts"), "interface IndexFallback {}")
            .expect("write disallowed index fallback");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_reference_path(&filename, "./referenced", &resolver),
            Some(referenced),
        );
        assert_eq!(
            resolve_vue3_type_reference_path(&filename, r#".\referenced"#, &resolver),
            Some(dir.path().join("referenced.ts")),
        );
        assert_eq!(
            resolve_vue3_type_reference_path(&filename, "./referenced.js", &resolver),
            None,
        );
        assert_eq!(
            resolve_vue3_type_reference_path(&filename, "./.hidden", &resolver),
            None,
        );
        assert_eq!(
            resolve_vue3_type_reference_path(&filename, "./modern", &resolver),
            None,
        );
        assert_eq!(
            resolve_vue3_type_reference_path(&filename, "./modern.mts", &resolver),
            Some(modern),
        );
        assert_eq!(
            resolve_vue3_type_reference_path(&filename, "./directory", &resolver),
            None,
        );
    }

    #[test]
    fn reference_path_resolution_cache_is_kind_scoped() {
        let dir = tempfile::tempdir().expect("temp dir");
        let module_dir = dir.path().join("shared");
        std::fs::create_dir_all(&module_dir).expect("create module directory");
        let module_index = module_dir.join("index.ts");
        std::fs::write(&module_index, "export interface Imported {}")
            .expect("write module index");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_import(&filename, "./shared", &resolver),
            Some(module_index),
        );
        assert_eq!(
            resolve_vue3_type_reference_path(&filename, "./shared", &resolver),
            None,
        );
        assert_eq!(resolver.external_type_session.stats().resolution_cache_hits, 0);
    }

    #[test]
    fn reference_path_resolution_honors_exact_generated_path_limit() {
        let dir = tempfile::tempdir().expect("temp dir");
        let referenced = dir.path().join("referenced.d.ts");
        std::fs::write(&referenced, "interface Referenced {}")
            .expect("write referenced declaration");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let required = referenced.as_os_str().as_encoded_bytes().len();
        let exact = type_resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: required,
            ..Vue3ExternalTypeLoadLimits::default()
        });

        assert_eq!(
            resolve_vue3_type_reference_path(&filename, "./referenced.d.ts", &exact),
            Some(referenced.clone()),
        );
        assert_eq!(exact.external_type_session.failure_epoch(), 0);

        let short = type_resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: required - 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert_eq!(
            resolve_vue3_type_reference_path(&filename, "./referenced.d.ts", &short),
            None,
        );
        assert_eq!(short.external_type_session.failure_epoch(), 1);
    }

    #[test]
    fn module_augmentation_scan_skips_non_script_side_effect_assets() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("style.css"), ".root { color: red }")
            .expect("write side-effect stylesheet");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let resolver = type_resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_import_files: 0,
            max_import_bytes: 0,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        let roots = [Vue3InlineModuleSource {
            filename: &filename,
            source: "import './style.css'",
            source_type: oxc_span::SourceType::ts(),
        }];

        assert_eq!(
            vue3_reachable_global_augmentation_files(&filename, &[], &roots, &resolver),
            Some(Vec::new()),
        );
        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.import_files_read, 0);
        assert_eq!(stats.import_bytes, 0);
    }

    #[test]
    fn module_augmentation_scan_uses_an_independent_import_budget() {
        let dir = tempfile::tempdir().expect("temp dir");
        let first = dir.path().join("runtime-first.ts");
        let second = dir.path().join("runtime-second.ts");
        let actual = dir.path().join("actual.ts");
        std::fs::write(&first, "import './runtime-second'").expect("write first runtime module");
        std::fs::write(&second, "export const value = true")
            .expect("write second runtime module");
        std::fs::write(&actual, "export interface Props { value: string }")
            .expect("write actual type module");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let resolver = type_resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_import_files: 1,
            max_import_bytes: 1024,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        let roots = [Vue3InlineModuleSource {
            filename: &filename,
            source: "import './runtime-first'",
            source_type: oxc_span::SourceType::ts(),
        }];

        assert!(
            vue3_reachable_global_augmentation_files(&filename, &[], &roots, &resolver).is_none()
        );
        assert_eq!(resolver.external_type_session.stats().import_files_read, 0);
        assert_eq!(
            resolve_vue3_type_import(&filename, "./actual", &resolver),
            Some(actual.clone()),
        );
        assert!(vue3_external_type_source_from_path(&actual, &resolver).is_some());
        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.resolution_lookups, 1);
        assert_eq!(stats.import_files_read, 1);
    }

    #[test]
    fn module_augmentation_scan_rejects_resolution_budget_partial_graphs() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("z-augmentation.ts"),
            "export {}; declare global { interface Augmented { value: string } }",
        )
        .expect("write augmentation module");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let resolver = type_resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_resolution_lookups: 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        let roots = [Vue3InlineModuleSource {
            filename: &filename,
            source: "import './a-missing'\nimport './z-augmentation'",
            source_type: oxc_span::SourceType::ts(),
        }];

        assert!(
            vue3_reachable_global_augmentation_files(&filename, &[], &roots, &resolver).is_none()
        );
        assert_eq!(resolver.external_type_session.stats().resolution_lookups, 0);
    }

    #[test]
    fn namespace_projection_budget_honors_exact_and_overflow_boundaries() {
        let mut budget = budget(3);
        assert!(budget.reserve(2));
        assert!(budget.reserve(1));
        assert_eq!(budget.remaining_work, 0);
        assert!(!budget.reserve(1));
        assert!(budget.exhausted);
    }

    #[test]
    fn namespace_projection_depth_honors_exact_and_overflow_boundaries() {
        for (depth, expected) in [(64, true), (65, false)] {
            let source = format!(
                "namespace {} {{ export interface Props {{ value: string }} }}",
                (0..depth).map(|_| "N").collect::<Vec<_>>().join(".")
            );
            let allocator = oxc_allocator::Allocator::default();
            let parsed = oxc_parser::Parser::new(
                &allocator,
                &source,
                oxc_span::SourceType::ts(),
            )
            .parse();
            assert!(!parsed.panicked && parsed.errors.is_empty());
            let mut budget = budget(1024);
            assert_eq!(
                validate_vue3_namespace_structure(&parsed.program.body, 0, &mut budget),
                expected
            );
            assert_eq!(budget.exhausted, !expected);
        }
    }

    #[test]
    fn nested_namespace_projection_exhaustion_discards_partial_results() {
        let source = r#"
export namespace Root {
  export namespace Child {
    export interface Props { value: string }
  }
}
"#;
        let allocator = oxc_allocator::Allocator::default();
        let parsed = oxc_parser::Parser::new(
            &allocator,
            source,
            oxc_span::SourceType::ts(),
        )
        .parse();
        assert!(!parsed.panicked && parsed.errors.is_empty());
        let mut analysis = Vue3ScriptSetupAnalysis::default();
        let mut budget = budget(18);

        project_vue3_namespace_groups_from_statements_with_budget(
            source,
            &parsed.program.body,
            false,
            0,
            &mut analysis,
            &mut budget,
        );

        assert!(budget.exhausted);
        assert!(!has_vue3_type_alias_projection(
            &analysis,
            "Root.Child.Props"
        ));
    }

    #[test]
    fn named_namespace_import_projection_is_bounded_and_transactional() {
        const LIMIT: usize = 1024 * 1024;

        let dir = tempfile::tempdir().expect("temp dir");
        let types = dir.path().join("types.ts");
        std::fs::write(
            &types,
            "export namespace N { export interface Props { value: string } }",
        )
        .expect("write namespace import type");
        let filename = dir.path().join("Comp.ts");
        let source = "import type { N as First, N as Second } from './types'";
        let resolver = Vue3TypeResolverContext::default();
        let imported = vue3_external_type_context_from_path(
            &types,
            &mut BTreeSet::new(),
            &resolver,
        )
        .expect("load namespace import context");
        assert!(vue3_type_context_names(&imported)
            .iter()
            .all(|name| imported.type_sources.contains_key(name)));

        let mut expected = Vue27TypeContext::default();
        expected
            .declared_types
            .insert("Stable".into(), vec!["String".into()]);
        let mut measured_context = expected.clone();
        let mut measured_budget = budget(LIMIT);
        assert!(extend_vue3_type_context_from_external_imports_with_seen(
            &filename.to_string_lossy(),
            source,
            oxc_span::SourceType::ts(),
            &mut measured_context,
            &mut BTreeSet::new(),
            &resolver,
            &mut measured_budget,
        ));
        let total_work = LIMIT.saturating_sub(measured_budget.remaining_work);
        assert!(total_work > 0);
        assert!(measured_context
            .props_type_declarations
            .contains_key("First.Props"));
        assert!(measured_context
            .props_type_declarations
            .contains_key("Second.Props"));

        let mut context = expected.clone();
        let mut overflow_budget = budget(total_work.saturating_sub(1));
        assert!(!extend_vue3_type_context_from_external_imports_with_seen(
            &filename.to_string_lossy(),
            source,
            oxc_span::SourceType::ts(),
            &mut context,
            &mut BTreeSet::new(),
            &resolver,
            &mut overflow_budget,
        ));
        assert_eq!(context, expected);

        let mut exact_context = expected;
        let mut exact_budget = budget(total_work);
        assert!(extend_vue3_type_context_from_external_imports_with_seen(
            &filename.to_string_lossy(),
            source,
            oxc_span::SourceType::ts(),
            &mut exact_context,
            &mut BTreeSet::new(),
            &resolver,
            &mut exact_budget,
        ));
        assert_eq!(exact_budget.remaining_work, 0);
        assert!(exact_context
            .props_type_declarations
            .contains_key("First.Props"));
        assert!(exact_context
            .props_type_declarations
            .contains_key("Second.Props"));
    }

    #[test]
    fn global_namespace_budget_is_shared_and_transactional() {
        const LIMIT: usize = 1024 * 1024;
        let one_block = r#"
export {}
declare global {
  namespace One { interface Props { value: string } }
}
"#;
        let allocator = oxc_allocator::Allocator::default();
        let parsed = oxc_parser::Parser::new(
            &allocator,
            one_block,
            oxc_span::SourceType::ts(),
        )
        .parse();
        assert!(!parsed.panicked && parsed.errors.is_empty());
        let mut measured_analysis = Vue3ScriptSetupAnalysis::default();
        let mut measured_budget = budget(LIMIT);
        assert!(collect_vue3_global_types_from_statements_with_budget(
            one_block,
            &parsed.program.body,
            false,
            &Vue27TypeContext::default(),
            &mut measured_analysis,
            &mut measured_budget,
        )
        .is_some());
        let one_block_work = LIMIT.saturating_sub(measured_budget.remaining_work);
        assert!(one_block_work > 0);

        let two_blocks = r#"
export {}
declare global {
  namespace One { interface Props { value: string } }
}
declare global {
  namespace Two { interface Props { value: string } }
}
"#;
        let allocator = oxc_allocator::Allocator::default();
        let parsed = oxc_parser::Parser::new(
            &allocator,
            two_blocks,
            oxc_span::SourceType::ts(),
        )
        .parse();
        assert!(!parsed.panicked && parsed.errors.is_empty());
        let mut analysis = Vue3ScriptSetupAnalysis::default();
        analysis
            .declared_types
            .insert("Existing".into(), vec!["String".into()]);
        let expected = analysis.clone();
        let mut bounded_budget = budget(one_block_work);

        assert!(collect_vue3_global_types_from_statements_with_budget(
            two_blocks,
            &parsed.program.body,
            false,
            &Vue27TypeContext::default(),
            &mut analysis,
            &mut bounded_budget,
        )
        .is_none());
        assert!(bounded_budget.exhausted);
        assert_eq!(analysis, expected);
    }
}
