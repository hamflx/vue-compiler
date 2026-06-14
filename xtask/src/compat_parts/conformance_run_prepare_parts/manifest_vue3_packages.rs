fn add_vue3_dom_manifest_entries(manifest: &mut PreparedTestManifest) {
    add_manifest_entry(
        manifest,
        "packages/compiler-dom/src/**",
        "packages/compiler-dom/src/**",
        "copied-official-source-boundary",
        None,
        &[
            "vue3.dom.compile",
            "vue3.dom.parse",
            "vue3.dom.transformStyle",
            "vue3.dom.transformVHtml",
            "vue3.dom.transformVText",
            "vue3.dom.transformShow",
            "vue3.dom.transformOn",
            "vue3.dom.transformModel",
        ],
        provenance(
            "prepared-official",
            "official-dom-source-plus-alias-runtime",
            "mixed-official-source-boundary",
            &["official-source-boundary", "package-alias-config"],
        ),
    );
    add_manifest_entry(
        manifest,
        "packages/compiler-dom/__tests__/index.spec.ts",
        "packages/compiler-dom/__tests__/index.spec.ts",
        "test-spec-public-api-import-rewrite",
        None,
        &["vue3.dom.compile"],
        provenance(
            "prepared-official",
            "public-package-alias-to-rust-bridge",
            "public-api",
            &["test-import-rewrite", "package-alias-adapter"],
        ),
    );
    for (path, commands) in [
        (
            "packages/compiler-dom/src/transforms/transformStyle.ts",
            &["vue3.dom.transformStyle"][..],
        ),
        (
            "packages/compiler-dom/src/transforms/stringifyStatic.ts",
            &["vue3.core.stringifyStatic"][..],
        ),
        (
            "packages/compiler-dom/src/transforms/vHtml.ts",
            &["vue3.dom.transformVHtml"][..],
        ),
        (
            "packages/compiler-dom/src/transforms/vText.ts",
            &["vue3.dom.transformVText"][..],
        ),
        (
            "packages/compiler-dom/src/transforms/vShow.ts",
            &["vue3.dom.transformShow"][..],
        ),
        (
            "packages/compiler-dom/src/transforms/vOn.ts",
            &["vue3.dom.transformOn"][..],
        ),
        (
            "packages/compiler-dom/src/transforms/vModel.ts",
            &["vue3.dom.transformModel"][..],
        ),
        (
            "packages/compiler-dom/src/transforms/Transition.ts",
            &["vue3.dom.transformTransition"][..],
        ),
        (
            "packages/compiler-dom/src/transforms/ignoreSideEffectTags.ts",
            &["vue3.dom.ignoreSideEffectTags"][..],
        ),
        (
            "packages/compiler-dom/src/transforms/validateHtmlNesting.ts",
            &["vue3.dom.validateHtmlNesting"][..],
        ),
        (
            "packages/compiler-dom/src/decodeHtmlBrowser.ts",
            &["vue3.dom.decodeHtmlBrowser"][..],
        ),
        (
            "packages/compiler-dom/src/htmlNesting.ts",
            &["vue3.dom.isValidHTMLNesting"][..],
        ),
    ] {
        add_manifest_entry(
            manifest,
            path,
            path,
            "dom-source-path-runtime-shim",
            None,
            commands,
            provenance(
                "prepared-official",
                "source-path-shim-to-alias-runtime",
                "projection-command",
                &["source-path-shim", "runtime-projection-adapter"],
            ),
        );
    }
    add_manifest_entry(
        manifest,
        "packages/compiler-core/__tests__/testUtils.ts",
        "packages/compiler-core/__tests__/testUtils.ts",
        "copied-cross-suite-test-helper",
        None,
        &[],
        provenance(
            "prepared-official",
            "copied-official-test-helper",
            "test-helper-boundary",
            &["official-helper-boundary"],
        ),
    );
}

