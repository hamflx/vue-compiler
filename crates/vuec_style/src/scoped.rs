use crate::*;

pub(crate) const STYLE_SCOPED_MAX_SOURCE_BYTES: usize = STYLE_PREPROCESS_MAX_OUTPUT_BYTES;
pub(crate) const STYLE_SCOPED_MAX_SCOPE_ID_BYTES: usize = 32 * 1024;
pub(crate) const STYLE_SCOPED_MAX_SYNTAX_DEPTH: usize = 128;
pub(crate) const STYLE_SCOPED_MAX_RECURSIVE_SCAN_BYTES: usize = 256 * 1024 * 1024;
pub(crate) const STYLE_SCOPED_MAX_KEYFRAMES: usize = 262_144;
pub(crate) const STYLE_SCOPED_MAX_KEYFRAME_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const STYLE_SCOPED_MAX_KEYFRAME_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const STYLE_SCOPED_MAX_KEYFRAME_RENDER_BYTES: usize = 256 * 1024 * 1024;
pub(crate) const STYLE_SCOPED_MAX_WARNINGS: usize = 65_536;
pub(crate) const STYLE_SCOPED_MAX_WARNING_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const STYLE_SCOPED_MAX_SELECTOR_WORK_BYTES: usize = 256 * 1024 * 1024;
pub(crate) const STYLE_SCOPED_MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const STYLE_SCOPED_MAX_RENDER_BYTES: usize = 256 * 1024 * 1024;
// Selector rewriting performs several full scans and can retain both formatted
// and scope-injected copies. These weights bound cumulative work before either
// kind of intermediate is allocated.
pub(crate) const STYLE_SCOPED_SELECTOR_SCAN_FACTOR: usize = 8;
pub(crate) const STYLE_SCOPED_SELECTOR_DEEP_COPY_FACTOR: usize = 8;
pub(crate) const STYLE_SCOPED_SELECTOR_SCOPE_FACTOR: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScopedStyleLimits {
    pub(crate) max_source_bytes: usize,
    pub(crate) max_scope_id_bytes: usize,
    pub(crate) max_syntax_depth: usize,
    pub(crate) max_recursive_scan_bytes: usize,
    pub(crate) max_keyframes: usize,
    pub(crate) max_keyframe_bytes: usize,
    pub(crate) max_keyframe_output_bytes: usize,
    pub(crate) max_keyframe_render_bytes: usize,
    pub(crate) max_warnings: usize,
    pub(crate) max_warning_bytes: usize,
    pub(crate) max_selector_work_bytes: usize,
    pub(crate) max_output_bytes: usize,
    pub(crate) max_render_bytes: usize,
}

impl Default for ScopedStyleLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: STYLE_SCOPED_MAX_SOURCE_BYTES,
            max_scope_id_bytes: STYLE_SCOPED_MAX_SCOPE_ID_BYTES,
            max_syntax_depth: STYLE_SCOPED_MAX_SYNTAX_DEPTH,
            max_recursive_scan_bytes: STYLE_SCOPED_MAX_RECURSIVE_SCAN_BYTES,
            max_keyframes: STYLE_SCOPED_MAX_KEYFRAMES,
            max_keyframe_bytes: STYLE_SCOPED_MAX_KEYFRAME_BYTES,
            max_keyframe_output_bytes: STYLE_SCOPED_MAX_KEYFRAME_OUTPUT_BYTES,
            max_keyframe_render_bytes: STYLE_SCOPED_MAX_KEYFRAME_RENDER_BYTES,
            max_warnings: STYLE_SCOPED_MAX_WARNINGS,
            max_warning_bytes: STYLE_SCOPED_MAX_WARNING_BYTES,
            max_selector_work_bytes: STYLE_SCOPED_MAX_SELECTOR_WORK_BYTES,
            max_output_bytes: STYLE_SCOPED_MAX_OUTPUT_BYTES,
            max_render_bytes: STYLE_SCOPED_MAX_RENDER_BYTES,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ScopedStyleBudget {
    pub(crate) limits: ScopedStyleLimits,
    pub(crate) keyframes: usize,
    pub(crate) keyframe_bytes: usize,
    pub(crate) keyframe_output_bytes: usize,
    pub(crate) keyframe_render_bytes: usize,
    pub(crate) warnings: usize,
    pub(crate) warning_bytes: usize,
    pub(crate) render_bytes: usize,
}

