use crate::*;

/// Rewrites selectors in `source` to include `scope_id`.
pub fn rewrite_scoped_selectors(source: &str, scope_id: &str) -> String {
    let short_id = scope_id.strip_prefix("data-v-").unwrap_or(scope_id);
    let keyframes = collect_scoped_keyframes(source, short_id);
    rewrite_css_items(source, scope_id, &keyframes, CssBlockContext::Root)
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
