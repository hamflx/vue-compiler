#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> Vue2CompileOptions {
        Vue2CompileOptions {
            comments: true,
            warn: true,
            preserve_whitespace: true,
            optimize: true,
            ..Vue2CompileOptions::default()
        }
    }

    include!("tests_parts/core_shapes_and_mir.rs");
    include!("tests_parts/dom_model_directives_and_events.rs");
    include!("tests_parts/slots_setup_parser.rs");
    include!("tests_parts/assets_warnings_optimizer.rs");
}
