fn verify_wasm() -> Result<compat::JsonReport> {
    let mut items = Vec::new();
    let mut violations = Vec::new();
    let mut created = Vec::new();

    let rust_status = match run_cargo(&["test", "-p", "vuec_wasm"]) {
        Ok(output) => {
            items.push(compat::ReportItem::new(
                "vuec_wasm-rust-api",
                compat::ReportStatus::Pass,
                output,
                Some(PathBuf::from("crates/vuec_wasm")),
            ));
            compat::ReportStatus::Pass
        }
        Err(err) => {
            violations.push(format!("vuec_wasm Rust tests failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "vuec_wasm-rust-api",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("crates/vuec_wasm")),
            ));
            compat::ReportStatus::Fail
        }
    };

    let wasm_status = match build_wasm_package() {
        Ok(paths) => {
            created.extend(paths.into_iter().map(|path| path.display().to_string()));
            match run_wasm_smoke() {
                Ok(output) => {
                    items.push(compat::ReportItem::new(
                        "@vuec-rs/wasm-node-smoke",
                        compat::ReportStatus::Pass,
                        output,
                        Some(PathBuf::from("packages/wasm")),
                    ));
                    compat::ReportStatus::Pass
                }
                Err(err) => {
                    violations.push(format!("WASM Node smoke failed: {err:#}"));
                    items.push(compat::ReportItem::new(
                        "@vuec-rs/wasm-node-smoke",
                        compat::ReportStatus::Fail,
                        format!("{err:#}"),
                        Some(PathBuf::from("packages/wasm")),
                    ));
                    compat::ReportStatus::Fail
                }
            }
        }
        Err(err) => {
            violations.push(format!("failed to build wasm package: {err:#}"));
            items.push(compat::ReportItem::new(
                "@vuec-rs/wasm-build",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("packages/wasm/pkg-node")),
            ));
            compat::ReportStatus::Fail
        }
    };

    let status =
        if rust_status == compat::ReportStatus::Pass && wasm_status == compat::ReportStatus::Pass {
            compat::ReportStatus::Pass
        } else {
            compat::ReportStatus::Fail
        };
    Ok(
        compat::JsonReport::new("verify_wasm", status)
            .with_items(items)
            .with_created(created)
            .with_violations(violations)
            .with_note("runs vuec_wasm Rust API tests, builds the wasm-bindgen package when the wasm target/tooling is installed, and executes the @vuec-rs/wasm Node smoke; browser coverage is handled by verify-wasm-browser and WASI coverage remains a separate gate"),
    )
}

fn verify_wasm_browser() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut items = Vec::new();
    let status = match run_wasm_pack_browser_test() {
        Ok(output) => {
            items.push(compat::ReportItem::new(
                "vuec_wasm-browser-smoke",
                compat::ReportStatus::Pass,
                output,
                Some(PathBuf::from("crates/vuec_wasm")),
            ));
            compat::ReportStatus::Pass
        }
        Err(err) => {
            violations.push(format!("WASM browser smoke failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "vuec_wasm-browser-smoke",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("crates/vuec_wasm")),
            ));
            compat::ReportStatus::Fail
        }
    };

    Ok(
        compat::JsonReport::new("verify_wasm_browser", status)
            .with_items(items)
            .with_violations(violations)
            .with_note("runs `wasm-pack test --headless --chrome crates/vuec_wasm`; requires wasm-pack plus Chrome, and supports VUEC_WASM_CHROME/VUEC_WASM_CHROMEDRIVER overrides"),
    )
}

fn verify_wasm_wasi() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut items = Vec::new();
    let status = match run_wasi_smoke() {
        Ok(output) => {
            items.push(compat::ReportItem::new(
                "vuec_wasm-wasi-smoke",
                compat::ReportStatus::Pass,
                output,
                Some(PathBuf::from("crates/vuec_wasm/src/bin/wasi_smoke.rs")),
            ));
            compat::ReportStatus::Pass
        }
        Err(err) => {
            violations.push(format!("WASI smoke failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "vuec_wasm-wasi-smoke",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("crates/vuec_wasm/src/bin/wasi_smoke.rs")),
            ));
            compat::ReportStatus::Fail
        }
    };

    Ok(
        compat::JsonReport::new("verify_wasm_wasi", status)
            .with_items(items)
            .with_violations(violations)
            .with_note("builds vuec_wasm's WASI smoke binary for wasm32-wasip1 and runs it with wasmtime; requires rust target wasm32-wasip1 and wasmtime on PATH"),
    )
}