impl ScopedStyleBudget {
    pub(crate) fn new(limits: ScopedStyleLimits) -> Self {
        Self {
            limits,
            keyframes: 0,
            keyframe_bytes: 0,
            keyframe_output_bytes: 0,
            keyframe_render_bytes: 0,
            warnings: 0,
            warning_bytes: 0,
            render_bytes: 0,
        }
    }

    pub(crate) fn claim_keyframe(
        &mut self,
        raw_bytes: usize,
        renamed_bytes: usize,
    ) -> Result<(), StylePreprocessError> {
        let keyframes = self.keyframes.checked_add(1).ok_or_else(|| {
            StylePreprocessError::scoped_limit("scoped style keyframe count overflowed")
        })?;
        if keyframes > self.limits.max_keyframes {
            return Err(StylePreprocessError::scoped_limit(format!(
                "scoped style keyframes exceed the maximum of {}",
                self.limits.max_keyframes
            )));
        }
        let retained_bytes = raw_bytes.checked_add(renamed_bytes).ok_or_else(|| {
            StylePreprocessError::scoped_limit("scoped style keyframe size overflowed")
        })?;
        let keyframe_bytes = self
            .keyframe_bytes
            .checked_add(retained_bytes)
            .ok_or_else(|| {
                StylePreprocessError::scoped_limit("scoped style keyframe size overflowed")
            })?;
        if keyframe_bytes > self.limits.max_keyframe_bytes {
            return Err(StylePreprocessError::scoped_limit(format!(
                "scoped style keyframes exceed the maximum total of {} bytes",
                self.limits.max_keyframe_bytes
            )));
        }
        self.keyframes = keyframes;
        self.keyframe_bytes = keyframe_bytes;
        Ok(())
    }

    pub(crate) fn claim_warning(&mut self, bytes: usize) -> Result<(), StylePreprocessError> {
        let warnings = self.warnings.checked_add(1).ok_or_else(|| {
            StylePreprocessError::scoped_limit("scoped style warning count overflowed")
        })?;
        if warnings > self.limits.max_warnings {
            return Err(StylePreprocessError::scoped_limit(format!(
                "scoped style warnings exceed the maximum of {}",
                self.limits.max_warnings
            )));
        }
        let warning_bytes = self.warning_bytes.checked_add(bytes).ok_or_else(|| {
            StylePreprocessError::scoped_limit("scoped style warning size overflowed")
        })?;
        if warning_bytes > self.limits.max_warning_bytes {
            return Err(StylePreprocessError::scoped_limit(format!(
                "scoped style warnings exceed the maximum total of {} bytes",
                self.limits.max_warning_bytes
            )));
        }
        self.warnings = warnings;
        self.warning_bytes = warning_bytes;
        Ok(())
    }

    pub(crate) fn begin_keyframe_rewrite(
        &mut self,
        source_bytes: usize,
    ) -> Result<(), StylePreprocessError> {
        if source_bytes > self.limits.max_keyframe_output_bytes {
            return Err(keyframe_output_limit_error(self.limits));
        }
        if source_bytes > self.limits.max_keyframe_render_bytes {
            return Err(keyframe_render_limit_error(self.limits));
        }
        self.keyframe_output_bytes = source_bytes;
        self.keyframe_render_bytes = source_bytes;
        Ok(())
    }

