#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BridgeCommandCategory {
    PublicCommand,
    ProjectionCommand,
    SuiteCommand,
}

impl BridgeCommandCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PublicCommand => "public-command",
            Self::ProjectionCommand => "projection-command",
            Self::SuiteCommand => "suite-command",
        }
    }

    pub const fn api_surface(self) -> &'static str {
        match self {
            Self::PublicCommand => "public-command",
            Self::ProjectionCommand => "projection-command",
            Self::SuiteCommand => "suite-only-bridge-command",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BridgeCommandMetadata {
    pub name: &'static str,
    pub category: BridgeCommandCategory,
    pub owner: &'static str,
    pub public_api_equivalent: Option<&'static str>,
    pub migration_note: &'static str,
}

const PUBLIC_COMMAND_NOTE: &str =
    "Public alias command; keep routed through Rust public compiler APIs before counting it as public completion evidence.";
const PROJECTION_COMMAND_NOTE: &str =
    "Projection/helper command; migrate callers to public Rust APIs or native Rust tests before counting it as public API completion evidence.";
const SUITE_COMMAND_NOTE: &str =
    "Prepared official-suite command; replace with public API coverage or Rust-native conformance before counting it as public API completion evidence.";

const fn public(
    name: &'static str,
    owner: &'static str,
    public_api_equivalent: &'static str,
) -> BridgeCommandMetadata {
    BridgeCommandMetadata {
        name,
        category: BridgeCommandCategory::PublicCommand,
        owner,
        public_api_equivalent: Some(public_api_equivalent),
        migration_note: PUBLIC_COMMAND_NOTE,
    }
}

const fn projection(
    name: &'static str,
    owner: &'static str,
    public_api_equivalent: Option<&'static str>,
) -> BridgeCommandMetadata {
    BridgeCommandMetadata {
        name,
        category: BridgeCommandCategory::ProjectionCommand,
        owner,
        public_api_equivalent,
        migration_note: PROJECTION_COMMAND_NOTE,
    }
}

const fn suite(
    name: &'static str,
    owner: &'static str,
    public_api_equivalent: &'static str,
) -> BridgeCommandMetadata {
    BridgeCommandMetadata {
        name,
        category: BridgeCommandCategory::SuiteCommand,
        owner,
        public_api_equivalent: Some(public_api_equivalent),
        migration_note: SUITE_COMMAND_NOTE,
    }
}

pub const BRIDGE_COMMANDS: &[BridgeCommandMetadata] = &[
    public(
        "sfc.compileScript",
        "vuec_sfc::script",
        "@vue/compiler-sfc.compileScript",
    ),
    public(
        "sfc.compileStyle",
        "vuec_style::compiler",
        "@vue/compiler-sfc.compileStyle",
    ),
    public(
        "sfc.compileStyleAsync",
        "vuec_style::compiler",
        "@vue/compiler-sfc.compileStyleAsync",
    ),
    public(
        "sfc.compileTemplate",
        "vuec_sfc::template",
        "@vue/compiler-sfc.compileTemplate",
    ),
    public("sfc.parse", "vuec_sfc::parse", "@vue/compiler-sfc.parse"),
    projection(
        "sfc.resolveType",
        "vuec_sfc::script_type",
        Some("@vue/compiler-sfc.resolveTypeElements"),
    ),
    public(
        "sfc.rewriteDefault",
        "vuec_sfc::rewrite_default",
        "@vue/compiler-sfc.rewriteDefault",
    ),
    projection(
        "sfc.templateUtils.isDataUrl",
        "vuec_vue3_asset::template_utils",
        None,
    ),
    projection(
        "sfc.templateUtils.isExternalUrl",
        "vuec_vue3_asset::template_utils",
        None,
    ),
    projection(
        "sfc.templateUtils.isRelativeUrl",
        "vuec_vue3_asset::template_utils",
        None,
    ),
    public(
        "sfc.vue27.compileScript",
        "vuec_sfc::vue27_script",
        "vue/compiler-sfc.compileScript",
    ),
    public(
        "sfc.vue27.compileStyle",
        "vuec_style::compiler",
        "vue/compiler-sfc.compileStyle",
    ),
    public(
        "sfc.vue27.compileStyleAsync",
        "vuec_style::compiler",
        "vue/compiler-sfc.compileStyleAsync",
    ),
    public(
        "sfc.vue27.compileTemplate",
        "vuec_sfc::vue27_template",
        "vue/compiler-sfc.compileTemplate",
    ),
    public(
        "sfc.vue27.parse",
        "vuec_sfc::vue27_parse",
        "vue/compiler-sfc.parse",
    ),
    public(
        "sfc.vue27.parseComponent",
        "vuec_sfc::vue27_parse",
        "vue/compiler-sfc.parseComponent",
    ),
    projection(
        "sfc.vue27.prefixIdentifiers",
        "vuec_sfc::vue27_template",
        None,
    ),
    public(
        "sfc.vue27.rewriteDefault",
        "vuec_sfc::rewrite_default",
        "vue/compiler-sfc.rewriteDefault",
    ),
    public(
        "vue2.compile",
        "vuec_vue2::compiler",
        "vue-template-compiler.compile",
    ),
    public(
        "vue2.compileToFunctions",
        "vuec_vue2::compiler",
        "vue-template-compiler.compileToFunctions",
    ),
    projection(
        "vue2.generate",
        "vuec_vue2::codegen",
        Some("vue-template-compiler.compile"),
    ),
    public(
        "vue2.generateCodeFrame",
        "vuec_vue2::code_frame",
        "vue-template-compiler.generateCodeFrame",
    ),
    projection(
        "vue2.optimize",
        "vuec_vue2::optimizer",
        Some("vue-template-compiler.compile"),
    ),
    public(
        "vue2.ssrCompile",
        "vuec_vue2::compiler",
        "vue-template-compiler.ssrCompile",
    ),
    public(
        "vue2.ssrCompileToFunctions",
        "vuec_vue2::compiler",
        "vue-template-compiler.ssrCompileToFunctions",
    ),
    public(
        "vue3.core.baseCompile",
        "vuec_vue3_core::compiler",
        "@vue/compiler-core.baseCompile",
    ),
    public(
        "vue3.core.baseParse",
        "vuec_vue3_core::parser",
        "@vue/compiler-core.baseParse",
    ),
    public(
        "vue3.core.generate",
        "vuec_vue3_core::codegen",
        "@vue/compiler-core.generate",
    ),
    projection(
        "vue3.core.advancePositionWithClone",
        "vuec_vue3_core::source_location",
        Some("@vue/compiler-core.advancePositionWithClone"),
    ),
    projection(
        "vue3.core.advancePositionWithMutation",
        "vuec_vue3_core::source_location",
        Some("@vue/compiler-core.advancePositionWithMutation"),
    ),
    projection(
        "vue3.core.buildDirectiveArgs",
        "vuec_vue3_core::transform_element",
        Some("@vue/compiler-core.buildDirectiveArgs"),
    ),
    projection(
        "vue3.core.buildSlots",
        "vuec_vue3_core::transform_slot",
        Some("@vue/compiler-core.buildSlots"),
    ),
    projection(
        "vue3.core.cacheStatic",
        "vuec_vue3_core::cache_static",
        Some("@vue/compiler-core.cacheStatic"),
    ),
    suite(
        "vue3.core.cacheStaticSuite",
        "vuec_node_bridge::vue3_core_suite",
        "@vue/compiler-core.cacheStatic",
    ),
    projection(
        "vue3.core.extractIdentifiers",
        "vuec_vue3_core::expression",
        Some("@vue/compiler-core.extractIdentifiers"),
    ),
    projection(
        "vue3.core.getConstantType",
        "vuec_vue3_core::constant",
        Some("@vue/compiler-core.getConstantType"),
    ),
    projection(
        "vue3.core.isFunctionType",
        "vuec_vue3_core::expression",
        Some("@vue/compiler-core.isFunctionType"),
    ),
    projection(
        "vue3.core.isInDestructureAssignment",
        "vuec_vue3_core::expression",
        Some("@vue/compiler-core.isInDestructureAssignment"),
    ),
    projection(
        "vue3.core.isMemberExpression",
        "vuec_vue3_core::expression",
        Some("@vue/compiler-core.isMemberExpression"),
    ),
    projection(
        "vue3.core.isReferencedIdentifier",
        "vuec_vue3_core::expression",
        Some("@vue/compiler-core.isReferencedIdentifier"),
    ),
    projection(
        "vue3.core.isStaticProperty",
        "vuec_vue3_core::expression",
        Some("@vue/compiler-core.isStaticProperty"),
    ),
    projection(
        "vue3.core.processExpression",
        "vuec_vue3_core::transform_expression",
        Some("@vue/compiler-core.processExpression"),
    ),
    projection(
        "vue3.core.resolveComponentType",
        "vuec_vue3_core::transform_element",
        Some("@vue/compiler-core.resolveComponentType"),
    ),
    projection(
        "vue3.core.rootCodegen",
        "vuec_vue3_core::codegen",
        Some("@vue/compiler-core.transform"),
    ),
    projection(
        "vue3.core.stringifyStatic",
        "vuec_vue3_core::stringify_static",
        Some("@vue/compiler-core.stringifyStatic"),
    ),
    projection(
        "vue3.core.toValidAssetId",
        "vuec_vue3_core::assets",
        Some("@vue/compiler-core.toValidAssetId"),
    ),
    projection(
        "vue3.core.trackSlotScopes",
        "vuec_vue3_core::transform_slot",
        Some("@vue/compiler-core.trackSlotScopes"),
    ),
    projection(
        "vue3.core.trackVForSlotScopes",
        "vuec_vue3_core::transform_slot",
        Some("@vue/compiler-core.trackVForSlotScopes"),
    ),
    projection(
        "vue3.core.transformBind",
        "vuec_vue3_core::transform_bind",
        Some("@vue/compiler-core.transformBind"),
    ),
    suite(
        "vue3.core.transformBindSuite",
        "vuec_node_bridge::vue3_core_suite",
        "@vue/compiler-core.transformBind",
    ),
    projection(
        "vue3.core.transformElementChildren",
        "vuec_vue3_core::transform_element",
        Some("@vue/compiler-core.transformElement"),
    ),
    projection(
        "vue3.core.transformElementProps",
        "vuec_vue3_core::transform_element",
        Some("@vue/compiler-core.transformElement"),
    ),
    suite(
        "vue3.core.transformElementSuite",
        "vuec_node_bridge::vue3_core_suite",
        "@vue/compiler-core.transformElement",
    ),
    projection(
        "vue3.core.transformExpression",
        "vuec_vue3_core::transform_expression",
        Some("@vue/compiler-core.transformExpression"),
    ),
    suite(
        "vue3.core.transformExpressionSuite",
        "vuec_node_bridge::vue3_core_suite",
        "@vue/compiler-core.transformExpression",
    ),
    projection(
        "vue3.core.transformFor",
        "vuec_vue3_core::transform_for",
        Some("@vue/compiler-core.transformFor"),
    ),
    suite(
        "vue3.core.transformForSuite",
        "vuec_node_bridge::vue3_core_suite",
        "@vue/compiler-core.transformFor",
    ),
    projection(
        "vue3.core.transformIf",
        "vuec_vue3_core::transform_if",
        Some("@vue/compiler-core.transformIf"),
    ),
    suite(
        "vue3.core.transformIfSuite",
        "vuec_node_bridge::vue3_core_suite",
        "@vue/compiler-core.transformIf",
    ),
    projection(
        "vue3.core.transformMemo",
        "vuec_vue3_core::transform_memo",
        Some("@vue/compiler-core.transformMemo"),
    ),
    projection(
        "vue3.core.transformModel",
        "vuec_vue3_core::transform_model",
        Some("@vue/compiler-core.transformModel"),
    ),
    suite(
        "vue3.core.transformModelSuite",
        "vuec_node_bridge::vue3_core_suite",
        "@vue/compiler-core.transformModel",
    ),
    projection(
        "vue3.core.transformOn",
        "vuec_vue3_core::transform_on",
        Some("@vue/compiler-core.transformOn"),
    ),
    suite(
        "vue3.core.transformOnSuite",
        "vuec_node_bridge::vue3_core_suite",
        "@vue/compiler-core.transformOn",
    ),
    projection(
        "vue3.core.transformOnce",
        "vuec_vue3_core::transform_once",
        Some("@vue/compiler-core.transformOnce"),
    ),
    suite(
        "vue3.core.transformOnceSuite",
        "vuec_node_bridge::vue3_core_suite",
        "@vue/compiler-core.transformOnce",
    ),
    projection(
        "vue3.core.transformSlotOutlet",
        "vuec_vue3_core::transform_slot_outlet",
        Some("@vue/compiler-core.transformSlotOutlet"),
    ),
    suite(
        "vue3.core.transformSlotOutletSuite",
        "vuec_node_bridge::vue3_core_suite",
        "@vue/compiler-core.transformSlotOutlet",
    ),
    suite(
        "vue3.core.transformSlotSuite",
        "vuec_node_bridge::vue3_core_suite",
        "@vue/compiler-core.buildSlots",
    ),
    suite(
        "vue3.core.transformSuite",
        "vuec_node_bridge::vue3_core_suite",
        "@vue/compiler-core.transform",
    ),
    projection(
        "vue3.core.transformText",
        "vuec_vue3_core::transform_text",
        Some("@vue/compiler-core.transformText"),
    ),
    suite(
        "vue3.core.transformTextSuite",
        "vuec_node_bridge::vue3_core_suite",
        "@vue/compiler-core.transformText",
    ),
    projection(
        "vue3.core.transformVBindShorthand",
        "vuec_vue3_core::transform_bind",
        None,
    ),
    projection(
        "vue3.core.walkIdentifiers",
        "vuec_vue3_core::expression",
        Some("@vue/compiler-core.walkIdentifiers"),
    ),
    public(
        "vue3.dom.compile",
        "vuec_vue3_dom::compiler",
        "@vue/compiler-dom.compile",
    ),
    public(
        "vue3.dom.parse",
        "vuec_vue3_dom::parser",
        "@vue/compiler-dom.parse",
    ),
    projection(
        "vue3.dom.decodeHtmlBrowser",
        "vuec_vue3_dom::parser",
        Some("@vue/compiler-dom.decodeHtmlBrowser"),
    ),
    projection(
        "vue3.dom.ignoreSideEffectTags",
        "vuec_vue3_dom::transform",
        Some("@vue/compiler-dom.ignoreSideEffectTags"),
    ),
    projection(
        "vue3.dom.isValidHTMLNesting",
        "vuec_vue3_dom::html_nesting",
        Some("@vue/compiler-dom.isValidHTMLNesting"),
    ),
    projection(
        "vue3.dom.transformModel",
        "vuec_vue3_dom::transform_model",
        Some("@vue/compiler-dom.transformModel"),
    ),
    projection(
        "vue3.dom.transformOn",
        "vuec_vue3_dom::transform_on",
        Some("@vue/compiler-dom.transformOn"),
    ),
    projection(
        "vue3.dom.transformShow",
        "vuec_vue3_dom::transform_show",
        Some("@vue/compiler-dom.transformShow"),
    ),
    projection(
        "vue3.dom.transformStyle",
        "vuec_vue3_dom::transform_style",
        Some("@vue/compiler-dom.transformStyle"),
    ),
    projection(
        "vue3.dom.transformTransition",
        "vuec_vue3_dom::transform_transition",
        Some("@vue/compiler-dom.transformTransition"),
    ),
    projection(
        "vue3.dom.transformVHtml",
        "vuec_vue3_dom::transform_v_html",
        Some("@vue/compiler-dom.transformVHtml"),
    ),
    projection(
        "vue3.dom.transformVText",
        "vuec_vue3_dom::transform_v_text",
        Some("@vue/compiler-dom.transformVText"),
    ),
    projection(
        "vue3.dom.validateHtmlNesting",
        "vuec_vue3_dom::html_nesting",
        Some("@vue/compiler-dom.validateHtmlNesting"),
    ),
    public(
        "vue3.ssr.compile",
        "vuec_vue3_ssr::compiler",
        "@vue/compiler-ssr.compile",
    ),
];

pub fn bridge_command(name: &str) -> Option<&'static BridgeCommandMetadata> {
    BRIDGE_COMMANDS.iter().find(|command| command.name == name)
}

