#[cfg(test)]
mod tests {
    include!("tests_parts/descriptor_and_rewrite.rs");
    include!("tests_parts/vue3_compile_core.rs");
    include!("tests_parts/vue3_imports_destructure_models.rs");
    include!("tests_parts/vue3_external_type_resolution.rs");
    include!("tests_parts/vue3_runtime_type_resolution.rs");
    include!("tests_parts/vue3_runtime_props_errors_inline_maps.rs");
    include!("tests_parts/vue27_compile_script.rs");
    include!("tests_parts/style_modules.rs");
    include!("tests_parts/style_scoped_template.rs");
}