    pub(crate) fn claim_keyframe_rewrite(
        &mut self,
        generated_bytes: usize,
        copies: usize,
    ) -> Result<(), StylePreprocessError> {
        let output_bytes = self
            .keyframe_output_bytes
            .checked_add(generated_bytes)
            .ok_or_else(|| {
                StylePreprocessError::scoped_limit("scoped style keyframe output size overflowed")
            })?;
        if output_bytes > self.limits.max_keyframe_output_bytes {
            return Err(keyframe_output_limit_error(self.limits));
        }
        let render_bytes = generated_bytes
            .checked_mul(copies)
            .and_then(|bytes| self.keyframe_render_bytes.checked_add(bytes))
            .ok_or_else(|| {
                StylePreprocessError::scoped_limit(
                    "scoped style keyframe render work size overflowed",
                )
            })?;
        if render_bytes > self.limits.max_keyframe_render_bytes {
            return Err(keyframe_render_limit_error(self.limits));
        }
        self.keyframe_output_bytes = output_bytes;
        self.keyframe_render_bytes = render_bytes;
        Ok(())
    }

    pub(crate) fn append_render_str(
        &mut self,
        output: &mut String,
        value: &str,
    ) -> Result<(), StylePreprocessError> {
        self.reserve_render_append(output, value.len())?;
        output.push_str(value);
        Ok(())
    }

    pub(crate) fn append_render_char(
        &mut self,
        output: &mut String,
        value: char,
    ) -> Result<(), StylePreprocessError> {
        self.reserve_render_append(output, value.len_utf8())?;
        output.push(value);
        Ok(())
    }

    pub(crate) fn claim_render_copy(&mut self, bytes: usize) -> Result<(), StylePreprocessError> {
        if bytes > self.limits.max_output_bytes {
            return Err(scoped_output_limit_error(self.limits));
        }
        self.claim_render_bytes(bytes)
    }

    fn reserve_render_append(
        &mut self,
        output: &mut String,
        bytes: usize,
    ) -> Result<(), StylePreprocessError> {
        let output_bytes = output.len().checked_add(bytes).ok_or_else(|| {
            StylePreprocessError::scoped_limit("scoped style output size overflowed")
        })?;
        if output_bytes > self.limits.max_output_bytes {
            return Err(scoped_output_limit_error(self.limits));
        }
        let render_bytes = self.next_render_bytes(bytes)?;
        output.try_reserve(bytes).map_err(|_| {
            StylePreprocessError::scoped_limit(
                "scoped style output could not reserve capacity within the configured limit",
            )
        })?;
        self.render_bytes = render_bytes;
        Ok(())
    }

    fn claim_render_bytes(&mut self, bytes: usize) -> Result<(), StylePreprocessError> {
        self.render_bytes = self.next_render_bytes(bytes)?;
        Ok(())
    }

    fn next_render_bytes(&self, bytes: usize) -> Result<usize, StylePreprocessError> {
        let render_bytes = self.render_bytes.checked_add(bytes).ok_or_else(|| {
            StylePreprocessError::scoped_limit("scoped style rendering work size overflowed")
        })?;
        if render_bytes > self.limits.max_render_bytes {
            return Err(scoped_render_limit_error(self.limits));
        }
        Ok(render_bytes)
    }
}

fn keyframe_output_limit_error(limits: ScopedStyleLimits) -> StylePreprocessError {
    StylePreprocessError::scoped_limit(format!(
        "scoped style keyframe output exceeds the maximum of {} bytes",
        limits.max_keyframe_output_bytes
    ))
}

fn keyframe_render_limit_error(limits: ScopedStyleLimits) -> StylePreprocessError {
    StylePreprocessError::scoped_limit(format!(
        "scoped style keyframe rendering exceeds the maximum work budget of {} bytes",
        limits.max_keyframe_render_bytes
    ))
}

fn scoped_output_limit_error(limits: ScopedStyleLimits) -> StylePreprocessError {
    StylePreprocessError::scoped_limit(format!(
        "scoped style output exceeds the maximum of {} bytes",
        limits.max_output_bytes
    ))
}

fn scoped_render_limit_error(limits: ScopedStyleLimits) -> StylePreprocessError {
    StylePreprocessError::scoped_limit(format!(
        "scoped style rendering exceeds the maximum work budget of {} bytes",
        limits.max_render_bytes
    ))
}

