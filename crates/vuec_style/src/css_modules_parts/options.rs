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
