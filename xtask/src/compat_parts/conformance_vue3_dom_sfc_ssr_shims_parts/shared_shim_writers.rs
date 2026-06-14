fn write_vue3_core_test_setup(prepared_root: &Path) -> Result<()> {
    fs::create_dir_all(prepared_root)
        .with_context(|| format!("failed to create {}", prepared_root.display()))?;
    write_vuec_vitest_provenance_setup(prepared_root)?;
    write_text(
        &prepared_root.join("vuec-vitest-setup.ts"),
        r#"
import './vuec-vitest-provenance'
import { beforeEach, expect } from 'vitest'

const vuecWarnings: string[] = []

beforeEach(() => {
  vuecWarnings.length = 0
})

console.warn = (...args: unknown[]) => {
  vuecWarnings.push(args.map(arg => String(arg)).join(' '))
}

expect.extend({
  toHaveBeenWarned(received) {
    const expected = String(received)
    const pass = vuecWarnings.some(warning => warning.includes(expected))
    return {
      pass,
      message: () => `expected ${JSON.stringify(expected)} ${pass ? 'not ' : ''}to have been warned`,
    }
  },
})
"#,
    )
}

fn write_reexport_module(path: &Path, request: &str) -> Result<()> {
    write_text(
        path,
        &format!("export * from {}\n", js_string_literal(request)),
    )
}

fn write_vue3_core_transform_shim(path: &Path, module: &str) -> Result<()> {
    let exports = match module {
        "transformElement" => {
            "transformElement, buildProps, buildDirectiveArgs, resolveComponentType"
        }
        "transformExpression" => "transformExpression, processExpression",
        "transformSlotOutlet" => "transformSlotOutlet, processSlotOutlet",
        "transformText" => "transformText",
        "transformVBindShorthand" => "transformVBindShorthand",
        "vBind" => "transformBind",
        "vFor" => "transformFor, processFor, createForLoopParams",
        "vIf" => "transformIf, processIf",
        "vMemo" => "transformMemo",
        "vModel" => "transformModel",
        "vOn" => "transformOn",
        "vOnce" => "transformOnce",
        "vSlot" => "buildSlots, trackSlotScopes, trackVForSlotScopes",
        _ => "",
    };
    if exports.is_empty() {
        write_reexport_module(path, "@vue/compiler-core")
    } else {
        write_text(
            path,
            &format!(
                "import {{ __vuecRuntime }} from {}\nconst r = __vuecRuntime\nexport const {{ {exports} }} = r\n",
                js_string_literal("@vue/compiler-core")
            ),
        )
    }
}

fn write_vue3_dom_transform_shim(path: &Path, export_name: &str) -> Result<()> {
    write_text(
        path,
        &format!(
            "import {{ __vuecRuntime }} from {}\nconst r = __vuecRuntime\nexport const {export_name} = r.{export_name}\n",
            js_string_literal("@vue/compiler-dom")
        ),
    )
}

fn write_vue3_dom_stringify_static_shim(path: &Path) -> Result<()> {
    write_text(
        path,
        &format!(
            "import {{ __vuecRuntime }} from {}\nconst r = __vuecRuntime\nexport enum StringifyThresholds {{\n  ELEMENT_WITH_BINDING_COUNT = 5,\n  NODE_COUNT = 20,\n}}\nexport const stringifyStatic = (children, context, parent) => r.stringifyStatic(children, context, parent)\n",
            js_string_literal("@vue/compiler-core")
        ),
    )
}

fn write_vue3_dom_v_on_transform_shim(path: &Path) -> Result<()> {
    write_text(
        path,
        &format!(
            "import {{ __vuecRuntime }} from {}\nimport {{ TO_HANDLER_KEY }} from '@vue/compiler-core'\nimport {{ V_ON_WITH_KEYS, V_ON_WITH_MODIFIERS }} from '../runtimeHelpers'\nconst r = __vuecRuntime\nexport const transformOn = (dir, node, context) => {{\n  if (context) {{\n    context.__vuecDomHelpers = {{ ...(context.__vuecDomHelpers || {{}}), TO_HANDLER_KEY, V_ON_WITH_KEYS, V_ON_WITH_MODIFIERS }}\n  }}\n  return r.transformOn(dir, node, context)\n}}\n",
            js_string_literal("@vue/compiler-dom")
        ),
    )
}

fn write_vue3_dom_v_model_transform_shim(path: &Path) -> Result<()> {
    write_text(
        path,
        &format!(
            "import {{ __vuecRuntime }} from {}\nimport {{ V_MODEL_CHECKBOX, V_MODEL_DYNAMIC, V_MODEL_RADIO, V_MODEL_SELECT, V_MODEL_TEXT }} from '../runtimeHelpers'\nconst r = __vuecRuntime\nexport const transformModel = (dir, node, context) => {{\n  if (context) {{\n    context.__vuecDomHelpers = {{ ...(context.__vuecDomHelpers || {{}}), V_MODEL_CHECKBOX, V_MODEL_DYNAMIC, V_MODEL_RADIO, V_MODEL_SELECT, V_MODEL_TEXT }}\n  }}\n  return r.transformModel(dir, node, context)\n}}\n",
            js_string_literal("@vue/compiler-dom")
        ),
    )
}

fn write_vue3_dom_transition_transform_shim(path: &Path) -> Result<()> {
    write_text(
        path,
        &format!(
            "import {{ __vuecRuntime }} from {}\nimport {{ TRANSITION }} from '../runtimeHelpers'\nconst r = __vuecRuntime\nexport const transformTransition = (node, context) => {{\n  if (context) {{\n    context.__vuecDomHelpers = {{ ...(context.__vuecDomHelpers || {{}}), TRANSITION }}\n  }}\n  return r.transformTransition(node, context)\n}}\n",
            js_string_literal("@vue/compiler-dom")
        ),
    )
}

fn write_vue3_dom_validate_html_nesting_shim(path: &Path) -> Result<()> {
    write_text(
        path,
        &format!(
            "import {{ __vuecRuntime }} from {}\nconst r = __vuecRuntime\nexport const validateHtmlNesting = r.validateHtmlNesting\n",
            js_string_literal("@vue/compiler-dom")
        ),
    )
}

fn write_vue3_dom_ignore_side_effect_tags_shim(path: &Path) -> Result<()> {
    write_text(
        path,
        &format!(
            "import {{ __vuecRuntime }} from {}\nconst r = __vuecRuntime\nexport const ignoreSideEffectTags = r.ignoreSideEffectTags\n",
            js_string_literal("@vue/compiler-dom")
        ),
    )
}

fn write_vue3_dom_decode_html_browser_shim(path: &Path) -> Result<()> {
    write_text(
        path,
        &format!(
            "import {{ __vuecRuntime }} from {}\nconst r = __vuecRuntime\nexport const decodeHtmlBrowser = r.decodeHtmlBrowser\n",
            js_string_literal("@vue/compiler-dom")
        ),
    )
}

fn write_vue3_dom_html_nesting_shim(path: &Path) -> Result<()> {
    write_text(
        path,
        &format!(
            "import {{ __vuecRuntime }} from {}\nconst r = __vuecRuntime\nexport const isValidHTMLNesting = r.isValidHTMLNesting\n",
            js_string_literal("@vue/compiler-dom")
        ),
    )
}