pub(crate) struct ScopedStyleTransform {
    pub(crate) code: String,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ScopedStyleItemFrame {
    start: usize,
    rewrites_selectors: bool,
}

/// Rewrites selectors in `source` to include `scope_id`.
///
/// Returns an empty string when the input exceeds the transform's resource
/// limits. This keeps the infallible public API fail-closed instead of
/// returning CSS without the requested scope attribute.
pub fn rewrite_scoped_selectors(source: &str, scope_id: &str) -> String {
    rewrite_scoped_selectors_with_limits(source, scope_id, ScopedStyleLimits::default())
        .unwrap_or_default()
}

pub(crate) fn rewrite_scoped_selectors_with_limits(
    source: &str,
    scope_id: &str,
    limits: ScopedStyleLimits,
) -> Result<String, StylePreprocessError> {
    transform_scoped_style_with_limits(source, scope_id, false, limits)
        .map(|transform| transform.code)
}

pub(crate) fn transform_scoped_style_with_limits(
    source: &str,
    scope_id: &str,
    warn_deprecated: bool,
    limits: ScopedStyleLimits,
) -> Result<ScopedStyleTransform, StylePreprocessError> {
    validate_scoped_style_resources(source, scope_id, limits)?;
    let short_id = scope_id.strip_prefix("data-v-").unwrap_or(scope_id);
    let mut budget = ScopedStyleBudget::new(limits);
    let keyframes = collect_scoped_keyframes(source, short_id, &mut budget)?;
    validate_scoped_keyframe_rewrite_work(source, &keyframes, &mut budget)?;
    let diagnostics = if warn_deprecated {
        scoped_selector_deprecation_warnings(source, &mut budget)?
    } else {
        Vec::new()
    };
    let code = rewrite_css_items(
        source,
        scope_id,
        &keyframes,
        CssBlockContext::Root,
        &mut budget,
    )?;
    Ok(ScopedStyleTransform { code, diagnostics })
}

pub(crate) fn validate_scoped_style_resources(
    source: &str,
    scope_id: &str,
    limits: ScopedStyleLimits,
) -> Result<(), StylePreprocessError> {
    if source.len() > limits.max_source_bytes {
        return Err(StylePreprocessError::scoped_limit(format!(
            "scoped style source exceeds the maximum of {} bytes",
            limits.max_source_bytes
        )));
    }
    if scope_id.len() > limits.max_scope_id_bytes {
        return Err(StylePreprocessError::scoped_limit(format!(
            "scoped style id exceeds the maximum of {} bytes",
            limits.max_scope_id_bytes
        )));
    }

    let mut recursive_scan_bytes = source.len();
    if recursive_scan_bytes > limits.max_recursive_scan_bytes {
        return Err(scoped_recursive_scan_limit_error(limits));
    }
    let mut brace_opens = Vec::new();
    let mut brace_is_structural = Vec::new();
    let mut paren_opens = Vec::new();
    let mut item_frames = Vec::new();
    item_frames.try_reserve(1).map_err(|_| {
        StylePreprocessError::scoped_limit(
            "scoped style item stack could not reserve capacity within the configured limit",
        )
    })?;
    item_frames.push(ScopedStyleItemFrame {
        start: 0,
        rewrites_selectors: true,
    });
    let mut bracket_depth = 0usize;
    let mut selector_work_bytes = source.len();
    if selector_work_bytes > limits.max_selector_work_bytes {
        return Err(scoped_selector_work_limit_error(limits));
    }
    let mut state = CssScannerState::Normal;
    let mut index = 0usize;
    while index < source.len() {
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        match state {
            CssScannerState::Normal => {
                if source[index..].starts_with("/*") {
                    state = CssScannerState::BlockComment;
                    index += 2;
                    continue;
                }
                match ch {
                    '\'' => state = CssScannerState::SingleQuote,
                    '"' => state = CssScannerState::DoubleQuote,
                    '\\' => {
                        index += ch.len_utf8();
                        if index < source.len() {
                            index += source[index..].chars().next().map_or(0, char::len_utf8);
                        }
                        continue;
                    }
                    '{' => {
                        let structural = paren_opens.is_empty() && bracket_depth == 0;
                        if structural {
                            let item_frame =
                                item_frames.last().copied().unwrap_or(ScopedStyleItemFrame {
                                    start: 0,
                                    rewrites_selectors: true,
                                });
                            let prelude = scoped_css_item_prelude(&source[item_frame.start..index]);
                            let is_at_rule = prelude.starts_with('@');
                            let is_block_declaration = css_prelude_is_block_declaration(prelude);
                            if item_frame.rewrites_selectors && !is_at_rule && !is_block_declaration
                            {
                                claim_scoped_selector_work(
                                    &mut selector_work_bytes,
                                    prelude,
                                    scope_id,
                                    limits,
                                )?;
                            }
                            let child_rewrites_selectors = if is_block_declaration {
                                false
                            } else if is_at_rule {
                                !is_keyframes_at_rule(prelude)
                            } else {
                                item_frame.rewrites_selectors
                            };
                            item_frames.try_reserve(1).map_err(|_| {
                                StylePreprocessError::scoped_limit(
                                    "scoped style item stack could not reserve capacity within the configured limit",
                                )
                            })?;
                            item_frames.push(ScopedStyleItemFrame {
                                start: index + ch.len_utf8(),
                                rewrites_selectors: child_rewrites_selectors,
                            });
                        }
                        push_scoped_syntax_open(
                            &mut brace_opens,
                            paren_opens.len(),
                            index,
                            limits,
                        )?;
                        brace_is_structural.try_reserve(1).map_err(|_| {
                            StylePreprocessError::scoped_limit(
                                "scoped style syntax stack could not reserve capacity within the configured limit",
                            )
                        })?;
                        brace_is_structural.push(structural);
                    }
                    '(' => {
                        push_scoped_syntax_open(&mut paren_opens, brace_opens.len(), index, limits)?
                    }
                    '}' => {
                        let structural = brace_is_structural.pop().unwrap_or(false);
                        if let Some(open) = brace_opens.pop() {
                            claim_scoped_recursive_span(
                                &mut recursive_scan_bytes,
                                open,
                                index,
                                limits,
                            )?;
                        }
                        if structural {
                            item_frames.pop();
                            if let Some(item_frame) = item_frames.last_mut() {
                                item_frame.start = index + ch.len_utf8();
                            }
                            paren_opens.clear();
                            bracket_depth = 0;
                        }
                    }
                    ')' => {
                        if let Some(open) = paren_opens.pop() {
                            claim_scoped_recursive_span(
                                &mut recursive_scan_bytes,
                                open,
                                index,
                                limits,
                            )?;
                        }
                    }
                    '[' => bracket_depth = bracket_depth.saturating_add(1),
                    ']' => bracket_depth = bracket_depth.saturating_sub(1),
                    ';' if paren_opens.is_empty() && bracket_depth == 0 => {
                        if let Some(item_frame) = item_frames.last_mut() {
                            item_frame.start = index + ch.len_utf8();
                        }
                    }
                    _ => {}
                }
            }
            CssScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        index += source[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '\'' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        index += source[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::BlockComment => {
                if source[index..].starts_with("*/") {
                    state = CssScannerState::Normal;
                    index += 2;
                    continue;
                }
            }
        }
        index += ch.len_utf8();
    }
    Ok(())
}

fn push_scoped_syntax_open(
    opens: &mut Vec<usize>,
    other_depth: usize,
    index: usize,
    limits: ScopedStyleLimits,
) -> Result<(), StylePreprocessError> {
    let depth = opens.len().checked_add(other_depth).ok_or_else(|| {
        StylePreprocessError::scoped_limit("scoped style syntax nesting depth overflowed")
    })?;
    if depth >= limits.max_syntax_depth {
        return Err(StylePreprocessError::scoped_limit(format!(
            "scoped style syntax nesting exceeds the maximum depth of {}",
            limits.max_syntax_depth
        )));
    }
    opens.try_reserve(1).map_err(|_| {
        StylePreprocessError::scoped_limit(
            "scoped style syntax stack could not reserve capacity within the configured limit",
        )
    })?;
    opens.push(index);
    Ok(())
}

fn claim_scoped_recursive_span(
    recursive_scan_bytes: &mut usize,
    open: usize,
    close: usize,
    limits: ScopedStyleLimits,
) -> Result<(), StylePreprocessError> {
    let span_bytes = close
        .checked_sub(open)
        .and_then(|bytes| bytes.checked_add(1))
        .ok_or_else(|| {
            StylePreprocessError::scoped_limit("scoped style recursive scan size overflowed")
        })?;
    *recursive_scan_bytes = recursive_scan_bytes
        .checked_add(span_bytes)
        .ok_or_else(|| {
            StylePreprocessError::scoped_limit("scoped style recursive scan size overflowed")
        })?;
    if *recursive_scan_bytes > limits.max_recursive_scan_bytes {
        return Err(scoped_recursive_scan_limit_error(limits));
    }
    Ok(())
}

fn scoped_recursive_scan_limit_error(limits: ScopedStyleLimits) -> StylePreprocessError {
    StylePreprocessError::scoped_limit(format!(
        "scoped style recursive scan span exceeds the maximum total of {} bytes",
        limits.max_recursive_scan_bytes
    ))
}

fn scoped_css_item_prelude(mut source: &str) -> &str {
    loop {
        source = source.trim_start();
        let Some(comment) = source.strip_prefix("/*") else {
            return source.trim_end();
        };
        let Some(close) = comment.find("*/") else {
            return "";
        };
        source = &comment[close + 2..];
    }
}

fn claim_scoped_selector_work(
    selector_work_bytes: &mut usize,
    prelude: &str,
    scope_id: &str,
    limits: ScopedStyleLimits,
) -> Result<(), StylePreprocessError> {
    if prelude.is_empty() {
        return Ok(());
    }
    let (commas, colons) = scoped_selector_complexity(prelude)?;
    let branches = commas.checked_add(1).ok_or_else(|| {
        StylePreprocessError::scoped_limit("scoped style selector branch count overflowed")
    })?;
    let selector_nodes = branches.checked_add(colons).ok_or_else(|| {
        StylePreprocessError::scoped_limit("scoped style selector node count overflowed")
    })?;
    let scan_bytes = prelude
        .len()
        .checked_mul(STYLE_SCOPED_SELECTOR_SCAN_FACTOR)
        .ok_or_else(|| {
            StylePreprocessError::scoped_limit("scoped style selector work size overflowed")
        })?;
    let scoped_node_bytes = scope_id
        .len()
        .checked_add(4)
        .and_then(|bytes| bytes.checked_mul(selector_nodes))
        .and_then(|bytes| bytes.checked_mul(STYLE_SCOPED_SELECTOR_SCOPE_FACTOR))
        .ok_or_else(|| {
            StylePreprocessError::scoped_limit("scoped style selector work size overflowed")
        })?;
    let deep_copy_bytes = scoped_deep_container_copy_bytes(prelude)?
        .checked_mul(STYLE_SCOPED_SELECTOR_DEEP_COPY_FACTOR)
        .ok_or_else(|| {
            StylePreprocessError::scoped_limit("scoped style selector work size overflowed")
        })?;
    let work_bytes = scan_bytes
        .checked_add(scoped_node_bytes)
        .and_then(|bytes| bytes.checked_add(deep_copy_bytes))
        .ok_or_else(|| {
            StylePreprocessError::scoped_limit("scoped style selector work size overflowed")
        })?;
    *selector_work_bytes = selector_work_bytes.checked_add(work_bytes).ok_or_else(|| {
        StylePreprocessError::scoped_limit("scoped style selector work size overflowed")
    })?;
    if *selector_work_bytes > limits.max_selector_work_bytes {
        return Err(scoped_selector_work_limit_error(limits));
    }
    Ok(())
}

pub(crate) fn scoped_selector_complexity(
    prelude: &str,
) -> Result<(usize, usize), StylePreprocessError> {
    let mut commas = 0usize;
    let mut colons = 0usize;
    visit_scoped_selector_punctuation(prelude, |_, punctuation, _| {
        let count = if punctuation == ',' {
            &mut commas
        } else {
            &mut colons
        };
        *count = count.checked_add(1).ok_or_else(|| {
            let message = if punctuation == ',' {
                "scoped style selector branch count overflowed"
            } else {
                "scoped style selector node count overflowed"
            };
            StylePreprocessError::scoped_limit(message)
        })?;
        Ok(())
    })?;
    Ok((commas, colons))
}

fn visit_scoped_selector_punctuation(
    selector: &str,
    mut visitor: impl FnMut(usize, char, usize) -> Result<(), StylePreprocessError>,
) -> Result<(), StylePreprocessError> {
    let mut state = SelectorScannerState::Normal;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut index = 0usize;
    while index < selector.len() {
        let Some(ch) = selector[index..].chars().next() else {
            break;
        };
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
                '\\' => {
                    index = consume_selector_escape(selector, index);
                    continue;
                }
                '/' if selector[index..].starts_with("/*") => {
                    index = skip_selector_comment(selector, index);
                    continue;
                }
                '(' => paren_depth = paren_depth.saturating_add(1),
                ')' => paren_depth = paren_depth.saturating_sub(1),
                '[' => bracket_depth = bracket_depth.saturating_add(1),
                ']' => bracket_depth = bracket_depth.saturating_sub(1),
                ',' | ':' if bracket_depth == 0 => visitor(index, ch, paren_depth)?,
                _ => {}
            },
            SelectorScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '\'' {
                    state = SelectorScannerState::Normal;
                }
            }
            SelectorScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = SelectorScannerState::Normal;
                }
            }
        }
        index += ch.len_utf8();
    }
    Ok(())
}