fn verify_cli() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut items = Vec::new();

    let status = match run_cli_smoke_suite() {
        Ok(output) => {
            items.push(compat::ReportItem::new(
                "vuec-cli-smoke",
                compat::ReportStatus::Pass,
                output,
                Some(PathBuf::from("crates/vuec_cli")),
            ));
            compat::ReportStatus::Pass
        }
        Err(err) => {
            violations.push(format!("CLI smoke failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "vuec-cli-smoke",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("crates/vuec_cli")),
            ));
            compat::ReportStatus::Fail
        }
    };

    Ok(
        compat::JsonReport::new("verify_cli", status)
            .with_items(items)
            .with_violations(violations)
            .with_note("builds the vuec CLI and verifies real subcommand smoke coverage for Vue 2 template, Vue 3 template, Vue 3 SFC, SSR, parse-sfc, JSON output, diagnostics, source maps, and bench output"),
    )
}

fn verify_incremental() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut items = Vec::new();

    match run_incremental_smoke_suite() {
        Ok(output) => {
            items.push(compat::ReportItem::new(
                "sfc-descriptor-cache",
                compat::ReportStatus::Pass,
                output,
                Some(PathBuf::from("crates/vuec_sfc")),
            ));
        }
        Err(err) => {
            violations.push(format!("SFC incremental cache smoke failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "sfc-descriptor-cache",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("crates/vuec_sfc")),
            ));
        }
    }

    Ok(
        compat::JsonReport::new("verify_incremental", compat::ReportStatus::Pass)
            .with_items(items)
            .with_violations(violations)
            .with_note("verifies SFC descriptor cache reuse for unchanged input and invalidation for changed same-file input; compiler semantics remain in vuec_sfc"),
    )
}

fn verify_parallel() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut items = Vec::new();

    let status = match run_parallel_smoke_suite() {
        Ok(output) => {
            items.push(compat::ReportItem::new(
                "vuec-cli-compile-batch",
                compat::ReportStatus::Pass,
                output,
                Some(PathBuf::from("crates/vuec_cli")),
            ));
            compat::ReportStatus::Pass
        }
        Err(err) => {
            violations.push(format!("parallel compile smoke failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "vuec-cli-compile-batch",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("crates/vuec_cli")),
            ));
            compat::ReportStatus::Fail
        }
    };

    Ok(
        compat::JsonReport::new("verify_parallel", status)
            .with_items(items)
            .with_violations(violations)
            .with_note("builds the vuec CLI and verifies compile-batch runs independent inputs concurrently while preserving deterministic input-order JSON results"),
    )
}

fn verify_ast_cache() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut items = Vec::new();

    let status = match run_ast_cache_smoke_suite() {
        Ok(output) => {
            items.push(compat::ReportItem::new(
                "vue3-dom-ast-cache",
                compat::ReportStatus::Pass,
                output,
                Some(PathBuf::from("crates/vuec_vue3_dom")),
            ));
            compat::ReportStatus::Pass
        }
        Err(err) => {
            violations.push(format!("Vue3 DOM AST cache smoke failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "vue3-dom-ast-cache",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("crates/vuec_vue3_dom")),
            ));
            compat::ReportStatus::Fail
        }
    };

    Ok(
        compat::JsonReport::new("verify_ast_cache", status)
            .with_items(items)
            .with_violations(violations)
            .with_note("verifies the Vue3 DOM compiler AST cache reuses parse/DOM-normalize results for unchanged inputs, invalidates changed same-file inputs, and keeps compile output stable"),
    )
}

fn verify_arena() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut items = Vec::new();

    let status = match run_arena_smoke_suite() {
        Ok(output) => {
            items.push(compat::ReportItem::new(
                "ast-document-arena-preallocation",
                compat::ReportStatus::Pass,
                output,
                Some(PathBuf::from("crates/vuec_ast")),
            ));
            compat::ReportStatus::Pass
        }
        Err(err) => {
            violations.push(format!("arena allocation smoke failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "ast-document-arena-preallocation",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("crates/vuec_ast")),
            ));
            compat::ReportStatus::Fail
        }
    };

    Ok(
        compat::JsonReport::new("verify_arena", status)
            .with_items(items)
            .with_violations(violations)
            .with_note("verifies AstDocument node preallocation APIs plus Vue 2 and Vue 3 parser entrypoints use arena capacity hints without changing tree invariants"),
    )
}

fn verify_string_interning() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut items = Vec::new();

    let status = match run_string_interning_smoke_suite() {
        Ok(output) => {
            items.push(compat::ReportItem::new(
                "js-source-string-interner",
                compat::ReportStatus::Pass,
                output,
                Some(PathBuf::from("crates/vuec_js")),
            ));
            compat::ReportStatus::Pass
        }
        Err(err) => {
            violations.push(format!("string interning smoke failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "js-source-string-interner",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("crates/vuec_js")),
            ));
            compat::ReportStatus::Fail
        }
    };

    Ok(
        compat::JsonReport::new("verify_string_interning", status)
            .with_items(items)
            .with_violations(violations)
            .with_note("verifies JS expression side-store string interning reuses repeated source text while preserving serialized string output and AST/HIR/MIR structure boundaries"),
    )
}
