fn conformance_coverage_transform_element_entries(
    path: &str,
    result: &serde_json::Value,
    default_reason: &str,
) -> Vec<ConformanceCoverageFile> {
    let Some(assertions) = result
        .get("assertionResults")
        .and_then(|value| value.as_array())
    else {
        return Vec::new();
    };

    let mut grouped: BTreeMap<&'static str, Vec<&serde_json::Value>> = BTreeMap::new();
    for assertion in assertions {
        let full_name = vitest_assertion_full_name(assertion);
        if full_name.is_empty() {
            return Vec::new();
        }
        let scope = conformance_coverage_transform_element_assertion_scope(&full_name);
        grouped.entry(scope).or_default().push(assertion);
    }

    grouped
        .into_iter()
        .map(|(scope, assertions)| {
            let markers = conformance_runtime_markers_from_assertions(&assertions);
            let provenance =
                conformance_transform_element_scope_provenance(scope).with_runtime_markers(markers);
            let source = provenance.legacy_source();
            let file_reason =
                conformance_coverage_transform_element_reason(scope, source, default_reason);
            conformance_coverage_file(
                path,
                Some(scope),
                provenance,
                file_reason,
                json_conformance_assertion_counts(assertions.iter().copied()),
            )
        })
        .collect()
}

fn conformance_transform_element_scope_provenance(scope: &str) -> ConformanceCoverageProvenance {
    match scope {
        "imported v-for helper" => ConformanceCoverageProvenance::new(
            "prepared-official",
            "hybrid-js-adapter-rust-projection",
            "suite-only-bridge-command",
            &[
                "test-import-rewrite",
                "suite-helper",
                "hydration-dehydration",
            ],
            &["vue3.core.transformForSuite"],
        ),
        "element transform rust suite" => ConformanceCoverageProvenance::new(
            "prepared-official",
            "hybrid-js-adapter-rust-projection",
            "suite-only-bridge-command",
            &[
                "test-import-rewrite",
                "suite-helper",
                "hydration-dehydration",
            ],
            &["vue3.core.transformElementSuite"],
        ),
        "js callback boundary" => ConformanceCoverageProvenance::new(
            "prepared-official",
            "mixed-js-callback-boundary",
            "suite-only-bridge-command",
            &[
                "test-import-rewrite",
                "suite-helper",
                "hydration-dehydration",
                "callback-materialization",
            ],
            &["vue3.core.transformElementSuite"],
        ),
        _ => ConformanceCoverageProvenance::new(
            "prepared-official",
            "hybrid-js-adapter-rust-projection",
            "suite-only-bridge-command",
            &[
                "test-import-rewrite",
                "suite-helper",
                "hydration-dehydration",
            ],
            &["vue3.core.transformElementSuite"],
        ),
    }
}

fn conformance_coverage_transform_element_assertion_scope(full_name: &str) -> &'static str {
    if full_name.starts_with("compiler: v-for ") {
        return "imported v-for helper";
    }
    if matches!(
        full_name,
        "compiler: element transform directiveTransforms"
            | "compiler: element transform directiveTransform with needRuntime: true"
            | "compiler: element transform directiveTransform with needRuntime: Symbol"
            | "compiler: element transform should process node when node has been replaced"
    ) {
        return "js callback boundary";
    }
    if full_name.starts_with("compiler: element transform ") {
        return "element transform rust suite";
    }
    "unclassified assertions"
}

