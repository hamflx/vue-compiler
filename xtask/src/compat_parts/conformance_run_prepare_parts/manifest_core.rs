fn prepared_test_manifest_for_suite(spec: ConformanceSuiteSpec) -> PreparedTestManifest {
    let mut manifest = PreparedTestManifest::new(spec.name);
    match spec.name {
        "vue2-compiler" => {
            add_vue2_compiler_manifest_entries(&mut manifest, false);
            add_manifest_entry(
                &mut manifest,
                "generated/vuec-jasmine-runner.js",
                "vuec-jasmine-runner.js",
                "runner-shim",
                None,
                &[],
                provenance(
                    "prepared-official",
                    "prepared-jasmine-runner",
                    "runner-harness",
                    &["runner-shim", "warning-matcher-adapter"],
                ),
            );
        }
        "vue27-compiler" => {
            add_vue2_compiler_manifest_entries(&mut manifest, true);
            add_manifest_entry(
                &mut manifest,
                "generated/vuec-vitest-setup.ts",
                "vuec-vitest-setup.ts",
                "runner-shim",
                None,
                &[],
                provenance(
                    "prepared-official",
                    "prepared-vitest-runner",
                    "runner-harness",
                    &["runner-shim", "warning-matcher-adapter"],
                ),
            );
            add_manifest_entry(
                &mut manifest,
                "generated/vitest.config.ts",
                "vitest.config.ts",
                "runner-config-alias",
                None,
                &[],
                provenance(
                    "prepared-official",
                    "prepared-vitest-runner",
                    "runner-harness",
                    &["package-alias-config"],
                ),
            );
            add_vitest_provenance_manifest_entry(&mut manifest);
        }
        "vue27-sfc" => {
            add_vue2_compiler_manifest_entries(&mut manifest, true);
            add_vue27_sfc_manifest_entries(&mut manifest);
            add_vitest_provenance_manifest_entry(&mut manifest);
        }
        "vue3-core" => {
            add_vue3_core_source_manifest_entries(&mut manifest);
            add_vue3_core_prepared_spec_manifest_entries(&mut manifest);
            add_vue3_vitest_manifest_entries(
                &mut manifest,
                "packages/compiler-core/__tests__/**/*.spec.ts",
            );
        }
        "vue3-dom" => {
            add_vue3_core_source_manifest_entries(&mut manifest);
            add_vue3_dom_manifest_entries(&mut manifest);
            add_vue3_vitest_manifest_entries(
                &mut manifest,
                "packages/compiler-dom/__tests__/**/*.spec.ts",
            );
        }
        "vue3-sfc" => {
            add_vue3_core_source_manifest_entries(&mut manifest);
            add_vue3_sfc_manifest_entries(&mut manifest);
            add_vue3_vitest_manifest_entries(
                &mut manifest,
                "packages/compiler-sfc/__tests__/**/*.spec.ts",
            );
        }
        "vue3-ssr" => {
            add_vue3_core_source_manifest_entries(&mut manifest);
            add_vue3_ssr_manifest_entries(&mut manifest);
            add_vue3_vitest_manifest_entries(
                &mut manifest,
                "packages/compiler-ssr/__tests__/**/*.spec.ts",
            );
        }
        _ => {}
    }
    manifest
}

fn add_manifest_entry(
    manifest: &mut PreparedTestManifest,
    original_path: &str,
    prepared_path: &str,
    rewrite_kind: &str,
    helper_path: Option<&str>,
    related_bridge_commands: &[&str],
    expected_provenance: PreparedTestProvenanceExpectation,
) {
    manifest.push(PreparedTestManifestEntry {
        original_path: original_path.to_string(),
        prepared_path: prepared_path.to_string(),
        rewrite_kind: rewrite_kind.to_string(),
        helper_path: helper_path.map(str::to_string),
        related_bridge_commands: related_bridge_commands
            .iter()
            .map(|command| (*command).to_string())
            .collect(),
        expected_provenance,
    });
}

fn add_vitest_provenance_manifest_entry(manifest: &mut PreparedTestManifest) {
    add_manifest_entry(
        manifest,
        "generated/vuec-vitest-provenance.ts",
        "vuec-vitest-provenance.ts",
        "runner-provenance-flush",
        None,
        &[],
        provenance(
            "prepared-official",
            "prepared-vitest-runner",
            "runner-harness",
            &["runner-shim", "provenance-flush"],
        ),
    );
}

