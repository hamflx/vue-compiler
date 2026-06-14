fn has_duplicate_attr(attrs: &[Vue2Attribute]) -> bool {
    let mut seen = BTreeMap::new();
    for attr in attrs {
        if seen.insert(attr.name.clone(), true).is_some() {
            return true;
        }
    }
    false
}

fn vue2_warning(code: &str, message: impl Into<String>, span: Option<Span>) -> Diagnostic {
    Diagnostic::vue2_warning(code, message, span)
}

fn vue2_tip(code: &str, message: impl Into<String>, span: Option<Span>) -> Diagnostic {
    Diagnostic::vue2_tip(code, message, span)
}

fn vue2_error(code: &str, message: impl Into<String>, span: Option<Span>) -> Diagnostic {
    Diagnostic::vue2_error(code, message, span)
}

fn split_compilation_issues(
    diagnostics: &DiagnosticSink,
    source: &str,
    leading_space_len: usize,
) -> (Vec<Vue2Error>, Vec<Vue2Warning>) {
    let mut errors = Vec::new();
    let mut tips = Vec::new();
    for diagnostic in diagnostics.as_slice() {
        match diagnostic.severity {
            Severity::Error | Severity::Warning => errors.push(Vue2Error {
                msg: diagnostic.message.clone(),
                start: vue2_issue_start(diagnostic, source, leading_space_len),
                end: vue2_issue_end(diagnostic, source, leading_space_len),
            }),
            Severity::Tip | Severity::Note => tips.push(Vue2Warning {
                msg: diagnostic.message.clone(),
                start: vue2_issue_start(diagnostic, source, leading_space_len),
                end: vue2_issue_end(diagnostic, source, leading_space_len),
                tip: matches!(diagnostic.severity, Severity::Tip),
            }),
        }
    }
    (errors, tips)
}

fn vue2_issue_start(
    diagnostic: &Diagnostic,
    source: &str,
    leading_space_len: usize,
) -> Option<usize> {
    diagnostic
        .span
        .map(|span| vue2_public_source_offset(source, leading_space_len + span.start.0))
}

fn vue2_issue_end(
    diagnostic: &Diagnostic,
    source: &str,
    leading_space_len: usize,
) -> Option<usize> {
    if diagnostic.code == "W_VUE2_TEXT_OUTSIDE_ROOT"
        && diagnostic
            .message
            .contains("requires a root element, rather than just text")
    {
        return None;
    }
    if diagnostic.code == "W_VUE2_MULTIPLE_ROOTS" {
        return None;
    }
    diagnostic
        .span
        .map(|span| vue2_public_source_offset(source, leading_space_len + span.end.0))
}

fn vue2_public_source_offset(source: &str, byte_offset: usize) -> usize {
    let mut offset = byte_offset.min(source.len());
    while offset > 0 && !source.is_char_boundary(offset) {
        offset -= 1;
    }
    source[..offset].encode_utf16().count()
}

fn render_diagnostic_message(diagnostic: &Diagnostic) -> String {
    match diagnostic.span {
        Some(span) => format!(
            "[{}] {} @ {}:{}-{}:{}",
            diagnostic.code,
            diagnostic.message,
            span.file_id.0,
            span.start.0,
            span.file_id.0,
            span.end.0
        ),
        None => format!("[{}] {}", diagnostic.code, diagnostic.message),
    }
}

impl Vue2Element {
    fn clone_without_conditions(&self) -> Self {
        let mut clone = self.clone();
        clone.if_exp = None;
        clone.if_span = None;
        clone.elseif = None;
        clone.elseif_span = None;
        clone.else_branch = false;
        clone.else_span = None;
        clone.if_conditions = Vec::new();
        clone
    }
}
