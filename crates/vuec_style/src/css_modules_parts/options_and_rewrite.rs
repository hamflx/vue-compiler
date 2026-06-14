#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CssModulesScopeBehaviour {
    Local,
    Global,
}

impl CssModulesScopeBehaviour {
    pub(crate) fn from_options(
        scope_behaviour: &str,
        global_module_paths: &[String],
        filename: &str,
    ) -> Self {
        if scope_behaviour.eq_ignore_ascii_case("global")
            || css_module_filename_matches_global_pattern(filename, global_module_paths)
        {
            Self::Global
        } else {
            Self::Local
        }
    }
}

pub(crate) fn css_module_filename_matches_global_pattern(
    filename: &str,
    patterns: &[String],
) -> bool {
    if patterns.is_empty() {
        return false;
    }
    let normalized = filename.replace('\\', "/");
    patterns.iter().any(|pattern| {
        regex::Regex::new(pattern)
            .map(|compiled| compiled.is_match(filename) || compiled.is_match(&normalized))
            .unwrap_or(false)
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CssModulesLocalsConvention {
    AsIs,
    CamelCase,
    CamelCaseOnly,
    Dashes,
    DashesOnly,
}

impl CssModulesLocalsConvention {
    pub(crate) fn from_option(value: &str) -> Self {
        match value {
            "camelCase" | "camel-case" => Self::CamelCase,
            "camelCaseOnly" | "camel-case-only" => Self::CamelCaseOnly,
            "dashes" => Self::Dashes,
            "dashesOnly" | "dashes-only" => Self::DashesOnly,
            _ => Self::AsIs,
        }
    }
}

pub(crate) fn rewrite_css_modules_items(
    source: &str,
    context: &mut CssModulesContext<'_>,
    block_context: CssBlockContext,
    native_nested_rule: bool,
) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;
    while cursor < source.len() {
        let output_len_before_whitespace = output.len();
        let whitespace_start = cursor;
        cursor = skip_css_whitespace(source, cursor);
        if cursor > whitespace_start {
            push_normalized_css_whitespace(&mut output, &source[whitespace_start..cursor]);
        }
        if cursor >= source.len() {
            break;
        }
        if source[cursor..].starts_with("/*") {
            let Some(end_offset) = source[cursor + 2..].find("*/") else {
                output.push_str(&source[cursor..]);
                break;
            };
            let end = cursor + 2 + end_offset + 2;
            output.push_str(&source[cursor..end]);
            cursor = end;
            continue;
        }

        let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(source, cursor) else {
            output.push_str(&source[cursor..]);
            break;
        };
        let raw_prelude = &source[cursor..delimiter];
        let prelude_end = raw_prelude.trim_end().len();
        let prelude = raw_prelude[..prelude_end].trim();
        let brace_spacing = &raw_prelude[prelude_end..];
        if delimiter_ch == ';' {
            output.push_str(prelude);
            output.push(';');
            cursor = delimiter + 1;
            continue;
        }

        let Some(close) = find_matching_brace(source, delimiter) else {
            output.push_str(&source[cursor..]);
            break;
        };
        let body = &source[delimiter + 1..close];
        let compose_local_names = css_module_composable_local_names(prelude, context);
        if let Some(import) = parse_css_module_import_prelude(prelude) {
            output.truncate(output_len_before_whitespace);
            register_css_module_icss_imports(import, body, context);
            cursor = skip_css_whitespace(source, close + 1);
            continue;
        }
        if prelude == ":export" {
            output.truncate(output_len_before_whitespace);
            register_css_module_icss_exports(body, context);
            cursor = skip_css_whitespace(source, close + 1);
            continue;
        }
        let rewritten_prelude = if prelude.starts_with('@') {
            rewrite_css_module_at_rule_prelude(prelude, context)
        } else {
            rewrite_css_modules_prelude(prelude, context, block_context)
        };
        output.push_str(&rewritten_prelude);
        output.push_str(brace_spacing);
        output.push('{');
        if prelude.starts_with('@') {
            let next_context = if is_keyframes_at_rule(prelude) {
                CssBlockContext::Keyframes
            } else {
                CssBlockContext::Container
            };
            let rewritten_body =
                rewrite_css_modules_items(body, context, next_context, native_nested_rule);
            if css_block_contains_style_rules(&rewritten_body)
                || css_block_contains_at_rule_with_style_rules(&rewritten_body)
            {
                output.push('\n');
                output.push_str(rewritten_body.trim());
                output.push('\n');
            } else {
                output.push_str(&rewritten_body);
            }
        } else {
            output.push_str(&rewrite_css_module_rule_body(
                prelude,
                body,
                context,
                block_context,
                &compose_local_names,
                delimiter + 1,
                native_nested_rule,
            ));
        }
        output.push('}');
        cursor = close + 1;
    }
    output
}

pub(crate) fn rewrite_css_module_rule_body(
    prelude: &str,
    body: &str,
    context: &mut CssModulesContext<'_>,
    block_context: CssBlockContext,
    compose_local_names: &[String],
    body_offset: usize,
    native_nested_rule: bool,
) -> String {
    if !css_block_has_nested_block(body) {
        return rewrite_css_module_declarations(
            prelude,
            body,
            context,
            block_context,
            compose_local_names,
            body_offset,
            native_nested_rule,
        );
    }

    let mut output = String::new();
    let mut declarations = String::new();
    let mut declarations_offset = None;
    let mut cursor = 0usize;
    while cursor < body.len() {
        let whitespace_start = cursor;
        cursor = skip_css_whitespace(body, cursor);
        if cursor > whitespace_start {
            declarations_offset.get_or_insert(body_offset + whitespace_start);
            push_normalized_css_whitespace(&mut declarations, &body[whitespace_start..cursor]);
        }
        if cursor >= body.len() {
            break;
        }
        if body[cursor..].starts_with("/*") {
            let Some(end_offset) = body[cursor + 2..].find("*/") else {
                declarations_offset.get_or_insert(body_offset + cursor);
                declarations.push_str(&body[cursor..]);
                break;
            };
            let end = cursor + 2 + end_offset + 2;
            declarations_offset.get_or_insert(body_offset + cursor);
            declarations.push_str(&body[cursor..end]);
            cursor = end;
            continue;
        }

        let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(body, cursor) else {
            declarations_offset.get_or_insert(body_offset + cursor);
            declarations.push_str(&body[cursor..]);
            break;
        };
        let raw_prelude = &body[cursor..delimiter];
        let prelude_end = raw_prelude.trim_end().len();
        let nested_prelude = raw_prelude[..prelude_end].trim();
        let brace_spacing = &raw_prelude[prelude_end..];
        if delimiter_ch == ';' {
            declarations_offset.get_or_insert(body_offset + cursor);
            declarations.push_str(nested_prelude);
            declarations.push(';');
            cursor = delimiter + 1;
            continue;
        }

        let Some(close) = find_matching_brace(body, delimiter) else {
            declarations.push_str(&body[cursor..]);
            break;
        };
        if css_prelude_is_block_declaration(nested_prelude) {
            let end = css_block_declaration_end(body, close);
            declarations_offset.get_or_insert(body_offset + cursor);
            declarations.push_str(&body[cursor..end]);
            cursor = end;
            continue;
        }

        flush_css_module_nested_declarations(
            &mut output,
            &mut declarations,
            context,
            prelude,
            compose_local_names,
            declarations_offset.take().unwrap_or(body_offset),
            native_nested_rule,
            true,
        );

        let nested_body = &body[delimiter + 1..close];
        let mut block = String::new();
        if nested_prelude.starts_with('@') {
            let rewritten_prelude = rewrite_css_module_at_rule_prelude(nested_prelude, context);
            let next_context = if is_keyframes_at_rule(nested_prelude) {
                CssBlockContext::Keyframes
            } else {
                CssBlockContext::Container
            };
            let nested_rewritten =
                rewrite_css_modules_items(nested_body, context, next_context, true);
            block.push_str(&rewritten_prelude);
            block.push_str(brace_spacing);
            block.push('{');
            if css_block_contains_style_rules(&nested_rewritten)
                || css_block_contains_at_rule_with_style_rules(&nested_rewritten)
            {
                block.push('\n');
                block.push_str(nested_rewritten.trim());
                block.push('\n');
            } else {
                block.push_str(&nested_rewritten);
            }
            block.push('}');
        } else {
            let nested_compose_local_names =
                css_module_composable_local_names(nested_prelude, context);
            let rewritten_prelude =
                rewrite_css_modules_prelude(nested_prelude, context, block_context);
            block.push_str(&rewritten_prelude);
            block.push_str(brace_spacing);
            block.push('{');
            block.push_str(&rewrite_css_module_rule_body(
                nested_prelude,
                nested_body,
                context,
                block_context,
                &nested_compose_local_names,
                body_offset + delimiter + 1,
                true,
            ));
            block.push('}');
        }

        let block = normalize_style_output(&block);
        if output.is_empty() || !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&block);
        cursor = close + 1;
    }

    flush_css_module_nested_declarations(
        &mut output,
        &mut declarations,
        context,
        prelude,
        compose_local_names,
        declarations_offset.take().unwrap_or(body_offset),
        native_nested_rule,
        false,
    );
    if !output.is_empty() {
        output.push('\n');
    }
    output
}

pub(crate) fn flush_css_module_nested_declarations(
    output: &mut String,
    declarations: &mut String,
    context: &mut CssModulesContext<'_>,
    prelude: &str,
    compose_local_names: &[String],
    body_offset: usize,
    native_nested_rule: bool,
    separate_before_next_block: bool,
) {
    if declarations.is_empty() {
        return;
    }
    let rewritten = rewrite_css_module_declarations(
        prelude,
        declarations,
        context,
        CssBlockContext::Container,
        compose_local_names,
        body_offset,
        native_nested_rule,
    );
    output.push_str(rewritten.trim_end());
    if separate_before_next_block && !output.ends_with('\n') {
        output.push('\n');
    }
    declarations.clear();
}

pub(crate) fn rewrite_css_modules_prelude(
    prelude: &str,
    context: &mut CssModulesContext<'_>,
    block_context: CssBlockContext,
) -> String {
    if prelude.starts_with('@') || matches!(block_context, CssBlockContext::Keyframes) {
        return prelude.to_string();
    }
    split_selector_list(prelude)
        .into_iter()
        .map(|part| rewrite_css_module_selector(part.trim(), context))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn rewrite_css_module_at_rule_prelude(
    prelude: &str,
    context: &mut CssModulesContext<'_>,
) -> String {
    let Some((name, params)) = parse_at_rule(prelude) else {
        return replace_css_module_import_symbols_in_text(prelude, context);
    };
    if !is_keyframes_name(name) {
        return replace_css_module_import_symbols_in_text(prelude, context);
    }
    let Some((local, global)) = css_module_keyframes_local_name(params, context) else {
        return format!(
            "@{name} {}",
            css_module_unwrap_global_keyframes_name(params)
        );
    };
    if global {
        return format!("@{name} {local}");
    }
    let scoped = context.scoped_name(local);
    context.register_local(local, &scoped);
    format!("@{name} {scoped}")
}

pub(crate) fn css_module_keyframes_local_name<'a>(
    params: &'a str,
    context: &CssModulesContext<'_>,
) -> Option<(&'a str, bool)> {
    if let Some(inner) = parse_css_module_keyframes_pseudo(params, ":global") {
        return Some((inner, true));
    }
    if let Some(inner) = parse_css_module_keyframes_pseudo(params, ":local") {
        return Some((inner, false));
    }
    let params = params.trim();
    (!params.is_empty() && context.is_local_default()).then_some((params, false))
}

pub(crate) fn parse_css_module_keyframes_pseudo<'a>(
    params: &'a str,
    pseudo: &str,
) -> Option<&'a str> {
    let params = params.trim();
    let inner = params.strip_prefix(pseudo)?.strip_suffix(')')?;
    let inner = inner.strip_prefix('(')?.trim();
    (!inner.is_empty()).then_some(inner)
}