fn scoped_deep_container_copy_bytes(selector: &str) -> Result<usize, StylePreprocessError> {
    let names = [":is", ":where", ":not", ":has"];
    let Some(container) = find_top_level_pseudo_function(selector, &names) else {
        return Ok(0);
    };
    let Some((open, close)) = container.parens else {
        return Ok(0);
    };
    let inner = &selector[open + 1..close];
    if !selector_has_deep(inner) {
        return Ok(0);
    }
    let name = matched_selector_name(selector, container.start, &names).unwrap_or_default();
    let suffix = &selector[close + 1..];
    let has_trailing_nodes = !suffix.trim().is_empty();
    let has_scope_anchor = selector_scope_anchor_before(selector, container.start);
    let mut has_deep = false;
    let mut has_normal = false;
    let branches = visit_scoped_selector_branches(inner, |branch| {
        if selector_has_deep(branch.trim()) {
            has_deep = true;
        } else {
            has_normal = true;
        }
        Ok(())
    })?;

    if name == ":not" && has_deep && has_normal && !has_scope_anchor && has_trailing_nodes {
        return Ok(0);
    }

    let mut copy_bytes = 0usize;
    visit_scoped_selector_branches(inner, |branch| {
        copy_bytes = copy_bytes
            .checked_add(scoped_deep_container_copy_bytes(branch.trim())?)
            .ok_or_else(|| {
                StylePreprocessError::scoped_limit(
                    "scoped style deep selector copy size overflowed",
                )
            })?;
        Ok(())
    })?;

    let can_split = matches!(name, ":is" | ":where" | ":has");
    if can_split && has_deep && has_normal && !has_scope_anchor && has_trailing_nodes {
        let repeated_bytes = container
            .start
            .checked_add(name.len())
            .and_then(|bytes| bytes.checked_add(2))
            .and_then(|bytes| bytes.checked_add(suffix.len()))
            .and_then(|bytes| bytes.checked_add(2))
            .ok_or_else(|| {
                StylePreprocessError::scoped_limit(
                    "scoped style deep selector copy size overflowed",
                )
            })?;
        let extra_branches = branches.saturating_sub(1);
        copy_bytes = copy_bytes
            .checked_add(repeated_bytes.checked_mul(extra_branches).ok_or_else(|| {
                StylePreprocessError::scoped_limit(
                    "scoped style deep selector copy size overflowed",
                )
            })?)
            .ok_or_else(|| {
                StylePreprocessError::scoped_limit(
                    "scoped style deep selector copy size overflowed",
                )
            })?;
    }
    Ok(copy_bytes)
}