fn provenance(
    test_origin: &str,
    execution_path: &str,
    api_surface: &str,
    adapter_roles: &[&str],
) -> PreparedTestProvenanceExpectation {
    PreparedTestProvenanceExpectation {
        test_origin: test_origin.to_string(),
        execution_path: execution_path.to_string(),
        api_surface: api_surface.to_string(),
        adapter_roles: adapter_roles
            .iter()
            .map(|role| (*role).to_string())
            .collect(),
    }
}

fn add_vue2_compiler_manifest_entries(manifest: &mut PreparedTestManifest, include_types: bool) {
    for (prepared_path, commands, roles) in [
        (
            "src/compiler/parser/index.ts",
            &["vue2.compile"][..],
            &["source-path-shim", "public-ast-hydration"][..],
        ),
        (
            "src/compiler/optimizer.ts",
            &["vue2.optimize"][..],
            &["source-path-shim", "rust-projection-helper"][..],
        ),
        (
            "src/compiler/codegen.ts",
            &["vue2.generate"][..],
            &["source-path-shim", "public-ast-normalizer"][..],
        ),
        (
            "src/compiler/codeframe.ts",
            &["vue2.generateCodeFrame"][..],
            &["source-path-shim", "public-api-reroute"][..],
        ),
        (
            "src/compiler/helpers.ts",
            &[][..],
            &["source-path-shim", "test-helper-shape-adapter"][..],
        ),
        (
            "src/platforms/web/compiler/index.ts",
            &["vue2.compile"][..],
            &["source-path-shim", "public-api-reroute"][..],
        ),
        (
            "src/platforms/web/compiler/options.ts",
            &[][..],
            &["source-path-shim", "platform-option-adapter"][..],
        ),
    ] {
        add_manifest_entry(
            manifest,
            prepared_path,
            prepared_path,
            "source-path-shim",
            None,
            commands,
            provenance(
                "prepared-official",
                "source-path-shim-to-rust-bridge",
                "public-and-projection-bridge",
                roles,
            ),
        );
    }
    if include_types {
        add_manifest_entry(
            manifest,
            "src/types/compiler.ts",
            "src/types/compiler.ts",
            "source-path-type-shim",
            None,
            &[],
            provenance(
                "prepared-official",
                "type-only-source-shim",
                "type-shape-adapter",
                &["type-shim"],
            ),
        );
    }
}

fn add_vue27_sfc_manifest_entries(manifest: &mut PreparedTestManifest) {
    for (module, commands) in [
        (
            "index",
            &[
                "sfc.vue27.parse",
                "sfc.vue27.compileTemplate",
                "sfc.vue27.compileScript",
                "sfc.vue27.compileStyle",
            ][..],
        ),
        ("parse", &["sfc.vue27.parse"][..]),
        ("parseComponent", &["sfc.vue27.parseComponent"][..]),
        ("compileTemplate", &["sfc.vue27.compileTemplate"][..]),
        ("compileScript", &["sfc.vue27.compileScript"][..]),
        (
            "compileStyle",
            &["sfc.vue27.compileStyle", "sfc.vue27.compileStyleAsync"][..],
        ),
        ("cssVars", &["sfc.vue27.compileStyle"][..]),
        ("rewriteDefault", &["sfc.vue27.rewriteDefault"][..]),
    ] {
        let path = format!("packages/compiler-sfc/src/{module}.ts");
        add_manifest_entry(
            manifest,
            &path,
            &path,
            "sfc-source-path-public-alias-shim",
            None,
            commands,
            provenance(
                "prepared-official",
                "source-path-shim-to-rust-bridge",
                "public-sfc-api",
                &["source-path-shim", "package-alias-adapter"],
            ),
        );
    }
    add_manifest_entry(
        manifest,
        "packages/compiler-sfc/src/prefixIdentifiers.ts",
        "packages/compiler-sfc/src/prefixIdentifiers.ts",
        "sfc-source-path-rust-helper-shim",
        None,
        &["sfc.vue27.prefixIdentifiers"],
        provenance(
            "prepared-official",
            "source-path-shim-to-rust-bridge",
            "projection-command",
            &["source-path-shim", "bridge-shape-adapter"],
        ),
    );
    add_manifest_entry(
        manifest,
        "generated/vuec-vitest-setup.ts",
        "vuec-vitest-setup.ts",
        "runner-shim",
        None,
        &[],
        provenance(
            "prepared-official",
            "prepared-vitest-runner",
            "runner-harness",
            &["runner-shim", "warning-matcher-adapter"],
        ),
    );
    add_manifest_entry(
        manifest,
        "generated/vitest.config.ts",
        "vitest.config.ts",
        "runner-config-alias",
        None,
        &[],
        provenance(
            "prepared-official",
            "prepared-vitest-runner",
            "runner-harness",
            &["package-alias-config"],
        ),
    );
}
