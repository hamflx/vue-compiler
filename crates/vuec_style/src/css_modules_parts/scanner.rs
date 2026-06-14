        let scoped = context.scoped_name(token.name);
        context.register_local(token.name, &scoped);
        output.push(token.sigil);
        output.push_str(&scoped);
        cursor = token.end;
    }
    output.push_str(&segment[cursor..]);
    output
}

pub(crate) fn register_css_module_globals(segment: &str, context: &mut CssModulesContext<'_>) {
    let mut cursor = 0usize;
    while let Some(token) = find_next_css_module_selector_token(segment, cursor) {
        context.register_global(token.name);
        cursor = token.end;
    }
}

pub(crate) fn register_css_module_icss_exports(body: &str, context: &mut CssModulesContext<'_>) {
    let mut segment_start = 0usize;
    for semicolon in top_level_semicolons(body) {
        register_css_module_icss_export_segment(&body[segment_start..semicolon], context);
        segment_start = semicolon + 1;
    }
    register_css_module_icss_export_segment(&body[segment_start..], context);
}

pub(crate) fn register_css_module_icss_export_segment(
    segment: &str,
    context: &mut CssModulesContext<'_>,
) {
    let Some(colon) = find_top_level_colon(segment) else {
        return;
    };
    let key = segment[..colon].trim();
    let value = segment[colon + 1..].trim();
    if key.is_empty() {
        return;
    }
    context.set_raw_export_values(key, vec![replace_css_module_export_symbols(value, context)]);
}

pub(crate) fn parse_css_module_import_prelude(prelude: &str) -> Option<&str> {
    let inner = prelude.strip_prefix(":import(")?.strip_suffix(')')?.trim();
    (!inner.is_empty()).then_some(inner)
}

pub(crate) fn register_css_module_icss_imports(
    import: &str,
    body: &str,
    context: &mut CssModulesContext<'_>,
) {
    let Some(result) = context.load_imported_module(import) else {
        return;
    };
    let mut segment_start = 0usize;
    for semicolon in top_level_semicolons(body) {
        register_css_module_icss_import_segment(
            &body[segment_start..semicolon],
            &result.raw_modules,
            context,
        );
        segment_start = semicolon + 1;
    }
    register_css_module_icss_import_segment(&body[segment_start..], &result.raw_modules, context);
}

pub(crate) fn register_css_module_icss_import_segment(
    segment: &str,
    modules: &BTreeMap<String, String>,
    context: &mut CssModulesContext<'_>,
) {
    let Some(colon) = find_top_level_colon(segment) else {
        return;
    };
    let local = segment[..colon].trim();
    let remote = segment[colon + 1..].trim();
    if local.is_empty() || remote.is_empty() {
        return;
    }
    let symbol = modules
        .get(remote)
        .cloned()
        .map(CssModuleImportSymbol::Found)
        .unwrap_or(CssModuleImportSymbol::Missing);
    context.import_symbols.insert(local.to_string(), symbol);
}

pub(crate) fn rewrite_css_module_declarations(
    prelude: &str,
    body: &str,
    context: &mut CssModulesContext<'_>,
    block_context: CssBlockContext,
    compose_local_names: &[String],
    body_offset: usize,
    native_nested_rule: bool,
) -> String {
    if matches!(block_context, CssBlockContext::Keyframes) {
        return body.to_string();
    }

    let mut output = String::new();
    let nested_compose_message =
        native_nested_rule.then(|| css_module_nested_compose_message(prelude, body, context));
    let mut nested_compose_reported = false;
    let mut segment_start = 0usize;
    for semicolon in top_level_semicolons(body) {
        rewrite_css_module_declaration_segment(
            &body[segment_start..semicolon],
            context,
            prelude,
            compose_local_names,
            body_offset + segment_start,
            true,
            nested_compose_message.as_deref(),
            &mut nested_compose_reported,
            &mut output,
        );
        segment_start = semicolon + 1;
    }
    rewrite_css_module_declaration_segment(
        &body[segment_start..],
        context,
        prelude,
        compose_local_names,
        body_offset + segment_start,
        false,
        nested_compose_message.as_deref(),
        &mut nested_compose_reported,
        &mut output,
    );
    output
}

