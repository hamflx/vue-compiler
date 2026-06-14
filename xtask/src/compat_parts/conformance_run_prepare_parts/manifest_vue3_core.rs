fn add_vue3_core_source_manifest_entries(manifest: &mut PreparedTestManifest) {
    for (module, commands) in [
        (
            "index",
            &[
                "vue3.core.baseCompile",
                "vue3.core.baseParse",
                "vue3.core.generate",
            ][..],
        ),
        ("ast", &[][..]),
        ("codegen", &["vue3.core.generate"][..]),
        ("compile", &["vue3.core.baseCompile"][..]),
        ("errors", &[][..]),
        ("options", &[][..]),
        ("parser", &["vue3.core.baseParse"][..]),
        ("runtimeHelpers", &[][..]),
        ("transform", &["vue3.core.rootCodegen"][..]),
        ("utils", &[][..]),
    ] {
        let path = format!("packages/compiler-core/src/{module}.ts");
        add_manifest_entry(
            manifest,
            &path,
            &path,
            "vue3-core-source-path-public-alias-shim",
            None,
            commands,
            provenance(
                "prepared-official",
                "source-path-shim-to-alias-runtime",
                "public-or-projection-bridge",
                &["source-path-shim", "package-api-adapter"],
            ),
        );
    }

    for (module, commands) in [
        (
            "transformElement",
            &[
                "vue3.core.transformElementProps",
                "vue3.core.transformElementChildren",
                "vue3.core.buildDirectiveArgs",
                "vue3.core.resolveComponentType",
            ][..],
        ),
        (
            "transformExpression",
            &[
                "vue3.core.transformExpression",
                "vue3.core.processExpression",
            ][..],
        ),
        (
            "transformSlotOutlet",
            &["vue3.core.transformSlotOutlet"][..],
        ),
        ("transformText", &["vue3.core.transformText"][..]),
        (
            "transformVBindShorthand",
            &["vue3.core.transformVBindShorthand"][..],
        ),
        ("vBind", &["vue3.core.transformBind"][..]),
        ("vFor", &["vue3.core.transformFor"][..]),
        ("vIf", &["vue3.core.transformIf"][..]),
        ("vMemo", &["vue3.core.transformMemo"][..]),
        ("vModel", &["vue3.core.transformModel"][..]),
        ("vOn", &["vue3.core.transformOn"][..]),
        ("vOnce", &["vue3.core.transformOnce"][..]),
        (
            "vSlot",
            &[
                "vue3.core.buildSlots",
                "vue3.core.trackSlotScopes",
                "vue3.core.trackVForSlotScopes",
            ][..],
        ),
    ] {
        let path = format!("packages/compiler-core/src/transforms/{module}.ts");
        add_manifest_entry(
            manifest,
            &path,
            &path,
            "vue3-core-transform-runtime-shim",
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
        "packages/compiler-dom/src/transforms/transformStyle.ts",
        "packages/compiler-dom/src/transforms/transformStyle.ts",
        "vue3-dom-transform-style-public-alias-shim",
        None,
        &["vue3.dom.transformStyle"],
        provenance(
            "prepared-official",
            "source-path-shim-to-alias-runtime",
            "projection-command",
            &["source-path-shim", "cross-package-transform-shim"],
        ),
    );
    add_manifest_entry(
        manifest,
        "packages/shared/src/index.ts",
        "packages/shared/src/index.ts",
        "shared-source-path-public-alias-shim",
        None,
        &[],
        provenance(
            "prepared-official",
            "source-path-shim-to-official-shared",
            "shared-runtime-boundary",
            &["source-path-shim"],
        ),
    );
}

fn add_vue3_core_prepared_spec_manifest_entries(manifest: &mut PreparedTestManifest) {
    for (spec, helper, commands) in [
        (
            "packages/compiler-core/__tests__/transforms/vBind.spec.ts",
            "packages/compiler-core/__tests__/transforms/vBind.rust-api.ts",
            &["vue3.core.transformBindSuite"][..],
        ),
        (
            "packages/compiler-core/__tests__/transforms/vModel.spec.ts",
            "packages/compiler-core/__tests__/transforms/vModel.rust-api.ts",
            &["vue3.core.transformModelSuite"][..],
        ),
        (
            "packages/compiler-core/__tests__/transforms/vOn.spec.ts",
            "packages/compiler-core/__tests__/transforms/vOn.rust-api.ts",
            &["vue3.core.transformOnSuite"][..],
        ),
        (
            "packages/compiler-core/__tests__/transforms/vFor.spec.ts",
            "packages/compiler-core/__tests__/transforms/vFor.rust-api.ts",
            &["vue3.core.transformForSuite"][..],
        ),
        (
            "packages/compiler-core/__tests__/transforms/transformElement.spec.ts",
            "packages/compiler-core/__tests__/transforms/transformElement.rust-api.ts",
            &[
                "vue3.core.transformElementSuite",
                "vue3.core.transformForSuite",
            ][..],
        ),
        (
            "packages/compiler-core/__tests__/transforms/noopDirectiveTransform.spec.ts",
            "packages/compiler-core/__tests__/transforms/noopDirectiveTransform.rust-api.ts",
            &["vue3.core.transformElementSuite"][..],
        ),
        (
            "packages/compiler-core/__tests__/transform.spec.ts",
            "packages/compiler-core/__tests__/transform.rust-api.ts",
            &["vue3.core.transformSuite"][..],
        ),
        (
            "packages/compiler-core/__tests__/transforms/vIf.spec.ts",
            "packages/compiler-core/__tests__/transforms/vIf.rust-api.ts",
            &["vue3.core.transformIfSuite"][..],
        ),
        (
            "packages/compiler-core/__tests__/transforms/transformSlotOutlet.spec.ts",
            "packages/compiler-core/__tests__/transforms/transformSlotOutlet.rust-api.ts",
            &["vue3.core.transformSlotOutletSuite"][..],
        ),
        (
            "packages/compiler-core/__tests__/transforms/vSlot.spec.ts",
            "packages/compiler-core/__tests__/transforms/vSlot.rust-api.ts",
            &["vue3.core.transformSlotSuite"][..],
        ),
        (
            "packages/compiler-core/__tests__/transforms/cacheStatic.spec.ts",
            "packages/compiler-core/__tests__/transforms/cacheStatic.rust-api.ts",
            &["vue3.core.cacheStaticSuite"][..],
        ),
        (
            "packages/compiler-core/__tests__/transforms/transformExpressions.spec.ts",
            "packages/compiler-core/__tests__/transforms/transformExpressions.rust-api.ts",
            &["vue3.core.transformExpressionSuite"][..],
        ),
        (
            "packages/compiler-core/__tests__/transforms/transformText.spec.ts",
            "packages/compiler-core/__tests__/transforms/transformText.rust-api.ts",
            &["vue3.core.transformTextSuite"][..],
        ),
        (
            "packages/compiler-core/__tests__/transforms/vOnce.spec.ts",
            "packages/compiler-core/__tests__/transforms/vOnce.rust-api.ts",
            &["vue3.core.transformOnceSuite"][..],
        ),
    ] {
        add_manifest_entry(
            manifest,
            spec,
            spec,
            "test-spec-suite-helper-reroute",
            Some(helper),
            commands,
            provenance(
                "prepared-official",
                "prepared-suite-helper-to-rust-bridge",
                "suite-only-bridge-command",
                &[
                    "test-import-rewrite",
                    "suite-helper",
                    "bridge-shape-adapter",
                ],
            ),
        );
        add_manifest_entry(
            manifest,
            helper,
            helper,
            "generated-suite-helper",
            None,
            commands,
            provenance(
                "prepared-official",
                "prepared-suite-helper-to-rust-bridge",
                "suite-only-bridge-command",
                &["suite-helper", "bridge-shape-adapter"],
            ),
        );
    }
}