pub(crate) fn visit_scoped_selector_branches(
    selector: &str,
    mut visitor: impl FnMut(&str) -> Result<(), StylePreprocessError>,
) -> Result<usize, StylePreprocessError> {
    let mut branches = 0usize;
    let mut start = 0usize;
    visit_scoped_selector_punctuation(selector, |index, punctuation, paren_depth| {
        if punctuation == ',' && paren_depth == 0 {
            visitor(&selector[start..index])?;
            branches = branches.checked_add(1).ok_or_else(|| {
                StylePreprocessError::scoped_limit("scoped style selector branch count overflowed")
            })?;
            start = index + punctuation.len_utf8();
        }
        Ok(())
    })?;
    visitor(&selector[start..])?;
    branches.checked_add(1).ok_or_else(|| {
        StylePreprocessError::scoped_limit("scoped style selector branch count overflowed")
    })
}

fn scoped_selector_work_limit_error(limits: ScopedStyleLimits) -> StylePreprocessError {
    StylePreprocessError::scoped_limit(format!(
        "scoped style selector rewriting exceeds the maximum work budget of {} bytes",
        limits.max_selector_work_bytes
    ))
}

pub(crate) const DEPRECATED_DEEP_COMBINATOR_MESSAGE: &str =
    "the >>> and /deep/ combinators have been deprecated. Use :deep() instead.";