pub(crate) fn rewrite_css_module_declaration_segment(
    segment: &str,
    context: &mut CssModulesContext<'_>,
    prelude: &str,
    compose_local_names: &[String],
    segment_offset: usize,
    has_semicolon: bool,
    nested_compose_message: Option<&str>,
    nested_compose_reported: &mut bool,
    output: &mut String,
) {
    let Some(colon) = find_top_level_colon(segment) else {
        output.push_str(segment);
        if has_semicolon {
            output.push(';');
        }
        return;
    };
    let prop = segment[..colon].trim();
    if !prop.eq_ignore_ascii_case("composes") && !prop.eq_ignore_ascii_case("compose-with") {
        let segment = rewrite_css_module_animation_declaration(segment, context);
        output.push_str(&replace_css_module_import_symbols(&segment, context));
        if has_semicolon {
            output.push(';');
        }
        return;
    }

    if let Some(message) = nested_compose_message {
        if !*nested_compose_reported {
            context.push_compose_diagnostic(
                message.to_string(),
                segment_offset,
                segment_offset + segment.len(),
            );
            *nested_compose_reported = true;
        }
        return;
    }

    if compose_local_names.is_empty() {
        let message = css_module_invalid_compose_selector_message(prelude, context);
        context.push_compose_diagnostic(message, segment_offset, segment_offset + segment.len());
        return;
    }
    match css_module_composed_values(&segment[colon + 1..], context, segment_offset + colon + 1) {
        CssModuleComposeResolution::Values(composed_values) => {
            if composed_values.is_empty() {
                output.push_str(segment);
                if has_semicolon {
                    output.push(';');
                }
                return;
            }

            for local_name in compose_local_names {
                context.compose(local_name, composed_values.clone());
            }
        }
        CssModuleComposeResolution::Unsupported => {
            output.push_str(segment);
            if has_semicolon {
                output.push(';');
            }
        }
        CssModuleComposeResolution::Invalid {
            class_name,
            start,
            end,
        } => {
            context.push_compose_diagnostic(
                format!("referenced class name \"{class_name}\" in {prop} not found"),
                start,
                end,
            );
        }
    }
}

pub(crate) fn rewrite_css_module_animation_declaration(
    segment: &str,
    context: &mut CssModulesContext<'_>,
) -> String {
    let Some(colon) = find_top_level_colon(segment) else {
        return segment.to_string();
    };
    let prop = segment[..colon].trim();
    if !is_animation_name_property(prop) && !is_animation_property(prop) {
        return segment.to_string();
    }
    let value_start = colon + 1;
    let value = &segment[value_start..];
    let leading_value_whitespace = value
        .char_indices()
        .find_map(|(index, ch)| (!ch.is_whitespace()).then_some(index))
        .unwrap_or(value.len());
    let value_prefix = &value[..leading_value_whitespace];
    let value_body = &value[leading_value_whitespace..];
    let rewritten = rewrite_css_module_animation_value(value_body.trim(), context);

    let mut output = String::new();
    output.push_str(&segment[..value_start]);
    output.push_str(value_prefix);
    output.push_str(&rewritten);
    output
}