pub(crate) fn css_module_unwrap_global_keyframes_name(params: &str) -> String {
    parse_css_module_keyframes_pseudo(params, ":global")
        .unwrap_or(params)
        .to_string()
}

pub(crate) fn rewrite_css_module_selector(
    selector: &str,
    context: &mut CssModulesContext<'_>,
) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;
    let mut default_local = context.is_local_default();
    while cursor < selector.len() {
        if let Some(global) =
            find_pseudo_function_from(selector, &[":global", "::v-global"], cursor)
        {
            output.push_str(&rewrite_css_module_default_segment(
                &selector[cursor..global.start],
                context,
                default_local,
            ));
            if let Some((open, close)) = global.parens {
                output.push_str(&rewrite_css_module_default_segment(
                    selector[open + 1..close].trim(),
                    context,
                    false,
                ));
                cursor = close + 1;
                continue;
            }
            cursor = global.end;
            default_local = false;
            continue;
        }
        if let Some(local) = find_pseudo_function_from(selector, &[":local", "::v-local"], cursor) {
            output.push_str(&rewrite_css_module_default_segment(
                &selector[cursor..local.start],
                context,
                default_local,
            ));
            if let Some((open, close)) = local.parens {
                output.push_str(&rewrite_css_module_default_segment(
                    selector[open + 1..close].trim(),
                    context,
                    true,
                ));
                cursor = close + 1;
                continue;
            }
            cursor = local.end;
            default_local = true;
            continue;
        }
        output.push_str(&rewrite_css_module_default_segment(
            &selector[cursor..],
            context,
            default_local,
        ));
        break;
    }
    output
}

pub(crate) fn rewrite_css_module_default_segment(
    segment: &str,
    context: &mut CssModulesContext<'_>,
    local: bool,
) -> String {
    if !local {
        if context.export_globals {
            register_css_module_globals(segment, context);
        }
        return segment.to_string();
    }
    let mut output = String::new();
    let mut cursor = 0usize;
    while let Some(token) = find_next_css_module_selector_token(segment, cursor) {
        output.push_str(&segment[cursor..token.start]);
        if context.import_symbol_is_imported(token.name) {
            if let Some(replacement) = context.import_symbol_value(token.name) {
                if replacement.starts_with('.') || replacement.starts_with('#') {
                    output.push_str(replacement);
                } else {
                    output.push(token.sigil);
                    output.push_str(replacement);
                }
            } else {
                output.push(token.sigil);
                output.push_str(token.name);
            }
            cursor = token.end;
            continue;
        }
        if let Some(replacement) = context.value_placeholder_replacement(token.name) {
            if replacement.starts_with('.') || replacement.starts_with('#') {
                output.push_str(replacement);
            } else {
                output.push(token.sigil);
                output.push_str(replacement);
            }
            cursor = token.end;
            continue;
        }
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
