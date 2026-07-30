pub(crate) fn prepare_css_module_values(
    source: &str,
    context: &mut CssModulesContext<'_>,
) -> String {
    let mut output = String::with_capacity(source.len());
    let mut replacements = BTreeMap::new();
    let mut exports = BTreeMap::new();
    let mut import_index = 0usize;
    let mut index = 0usize;
    let mut drop_leading_whitespace = false;
    while index < source.len() {
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        if drop_leading_whitespace && ch.is_whitespace() {
            index += ch.len_utf8();
            continue;
        }
        drop_leading_whitespace = false;
        if source[index..].starts_with("/*") {
            let Some(end_offset) = source[index + 2..].find("*/") else {
                output.push_str(&source[index..]);
                break;
            };
            let end = index + 2 + end_offset + 2;
            output.push_str(&source[index..end]);
            index = end;
            continue;
        }
        if source[index..].starts_with(['\'', '"']) {
            let end = skip_css_string(source, index);
            output.push_str(&source[index..end]);
            index = end;
            continue;
        }
        if source[index..].starts_with("@value")
            && css_module_value_keyword_boundary(source, index + "@value".len())
        {
            if let Some(end) = css_module_value_statement_end(source, index + "@value".len()) {
                let statement = &source[index..end];
                if let Some(import) = parse_css_module_value_import_statement(statement) {
                    if register_css_module_value_import(
                        import,
                        context,
                        &mut replacements,
                        &mut exports,
                        &mut import_index,
                    ) {
                        if output.trim().is_empty() {
                            output.clear();
                            drop_leading_whitespace = true;
                        }
                        index = end;
                        continue;
                    }
                } else if let Some(value) =
                    parse_css_module_local_value_statement(statement, &replacements, &exports)
                {
                    if output.trim().is_empty() {
                        output.clear();
                        drop_leading_whitespace = true;
                    }
                    replacements.insert(value.name.clone(), value.replacement.clone());
                    exports.insert(value.name.clone(), value.export.clone());
                    context.set_raw_export_values(&value.name, vec![value.export]);
                    index = end;
                    continue;
                }
            }
        }
        output.push(ch);
        index += ch.len_utf8();
    }
    replace_css_module_values(&output, &replacements)
}

pub(crate) fn css_module_value_keyword_boundary(source: &str, index: usize) -> bool {
    source[index..]
        .chars()
        .next()
        .is_none_or(|ch| !is_css_module_identifier_continue(ch))
}

pub(crate) fn css_module_value_statement_end(source: &str, mut index: usize) -> Option<usize> {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    while index < source.len() {
        if source[index..].starts_with("/*") {
            let end_offset = source[index + 2..].find("*/")?;
            index += 2 + end_offset + 2;
            continue;
        }
        if source[index..].starts_with(['\'', '"']) {
            index = skip_css_string(source, index);
            continue;
        }
        let ch = source[index..].chars().next()?;
        match ch {
            '(' => paren_depth += 1,
            ')' if paren_depth > 0 => paren_depth -= 1,
            '[' => bracket_depth += 1,
            ']' if bracket_depth > 0 => bracket_depth -= 1,
            ';' if paren_depth == 0 && bracket_depth == 0 => return Some(index + ch.len_utf8()),
            '{' if paren_depth == 0 && bracket_depth == 0 => return None,
            _ => {}
        }
        index += ch.len_utf8();
    }
    None
}

