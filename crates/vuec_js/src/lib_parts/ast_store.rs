/// Registry for JavaScript snippets referenced by AST/HIR/MIR nodes.
pub struct JsAstStore {
    allocator: Allocator,
    sources: BTreeMap<String, Arc<str>>,
    interner_hits: usize,
    interner_misses: usize,
    expressions: Vec<JsEntry>,
    statements: Vec<JsEntry>,
    patterns: Vec<JsEntry>,
    programs: Vec<JsEntry>,
}

impl JsAstStore {
    /// Creates an empty JavaScript AST store.
    pub fn new() -> Self {
        Self {
            allocator: Allocator::default(),
            sources: BTreeMap::new(),
            interner_hits: 0,
            interner_misses: 0,
            expressions: Vec::new(),
            statements: Vec::new(),
            patterns: Vec::new(),
            programs: Vec::new(),
        }
    }

    /// Registers a JavaScript expression and returns its stable expression id.
    pub fn register_expr(
        &mut self,
        source: impl Into<String>,
        span: Span,
        source_type: SourceType,
    ) -> JsExprId {
        self.push_expr(source, span, JsParseMode::Expression, source_type)
    }

    /// Registers a Vue `v-for` expression and returns its stable expression id.
    pub fn register_for_expression(
        &mut self,
        source: impl Into<String>,
        span: Span,
        source_type: SourceType,
    ) -> JsExprId {
        self.push_expr(source, span, JsParseMode::ForExpression, source_type)
    }

    /// Registers JavaScript statement source and returns its stable statement id.
    pub fn register_stmt(
        &mut self,
        source: impl Into<String>,
        span: Span,
        source_type: SourceType,
    ) -> JsStmtId {
        let id = JsStmtId(self.statements.len() as u32);
        let source = self.intern_source(source);
        self.statements.push(JsEntry {
            source,
            span,
            mode: JsParseMode::Statements,
            source_type: JsSourceType::from_oxc(source_type),
        });
        id
    }

    /// Registers a parameter or binding pattern list and returns its stable id.
    pub fn register_pattern(
        &mut self,
        source: impl Into<String>,
        span: Span,
        source_type: SourceType,
    ) -> JsPatternId {
        let id = JsPatternId(self.patterns.len() as u32);
        let source = self.intern_source(source);
        self.patterns.push(JsEntry {
            source,
            span,
            mode: JsParseMode::Params,
            source_type: JsSourceType::from_oxc(source_type),
        });
        id
    }

    /// Registers a full JavaScript or TypeScript program and returns its id.
    pub fn register_program(
        &mut self,
        source: impl Into<String>,
        span: Span,
        mode: JsParseMode,
        source_type: SourceType,
    ) -> JsProgramId {
        let id = JsProgramId(self.programs.len() as u32);
        let source = self.intern_source(source);
        self.programs.push(JsEntry {
            source,
            span,
            mode,
            source_type: JsSourceType::from_oxc(source_type),
        });
        id
    }

    fn push_expr(
        &mut self,
        source: impl Into<String>,
        span: Span,
        mode: JsParseMode,
        source_type: SourceType,
    ) -> JsExprId {
        let id = JsExprId(self.expressions.len() as u32);
        let source = self.intern_source(source);
        self.expressions.push(JsEntry {
            source,
            span,
            mode,
            source_type: JsSourceType::from_oxc(source_type),
        });
        id
    }

    /// Returns source interning statistics for registered entries.
    pub fn string_interner_stats(&self) -> JsStringInternerStats {
        JsStringInternerStats {
            hits: self.interner_hits,
            misses: self.interner_misses,
            entries: self.sources.len(),
        }
    }

    /// Clears registered JavaScript snippets and interned source strings.
    ///
    /// This invalidates all ids returned by prior `register_*` calls, but does
    /// not reset parser arena allocations by itself.
    pub fn clear_registered(&mut self) {
        self.sources.clear();
        self.interner_hits = 0;
        self.interner_misses = 0;
        self.expressions.clear();
        self.statements.clear();
        self.patterns.clear();
        self.programs.clear();
    }

    /// Replaces the Oxc parse arena used by subsequent parse calls.
    ///
    /// Previously returned Oxc AST references must not be used after calling
    /// this method.
    pub fn clear_parse_arena(&mut self) {
        self.allocator = Allocator::default();
    }

    /// Clears registered snippets, interned strings, and parse arena state.
    pub fn clear(&mut self) {
        self.clear_registered();
        self.clear_parse_arena();
    }

    /// Returns whether two entries share the same interned source allocation.
    pub fn interned_source_ptr_eq(&self, left: &JsEntry, right: &JsEntry) -> bool {
        left.source.ptr_eq(&right.source)
    }

    fn intern_source(&mut self, source: impl Into<String>) -> JsSource {
        let source = source.into();
        if let Some(existing) = self.sources.get(source.as_str()) {
            self.interner_hits += 1;
            return JsSource::from(existing.clone());
        }
        let interned = Arc::<str>::from(source.as_str());
        self.sources.insert(source, interned.clone());
        self.interner_misses += 1;
        JsSource::from(interned)
    }