fn conformance_coverage_transform_element_reason(
    scope: &str,
    source: ConformanceCoverageKind,
    default_reason: &str,
) -> String {
    match (scope, source) {
        ("imported v-for helper", _) => {
            "Official Vue 3 compiler-core transformElement file re-imports parseWithForTransform from the prepared vFor Rust API helper; these duplicated v-for assertions route through suite-only vuec_node_bridge command vue3.core.transformForSuite and Rust transformFor/codegen projections, so they are hybrid projection evidence rather than public API completion evidence."
                .to_string()
        }
        ("element transform rust suite", _) => {
            "Official Vue 3 compiler-core transformElement file imports a prepared Rust API helper that forwards ordinary parseWithElementTransform/parseWithBind assertions through @vue/compiler-core.__vuecRuntime into suite-only vuec_node_bridge command vue3.core.transformElementSuite. Rust parser and transform projections execute, but the tested surface is a prepared suite helper rather than the public package API."
                .to_string()
        }
        ("js callback boundary", ConformanceCoverageKind::Mixed) => {
            "Official Vue 3 compiler-core transformElement assertion group exercises caller-provided JavaScript directiveTransforms or NodeTransform callbacks. Those callback extension points cannot be serialized into the Rust bridge and remain mixed coverage rather than Rust compiler completion evidence."
                .to_string()
        }
        _ => default_reason.to_string(),
    }
}

fn conformance_coverage_transform_entries(
    path: &str,
    result: &serde_json::Value,
    default_reason: &str,
) -> Vec<ConformanceCoverageFile> {
    let Some(assertions) = result
        .get("assertionResults")
        .and_then(|value| value.as_array())
    else {
        return Vec::new();
    };

    let mut grouped: BTreeMap<&'static str, Vec<&serde_json::Value>> = BTreeMap::new();
    for assertion in assertions {
        let full_name = vitest_assertion_full_name(assertion);
        if full_name.is_empty() {
            return Vec::new();
        }
        let scope = conformance_coverage_transform_assertion_scope(&full_name);
        grouped.entry(scope).or_default().push(assertion);
    }

    grouped
        .into_iter()
        .map(|(scope, assertions)| {
            let markers = conformance_runtime_markers_from_assertions(&assertions);
            let provenance =
                conformance_transform_scope_provenance(scope).with_runtime_markers(markers);
            let source = provenance.legacy_source();
            let file_reason = conformance_coverage_transform_reason(scope, source, default_reason);
            conformance_coverage_file(
                path,
                Some(scope),
                provenance,
                file_reason,
                json_conformance_assertion_counts(assertions.iter().copied()),
            )
        })
        .collect()
}

fn conformance_transform_scope_provenance(scope: &str) -> ConformanceCoverageProvenance {
    match scope {
        "transform rust suite" => ConformanceCoverageProvenance::new(
            "prepared-official",
            "hybrid-js-adapter-rust-projection",
            "suite-only-bridge-command",
            &[
                "test-import-rewrite",
                "suite-helper",
                "hydration-dehydration",
            ],
            &["vue3.core.transformSuite"],
        ),
        "js transform context boundary" => ConformanceCoverageProvenance::new(
            "prepared-official",
            "mixed-js-callback-boundary",
            "suite-only-bridge-command",
            &[
                "test-import-rewrite",
                "suite-helper",
                "hydration-dehydration",
                "callback-materialization",
            ],
            &["vue3.core.transformSuite"],
        ),
        _ => ConformanceCoverageProvenance::new(
            "prepared-official",
            "hybrid-js-adapter-rust-projection",
            "suite-only-bridge-command",
            &[
                "test-import-rewrite",
                "suite-helper",
                "hydration-dehydration",
            ],
            &["vue3.core.transformSuite"],
        ),
    }
}

fn conformance_coverage_transform_assertion_scope(full_name: &str) -> &'static str {
    if matches!(
        full_name,
        "compiler: transform should inject toString helper for interpolations"
            | "compiler: transform should inject createVNode and Comment for comments"
    ) || full_name.starts_with("compiler: transform root codegenNode ")
    {
        return "transform rust suite";
    }
    if matches!(
        full_name,
        "compiler: transform context state"
            | "compiler: transform context.replaceNode"
            | "compiler: transform context.removeNode"
            | "compiler: transform context.removeNode (prev sibling)"
            | "compiler: transform context.removeNode (next sibling)"
            | "compiler: transform context.hoist"
            | "compiler: transform context.filename and selfName"
            | "compiler: transform onError option"
    ) {
        return "js transform context boundary";
    }
    "unclassified assertions"
}