pub(crate) fn rewrite_css_module_animation_value(
    value: &str,
    context: &mut CssModulesContext<'_>,
) -> String {
    split_selector_list(value)
        .into_iter()
        .map(|part| rewrite_css_module_animation_part(part.trim(), context))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn rewrite_css_module_animation_part(
    part: &str,
    context: &mut CssModulesContext<'_>,
) -> String {
    let mut parsed_keywords = BTreeMap::new();
    tokenize_css_module_animation_part(part)
        .into_iter()
        .map(|token| rewrite_css_module_animation_token(token, context, &mut parsed_keywords))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn rewrite_css_module_animation_token(
    token: &str,
    context: &mut CssModulesContext<'_>,
    parsed_keywords: &mut BTreeMap<String, usize>,
) -> String {
    if let Some(global) = parse_css_module_animation_function(token, "global") {
        return global.to_string();
    }
    if let Some(local) = parse_css_module_animation_function(token, "local") {
        return context.scoped_local_value(local);
    }
    if let Some(replacement) = context.value_placeholder_replacement(token) {
        return replacement.to_string();
    }
    if !context.is_local_default()
        || context.import_symbol_is_imported(token)
        || !is_css_module_animation_identifier(token)
    {
        return token.to_string();
    }
    let lower = token.to_ascii_lowercase();
    if let Some(limit) = css_module_animation_keyword_limit(&lower) {
        let count = parsed_keywords.entry(lower).or_insert(0);
        let should_localize = *count >= limit;
        *count = count.saturating_add(1);
        if !should_localize {
            return token.to_string();
        }
    }
    context.scoped_local_value(token)
}

pub(crate) fn tokenize_css_module_animation_part(part: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut state = CssScannerState::Normal;
    let mut paren_depth = 0usize;
    let mut token_start = None;
    let mut index = 0usize;
    while index < part.len() {
        let Some(ch) = part[index..].chars().next() else {
            break;
        };
        match state {
            CssScannerState::Normal => {
                if ch.is_whitespace() && paren_depth == 0 {
                    if let Some(start) = token_start.take() {
                        tokens.push(&part[start..index]);
                    }
                    index += ch.len_utf8();
                    continue;
                }
                if token_start.is_none() {
                    token_start = Some(index);
                }
                match ch {
                    '\'' => state = CssScannerState::SingleQuote,
                    '"' => state = CssScannerState::DoubleQuote,
                    '(' => paren_depth += 1,
                    ')' if paren_depth > 0 => paren_depth -= 1,
                    _ => {}
                }
            }
            CssScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < part.len() {
                        index += part[index..].chars().next().map_or(0, char::len_utf8);
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
                    if index < part.len() {
                        index += part[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::BlockComment => {}
        }
        index += ch.len_utf8();
    }
    if let Some(start) = token_start {
        tokens.push(&part[start..]);
    }
    tokens
}

pub(crate) fn parse_css_module_animation_function<'a>(
    token: &'a str,
    name: &str,
) -> Option<&'a str> {
    let inner = token
        .strip_prefix(name)?
        .strip_prefix('(')?
        .strip_suffix(')')?
        .trim();
    (!inner.is_empty()).then_some(inner)
}

pub(crate) fn is_css_module_animation_identifier(token: &str) -> bool {
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if first == '-' {
        let Some(second) = chars.next() else {
            return false;
        };
        if second.is_ascii_digit() {
            return false;
        }
        if !is_css_module_identifier_start(second) && second != '-' {
            return false;
        }
    } else if !is_css_module_identifier_start(first) {
        return false;
    }
    chars.all(is_css_module_identifier_continue)
}

pub(crate) fn is_css_module_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic() || !ch.is_ascii()
}

pub(crate) fn is_css_module_identifier_continue(ch: char) -> bool {
    is_css_module_identifier_start(ch) || ch.is_ascii_digit() || ch == '-'
}

pub(crate) fn css_module_animation_keyword_limit(value: &str) -> Option<usize> {
    match value {
        "normal" | "reverse" | "alternate" | "alternate-reverse" | "forwards" | "backwards"
        | "both" | "infinite" | "paused" | "running" | "ease" | "ease-in" | "ease-out"
        | "ease-in-out" | "linear" | "step-end" | "step-start" => Some(1),
        "none" | "initial" | "inherit" | "unset" | "revert" | "revert-layer" => Some(usize::MAX),
        _ => None,
    }
}

pub(crate) fn css_module_invalid_compose_selector_message(
    prelude: &str,
    context: &CssModulesContext<'_>,
) -> String {
    let selector = css_module_localized_selector_for_message(prelude, context);
    format!("composition is only allowed when selector is single :local class name not in \"{selector}\"")
}

pub(crate) fn css_module_nested_compose_message(
    prelude: &str,
    body: &str,
    context: &CssModulesContext<'_>,
) -> String {
    let selector = css_module_localized_selector_for_message(prelude, context);
    let mut body = css_module_nested_compose_message_body(body);
    if !body.ends_with(';') {
        body.push(';');
    }
    format!("composition is not allowed in nested rule \n\n{selector} {{ {body}\n}}")
}

pub(crate) fn css_module_nested_compose_message_body(body: &str) -> String {
    let mut output = Vec::new();
    let mut segment_start = 0usize;
    for semicolon in top_level_semicolons(body) {
        let segment = css_module_nested_compose_message_segment(&body[segment_start..semicolon]);
        if !segment.is_empty() {
            output.push(format!("{segment};"));
        }
        segment_start = semicolon + 1;
    }
    let segment = css_module_nested_compose_message_segment(&body[segment_start..]);
    if !segment.is_empty() {
        output.push(segment);
    }
    output.join(" ")
}

pub(crate) fn css_module_nested_compose_message_segment(segment: &str) -> String {
    let segment = normalize_style_output(segment).trim().to_string();
    let Some(colon) = find_top_level_colon(&segment) else {
        return segment;
    };
    let prop = segment[..colon].trim();
    if !prop.eq_ignore_ascii_case("composes") && !prop.eq_ignore_ascii_case("compose-with") {
        return segment;
    }
    let value = css_module_nested_compose_message_value(segment[colon + 1..].trim());
    format!("{prop}: {value}")
}

pub(crate) fn css_module_nested_compose_message_value(value: &str) -> String {
    let mut output = Vec::new();
    for part in value.split(',') {
        let tokens = css_module_compose_tokens(part, value, 0);
        if let Some(from_index) = tokens.iter().position(|token| token.value == "from") {
            if from_index > 0
                && from_index + 2 == tokens.len()
                && tokens[from_index + 1].value == "global"
            {
                output.push(
                    tokens[..from_index]
                        .iter()
                        .map(|token| {
                            if parse_css_module_global_compose(token.value).is_some() {
                                token.value.to_string()
                            } else {
                                format!("global({})", token.value)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" "),
                );
                continue;
            }
        }
        output.push(part.trim().to_string());
    }
    output.join(", ")
}

pub(crate) fn css_module_localized_selector_for_message(
    prelude: &str,
    context: &CssModulesContext<'_>,
) -> String {
    split_selector_list(prelude)
        .into_iter()
        .map(|selector| css_module_localized_selector_part_for_message(selector.trim(), context))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn css_module_localized_selector_part_for_message(
    selector: &str,
    context: &CssModulesContext<'_>,
) -> String {
    if !context.is_local_default() {
        return selector.to_string();
    }
    css_module_localized_selector_segment_for_message(selector, true)
}

pub(crate) fn css_module_localized_selector_segment_for_message(
    selector: &str,
    mut default_local: bool,
) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;
    while cursor < selector.len() {
        if let Some(global) =
            find_pseudo_function_from(selector, &[":global", "::v-global"], cursor)
        {
            output.push_str(&css_module_localized_default_segment_for_message(
                &selector[cursor..global.start],
                default_local,
            ));
            if let Some((open, close)) = global.parens {
                output.push_str(":global(");
                output.push_str(selector[open + 1..close].trim());
                output.push(')');
                cursor = close + 1;
                continue;
            }
            output.push_str(&selector[global.start..global.end]);
            cursor = global.end;
            default_local = false;
            continue;
        }
        if let Some(local) = find_pseudo_function_from(selector, &[":local", "::v-local"], cursor) {
            output.push_str(&css_module_localized_default_segment_for_message(
                &selector[cursor..local.start],
                default_local,
            ));
            if let Some((open, close)) = local.parens {
                output.push_str(":local(");
                output.push_str(selector[open + 1..close].trim());
                output.push(')');
                cursor = close + 1;
                continue;
            }
            output.push_str(&selector[local.start..local.end]);
            cursor = local.end;
            default_local = true;
            continue;
        }
        output.push_str(&css_module_localized_default_segment_for_message(
            &selector[cursor..],
            default_local,
        ));
        break;
    }
    output
}

pub(crate) fn css_module_localized_default_segment_for_message(
    segment: &str,
    local: bool,
) -> String {
    if !local {
        return segment.to_string();
    }
    let mut output = String::new();
    let mut cursor = 0usize;
    while let Some(token) = find_next_css_module_selector_token(segment, cursor) {
        output.push_str(&segment[cursor..token.start]);
        output.push_str(":local(");
        output.push(token.sigil);
        output.push_str(token.name);
        output.push(')');
        cursor = token.end;
    }
    output.push_str(&segment[cursor..]);
    output
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CssModuleComposeResolution {
    Values(Vec<String>),
    Unsupported,
    Invalid {
        class_name: String,
        start: usize,
        end: usize,
    },
}

pub(crate) fn unsupported_css_module_compose() -> CssModuleComposeResolution {
    CssModuleComposeResolution::Unsupported
}

pub(crate) fn invalid_css_module_compose(
    class_name: &str,
    start: usize,
    end: usize,
) -> CssModuleComposeResolution {
    CssModuleComposeResolution::Invalid {
        class_name: class_name.to_string(),
        start,
        end,
    }
}

pub(crate) fn css_module_composed_values(
    value: &str,
    context: &mut CssModulesContext<'_>,
    value_offset: usize,
) -> CssModuleComposeResolution {
    let mut composed = Vec::new();
    for part in value.split(',') {
        let tokens = css_module_compose_tokens(part, value, value_offset);
        if let Some(from_index) = tokens.iter().position(|token| token.value == "from") {
            if from_index == 0 || from_index + 2 != tokens.len() {
                return unsupported_css_module_compose();
            }
            let import = tokens[from_index + 1].value;
            if import == "global" {
                for token in &tokens[..from_index] {
                    push_unique_css_module_value(&mut composed, token.value.to_string());
                }
            } else {
                for token in &tokens[..from_index] {
                    let Some(values) =
                        css_module_external_composed_values(token.value, import, context)
                    else {
                        return unsupported_css_module_compose();
                    };
                    for value in values {
                        push_unique_css_module_value(&mut composed, value);
                    }
                }
            }
            continue;
        }
        for token in tokens {
            let class_name = token.value;
            if let Some(global) = parse_css_module_global_compose(class_name) {
                push_unique_css_module_value(&mut composed, global);
            } else if let Some(values) = context.raw_export_values(class_name) {
                for value in values {
                    push_unique_css_module_value(&mut composed, value);
                }
            } else if let Some(value) = context.value_placeholder_module_value(class_name) {
                push_unique_css_module_value(&mut composed, value.to_string());
            } else if let Some(value) = context.import_symbol_module_value(class_name) {
                push_unique_css_module_value(&mut composed, value);
            } else if class_name.starts_with('"') || class_name.starts_with('\'') {
                return unsupported_css_module_compose();
            } else {
                return invalid_css_module_compose(class_name, token.start, token.end);
            }
        }
    }
    CssModuleComposeResolution::Values(composed)
}

#[derive(Debug)]
pub(crate) struct CssModuleComposeToken<'a> {
    pub(crate) value: &'a str,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

pub(crate) fn css_module_compose_tokens<'a>(
    part: &'a str,
    value: &'a str,
    value_offset: usize,
) -> Vec<CssModuleComposeToken<'a>> {
    let part_offset = part.as_ptr() as usize - value.as_ptr() as usize;
    let mut tokens = Vec::new();
    let mut cursor = 0usize;
    while cursor < part.len() {
        cursor = skip_css_whitespace(part, cursor);
        if cursor >= part.len() {
            break;
        }
        let start = cursor;
        while cursor < part.len() {
            let Some(ch) = part[cursor..].chars().next() else {
                break;
            };
            if ch.is_whitespace() {
                break;
            }
            cursor += ch.len_utf8();
        }
        tokens.push(CssModuleComposeToken {
            value: &part[start..cursor],
            start: value_offset + part_offset + start,
            end: value_offset + part_offset + cursor,
        });
    }
    tokens
}

pub(crate) fn css_module_composable_local_names(
    prelude: &str,
    context: &CssModulesContext<'_>,
) -> Vec<String> {
    if prelude.starts_with('@') {
        return Vec::new();
    }
    let mut names = Vec::new();
    for selector in split_selector_list(prelude) {
        let Some(name) =
            css_module_composable_local_name(selector.trim(), context.is_local_default())
        else {
            return Vec::new();
        };
        names.push(name);
    }
    names
}

pub(crate) fn css_module_composable_local_name(
    selector: &str,
    default_local: bool,
) -> Option<String> {
    if let Some(local) = find_pseudo_function(selector, &[":local", "::v-local"]) {
        if local.start == 0 && local.end == selector.len() {
            let (open, close) = local.parens?;
            return css_module_single_class_selector_name(selector[open + 1..close].trim());
        }
    }
    if default_local {
        css_module_single_class_selector_name(selector)
    } else {
        None
    }
}

pub(crate) fn css_module_single_class_selector_name(selector: &str) -> Option<String> {
    let token = find_next_css_module_selector_token(selector, 0)?;
    (token.sigil == '.' && token.start == 0 && token.end == selector.len())
        .then(|| token.name.to_string())
}

pub(crate) fn css_module_external_composed_values(
    class_name: &str,
    import: &str,
    context: &mut CssModulesContext<'_>,
) -> Option<Vec<String>> {
    let result = context.load_imported_module(import)?;
    Some(
        result
            .raw_modules
            .get(class_name)
            .map(|value| value.split_whitespace().map(ToOwned::to_owned).collect())
            .unwrap_or_else(|| vec!["undefined".to_string()]),
    )
}

pub(crate) fn parse_css_module_global_compose(value: &str) -> Option<String> {
    let inner = value.strip_prefix("global(")?.strip_suffix(')')?;
    if inner.is_empty() {
        None
    } else {
        Some(inner.to_string())
    }
}

pub(crate) fn push_unique_css_module_value(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

pub(crate) fn replace_css_module_import_symbols(
    segment: &str,
    context: &CssModulesContext<'_>,
) -> String {
    if context.import_symbols.is_empty() {
        return segment.to_string();
    }
    let Some(colon) = find_top_level_colon(segment) else {
        return segment.to_string();
    };
    let value = &segment[colon + 1..];
    let replaced = replace_css_module_import_symbols_in_text(value, context);
    let mut output = String::new();
    output.push_str(&segment[..colon + 1]);
    output.push_str(&replaced);
    output
}

pub(crate) fn replace_css_module_import_symbols_in_text(
    source: &str,
    context: &CssModulesContext<'_>,
) -> String {
    if context.import_symbols.is_empty() {
        return source.to_string();
    }
    let symbols = context
        .import_symbols
        .iter()
        .filter_map(|(name, symbol)| match symbol {
            CssModuleImportSymbol::Found(value) => Some((name.clone(), value.clone())),
            CssModuleImportSymbol::Missing => None,
        })
        .collect::<BTreeMap<_, _>>();
    replace_css_module_value_symbols(source, &symbols)
}

pub(crate) fn replace_css_module_export_symbols(
    source: &str,
    context: &CssModulesContext<'_>,
) -> String {
    if context.import_symbols.is_empty() {
        return source.to_string();
    }
    let symbols = context
        .import_symbols
        .iter()
        .map(|(name, symbol)| {
            let value = match symbol {
                CssModuleImportSymbol::Found(value) => value.clone(),
                CssModuleImportSymbol::Missing => "undefined".to_string(),
            };
            (name.clone(), value)
        })
        .collect::<BTreeMap<_, _>>();
    replace_css_module_value_symbols(source, &symbols)
}

pub(crate) fn replace_css_module_value_symbols(
    value: &str,
    symbols: &BTreeMap<String, String>,
) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;
    while cursor < value.len() {
        let Some((start, end, token)) = find_next_css_module_symbol(value, cursor) else {
            output.push_str(&value[cursor..]);
            break;
        };
        output.push_str(&value[cursor..start]);
        if let Some(replacement) = symbols.get(token) {
            output.push_str(replacement);
        } else {
            output.push_str(token);
        }
        cursor = end;
    }
    output
}

pub(crate) fn find_next_css_module_symbol(
    source: &str,
    mut cursor: usize,
) -> Option<(usize, usize, &str)> {
    while cursor < source.len() {
        let ch = source[cursor..].chars().next()?;
        if ch == '$' || ch == '_' || ch == '-' || ch.is_ascii_alphanumeric() {
            let start = cursor;
            cursor += ch.len_utf8();
            while cursor < source.len() {
                let next = source[cursor..].chars().next()?;
                if next == '_' || next == '-' || next.is_ascii_alphanumeric() {
                    cursor += next.len_utf8();
                } else {
                    break;
                }
            }
            return Some((start, cursor, &source[start..cursor]));
        }
        cursor += ch.len_utf8();
    }
    None
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CssModuleResolvedImport {
    pub(crate) path: PathBuf,
    pub(crate) logical_filename: String,
}

pub(crate) fn resolve_css_module_import(
    import: &str,
    filename: &str,
) -> Option<CssModuleResolvedImport> {
    let import = unquote_css_module_path(import);
    let import_path = Path::new(&import);
    let importer_dir = Path::new(filename)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    if import_path.is_absolute() {
        return css_module_resolved_import(import_path.to_path_buf(), import_path.to_path_buf());
    }

    if is_relative_css_module_import(&import) {
        let logical = importer_dir.join(import_path);
        return css_module_resolved_import(logical.clone(), logical);
    }

    resolve_css_module_node_modules_import(&import, importer_dir).or_else(|| {
        let logical = importer_dir.join(import_path);
        css_module_resolved_import(logical.clone(), logical)
    })
}

pub(crate) fn css_module_resolved_import(
    path: PathBuf,
    logical_filename: PathBuf,
) -> Option<CssModuleResolvedImport> {
    if !path.is_file() {
        return None;
    }
    Some(CssModuleResolvedImport {
        path: std::fs::canonicalize(&path).unwrap_or(path),
        logical_filename: logical_filename.to_string_lossy().to_string(),
    })
}

pub(crate) fn is_relative_css_module_import(import: &str) -> bool {
    import.starts_with("./") || import.starts_with("../") || import == "." || import == ".."
}

pub(crate) fn resolve_css_module_node_modules_import(
    import: &str,
    importer_dir: &Path,
) -> Option<CssModuleResolvedImport> {
    let (package_name, subpath) = split_css_module_package_specifier(import)?;
    for dir in css_module_import_ancestor_dirs(importer_dir) {
        let package_dir = dir.join("node_modules").join(&package_name);
        if !package_dir.is_dir() {
            continue;
        }
        let path = if subpath.as_os_str().is_empty() {
            css_module_package_main_file(&package_dir)?
        } else {
            match css_module_package_exports_file(&package_dir, &subpath) {
                CssModulePackageExportsResolution::Resolved(path) => path,
                CssModulePackageExportsResolution::Blocked => return None,
                CssModulePackageExportsResolution::NoExports => package_dir.join(&subpath),
            }
        };
        let logical = importer_dir.join(import);
        if let Some(resolved) = css_module_resolved_import(path, logical) {
            return Some(resolved);
        }
    }
    None
}

pub(crate) fn split_css_module_package_specifier(import: &str) -> Option<(String, PathBuf)> {
    if import.is_empty() || import.starts_with('/') || import.starts_with('\\') {
        return None;
    }
    let parts = import.split('/').collect::<Vec<_>>();
    if parts.first().is_some_and(|part| part.is_empty()) {
        return None;
    }
    if import.starts_with('@') {
        let scope = *parts.first()?;
        let name = *parts.get(1)?;
        if scope.len() <= 1 || name.is_empty() {
            return None;
        }
        let package = format!("{scope}/{name}");
        let subpath = parts.iter().skip(2).collect::<PathBuf>();
        Some((package, subpath))
    } else {
        let package = (*parts.first()?).to_string();
        let subpath = parts.iter().skip(1).collect::<PathBuf>();
        Some((package, subpath))
    }
}

pub(crate) fn css_module_import_ancestor_dirs(importer_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let start = if importer_dir.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        importer_dir.to_path_buf()
    };
    for ancestor in start.ancestors() {
        dirs.push(ancestor.to_path_buf());
    }
    dirs
}

pub(crate) fn css_module_package_main_file(package_dir: &Path) -> Option<PathBuf> {
    match css_module_package_exports_file(package_dir, Path::new("")) {
        CssModulePackageExportsResolution::Resolved(path) => return Some(path),
        CssModulePackageExportsResolution::Blocked => return None,
        CssModulePackageExportsResolution::NoExports => {}
    }
    let package_json = package_dir.join("package.json");
    if let Ok(source) = std::fs::read_to_string(package_json) {
        if let Ok(value) = serde_json::from_str::<CssModulePackageJson>(&source) {
            if let Some(main) = value.main {
                let candidate = package_dir.join(main);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    let index_css = package_dir.join("index.css");
    index_css.is_file().then_some(index_css)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CssModulePackageExportsResolution {
    NoExports,
    Resolved(PathBuf),
    Blocked,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CssModulePackageJson {
    #[serde(default)]
    pub(crate) main: Option<String>,
    #[serde(default)]
    pub(crate) exports: Option<CssModulePackageJsonValue>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum CssModulePackageJsonValue {
    String(String),
    Object(CssModulePackageJsonObject),
    Other,
}