    /// Looks up a registered expression entry.
    pub fn expr_entry(&self, id: JsExprId) -> Option<&JsEntry> {
        self.expressions.get(id.0 as usize)
    }

    /// Looks up a registered statement entry.
    pub fn stmt_entry(&self, id: JsStmtId) -> Option<&JsEntry> {
        self.statements.get(id.0 as usize)
    }

    /// Looks up a registered pattern entry.
    pub fn pattern_entry(&self, id: JsPatternId) -> Option<&JsEntry> {
        self.patterns.get(id.0 as usize)
    }

    /// Looks up a registered program entry.
    pub fn program_entry(&self, id: JsProgramId) -> Option<&JsEntry> {
        self.programs.get(id.0 as usize)
    }

    /// Returns all registered expression entries.
    pub fn expressions(&self) -> &[JsEntry] {
        &self.expressions
    }

    /// Returns all registered statement entries.
    pub fn statements(&self) -> &[JsEntry] {
        &self.statements
    }

    /// Returns all registered pattern entries.
    pub fn patterns(&self) -> &[JsEntry] {
        &self.patterns
    }

    /// Returns all registered program entries.
    pub fn programs(&self) -> &[JsEntry] {
        &self.programs
    }

    /// Parses a full program with Oxc and returns the raw parser result.
    pub fn parse_program<'a>(
        &'a self,
        source_text: &'a str,
        source_type: SourceType,
    ) -> ParserReturn<'a> {
        Parser::new(&self.allocator, source_text, source_type)
            .with_options(ParseOptions {
                parse_regular_expression: true,
                ..ParseOptions::default()
            })
            .parse()
    }

    /// Parses a single JavaScript expression with Oxc.
    pub fn parse_expression<'a>(
        &'a self,
        source_text: &'a str,
        source_type: SourceType,
    ) -> Result<Expression<'a>, JsParseError> {
        Parser::new(&self.allocator, source_text, source_type)
            .with_options(ParseOptions {
                parse_regular_expression: true,
                ..ParseOptions::default()
            })
            .parse_expression()
            .map_err(|diagnostics: Vec<OxcDiagnostic>| JsParseError::from_diagnostics(diagnostics))
    }

    /// Validates source text as one complete JavaScript expression.
    pub fn validate_expression(
        &self,
        source_text: &str,
        source_type: SourceType,
    ) -> Result<(), JsParseError> {
        let wrapped = format!("({source_text});");
        self.parse_program_checked(&wrapped, source_type)
            .map(|_| ())
    }

    /// Parses a registered expression by id.
    pub fn parse_expr(&self, id: JsExprId) -> Result<Expression<'_>, JsParseError> {
        let entry = self
            .expr_entry(id)
            .ok_or_else(|| JsParseError::new(format!("unknown JS expression id {}", id.0)))?;
        match entry.mode {
            JsParseMode::Expression => {
                self.parse_expression(&entry.source, entry.source_type.to_oxc())
            }
            JsParseMode::ForExpression => {
                let parsed =
                    self.parse_for_expression(&entry.source, entry.source_type.to_oxc())?;
                self.parse_expression(parsed.iterable, entry.source_type.to_oxc())
            }
            _ => Err(JsParseError::new(format!(
                "JS expression id {} has incompatible mode {:?}",
                id.0, entry.mode
            ))),
        }
    }

    /// Parses registered statement source by id as a checked program.
    pub fn parse_stmt(&self, id: JsStmtId) -> Result<ParserReturn<'_>, JsParseError> {
        let entry = self
            .stmt_entry(id)
            .ok_or_else(|| JsParseError::new(format!("unknown JS statement id {}", id.0)))?;
        self.parse_program_checked(&entry.source, entry.source_type.to_oxc())
    }

    /// Parses registered statement source by id and returns its first statement.
    pub fn parse_single_stmt(&self, id: JsStmtId) -> Result<Statement<'_>, JsParseError> {
        let parsed = self.parse_stmt(id)?;
        let mut body = parsed.program.body;
        if body.len() != 1 {
            return Err(JsParseError::new(format!(
                "JS statement id {} parsed to {} statements",
                id.0,
                body.len()
            )));
        }
        Ok(body.pop().expect("checked one statement"))
    }

    /// Parses a registered parameter or binding pattern list by id.
    pub fn parse_pattern(&self, id: JsPatternId) -> Result<ParsedParams<'_>, JsParseError> {
        let entry = self
            .pattern_entry(id)
            .ok_or_else(|| JsParseError::new(format!("unknown JS pattern id {}", id.0)))?;
        self.parse_params(&entry.source)
    }

    /// Parses a registered program by id.
    pub fn parse_registered_program(
        &self,
        id: JsProgramId,
    ) -> Result<ParserReturn<'_>, JsParseError> {
        let entry = self
            .program_entry(id)
            .ok_or_else(|| JsParseError::new(format!("unknown JS program id {}", id.0)))?;
        self.parse_program_checked(&entry.source, entry.source_type.to_oxc())
    }

    fn parse_program_checked<'a>(
        &'a self,
        source_text: &'a str,
        source_type: SourceType,
    ) -> Result<ParserReturn<'a>, JsParseError> {
        let ret = self.parse_program(source_text, source_type);
        if ret.panicked || !ret.errors.is_empty() {
            return Err(JsParseError::from_diagnostics(ret.errors));
        }
        Ok(ret)
    }

    /// Parses source text according to a Vue compiler parse mode.
    pub fn parse_mode<'a>(
        &'a self,
        source_text: &'a str,
        mode: JsParseMode,
        source_type: SourceType,
    ) -> Result<JsParseResult<'a>, JsParseError> {
        match mode {
            JsParseMode::Expression => self
                .parse_expression(source_text, source_type)
                .map(JsParseResult::Expression),
            JsParseMode::Statements
            | JsParseMode::ScriptModule
            | JsParseMode::ScriptClassic
            | JsParseMode::TypeScript => Ok(JsParseResult::Program(
                self.parse_program_checked(source_text, source_type)?,
            )),
            JsParseMode::Params => self.parse_params(source_text).map(JsParseResult::Params),
            JsParseMode::ForExpression => self
                .parse_for_expression(source_text, source_type)
                .map(JsParseResult::ForExpression),
        }
    }

    /// Parses a Vue 2 filter chain and validates the base and argument expressions with Oxc.
    pub fn parse_vue2_filter_expression<'a>(
        &'a self,
        source_text: &'a str,
        source_type: SourceType,
    ) -> Result<Vue2FilterExpression<'a>, JsParseError> {
        let parsed = parse_vue2_filter_expression(source_text);
        self.validate_expression(parsed.base, source_type)?;
        for filter in &parsed.filters {
            self.validate_vue2_filter_call(filter, source_type)?;
            for arg in &filter.args {
                self.validate_expression(arg, source_type)?;
            }
        }
        Ok(parsed)
    }

    fn validate_vue2_filter_call(
        &self,
        filter: &Vue2FilterCall<'_>,
        source_type: SourceType,
    ) -> Result<(), JsParseError> {
        let Some(open) = filter_call_open_paren(filter.raw) else {
            return Ok(());
        };
        let wrapped = format!("__vuec_filter__({}", &filter.raw[open + 1..]);
        self.validate_expression(&wrapped, source_type)
    }

    /// Validates JavaScript source as a Vue event handler function body.
    pub fn validate_function_body(
        &self,
        source_text: &str,
        source_type: SourceType,
    ) -> Result<(), JsParseError> {
        let wrapped = format!("function __vuec__($event){{\n{source_text}\n}}");
        self.parse_program_checked(&wrapped, source_type)
            .map(|_| ())
    }

    /// Converts a Vue 2 filter chain into the official runtime helper shape.
    pub fn rewrite_vue2_filter_expression(&self, source_text: &str) -> String {
        rewrite_vue2_filter_expression(source_text)
    }

    /// Parses a parameter or binding pattern list.
    pub fn parse_params<'a>(
        &'a self,
        source_text: &'a str,
    ) -> Result<ParsedParams<'a>, JsParseError> {
        let wrapped = format!("function __vuec__({source_text}) {{}}");
        let ret = self.parse_program(&wrapped, SourceType::script());
        if ret.panicked || !ret.errors.is_empty() {
            return Err(JsParseError::from_diagnostics(ret.errors));
        }

        Ok(ParsedParams {
            raw: source_text,
            items: split_top_level(source_text, ','),
        })
    }

    /// Parses a Vue `v-for` expression and validates its iterable expression.
    pub fn parse_for_expression<'a>(
        &'a self,
        source_text: &'a str,
        source_type: SourceType,
    ) -> Result<ParsedForExpression<'a>, JsParseError> {
        let (aliases, iterable) = split_for_expression(source_text)
            .ok_or_else(|| JsParseError::new("missing `in`/`of` in v-for expression"))?;
        self.validate_expression(iterable, source_type)?;
        Ok(ParsedForExpression {
            raw: source_text,
            aliases,
            iterable,
            items: split_top_level(aliases, ','),
        })
    }

    /// Summarizes top-level program bindings, imports, exports, and parse errors.
    pub fn summarize_program(
        &self,
        source_text: &str,
        source_type: SourceType,
    ) -> JsProgramSummary {
        let parsed = self.parse_program(source_text, source_type);
        let mut summary = JsProgramSummary {
            errors: parsed.errors.iter().map(ToString::to_string).collect(),
            panicked: parsed.panicked,
            ..JsProgramSummary::default()
        };
        for statement in &parsed.program.body {
            collect_statement_summary(statement, &mut summary);
        }
        summary.bindings.sort();
        summary.bindings.dedup();
        summary.imports.sort();
        summary.imports.dedup();
        summary.exports.sort();
        summary.exports.dedup();
        summary
    }
}

impl Default for JsAstStore {
    fn default() -> Self {
        Self::new()
    }
}
