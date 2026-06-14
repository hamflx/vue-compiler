#[derive(Default)]
struct JsScanState {
    in_single: bool,
    in_double: bool,
    in_template: bool,
    in_regex: bool,
    curly: usize,
    square: usize,
    paren: usize,
}

impl JsScanState {
    fn consume(&mut self, source: &str, index: usize, ch: char, prev: char) -> bool {
        if self.in_single {
            if ch == '\'' && prev != '\\' {
                self.in_single = false;
            }
            return true;
        }
        if self.in_double {
            if ch == '"' && prev != '\\' {
                self.in_double = false;
            }
            return true;
        }
        if self.in_template {
            if ch == '`' && prev != '\\' {
                self.in_template = false;
            }
            return true;
        }
        if self.in_regex {
            if ch == '/' && prev != '\\' {
                self.in_regex = false;
            }
            return true;
        }

        match ch {
            '\'' => self.in_single = true,
            '"' => self.in_double = true,
            '`' => self.in_template = true,
            '(' => self.paren += 1,
            ')' => self.paren = self.paren.saturating_sub(1),
            '[' => self.square += 1,
            ']' => self.square = self.square.saturating_sub(1),
            '{' => self.curly += 1,
            '}' => self.curly = self.curly.saturating_sub(1),
            '/' if !valid_division_before(source, index) => self.in_regex = true,
            _ => {}
        }
        false
    }

    fn depth_is_zero(&self) -> bool {
        self.curly == 0 && self.square == 0 && self.paren == 0
    }
}

fn valid_division_before(source: &str, slash_index: usize) -> bool {
    let previous = source[..slash_index]
        .chars()
        .rev()
        .find(|ch| !ch.is_whitespace());
    previous.is_some_and(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, ')' | '.' | '+' | '-' | '_' | '$' | ']')
    })
}

fn previous_non_ws(source: &str, offset: usize) -> Option<char> {
    source
        .get(..offset)?
        .chars()
        .rev()
        .find(|ch| !ch.is_whitespace())
}

fn next_non_ws(source: &str, offset: usize) -> Option<char> {
    source.get(offset..)?.chars().find(|ch| !ch.is_whitespace())
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    is_identifier_start(ch) || ch.is_ascii_digit()
}

fn is_keyword(value: &str) -> bool {
    matches!(
        value,
        "const"
            | "let"
            | "var"
            | "function"
            | "return"
            | "if"
            | "else"
            | "for"
            | "while"
            | "do"
            | "switch"
            | "case"
            | "default"
            | "break"
            | "continue"
            | "new"
            | "class"
            | "extends"
            | "super"
            | "import"
            | "export"
            | "from"
            | "as"
            | "async"
            | "await"
            | "yield"
            | "try"
            | "catch"
            | "finally"
            | "throw"
            | "typeof"
            | "void"
            | "delete"
            | "in"
            | "of"
            | "instanceof"
    )
}

fn is_global_or_literal(value: &str) -> bool {
    matches!(
        value,
        "true"
            | "false"
            | "null"
            | "undefined"
            | "NaN"
            | "Infinity"
            | "this"
            | "Math"
            | "Number"
            | "String"
            | "Boolean"
            | "Array"
            | "Object"
            | "Date"
            | "RegExp"
            | "JSON"
            | "Promise"
            | "Symbol"
            | "Map"
            | "Set"
            | "WeakMap"
            | "WeakSet"
            | "BigInt"
            | "console"
            | "Reflect"
            | "globalThis"
            | "Error"
    )
}

fn collect_statement_summary(statement: &Statement<'_>, summary: &mut JsProgramSummary) {
    match statement {
        Statement::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                collect_binding_pattern(&declarator.id, &mut summary.bindings);
            }
        }
        Statement::FunctionDeclaration(function) => {
            if let Some(id) = &function.id {
                summary.bindings.push(id.name.to_string());
            }
        }
        Statement::ClassDeclaration(class) => {
            if let Some(id) = &class.id {
                summary.bindings.push(id.name.to_string());
            }
        }
        Statement::ImportDeclaration(declaration) => {
            summary.imports.push(declaration.source.value.to_string());
            if let Some(specifiers) = &declaration.specifiers {
                for specifier in specifiers {
                    match specifier {
                        ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                            summary.bindings.push(specifier.local.name.to_string());
                        }
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                            summary.bindings.push(specifier.local.name.to_string());
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                            summary.bindings.push(specifier.local.name.to_string());
                        }
                    }
                }
            }
        }
        Statement::ExportDefaultDeclaration(_) => {
            summary.exports.push("default".into());
        }
        Statement::ExportNamedDeclaration(declaration) => {
            if let Some(declaration) = &declaration.declaration {
                collect_declaration_summary(declaration, summary);
            }
            for specifier in &declaration.specifiers {
                summary.exports.push(specifier.local.name().to_string());
            }
        }
        Statement::ExportAllDeclaration(declaration) => {
            summary
                .exports
                .push(format!("* from {}", declaration.source.value));
        }
        Statement::BlockStatement(block) => {
            for statement in &block.body {
                collect_statement_summary(statement, summary);
            }
        }
        _ => {}
    }
}

fn collect_declaration_summary(declaration: &Declaration<'_>, summary: &mut JsProgramSummary) {
    match declaration {
        Declaration::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                collect_binding_pattern(&declarator.id, &mut summary.bindings);
            }
        }
        Declaration::FunctionDeclaration(function) => {
            if let Some(id) = &function.id {
                summary.bindings.push(id.name.to_string());
                summary.exports.push(id.name.to_string());
            }
        }
        Declaration::ClassDeclaration(class) => {
            if let Some(id) = &class.id {
                summary.bindings.push(id.name.to_string());
                summary.exports.push(id.name.to_string());
            }
        }
        _ => {}
    }
}

fn collect_binding_pattern(pattern: &BindingPattern<'_>, bindings: &mut Vec<String>) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            bindings.push(identifier.name.to_string());
        }
        BindingPattern::ObjectPattern(object) => {
            for property in &object.properties {
                collect_binding_pattern(&property.value, bindings);
            }
            if let Some(rest) = &object.rest {
                collect_binding_pattern(&rest.argument, bindings);
            }
        }
        BindingPattern::ArrayPattern(array) => {
            for element in array.elements.iter().flatten() {
                collect_binding_pattern(element, bindings);
            }
            if let Some(rest) = &array.rest {
                collect_binding_pattern(&rest.argument, bindings);
            }
        }
        BindingPattern::AssignmentPattern(assignment) => {
            collect_binding_pattern(&assignment.left, bindings);
        }
    }
}
