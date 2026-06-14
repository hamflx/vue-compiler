#[cfg(test)]
mod tests {
    include!("tests_parts/compile_and_public_ast.rs");
    include!("tests_parts/dom_mir_lowering.rs");
    include!("tests_parts/dom_mir_codegen.rs");
    include!("tests_parts/ssr_mir_lowering.rs");
    include!("tests_parts/ssr_mir_codegen.rs");
    include!("tests_parts/transform_and_expression_projection.rs");
    include!("tests_parts/base_compile_codegen.rs");
    include!("tests_parts/projection_helpers.rs");
    include!("tests_parts/parser_entities.rs");
}
