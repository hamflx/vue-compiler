fn push_unique_string(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn canonical_adapter_roles(roles: &[String]) -> Vec<String> {
    let mut canonical = Vec::new();
    for role in roles {
        let mapped = if role.contains("runner") || role.contains("warning-matcher") {
            "runner-support"
        } else if role.contains("callback") || role.contains("postcss") {
            "callback-materialization"
        } else if role.contains("semantic") {
            "semantic-shim"
        } else if role.contains("rewrite")
            || role.contains("source-path")
            || role.contains("alias-config")
            || role.contains("type-shim")
        {
            "import-rewrite"
        } else {
            "hydration-dehydration"
        };
        push_unique_string(&mut canonical, mapped);
    }
    if canonical.is_empty() {
        canonical.push("hydration-dehydration".into());
    }
    canonical
}

fn canonical_api_surface(api_surface: &str) -> String {
    if matches!(
        api_surface,
        "public-command" | "projection-command" | "suite-only-bridge-command"
    ) {
        api_surface.into()
    } else if api_surface == "suite-only-bridge-command" {
        "suite-only-bridge-command".into()
    } else if api_surface.contains("mixed-official-source-boundary")
        || api_surface.contains("projection")
        || api_surface.contains("helper")
        || api_surface.contains("runner")
        || api_surface.contains("type-shape")
        || api_surface.contains("shared-runtime")
    {
        "internal-helper-import".into()
    } else if api_surface.contains("rust-api") {
        "public-rust-api".into()
    } else {
        "public-package-api".into()
    }
}

fn canonical_bridge_api_surface(commands: &[String], fallback_api_surface: &str) -> String {
    if preserves_manifest_api_surface_boundary(fallback_api_surface) {
        return canonical_api_surface(fallback_api_surface);
    }
    let mut has_suite = false;
    let mut has_projection = false;
    let mut has_public = false;
    for command in commands {
        match bridge_command_api_surface(command) {
            Some("suite-only-bridge-command") => has_suite = true,
            Some("projection-command") => has_projection = true,
            Some("public-command") => has_public = true,
            _ => {}
        }
    }
    if has_suite {
        "suite-only-bridge-command".into()
    } else if has_projection {
        "projection-command".into()
    } else if has_public {
        "public-command".into()
    } else {
        canonical_api_surface(fallback_api_surface)
    }
}

fn preserves_manifest_api_surface_boundary(api_surface: &str) -> bool {
    api_surface.contains("mixed-official-source-boundary")
        || api_surface.contains("runner")
        || api_surface.contains("type-shape")
        || api_surface.contains("shared-runtime")
}

fn canonical_execution_path(
    expected: &PreparedTestProvenanceExpectation,
    adapter_roles: &[String],
    api_surface: &str,
) -> String {
    if adapter_roles.iter().any(|role| role == "semantic-shim") {
        return "shim-backed-semantic-js".into();
    }
    if adapter_roles
        .iter()
        .any(|role| role == "callback-materialization")
        || expected.execution_path.contains("callback")
        || expected.execution_path.contains("postcss")
    {
        return "mixed-js-callback-boundary".into();
    }
    if api_surface == "suite-only-bridge-command"
        || expected
            .api_surface
            .contains("mixed-official-source-boundary")
        || expected.execution_path.contains("official-")
        || expected.execution_path.contains("copied-official")
        || expected.execution_path.contains("prepared-suite-helper")
    {
        return "hybrid-js-adapter-rust-projection".into();
    }
    if matches!(
        api_surface,
        "public-command" | "public-package-api" | "public-rust-api"
    ) {
        return "rust-bridge-shape-adapter".into();
    }
    "hybrid-js-adapter-rust-projection".into()
}

fn marker_is_callback_boundary(marker: &str) -> bool {
    marker.contains("callback")
        || marker.contains("postcss")
        || marker.contains("transformContext")
        || marker.contains("directiveTransform")
        || marker.contains("nodeTransform")
}

fn marker_is_semantic_js(marker: &str) -> bool {
    marker.contains("semantic")
        || marker.contains("shim")
        || marker.contains("js.transformElement")
        || marker.contains("js.compiler")
}