pub fn bridge_command_api_surface(name: &str) -> Option<&'static str> {
    bridge_command(name).map(|command| command.category.api_surface())
}

pub fn bridge_commands() -> &'static [BridgeCommandMetadata] {
    BRIDGE_COMMANDS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_commands_are_unique() {
        let mut names = std::collections::BTreeSet::new();
        for command in BRIDGE_COMMANDS {
            assert!(
                names.insert(command.name),
                "duplicate bridge command registry entry {}",
                command.name
            );
        }
    }

    #[test]
    fn bridge_registry_classifies_public_projection_and_suite_commands() {
        assert_eq!(
            bridge_command("sfc.compileScript").map(|command| command.category),
            Some(BridgeCommandCategory::PublicCommand)
        );
        assert_eq!(
            bridge_command("vue3.core.transformBindSuite").map(|command| command.category),
            Some(BridgeCommandCategory::SuiteCommand)
        );
        assert_eq!(
            bridge_command("vue3.core.transformElementProps").map(|command| command.category),
            Some(BridgeCommandCategory::ProjectionCommand)
        );
        assert_eq!(
            bridge_command_api_surface("vue3.ssr.compile"),
            Some("public-command")
        );
        assert_eq!(
            bridge_command_api_surface("vue3.core.transformElementSuite"),
            Some("suite-only-bridge-command")
        );
    }

    #[test]
    fn every_bridge_command_has_owner_and_migration_note() {
        for command in BRIDGE_COMMANDS {
            assert!(!command.owner.is_empty(), "{} owner", command.name);
            assert!(
                command.migration_note.contains("evidence"),
                "{} migration note",
                command.name
            );
            if command.category == BridgeCommandCategory::PublicCommand {
                assert!(
                    command.public_api_equivalent.is_some(),
                    "{} public equivalent",
                    command.name
                );
            }
        }
    }
}