fn add_vue3_sfc_manifest_entries(manifest: &mut PreparedTestManifest) {
    add_manifest_entry(
        manifest,
        "packages/compiler-sfc/src/**",
        "packages/compiler-sfc/src/**",
        "copied-official-source-boundary",
        None,
        &[
            "sfc.parse",
            "sfc.compileTemplate",
            "sfc.compileScript",
            "sfc.compileStyle",
            "sfc.resolveType",
        ],
        provenance(
            "prepared-official",
            "official-sfc-source-plus-alias-runtime",
            "mixed-official-source-boundary",
            &["official-source-boundary", "package-alias-config"],
        ),
    );
    add_manifest_entry(
        manifest,
        "packages/compiler-sfc/src/compileTemplate.ts",
        "packages/compiler-sfc/src/compileTemplate.ts",
        "prepared-source-patch",
        None,
        &["sfc.compileTemplate"],
        provenance(
            "prepared-official",
            "official-sfc-source-plus-prepared-patch",
            "mixed-official-source-boundary",
            &["official-source-boundary", "source-patch"],
        ),
    );

    for (spec, helper, commands) in [
        (
            "packages/compiler-sfc/__tests__/parse.spec.ts",
            None,
            &["sfc.parse"][..],
        ),
        (
            "packages/compiler-sfc/__tests__/rewriteDefault.spec.ts",
            None,
            &["sfc.rewriteDefault"][..],
        ),
        (
            "packages/compiler-sfc/__tests__/compileStyle.spec.ts",
            None,
            &["sfc.compileStyle", "sfc.compileStyleAsync"][..],
        ),
        (
            "packages/compiler-sfc/__tests__/compileTemplate.spec.ts",
            Some("packages/compiler-sfc/__tests__/utils.public-api.ts"),
            &["sfc.compileTemplate", "sfc.parse", "sfc.compileScript"][..],
        ),
        (
            "packages/compiler-sfc/__tests__/cssVars.spec.ts",
            Some("packages/compiler-sfc/__tests__/utils.public-api.ts"),
            &["sfc.compileStyle", "sfc.parse", "sfc.compileScript"][..],
        ),
        (
            "packages/compiler-sfc/__tests__/compileScript.spec.ts",
            Some("packages/compiler-sfc/__tests__/utils.public-api.ts"),
            &["sfc.compileScript", "sfc.parse"][..],
        ),
        (
            "packages/compiler-sfc/__tests__/templateUtils.spec.ts",
            Some("packages/compiler-sfc/__tests__/templateUtils.rust-api.ts"),
            &[
                "sfc.templateUtils.isRelativeUrl",
                "sfc.templateUtils.isExternalUrl",
                "sfc.templateUtils.isDataUrl",
            ][..],
        ),
        (
            "packages/compiler-sfc/__tests__/templateTransformAssetUrl.spec.ts",
            Some("packages/compiler-sfc/__tests__/templateTransforms.public-api.ts"),
            &["sfc.compileTemplate"][..],
        ),
        (
            "packages/compiler-sfc/__tests__/templateTransformSrcset.spec.ts",
            Some("packages/compiler-sfc/__tests__/templateTransforms.public-api.ts"),
            &["sfc.compileTemplate"][..],
        ),
        (
            "packages/compiler-sfc/__tests__/compileScript/resolveType.spec.ts",
            Some("packages/compiler-sfc/__tests__/compileScript/resolveType.rust-api.ts"),
            &["sfc.resolveType"][..],
        ),
    ] {
        add_manifest_entry(
            manifest,
            spec,
            spec,
            "test-spec-public-api-import-rewrite",
            helper,
            commands,
            provenance(
                "prepared-official",
                "public-package-alias-to-rust-bridge",
                "public-api",
                &["test-import-rewrite", "helper-shape-adapter"],
            ),
        );
    }

    for spec in [
        "defineProps.spec.ts",
        "definePropsDestructure.spec.ts",
        "defineEmits.spec.ts",
        "defineExpose.spec.ts",
        "defineModel.spec.ts",
        "defineOptions.spec.ts",
        "defineSlots.spec.ts",
        "hoistStatic.spec.ts",
        "importUsageCheck.spec.ts",
    ] {
        let path = format!("packages/compiler-sfc/__tests__/compileScript/{spec}");
        add_manifest_entry(
            manifest,
            &path,
            &path,
            "test-spec-public-api-helper-rewrite",
            Some("packages/compiler-sfc/__tests__/utils.public-api.ts"),
            &["sfc.compileScript", "sfc.parse"],
            provenance(
                "prepared-official",
                "public-package-alias-to-rust-bridge",
                "public-api",
                &["test-import-rewrite", "helper-shape-adapter"],
            ),
        );
    }

    for (helper, commands, roles) in [
        (
            "packages/compiler-sfc/__tests__/utils.public-api.ts",
            &["sfc.parse", "sfc.compileScript"][..],
            &["public-api-test-helper", "assertion-shape-adapter"][..],
        ),
        (
            "packages/compiler-sfc/__tests__/templateUtils.rust-api.ts",
            &[
                "sfc.templateUtils.isRelativeUrl",
                "sfc.templateUtils.isExternalUrl",
                "sfc.templateUtils.isDataUrl",
            ][..],
            &["rust-projection-helper"][..],
        ),
        (
            "packages/compiler-sfc/__tests__/templateTransforms.public-api.ts",
            &["sfc.compileTemplate"][..],
            &["public-api-test-helper", "option-shape-adapter"][..],
        ),
        (
            "packages/compiler-sfc/__tests__/compileScript/resolveType.rust-api.ts",
            &["sfc.resolveType"][..],
            &["rust-projection-helper", "virtual-file-materializer"][..],
        ),
    ] {
        add_manifest_entry(
            manifest,
            helper,
            helper,
            "generated-test-helper",
            None,
            commands,
            provenance(
                "prepared-official",
                "prepared-helper-to-rust-bridge",
                "public-api-or-projection-command",
                roles,
            ),
        );
    }

    add_manifest_entry(
        manifest,
        "packages/compiler-dom/src/transforms/stringifyStatic.ts",
        "packages/compiler-dom/src/transforms/stringifyStatic.ts",
        "copied-official-source-boundary",
        None,
        &["vue3.core.stringifyStatic"],
        provenance(
            "prepared-official",
            "copied-official-dom-source-helper",
            "mixed-official-source-boundary",
            &["official-source-boundary"],
        ),
    );
    add_manifest_entry(
        manifest,
        "generated/package.json",
        "package.json",
        "runner-package-module-config",
        None,
        &[],
        provenance(
            "prepared-official",
            "prepared-vitest-runner",
            "runner-harness",
            &["runner-shim"],
        ),
    );
}

