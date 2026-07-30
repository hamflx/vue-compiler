use crate::*;

pub(crate) const STYLE_SCOPED_MAX_SOURCE_BYTES: usize = STYLE_PREPROCESS_MAX_OUTPUT_BYTES;
pub(crate) const STYLE_SCOPED_MAX_SCOPE_ID_BYTES: usize = 32 * 1024;
pub(crate) const STYLE_SCOPED_MAX_SYNTAX_DEPTH: usize = 128;
pub(crate) const STYLE_SCOPED_MAX_RECURSIVE_SCAN_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ScopedStyleLimits {
    pub(crate) max_source_bytes: usize,
    pub(crate) max_scope_id_bytes: usize,
    pub(crate) max_syntax_depth: usize,
    pub(crate) max_recursive_scan_bytes: usize,
}

impl Default for ScopedStyleLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: STYLE_SCOPED_MAX_SOURCE_BYTES,
            max_scope_id_bytes: STYLE_SCOPED_MAX_SCOPE_ID_BYTES,
            max_syntax_depth: STYLE_SCOPED_MAX_SYNTAX_DEPTH,
            max_recursive_scan_bytes: STYLE_SCOPED_MAX_RECURSIVE_SCAN_BYTES,
        }
    }
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
    validate_scoped_style_resources(source, scope_id, limits)?;
    Ok(rewrite_scoped_selectors_unchecked(source, scope_id))
}

pub(crate) fn rewrite_scoped_selectors_unchecked(source: &str, scope_id: &str) -> String {
    let short_id = scope_id.strip_prefix("data-v-").unwrap_or(scope_id);
    let keyframes = collect_scoped_keyframes(source, short_id);
    rewrite_css_items(source, scope_id, &keyframes, CssBlockContext::Root)
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
    let mut paren_opens = Vec::new();
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
                    '{' => {
                        push_scoped_syntax_open(&mut brace_opens, paren_opens.len(), index, limits)?
                    }
                    '(' => {
                        push_scoped_syntax_open(&mut paren_opens, brace_opens.len(), index, limits)?
                    }
                    '}' => {
                        if let Some(open) = brace_opens.pop() {
                            claim_scoped_recursive_span(
                                &mut recursive_scan_bytes,
                                open,
                                index,
                                limits,
                            )?;
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

pub(crate) const DEPRECATED_DEEP_COMBINATOR_MESSAGE: &str =
    "the >>> and /deep/ combinators have been deprecated. Use :deep() instead.";

pub(crate) fn deprecated_deep_pseudo_message(value: &str) -> String {
    format!(
        "{value} usage as a combinator has been deprecated. Use :deep(<inner-selector>) instead of {value} <inner-selector>."
    )
}

pub(crate) fn deprecated_scoped_selector_diagnostic(message: impl Into<String>) -> Diagnostic {
    Diagnostic::warning("VUEC_STYLE_DEPRECATED_SCOPED_SELECTOR", message)
}

pub(crate) fn scoped_selector_deprecation_warnings(source: &str) -> Vec<Diagnostic> {
    let mut warnings = Vec::new();
    collect_scoped_selector_deprecation_warnings(source, CssBlockContext::Root, &mut warnings);
    warnings
        .into_iter()
        .map(deprecated_scoped_selector_diagnostic)
        .collect()
}