pub(crate) const DEPRECATED_DEEP_PSEUDO_MIDDLE: &str =
    " usage as a combinator has been deprecated. Use :deep(<inner-selector>) instead of ";
pub(crate) const DEPRECATED_DEEP_PSEUDO_SUFFIX: &str = " <inner-selector>.";

pub(crate) fn deprecated_deep_pseudo_message_bytes(
    value: &str,
) -> Result<usize, StylePreprocessError> {
    value
        .len()
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(DEPRECATED_DEEP_PSEUDO_MIDDLE.len()))
        .and_then(|bytes| bytes.checked_add(DEPRECATED_DEEP_PSEUDO_SUFFIX.len()))
        .ok_or_else(|| StylePreprocessError::scoped_limit("scoped style warning size overflowed"))
}

pub(crate) fn deprecated_deep_pseudo_message(value: &str) -> Result<String, StylePreprocessError> {
    let bytes = deprecated_deep_pseudo_message_bytes(value)?;
    let mut message = String::new();
    message.try_reserve_exact(bytes).map_err(|_| {
        StylePreprocessError::scoped_limit(
            "scoped style warning could not reserve capacity within the configured limit",
        )
    })?;
    message.push_str(value);
    message.push_str(DEPRECATED_DEEP_PSEUDO_MIDDLE);
    message.push_str(value);
    message.push_str(DEPRECATED_DEEP_PSEUDO_SUFFIX);
    Ok(message)
}

pub(crate) fn deprecated_scoped_selector_diagnostic(message: impl Into<String>) -> Diagnostic {
    Diagnostic::warning("VUEC_STYLE_DEPRECATED_SCOPED_SELECTOR", message)
}

pub(crate) fn scoped_selector_deprecation_warnings(
    source: &str,
    budget: &mut ScopedStyleBudget,
) -> Result<Vec<Diagnostic>, StylePreprocessError> {
    let mut warnings = Vec::new();
    collect_scoped_selector_deprecation_warnings(
        source,
        CssBlockContext::Root,
        &mut warnings,
        budget,
    )?;
    Ok(warnings)
}
