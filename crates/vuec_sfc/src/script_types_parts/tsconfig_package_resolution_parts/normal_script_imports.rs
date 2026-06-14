pub(crate) fn vue3_normal_script_user_imports(descriptor: &SfcDescriptor) -> Vue3UserImports {
    let Some(script) = descriptor.script.as_ref() else {
        return Vue3UserImports::default();
    };
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        script.content.as_str(),
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue3UserImports::default();
    }
    let mut user_imports = Vue3UserImports::default();
    for statement in &parsed.program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let source = import.source.value.as_str();
        if let Some(specifiers) = &import.specifiers {
            for specifier in specifiers {
                if let Some(imported) = import_specifier_imported(specifier) {
                    user_imports.record(Vue27ScriptImport {
                        local: import_specifier_local(specifier),
                        source: source.to_string(),
                        imported,
                        is_type: vue27_import_specifier_is_type(import, specifier),
                    });
                }
            }
        }
    }
    user_imports
}