pub(crate) fn skip_css_string(source: &str, start: usize) -> usize {
    let Some(quote) = source[start..].chars().next() else {
        return start;
    };
    let mut index = start + quote.len_utf8();
    while index < source.len() {
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        index += ch.len_utf8();
        if ch == '\\' {
            if index < source.len() {
                index += source[index..].chars().next().map_or(0, char::len_utf8);
            }
            continue;
        }
        if ch == quote {
            return index;
        }
    }
    source.len()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CssModuleLocalValue {
    pub(crate) name: String,
    pub(crate) replacement: String,
    pub(crate) export: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CssModuleValueImport<'a> {
    pub(crate) import: &'a str,
    pub(crate) specs: Vec<CssModuleValueImportSpec<'a>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CssModuleValueImportSpec<'a> {
    pub(crate) remote: &'a str,
    pub(crate) local: &'a str,
}

pub(crate) fn parse_css_module_local_value_statement(
    statement: &str,
    replacements: &BTreeMap<String, String>,
    exports: &BTreeMap<String, String>,
) -> Option<CssModuleLocalValue> {
    let body = statement.strip_prefix("@value")?.strip_suffix(';')?.trim();
    let colon = find_top_level_colon(body)?;
    let name = body[..colon].trim();
    let value = body[colon + 1..].trim();
    if !is_css_module_value_name(name) || value.is_empty() {
        return None;
    }
    Some(CssModuleLocalValue {
        name: name.to_string(),
        replacement: replace_css_module_values(value, replacements),
        export: replace_css_module_values(value, exports),
    })
}

pub(crate) fn parse_css_module_value_import_statement(
    statement: &str,
) -> Option<CssModuleValueImport<'_>> {
    let body = statement.strip_prefix("@value")?.strip_suffix(';')?.trim();
    if find_top_level_colon(body).is_some() {
        return None;
    }
    let from = find_css_module_value_from_keyword(body)?;
    let specs = body[..from].trim();
    let import = body[from + "from".len()..].trim();
    if specs.is_empty() || import.is_empty() {
        return None;
    }
    let specs = split_selector_list(specs)
        .into_iter()
        .map(|spec| parse_css_module_value_import_spec(spec.trim()))
        .collect::<Option<Vec<_>>>()?;
    (!specs.is_empty()).then_some(CssModuleValueImport { import, specs })
}

pub(crate) fn find_css_module_value_from_keyword(source: &str) -> Option<usize> {
    let mut index = 0usize;
    while index < source.len() {
        if source[index..].starts_with("/*") {
            let end_offset = source[index + 2..].find("*/")?;
            index += 2 + end_offset + 2;
            continue;
        }
        if source[index..].starts_with(['\'', '"']) {
            index = skip_css_string(source, index);
            continue;
        }
        if source[index..].starts_with("from")
            && source[..index]
                .chars()
                .next_back()
                .is_none_or(|ch| !is_css_module_identifier_continue(ch))
            && css_module_value_keyword_boundary(source, index + "from".len())
        {
            return Some(index);
        }
        let ch = source[index..].chars().next()?;
        index += ch.len_utf8();
    }
    None
}

pub(crate) fn parse_css_module_value_import_spec(
    spec: &str,
) -> Option<CssModuleValueImportSpec<'_>> {
    let tokens = spec.split_whitespace().collect::<Vec<_>>();
    match tokens.as_slice() {
        [name] if is_css_module_value_name(name) => Some(CssModuleValueImportSpec {
            remote: name,
            local: name,
        }),
        [remote, keyword, local]
            if keyword.eq_ignore_ascii_case("as")
                && is_css_module_value_name(remote)
                && is_css_module_value_name(local) =>
        {
            Some(CssModuleValueImportSpec { remote, local })
        }
        _ => None,
    }
}

pub(crate) fn register_css_module_value_import(
    import: CssModuleValueImport<'_>,
    context: &mut CssModulesContext<'_>,
    replacements: &mut BTreeMap<String, String>,
    exports: &mut BTreeMap<String, String>,
    import_index: &mut usize,
) -> bool {
    let Some(result) = context.load_imported_module(import.import) else {
        return false;
    };
    for spec in import.specs {
        let (replacement, export) = if let Some(value) = result.raw_modules.get(spec.remote) {
            (value.clone(), value.clone())
        } else {
            (
                format!("i__const_{}_{}", spec.local, *import_index),
                "undefined".to_string(),
            )
        };
        let replacement = context.import_value_placeholder(replacement, export.clone());
        replacements.insert(spec.local.to_string(), replacement);
        exports.insert(spec.local.to_string(), export.clone());
        context.set_raw_export_values(spec.local, vec![export]);
        *import_index += 1;
    }
    true
}

pub(crate) fn is_css_module_value_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    is_css_module_identifier_start(first) && chars.all(is_css_module_identifier_continue)
}

pub(crate) fn replace_css_module_values(source: &str, values: &BTreeMap<String, String>) -> String {
    if values.is_empty() {
        return source.to_string();
    }
    debug_assert!(values.keys().all(|name| is_css_module_value_name(name)));
    let mut output = String::with_capacity(source.len());
    let mut index = 0usize;
    while index < source.len() {
        if source[index..].starts_with("/*") {
            let Some(end_offset) = source[index + 2..].find("*/") else {
                output.push_str(&source[index..]);
                break;
            };
            let end = index + 2 + end_offset + 2;
            output.push_str(&source[index..end]);
            index = end;
            continue;
        }
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        let starts_identifier = is_css_module_identifier_start(ch)
            && source[..index]
                .chars()
                .next_back()
                .is_none_or(|previous| !is_css_module_identifier_continue(previous));
        if starts_identifier {
            let mut end = index + ch.len_utf8();
            while let Some(next) = source[end..].chars().next() {
                if !is_css_module_identifier_continue(next) {
                    break;
                }
                end += next.len_utf8();
            }
            let name = &source[index..end];
            output.push_str(values.get(name).map_or(name, String::as_str));
            index = end;
            continue;
        }
        output.push(ch);
        index += ch.len_utf8();
    }
    output
}
