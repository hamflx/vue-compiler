//! Runtime smoke tests for generated Vue compiler output.
//!
//! This crate compiles representative Vue 2 and Vue 3 templates, executes the
//! generated render/SSR/hydration output against lock-provisioned official Vue
//! runtimes through Node/jsdom, and returns deterministic smoke evidence.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use vuec_vue2::{Vue2CompileOptions, Vue2CompiledResult};
use vuec_vue3_core::{CodegenResult, TemplateSource, Vue3CompilerOptions};
use vuec_vue3_dom::{apply_dom_parser_defaults, compile as compile_dom, DomCompilerOptions};
use vuec_vue3_ssr::{compile as compile_ssr, SsrCompileResult, SsrCompilerOptions};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Official Vue runtime version used by smoke execution.
pub enum RuntimeVersion {
    /// Vue 2.6 runtime.
    Vue2_6,
    /// Vue 2.7 runtime.
    Vue2_7,
    /// Vue 3 runtime.
    Vue3,
}

impl RuntimeVersion {
    /// Returns the lock-provisioned npm root for this runtime version.
    pub fn npm_root(self) -> PathBuf {
        let version = match self {
            Self::Vue2_6 => "vue2_6",
            Self::Vue2_7 => "vue2_7",
            Self::Vue3 => "vue3",
        };
        workspace_root()
            .join("target")
            .join("compat")
            .join("npm")
            .join(version)
            .join("node_modules")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Result of executing generated output against a Vue runtime.
pub struct RuntimeSmokeResult {
    /// Runtime smoke kind.
    pub kind: String,
    /// Rendered or hydrated HTML.
    pub html: String,
    /// Runtime warnings captured during execution.
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Representative runtime smoke fixture.
pub struct RuntimeSmokeFixture {
    /// Fixture name.
    pub name: &'static str,
    /// Template source.
    pub template: &'static str,
    /// Expected Vue 2 DOM-rendered HTML.
    pub vue2_dom_html: &'static str,
    /// Expected Vue 3 DOM-rendered HTML.
    pub vue3_dom_html: &'static str,
    /// Expected Vue 3 SSR HTML.
    pub vue3_ssr_html: &'static str,
    /// Expected Vue 3 hydrated DOM HTML.
    pub vue3_hydrated_html: &'static str,
}

/// Built-in runtime smoke fixtures.
pub const RUNTIME_SMOKE_FIXTURES: &[RuntimeSmokeFixture] = &[RuntimeSmokeFixture {
    name: "basic-interpolation",
    template: "<div>{{ msg }}</div>",
    vue2_dom_html: "<div>hello</div>",
    vue3_dom_html: r#"<div id="app" data-v-app=""><div>hello</div></div>"#,
    vue3_ssr_html: "<div>hello</div>",
    vue3_hydrated_html: r#"<div id="app"><div>hello</div></div>"#,
}];

/// Compiles a Vue 2 template and mounts it with the Vue 2.6 runtime.
pub fn compile_vue2_and_mount(template: &str) -> Result<RuntimeSmokeResult> {
    let compiled = vuec_vue2::compile(template, Vue2CompileOptions::default());
    mount_vue2(&compiled, RuntimeVersion::Vue2_6)
}

/// Compiles a Vue 3 template and mounts it with the Vue 3 runtime.
pub fn compile_vue3_and_mount(template: &str) -> Result<RuntimeSmokeResult> {
    let compiled = compile_vue3_dom_template(template);
    mount_vue3(&compiled, RuntimeVersion::Vue3)
}

/// Compiles a Vue 3 template for SSR and renders it with the Vue 3 runtime.
pub fn compile_vue3_and_render_ssr(template: &str) -> Result<RuntimeSmokeResult> {
    let compiled = compile_vue3_ssr_template(template);
    render_vue3_ssr(&compiled, RuntimeVersion::Vue3)
}

/// Renders a Vue 3 template with the official Vue 3 SSR compiler/runtime.
pub fn render_vue3_official_ssr(template: &str) -> Result<RuntimeSmokeResult> {
    run_node(RuntimeJob::Vue3OfficialSsr {
        root: RuntimeVersion::Vue3.npm_root(),
        template: template.into(),
    })
}

/// Compiles a Vue 3 DOM/SSR pair and hydrates the SSR output.
pub fn compile_vue3_and_hydrate(template: &str) -> Result<RuntimeSmokeResult> {
    let dom = compile_vue3_dom_template(template);
    let ssr = compile_vue3_ssr_template(template);
    hydrate_vue3(&dom, &ssr, RuntimeVersion::Vue3)
}

/// Mounts an already compiled Vue 2 render result.
pub fn mount_vue2(
    compiled: &Vue2CompiledResult,
    version: RuntimeVersion,
) -> Result<RuntimeSmokeResult> {
    run_node(RuntimeJob::Vue2Mount {
        root: version.npm_root(),
        render: compiled.render.clone(),
        static_render_fns: compiled.static_render_fns.clone(),
    })
}

/// Mounts an already compiled Vue 3 DOM render result.
pub fn mount_vue3(compiled: &CodegenResult, version: RuntimeVersion) -> Result<RuntimeSmokeResult> {
    run_node(RuntimeJob::Vue3Mount {
        root: version.npm_root(),
        code: compiled.code.clone(),
    })
}

/// Renders an already compiled Vue 3 SSR result.
pub fn render_vue3_ssr(
    compiled: &SsrCompileResult,
    version: RuntimeVersion,
) -> Result<RuntimeSmokeResult> {
    run_node(RuntimeJob::Vue3Ssr {
        root: version.npm_root(),
        code: compiled.code.clone(),
    })
}

/// Hydrates already compiled Vue 3 DOM and SSR results.
pub fn hydrate_vue3(
    dom: &CodegenResult,
    ssr: &SsrCompileResult,
    version: RuntimeVersion,
) -> Result<RuntimeSmokeResult> {
    run_node(RuntimeJob::Vue3Hydrate {
        root: version.npm_root(),
        dom_code: dom.code.clone(),
        ssr_code: ssr.code.clone(),
    })
}

fn compile_vue3_dom_template(template: &str) -> CodegenResult {
    let mut core = Vue3CompilerOptions {
        mode: "module".into(),
        prefix_identifiers: true,
        ..Vue3CompilerOptions::default()
    };
    apply_dom_parser_defaults(&mut core);
    compile_dom(
        template_source(template),
        DomCompilerOptions {
            core,
            ..DomCompilerOptions::default()
        },
    )
}

fn compile_vue3_ssr_template(template: &str) -> SsrCompileResult {
    let mut core = Vue3CompilerOptions {
        mode: "module".into(),
        prefix_identifiers: true,
        ..Vue3CompilerOptions::default()
    };
    apply_dom_parser_defaults(&mut core);
    compile_ssr(
        template_source(template),
        SsrCompilerOptions {
            core,
            mode_is_explicit: true,
            ..SsrCompilerOptions::default()
        },
    )
}

fn template_source(template: &str) -> TemplateSource {
    TemplateSource {
        filename: "runtime-smoke.vue".into(),
        source: template.into(),
        file_id: vuec_source::FileId(0),
        base_offset: 0,
    }
}

enum RuntimeJob {
    Vue2Mount {
        root: PathBuf,
        render: String,
        static_render_fns: Vec<String>,
    },
    Vue3Mount {
        root: PathBuf,
        code: String,
    },
    Vue3Ssr {
        root: PathBuf,
        code: String,
    },
    Vue3OfficialSsr {
        root: PathBuf,
        template: String,
    },
    Vue3Hydrate {
        root: PathBuf,
        dom_code: String,
        ssr_code: String,
    },
}

fn run_node(runtime: RuntimeJob) -> Result<RuntimeSmokeResult> {
    let (root, payload) = match runtime {
        RuntimeJob::Vue2Mount {
            root,
            render,
            static_render_fns,
        } => (
            root,
            json!({
                "kind": "vue2-mount",
                "render": render,
                "staticRenderFns": static_render_fns,
            }),
        ),
        RuntimeJob::Vue3Mount { root, code } => (
            root,
            json!({
                "kind": "vue3-mount",
                "code": code,
            }),
        ),
        RuntimeJob::Vue3Ssr { root, code } => (
            root,
            json!({
                "kind": "vue3-ssr",
                "code": code,
            }),
        ),
        RuntimeJob::Vue3OfficialSsr { root, template } => (
            root,
            json!({
                "kind": "vue3-official-ssr",
                "template": template,
            }),
        ),
        RuntimeJob::Vue3Hydrate {
            root,
            dom_code,
            ssr_code,
        } => (
            root,
            json!({
                "kind": "vue3-hydrate",
                "domCode": dom_code,
                "ssrCode": ssr_code,
            }),
        ),
    };
    ensure_runtime_root(&root)?;
    let script = runtime_script();
    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .env("VUEC_RUNTIME_ROOT", &root)
        .env("VUEC_RUNTIME_PAYLOAD", serde_json::to_string(&payload)?)
        .output()
        .context("failed to spawn node runtime smoke")?;
    if !output.status.success() {
        bail!(
            "node runtime smoke failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    serde_json::from_slice(&output.stdout).with_context(|| {
        format!(
            "failed to parse node runtime smoke output\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn ensure_runtime_root(root: &Path) -> Result<()> {
    if !root.join("vue").exists() {
        bail!(
            "missing Vue runtime dependencies at {}; run `cargo xtask sync-official-tests --locked` or any compat command that provisions npm fixtures",
            root.display()
        );
    }
    if !root.join("jsdom").exists() {
        bail!(
            "missing jsdom runtime dependency at {}; run `cargo xtask run-conformance --suite vue3-sfc` or another compat command that provisions runner dependencies",
            root.display()
        );
    }
    Ok(())
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn runtime_script() -> &'static str {
    r#"
const path = require('path');
const { createRequire } = require('module');
const root = process.env.VUEC_RUNTIME_ROOT;
const payload = JSON.parse(process.env.VUEC_RUNTIME_PAYLOAD || '{}');
const requireFromRoot = createRequire(path.join(root, 'package.json'));
const { JSDOM } = requireFromRoot('jsdom');

function installDom(html = '<div id="app"></div>') {
  const dom = new JSDOM(`<!doctype html><html><body>${html}</body></html>`);
  const win = dom.window;
  global.window = win;
  global.document = win.document;
  global.Node = win.Node;
  global.Element = win.Element;
  global.HTMLElement = win.HTMLElement;
  global.SVGElement = win.SVGElement || win.HTMLElement;
  global.navigator = win.navigator;
  return dom;
}

function warnings() {
  const out = [];
  const originalLog = console.log;
  const originalInfo = console.info;
  const originalWarn = console.warn;
  const originalError = console.error;
  console.log = (...args) => out.push(args.map(String).join(' '));
  console.info = (...args) => out.push(args.map(String).join(' '));
  console.warn = (...args) => out.push(args.map(String).join(' '));
  console.error = (...args) => out.push(args.map(String).join(' '));
  return {
    out,
    restore() {
      console.log = originalLog;
      console.info = originalInfo;
      console.warn = originalWarn;
      console.error = originalError;
    }
  };
}

function evaluateVue2Render(source) {
  return new Function(String(source));
}

function evaluateVue3Render(source) {
  const transformed = transformModuleSource(String(source));
  const Vue = requireFromRoot('vue');
  const factory = new Function('Vue', `const require = (id) => {
    if (id === 'vue') return Vue;
    throw new Error('unsupported require ' + id);
  };
${transformed}
return render;`);
  const render = factory(Vue);
  if (typeof render !== 'function') throw new Error('Vue 3 render did not evaluate to a function');
  return render;
}

function evaluateVue3Ssr(source) {
  const transformed = transformModuleSource(String(source));
  const Vue = requireFromRoot('vue');
  const serverRenderer = requireFromRoot('vue/server-renderer');
  const factory = new Function('Vue', 'serverRenderer', `const require = (id) => {
    if (id === 'vue') return Vue;
    if (id === 'vue/server-renderer') return serverRenderer;
    throw new Error('unsupported require ' + id);
  };
${transformed}
return ssrRender;`);
  const ssrRender = factory(Vue, serverRenderer);
  if (typeof ssrRender !== 'function') throw new Error('Vue 3 SSR render did not evaluate to a function');
  return ssrRender;
}

function compileOfficialVue3Ssr(template) {
  const compilerSsr = requireFromRoot('@vue/compiler-ssr');
  const compiled = compilerSsr.compile(String(template), { mode: 'function' });
  const ssrRender = new Function('require', compiled.code)(requireFromRoot);
  if (typeof ssrRender !== 'function') throw new Error('Official Vue 3 SSR render did not evaluate to a function');
  return ssrRender;
}

function transformModuleSource(source) {
  return source
    .replace(/import\s+\{([\s\S]*?)\}\s+from\s+["']vue["'];?\s*/g, (_, specifiers) => {
      return `const { ${specifiers.replace(/\s+as\s+/g, ': ')} } = Vue;\n`;
    })
    .replace(/import\s+\{([\s\S]*?)\}\s+from\s+["']vue\/server-renderer["'];?\s*/g, (_, specifiers) => {
      return `const { ${specifiers.replace(/\s+as\s+/g, ': ')} } = serverRenderer;\n`;
    })
    .replace(/export\s+function\s+(render|ssrRender)/g, 'function $1');
}

async function run() {
  if (payload.kind === 'vue2-mount') {
    installDom();
    const capture = warnings();
    try {
      const Vue = requireFromRoot('vue');
      Vue.config.productionTip = false;
      Vue.config.devtools = false;
      const app = document.getElementById('app');
      const vm = new Vue({
        data: () => ({ msg: 'hello', ok: true, count: 1, items: ['a', 'b'] }),
        render: evaluateVue2Render(payload.render),
        staticRenderFns: (payload.staticRenderFns || []).map(evaluateVue2Render),
      }).$mount(app);
      return { kind: payload.kind, html: vm.$el.outerHTML, warnings: capture.out };
    } finally {
      capture.restore();
    }
  }
  if (payload.kind === 'vue3-mount') {
    installDom();
    const capture = warnings();
    try {
      const Vue = requireFromRoot('vue');
      const app = document.getElementById('app');
      Vue.createApp({
        data: () => ({ msg: 'hello', ok: true, count: 1, items: ['a', 'b'] }),
        render: evaluateVue3Render(payload.code),
      }).mount(app);
      return { kind: payload.kind, html: app.outerHTML, warnings: capture.out };
    } finally {
      capture.restore();
    }
  }
  if (payload.kind === 'vue3-ssr') {
    const Vue = requireFromRoot('vue');
    const { renderToString } = requireFromRoot('vue/server-renderer');
    const html = await renderToString(Vue.createSSRApp({
      data: () => ({ msg: 'hello', ok: true, count: 1, items: ['a', 'b'] }),
      ssrRender: evaluateVue3Ssr(payload.code),
    }));
    return { kind: payload.kind, html, warnings: [] };
  }
  if (payload.kind === 'vue3-official-ssr') {
    const Vue = requireFromRoot('vue');
    const { renderToString } = requireFromRoot('vue/server-renderer');
    const html = await renderToString(Vue.createSSRApp({
      data: () => ({ msg: 'hello', ok: true, count: 1, items: ['a', 'b'] }),
      ssrRender: compileOfficialVue3Ssr(payload.template),
    }));
    return { kind: payload.kind, html, warnings: [] };
  }
  if (payload.kind === 'vue3-hydrate') {
    const Vue = requireFromRoot('vue');
    const { renderToString } = requireFromRoot('vue/server-renderer');
    const ssrHtml = await renderToString(Vue.createSSRApp({
      data: () => ({ msg: 'hello', ok: true, count: 1, items: ['a', 'b'] }),
      ssrRender: evaluateVue3Ssr(payload.ssrCode),
    }));
    installDom(`<div id="app">${ssrHtml}</div>`);
    const capture = warnings();
    try {
      Vue.createSSRApp({
        data: () => ({ msg: 'hello', ok: true, count: 1, items: ['a', 'b'] }),
        render: evaluateVue3Render(payload.domCode),
      }).mount(document.getElementById('app'));
      return { kind: payload.kind, html: document.getElementById('app').outerHTML, warnings: capture.out };
    } finally {
      capture.restore();
    }
  }
  throw new Error('unknown runtime smoke kind ' + payload.kind);
}

run()
  .then(result => process.stdout.write(JSON.stringify(result)))
  .catch(error => {
    console.error(error && error.stack || error);
    process.exit(1);
  });
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vue2_generated_render_mounts_to_dom() {
        for fixture in RUNTIME_SMOKE_FIXTURES {
            let result = compile_vue2_and_mount(fixture.template).expect(fixture.name);
            assert_eq!(result.kind, "vue2-mount");
            assert_eq!(result.html, fixture.vue2_dom_html);
            assert!(result.warnings.is_empty());
        }
    }

    #[test]
    fn vue27_generated_render_mounts_to_dom() {
        for fixture in RUNTIME_SMOKE_FIXTURES {
            let compiled = vuec_vue2::compile(fixture.template, Vue2CompileOptions::default());
            let result = mount_vue2(&compiled, RuntimeVersion::Vue2_7).expect(fixture.name);
            assert_eq!(result.kind, "vue2-mount");
            assert_eq!(result.html, fixture.vue2_dom_html);
            assert!(result.warnings.is_empty());
        }
    }

    #[test]
    fn vue3_generated_render_mounts_to_dom() {
        for fixture in RUNTIME_SMOKE_FIXTURES {
            let result = compile_vue3_and_mount(fixture.template).expect(fixture.name);
            assert_eq!(result.kind, "vue3-mount");
            assert_eq!(result.html, fixture.vue3_dom_html);
            assert!(result.warnings.is_empty());
        }
    }

    #[test]
    fn vue3_generated_ssr_renders_string() {
        for fixture in RUNTIME_SMOKE_FIXTURES {
            let result = compile_vue3_and_render_ssr(fixture.template).expect(fixture.name);
            assert_eq!(result.kind, "vue3-ssr");
            assert_eq!(result.html, fixture.vue3_ssr_html);
            assert!(result.warnings.is_empty());
        }
    }

    #[test]
    fn vue3_generated_ssr_matches_official_runtime_html() {
        for fixture in RUNTIME_SMOKE_FIXTURES {
            let rust = compile_vue3_and_render_ssr(fixture.template).expect("rust ssr");
            let official = render_vue3_official_ssr(fixture.template).expect("official ssr");
            assert_eq!(official.kind, "vue3-official-ssr");
            assert_eq!(official.html, fixture.vue3_ssr_html);
            assert_eq!(rust.html, official.html);
            assert!(rust.warnings.is_empty());
            assert!(official.warnings.is_empty());
        }
    }

    #[test]
    fn vue3_hydration_smoke_has_no_mismatch_warning() {
        for fixture in RUNTIME_SMOKE_FIXTURES {
            let result = compile_vue3_and_hydrate(fixture.template).expect(fixture.name);
            assert_eq!(result.kind, "vue3-hydrate");
            assert_eq!(result.html, fixture.vue3_hydrated_html);
            assert!(result
                .warnings
                .iter()
                .all(|warning| !warning.to_lowercase().contains("hydration")));
        }
    }
}