fn conformance_coverage_transform_reason(
    scope: &str,
    source: ConformanceCoverageKind,
    default_reason: &str,
) -> String {
    match (scope, source) {
        ("transform rust suite", ConformanceCoverageKind::RustBacked) => {
            "Official Vue 3 compiler-core transform file imports a prepared Rust API helper that forwards helper-injection and root-codegen assertions through @vue/compiler-core.__vuecRuntime into vuec_node_bridge command vue3.core.transformSuite; the helper only hydrates public AST helper symbols and undefined fields while Rust parser, transformIf/transformFor/transformText/transformSlotOutlet/transformElement projections, helper collection, and createRootCodegen-compatible root projection execute through Rust."
                .to_string()
        }
        ("transform rust suite", ConformanceCoverageKind::Mixed) => {
            "Official Vue 3 compiler-core transform file imports a prepared Rust API helper that forwards helper-injection and root-codegen assertions through @vue/compiler-core.__vuecRuntime into suite-only vuec_node_bridge command vue3.core.transformSuite. Rust parser and transform projections execute, but the tested surface is a prepared suite helper rather than the public package API, so this remains hybrid projection evidence."
                .to_string()
        }
        ("js transform context boundary", ConformanceCoverageKind::Mixed) => {
            "Official Vue 3 compiler-core transform assertion group exercises caller-provided JavaScript NodeTransform callbacks and mutable transform context APIs such as replaceNode, removeNode, hoist, filename/selfName, and onError. These extension points cannot be serialized into the Rust bridge and remain mixed coverage rather than Rust compiler completion evidence."
                .to_string()
        }
        _ => default_reason.to_string(),
    }
}