fn add_vue3_ssr_manifest_entries(manifest: &mut PreparedTestManifest) {
    add_manifest_entry(
        manifest,
        "packages/compiler-ssr/src/**",
        "packages/compiler-ssr/src/**",
        "copied-official-source-boundary",
        None,
        &["vue3.ssr.compile"],
        provenance(
            "prepared-official",
            "official-ssr-source-plus-alias-runtime",
            "mixed-official-source-boundary",
            &["official-source-boundary", "package-alias-config"],
        ),
    );
    add_manifest_entry(
        manifest,
        "packages/compiler-dom/src/**",
        "packages/compiler-dom/src/**",
        "copied-official-source-boundary",
        None,
        &["vue3.dom.compile", "vue3.dom.parse"],
        provenance(
            "prepared-official",
            "official-dom-source-plus-ssr-source",
            "mixed-official-source-boundary",
            &["official-source-boundary", "package-alias-config"],
        ),
    );

    for spec in [
        "ssrVIf.spec.ts",
        "ssrVFor.spec.ts",
        "ssrScopeId.spec.ts",
        "ssrFallthroughAttrs.spec.ts",
        "ssrInjectCssVars.spec.ts",
        "ssrVShow.spec.ts",
        "ssrVModel.spec.ts",
        "ssrSlotOutlet.spec.ts",
        "ssrPortal.spec.ts",
        "ssrSuspense.spec.ts",
        "ssrTransition.spec.ts",
        "ssrTransitionGroup.spec.ts",
        "ssrComponent.spec.ts",
    ] {
        let path = format!("packages/compiler-ssr/__tests__/{spec}");
        add_manifest_entry(
            manifest,
            &path,
            &path,
            "test-spec-public-ssr-compile-import-rewrite",
            None,
            &["vue3.ssr.compile"],
            provenance(
                "prepared-official",
                "public-package-alias-to-rust-bridge",
                "public-api",
                &["test-import-rewrite", "package-alias-adapter"],
            ),
        );
    }

    for spec in ["ssrText.spec.ts", "ssrElement.spec.ts"] {
        let path = format!("packages/compiler-ssr/__tests__/{spec}");
        add_manifest_entry(
            manifest,
            &path,
            &path,
            "test-spec-public-ssr-compile-import-and-helper-rewrite",
            Some("packages/compiler-ssr/__tests__/utils.rust-ssr-text.ts"),
            &["vue3.ssr.compile"],
            provenance(
                "prepared-official",
                "public-package-alias-to-rust-bridge",
                "public-api",
                &["test-import-rewrite", "helper-shape-adapter"],
            ),
        );
    }
    add_manifest_entry(
        manifest,
        "packages/compiler-ssr/__tests__/utils.ts",
        "packages/compiler-ssr/__tests__/utils.rust-ssr-text.ts",
        "generated-public-ssr-compile-helper",
        None,
        &["vue3.ssr.compile"],
        provenance(
            "prepared-official",
            "prepared-helper-to-rust-bridge",
            "public-api",
            &["helper-shape-adapter"],
        ),
    );
}

fn add_vue3_vitest_manifest_entries(manifest: &mut PreparedTestManifest, include_glob: &str) {
    add_vitest_provenance_manifest_entry(manifest);
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
    add_manifest_entry(
        manifest,
        include_glob,
        include_glob,
        "runner-include-glob",
        None,
        &[],
        provenance(
            "prepared-official",
            "prepared-vitest-runner",
            "runner-harness",
            &["runner-config"],
        ),
    );
}