fn conformance_coverage_file_reason(
    path: &str,
    provenance: &ConformanceCoverageProvenance,
    default_reason: &str,
) -> String {
    let source = provenance.legacy_source();
    if source == ConformanceCoverageKind::Mixed
        && provenance.execution_path == "mixed-js-callback-boundary"
    {
        if path.ends_with("packages/compiler-sfc/test/compileStyle.spec.ts")
            || provenance
                .runtime_markers
                .iter()
                .any(|marker| marker.contains("postcss") || marker.contains("PostCSS"))
        {
            return "Official file exercises a mixed path: Rust SFC style compilation participates, while caller-provided PostCSS plugin callbacks/options and Promise/LazyResult API behavior execute in the JavaScript adapter because those callbacks cannot cross the JSON bridge."
                .to_string();
        }
        return "Official file exercises a mixed JavaScript callback boundary. Runtime provenance or prepared adapter metadata shows caller-provided JavaScript callbacks/context APIs participating, so this entry is not counted as Rust compiler completion evidence."
            .to_string();
    }
    if source == ConformanceCoverageKind::Mixed
        && provenance.api_surface == "suite-only-bridge-command"
    {
        let commands = if provenance.bridge_commands.is_empty() {
            "suite-only vuec_node_bridge command".to_string()
        } else {
            format!(
                "suite-only vuec_node_bridge command(s) {}",
                provenance.bridge_commands.join(", ")
            )
        };
        return format!(
            "Official prepared test file routes assertions through {commands}. Rust parser/transform/codegen projections may execute, but the asserted surface is a prepared suite helper rather than the public Vue package API, so this is hybrid projection evidence rather than Rust-backed public API completion."
        );
    }
    if source == ConformanceCoverageKind::Mixed && provenance.api_surface == "projection-command" {
        let commands = if provenance.bridge_commands.is_empty() {
            "vuec_node_bridge projection command".to_string()
        } else {
            format!(
                "vuec_node_bridge projection command(s) {}",
                provenance.bridge_commands.join(", ")
            )
        };
        return format!(
            "Official prepared test file routes assertions through {commands}. Rust implementation participates, but the asserted bridge surface is an internal projection/helper command rather than the public Vue package API, so this is projection evidence rather than Rust-backed public API completion."
        );
    }
    if source == ConformanceCoverageKind::Mixed
        && provenance.execution_path == "hybrid-js-adapter-rust-projection"
    {
        return "Official prepared test file executes through generated import/helper adapters and Rust bridge projections. This is useful hybrid conformance evidence, but the file is not counted as Rust-backed public API completion because official source, helper imports, or adapter materialization still participate."
            .to_string();
    }
    match source {
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-core/__tests__/transforms/vBind.spec.ts") =>
        {
            "Official Vue 3 compiler-core vBind file imports a prepared Rust API helper that forwards parseWithVBind through @vue/compiler-core.__vuecRuntime into vuec_node_bridge command vue3.core.transformBindSuite; the helper only hydrates public AST helper symbols and emits Rust-projected errors while Rust parser, transformVBindShorthand projection, processExpression projection, transformBind projection, public VNode props projection, and NORMALIZE_PROPS wrapping execute through Rust."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-core/__tests__/transforms/cacheStatic.spec.ts") =>
        {
            "Official Vue 3 compiler-core cacheStatic file imports a prepared Rust API helper that forwards transformWithCache through @vue/compiler-core.__vuecRuntime into vuec_node_bridge command vue3.core.cacheStaticSuite; the helper only normalizes serializable options, hydrates public AST helper symbols, restores public undefined fields, and emits Rust-projected errors while Rust parser, transformVBindShorthand projection, transformIf projection, transformFor projection, processExpression projection, transformElement projection, transformText projection, cacheStatic projection, getConstantType projection, cache materialization, and hoist materialization execute through Rust."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-core/__tests__/transforms/vModel.spec.ts") =>
        {
            "Official Vue 3 compiler-core vModel file imports a prepared Rust API helper that forwards parseWithVModel through @vue/compiler-core.__vuecRuntime into vuec_node_bridge command vue3.core.transformModelSuite; the helper only normalizes serializable options, hydrates public AST helper symbols, and emits Rust-projected errors while Rust parser, transformFor projection, processExpression projection, trackSlotScopes projection, transformModel projection, cache handling, public VNode props projection, dynamicProps projection, and generate snapshots execute through Rust."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-core/__tests__/transforms/vOn.spec.ts") =>
        {
            "Official Vue 3 compiler-core vOn file imports a prepared Rust API helper that forwards parseWithVOn through @vue/compiler-core.__vuecRuntime into vuec_node_bridge command vue3.core.transformOnSuite; the helper only normalizes serializable options including isNativeTag predicate hits, hydrates public AST helper symbols, restores public undefined fields, and emits Rust-projected errors while Rust parser, transformFor projection, processExpression projection for dynamic event args, transformOn projection, cache/scope handling, public VNode props projection, and root codegen projection execute through Rust."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-core/__tests__/transforms/vFor.spec.ts") =>
        {
            "Official Vue 3 compiler-core vFor file imports a prepared Rust API helper that forwards parseWithForTransform through @vue/compiler-core.__vuecRuntime into vuec_node_bridge command vue3.core.transformForSuite; the helper only normalizes serializable options, hydrates public AST helper symbols, restores public undefined fields, and emits Rust-projected errors while Rust parser, transformVBindShorthand projection, transformIf projection, transformFor projection, processExpression projection, transformBind projection, transformSlotOutlet projection, public VNode props/codegen projection, key injection, fragment flags, ref_for marker materialization, and root helper projection execute through Rust."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-core/__tests__/transforms/vIf.spec.ts") =>
        {
            "Official Vue 3 compiler-core vIf file imports a prepared Rust API helper that forwards parseWithIfTransform through @vue/compiler-core.__vuecRuntime into vuec_node_bridge command vue3.core.transformIfSuite; the helper only normalizes serializable options, hydrates public AST helper symbols, restores public undefined branch fields, and emits Rust-projected errors while Rust parser, transformVBindShorthand projection, transformIf/processIf projection, processExpression projection, transformBind projection, transformOn projection, transformSlotOutlet projection, public branch/codegen projection, key injection, prop merging, runtime directive materialization, and generate snapshots execute through Rust."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-core/__tests__/transforms/vSlot.spec.ts") =>
        {
            "Official Vue 3 compiler-core vSlot file imports a prepared Rust API helper that forwards parseWithSlots through @vue/compiler-core.__vuecRuntime into vuec_node_bridge command vue3.core.transformSlotSuite; the helper only normalizes serializable options, hydrates public AST helper symbols, restores public undefined fields, and emits Rust-projected errors while Rust parser, transformVBindShorthand projection, transformIf projection, transformFor projection, processExpression projection, trackSlotScopes/trackVForSlotScopes projection, transformSlotOutlet projection, buildSlots projection, public slot object/dynamic slot/codegen projection, forwarded slot flags, and root helper projection execute through Rust."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-core/__tests__/transforms/transformExpressions.spec.ts") =>
        {
            "Official Vue 3 compiler-core transformExpressions file imports a prepared Rust API helper that forwards parseWithExpressionTransform through @vue/compiler-core.__vuecRuntime into vuec_node_bridge command vue3.core.transformExpressionSuite; the helper only normalizes serializable options, hydrates public AST helper symbols, and emits Rust-projected SyntaxError objects while Rust parser, transformExpression projection, processExpression projection, bindingMetadata handling, expression plugin option handling, directive expression materialization, and public baseCompile snapshots execute through Rust."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-core/__tests__/transforms/transformText.spec.ts") =>
        {
            "Official Vue 3 compiler-core transformText file imports a prepared Rust API helper that forwards transformWithTextOpt through @vue/compiler-core.__vuecRuntime into vuec_node_bridge command vue3.core.transformTextSuite; the helper only hydrates public AST helper symbols while Rust parser, transformExpression/processFor/transformText projections, public AST codegen, and generate snapshots execute through Rust."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-core/__tests__/transforms/transformSlotOutlet.spec.ts") =>
        {
            "Official Vue 3 compiler-core transformSlotOutlet file imports a prepared Rust API helper that forwards parseWithSlots through @vue/compiler-core.__vuecRuntime into vuec_node_bridge command vue3.core.transformSlotOutletSuite; the helper only hydrates public AST helper symbols and emits Rust-projected errors while Rust parser, transformSlotOutlet/processSlotOutlet projection, slot props/fallback materialization, and public codegen node projection execute through Rust."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-core/__tests__/transforms/vOnce.spec.ts") =>
        {
            "Official Vue 3 compiler-core vOnce file imports a prepared Rust API helper that forwards transformWithOnce through @vue/compiler-core.__vuecRuntime into vuec_node_bridge command vue3.core.transformOnceSuite; the helper only hydrates public AST helper symbols while Rust parser, transformOnce cache intent, structural root/if/for public AST projection, and generate snapshots execute through Rust."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-core/__tests__/transforms/vMemo.spec.ts") =>
        {
            "Official Vue 3 compiler-core vMemo file imports public baseCompile from ../../src, whose prepared source re-exports @vue/compiler-core; the public alias routes baseCompile through vuec_node_bridge into Rust when no caller-provided JavaScript transform callbacks are present, so v-memo codegen and snapshots are exercised by the Rust Vue 3 core compiler."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-sfc/__tests__/compileTemplate.spec.ts") =>
        {
            "Official Vue 3 SFC compileTemplate file imports the public @vue/compiler-sfc API and, when no JavaScript callback provenance marker is observed for this entry, routes ordinary DOM/SSR template compilation, preprocessing, AST reuse, diagnostics, asset URL transforms, and source maps through vuec_node_bridge into Rust; the generated JavaScript package boundary only hydrates/dehydrates public AST and error shapes."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-sfc/__tests__/compileStyle.spec.ts") =>
        {
            "Official Vue 3 SFC compileStyle file imports the public @vue/compiler-sfc API and, when no JavaScript callback provenance marker is observed for this entry, routes CSS scoped/modules/preprocess compilation through vuec_node_bridge into Rust; the generated JavaScript package boundary only normalizes the public result shape."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-sfc/__tests__/cssVars.spec.ts") =>
        {
            "Official Vue 3 SFC cssVars file imports the public @vue/compiler-sfc API and routes parse, compileStyle, and compileScript through vuec_node_bridge into Rust; the generated per-file helper only preserves the official test utility shape and Babel syntax assertion."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-sfc/__tests__/templateUtils.spec.ts") =>
        {
            "Official Vue 3 SFC templateUtils file imports a prepared Rust API helper that forwards URL classification calls through the generated @vue/compiler-sfc alias runtime into vuec_node_bridge and the Rust vuec_vue3_asset implementation; the helper only preserves the official test import shape."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-sfc/__tests__/templateTransformAssetUrl.spec.ts")
                || path.ends_with("packages/compiler-sfc/__tests__/templateTransformSrcset.spec.ts") =>
        {
            "Official Vue 3 SFC template asset/srcset transform file imports a prepared public API helper that maps the original local test helper arguments to public compileTemplate options, so asset URL/srcset transforms, hoistStatic, and stringifyStatic route through @vue/compiler-sfc, vuec_node_bridge, and the Rust SFC template compiler; the helper only materializes public API options and preserves the official test helper shape."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-sfc/__tests__/compileScript/resolveType.spec.ts") =>
        {
            "Official Vue 3 SFC resolveType file imports a prepared public Rust API helper that forwards type-resolution calls through @vue/compiler-sfc, vuec_node_bridge, and Rust vuec_sfc::resolve_vue3_type; the helper only materializes virtual files, maps serializable options, and translates dependency paths back to the official test shape."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-sfc/__tests__/compileScript/defineEmits.spec.ts")
                || path.ends_with("packages/compiler-sfc/__tests__/compileScript.spec.ts")
                || path.ends_with("packages/compiler-sfc/__tests__/compileScript/defineExpose.spec.ts")
                || path.ends_with("packages/compiler-sfc/__tests__/compileScript/defineModel.spec.ts")
                || path.ends_with("packages/compiler-sfc/__tests__/compileScript/defineOptions.spec.ts")
                || path.ends_with("packages/compiler-sfc/__tests__/compileScript/defineProps.spec.ts")
                || path.ends_with("packages/compiler-sfc/__tests__/compileScript/definePropsDestructure.spec.ts")
                || path.ends_with("packages/compiler-sfc/__tests__/compileScript/defineSlots.spec.ts")
                || path.ends_with("packages/compiler-sfc/__tests__/compileScript/hoistStatic.spec.ts")
                || path.ends_with("packages/compiler-sfc/__tests__/compileScript/importUsageCheck.spec.ts") =>
        {
            "Official Vue 3 SFC compileScript file imports the prepared public API test helper, so parse and compileScript route through @vue/compiler-sfc, vuec_node_bridge, and the Rust vuec_sfc compileScript implementation; the helper only preserves official assertCode and compileSFCScript test utility shape."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked => {
            "Official file exercises compiler behavior routed through vuec_node_bridge into Rust parser/transform/codegen or Rust-backed projection implementation; generated import shims only preserve official import paths and materialize Rust projection results."
                .to_string()
        }
        ConformanceCoverageKind::ShimBacked | ConformanceCoverageKind::Mixed => default_reason.to_string(),
    }
}
