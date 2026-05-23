#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fmt::{self, Display},
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
pub enum VersionLine {
    #[serde(rename = "vue2_6", alias = "vue26")]
    #[value(name = "vue2_6", alias = "vue26")]
    Vue26,
    #[serde(rename = "vue2_7", alias = "vue27")]
    #[value(name = "vue2_7", alias = "vue27")]
    Vue27,
    #[serde(rename = "vue3")]
    #[value(name = "vue3")]
    Vue3,
}

impl Display for VersionLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl VersionLine {
    pub const fn as_str(self) -> &'static str {
        match self {
            VersionLine::Vue26 => "vue2_6",
            VersionLine::Vue27 => "vue2_7",
            VersionLine::Vue3 => "vue3",
        }
    }
}

#[derive(Clone, Debug, Default, Args, Serialize)]
pub struct SelectionArgs {
    #[arg(long)]
    pub all: bool,

    #[arg(long)]
    pub official: bool,

    #[arg(long)]
    pub rust: bool,

    #[arg(long)]
    pub version_line: Option<VersionLine>,

    #[arg(long)]
    pub package: Option<String>,

    #[arg(long)]
    pub entry: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum ConformanceSuite {
    Vue2Compiler,
    Vue27Compiler,
    Vue27Sfc,
    Vue3Core,
    Vue3Dom,
    Vue3Sfc,
    Vue3Ssr,
}

#[derive(Clone, Debug, Args, Serialize)]
pub struct ConformanceArgs {
    #[arg(long)]
    pub suite: Option<ConformanceSuite>,

    #[arg(long)]
    pub all: bool,

    #[arg(long, default_value = "compat/official-revisions.lock")]
    pub lock: PathBuf,

    #[arg(long, default_value = "vendor")]
    pub vendor_dir: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportStatus {
    Pass,
    Pending,
    Fail,
}

impl ReportStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            ReportStatus::Pass => "pass",
            ReportStatus::Pending => "pending",
            ReportStatus::Fail => "fail",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ReportSummary {
    pub total: usize,
    pub pass: usize,
    pub pending: usize,
    pub fail: usize,
}

impl ReportSummary {
    pub fn from_items(items: &[ReportItem]) -> Self {
        let mut summary = ReportSummary {
            total: items.len(),
            pass: 0,
            pending: 0,
            fail: 0,
        };
        for item in items {
            match item.status {
                ReportStatus::Pass => summary.pass += 1,
                ReportStatus::Pending => summary.pending += 1,
                ReportStatus::Fail => summary.fail += 1,
            }
        }
        summary
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ReportItem {
    pub target: String,
    pub status: ReportStatus,
    pub detail: String,
    pub path: Option<String>,
}

impl ReportItem {
    pub fn new(
        target: impl Into<String>,
        status: ReportStatus,
        detail: impl Into<String>,
        path: Option<PathBuf>,
    ) -> Self {
        Self {
            target: target.into(),
            status,
            detail: detail.into(),
            path: path.map(|p| p.display().to_string()),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct JsonReport {
    pub command: String,
    pub status: String,
    pub metadata: ReportMetadata,
    pub scope: Option<SelectionArgsSnapshot>,
    pub summary: ReportSummary,
    pub items: Vec<ReportItem>,
    pub violations: Vec<String>,
    pub created: Vec<String>,
    pub note: Option<String>,
}

impl JsonReport {
    pub fn new(command: impl Into<String>, status: ReportStatus) -> Self {
        Self {
            command: command.into(),
            status: status.as_str().to_string(),
            metadata: ReportMetadata::capture(),
            scope: None,
            summary: ReportSummary {
                total: 0,
                pass: 0,
                pending: 0,
                fail: 0,
            },
            items: Vec::new(),
            violations: Vec::new(),
            created: Vec::new(),
            note: None,
        }
    }

    pub fn with_scope(mut self, scope: &SelectionArgs) -> Self {
        self.scope = Some(SelectionArgsSnapshot::from(scope));
        self
    }

    pub fn with_items(mut self, items: Vec<ReportItem>) -> Self {
        self.summary = ReportSummary::from_items(&items);
        self.status = aggregate_status(&items).as_str().to_string();
        self.items = items;
        self
    }

    pub fn with_violations(mut self, violations: Vec<String>) -> Self {
        if !violations.is_empty() {
            self.status = ReportStatus::Fail.as_str().to_string();
        }
        self.violations = violations;
        self
    }

    pub fn with_created(mut self, created: Vec<String>) -> Self {
        self.created = created;
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ReportMetadata {
    pub lock_hash: Option<String>,
    pub os: String,
    pub rustc: Option<String>,
    pub node: Option<String>,
    pub created_unix: u64,
}

impl ReportMetadata {
    fn capture() -> Self {
        Self {
            lock_hash: None,
            os: std::env::consts::OS.to_string(),
            rustc: command_output("rustc", &["--version"]),
            node: command_output("node", &["--version"]),
            created_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or_default(),
        }
    }

    fn with_lock_hash(mut self, lock_hash: Option<String>) -> Self {
        self.lock_hash = lock_hash;
        self
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SelectionArgsSnapshot {
    pub all: bool,
    pub official: bool,
    pub rust: bool,
    pub version_line: Option<VersionLine>,
    pub package: Option<String>,
    pub entry: Option<String>,
}

impl From<&SelectionArgs> for SelectionArgsSnapshot {
    fn from(value: &SelectionArgs) -> Self {
        Self {
            all: value.all,
            official: value.official,
            rust: value.rust,
            version_line: value.version_line,
            package: value.package.clone(),
            entry: value.entry.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TargetKind {
    Vue26Template,
    Vue27Template,
    Vue27Sfc,
    Vue3Core,
    Vue3Dom,
    Vue3Ssr,
    Vue3Sfc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ApiManifestSide {
    Official,
    Rust,
}

impl ApiManifestSide {
    const fn as_str(self) -> &'static str {
        match self {
            ApiManifestSide::Official => "official",
            ApiManifestSide::Rust => "rust",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TargetSpec {
    version_line: VersionLine,
    package: &'static str,
    entry: &'static str,
    kind: TargetKind,
}

impl TargetSpec {
    fn display(&self) -> String {
        format!(
            "{}::{}/{}",
            self.version_line.as_str(),
            self.package,
            self.entry
        )
    }

    fn output_modes(&self) -> &'static [&'static str] {
        match self.kind {
            TargetKind::Vue26Template => &[
                "schema-parity",
                "exact-js-output",
                "diagnostic-parity",
                "source-map-parity",
                "runtime-parity",
            ],
            TargetKind::Vue27Template => &[
                "schema-parity",
                "exact-js-output",
                "diagnostic-parity",
                "source-map-parity",
                "runtime-parity",
            ],
            TargetKind::Vue27Sfc => &[
                "schema-parity",
                "exact-js-output",
                "diagnostic-parity",
                "source-map-parity",
                "runtime-parity",
            ],
            TargetKind::Vue3Core
            | TargetKind::Vue3Dom
            | TargetKind::Vue3Ssr
            | TargetKind::Vue3Sfc => &[
                "schema-parity",
                "exact-js-output",
                "diagnostic-parity",
                "source-map-parity",
                "runtime-parity",
            ],
        }
    }

    fn relative_api_manifest_path(&self, side: &str) -> PathBuf {
        PathBuf::from("compat")
            .join("api")
            .join(side)
            .join(self.version_line.as_str())
            .join(sanitize_segment(self.package))
            .join(format!("{}.json", sanitize_segment(self.entry)))
    }

    fn relative_option_matrix_path(&self) -> PathBuf {
        PathBuf::from("compat")
            .join("options")
            .join(self.version_line.as_str())
            .join(sanitize_segment(self.package))
            .join(format!("{}.json", sanitize_segment(self.entry)))
    }

    fn relative_output_contract_path(&self) -> PathBuf {
        PathBuf::from("compat")
            .join("output")
            .join(self.version_line.as_str())
            .join(sanitize_segment(self.package))
            .join(format!("{}.json", sanitize_segment(self.entry)))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ManifestFile {
    schema_version: u8,
    version_line: VersionLine,
    package: String,
    entry: String,
    package_version: Option<String>,
    exports: Vec<String>,
    export_details: BTreeMap<String, ApiExportDetail>,
    require: ApiRequireRecord,
    types: ApiTypesRecord,
    status: String,
    source: String,
    lock_hash: Option<String>,
    official_revision: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
struct ApiExportDetail {
    kind: String,
    tag: String,
    name: Option<String>,
    function_arity: Option<u32>,
    is_async_function: Option<bool>,
    is_class_like: Option<bool>,
    own_property_names: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
struct ApiRequireRecord {
    request: String,
    success: bool,
    resolved: Option<String>,
    error_name: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
struct ApiTypesRecord {
    package_types: Option<String>,
    resolved: Option<String>,
    exists: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct ApiProbeOutput {
    package_version: Option<String>,
    exports: Vec<String>,
    #[serde(default)]
    export_details: BTreeMap<String, ApiExportDetail>,
    require: ApiRequireRecord,
    #[serde(default)]
    types: ApiTypesRecord,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct AllowedApiDiffFile {
    #[serde(default)]
    entries: Vec<AllowedApiDiffEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AllowedApiDiffEntry {
    version_line: VersionLine,
    package: String,
    entry: String,
    diff: String,
    reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct OptionMatrixFile {
    schema_version: u8,
    version_line: VersionLine,
    package: String,
    entry: String,
    status: String,
    rows: Vec<OptionMatrixRow>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct OptionMatrixRow {
    option_name: String,
    option_path: String,
    entry: String,
    version_line: VersionLine,
    accepted_types: Vec<String>,
    default_when_missing: String,
    behavior_when_undefined: String,
    behavior_when_null: String,
    side_effects: Vec<String>,
    diagnostics: Vec<String>,
    output_fields_affected: Vec<String>,
    official_fixture_ids: Vec<String>,
    variant: String,
    input_kind: String,
    method: String,
    fixture_id: String,
    fixture_source: String,
    option_value: Option<serde_json::Value>,
    execution_mode: String,
    status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct OptionProbeOutput {
    request: String,
    fixture_id: String,
    option_name: String,
    option_path: String,
    method: String,
    side: String,
    ok: bool,
    value: Option<serde_json::Value>,
    error: Option<OptionProbeError>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct OptionProbeError {
    name: Option<String>,
    code: Option<String>,
    message: String,
}

#[derive(Clone, Debug)]
struct OptionMatrixCase {
    option_name: &'static str,
    option_path: &'static str,
    accepted_types: &'static [&'static str],
    default_when_missing: &'static str,
    behavior_when_undefined: &'static str,
    behavior_when_null: &'static str,
    side_effects: &'static [&'static str],
    diagnostics: &'static [&'static str],
    output_fields_affected: &'static [&'static str],
    official_fixture_ids: &'static [&'static str],
    variant: &'static str,
    method: &'static str,
    fixture_id: &'static str,
    fixture_source: &'static str,
    option_value: Option<serde_json::Value>,
    execution_mode: &'static str,
    pending: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct OutputContractFile {
    version_line: VersionLine,
    package: String,
    entry: String,
    required_modes: Vec<String>,
    status: String,
}

fn all_targets() -> &'static [TargetSpec] {
    &[
        TargetSpec {
            version_line: VersionLine::Vue26,
            package: "vue-template-compiler",
            entry: "index",
            kind: TargetKind::Vue26Template,
        },
        TargetSpec {
            version_line: VersionLine::Vue27,
            package: "vue-template-compiler",
            entry: "index",
            kind: TargetKind::Vue27Template,
        },
        TargetSpec {
            version_line: VersionLine::Vue27,
            package: "vue",
            entry: "vue/compiler-sfc",
            kind: TargetKind::Vue27Sfc,
        },
        TargetSpec {
            version_line: VersionLine::Vue3,
            package: "@vue/compiler-core",
            entry: "index",
            kind: TargetKind::Vue3Core,
        },
        TargetSpec {
            version_line: VersionLine::Vue3,
            package: "@vue/compiler-dom",
            entry: "index",
            kind: TargetKind::Vue3Dom,
        },
        TargetSpec {
            version_line: VersionLine::Vue3,
            package: "@vue/compiler-ssr",
            entry: "index",
            kind: TargetKind::Vue3Ssr,
        },
        TargetSpec {
            version_line: VersionLine::Vue3,
            package: "@vue/compiler-sfc",
            entry: "index",
            kind: TargetKind::Vue3Sfc,
        },
    ]
}

fn option_matrix_cases(target: TargetSpec) -> Vec<OptionMatrixCase> {
    match target.kind {
        TargetKind::Vue26Template => vec![
            option_case(
                "warn",
                "warn",
                &["boolean"],
                "true",
                "false",
                "true",
                &["emits diagnostics"],
                &["warning-order"],
                &["errors", "tips", "diagnostics"],
                &["vue2-compiler-warning"],
                "base",
                "compile",
                "vue2-compiler-warning",
                r#"<script></script>"#,
                Some(serde_json::json!({"warn": true})),
                false,
            ),
            option_case(
                "outputSourceRange",
                "outputSourceRange",
                &["boolean"],
                "false",
                "false",
                "true",
                &["adds source spans"],
                &["codeframe"],
                &["errors", "render", "staticRenderFns"],
                &["vue2-output-source-range"],
                "base",
                "compile",
                "vue2-output-source-range",
                r#"<div><span></div>"#,
                Some(serde_json::json!({"outputSourceRange": true})),
                false,
            ),
            option_case(
                "comments",
                "comments",
                &["boolean"],
                "false",
                "false",
                "true",
                &["preserves comment nodes"],
                &["comment-order"],
                &["render"],
                &["vue2-comments"],
                "base",
                "compile",
                "vue2-comments",
                r#"<div><!--x-->{{ msg }}</div>"#,
                Some(serde_json::json!({"comments": true})),
                false,
            ),
            option_case(
                "delimiters",
                "delimiters",
                &["array"],
                "default",
                "undefined",
                "undefined",
                &["changes interpolation parser"],
                &["interpolation"],
                &["render"],
                &["vue2-delimiters"],
                "base",
                "compile",
                "vue2-delimiters",
                r#"<div>[[ msg ]]</div>"#,
                Some(serde_json::json!({"delimiters": ["[[", "]]"]})),
                false,
            ),
            option_case(
                "whitespace",
                "whitespace",
                &["string"],
                "preserve",
                "undefined",
                "undefined",
                &["condenses whitespace"],
                &["whitespace"],
                &["render"],
                &["vue2-whitespace"],
                "base",
                "compile",
                "vue2-whitespace",
                r#"<div> a  b </div>"#,
                Some(serde_json::json!({"whitespace": "condense"})),
                false,
            ),
            option_case(
                "preserveWhitespace",
                "preserveWhitespace",
                &["boolean"],
                "true",
                "false",
                "true",
                &["keeps whitespace text nodes"],
                &["whitespace"],
                &["render"],
                &["vue2-preserve-whitespace"],
                "base",
                "compile",
                "vue2-preserve-whitespace",
                r#"<div> a </div>"#,
                Some(serde_json::json!({"preserveWhitespace": false})),
                false,
            ),
            option_case(
                "shouldDecodeNewlines",
                "shouldDecodeNewlines",
                &["boolean"],
                "false",
                "false",
                "true",
                &["decodes newline entities in attributes"],
                &["attribute decode"],
                &["render"],
                &["vue2-decode-newlines"],
                "base",
                "compile",
                "vue2-decode-newlines",
                r#"<a href="a&#10;b"></a>"#,
                Some(serde_json::json!({"shouldDecodeNewlines": true})),
                false,
            ),
            option_case(
                "shouldDecodeNewlinesForHref",
                "shouldDecodeNewlinesForHref",
                &["boolean"],
                "false",
                "false",
                "true",
                &["decodes href newline entities"],
                &["attribute decode"],
                &["render"],
                &["vue2-decode-newlines-href"],
                "base",
                "compile",
                "vue2-decode-newlines-href",
                r#"<a href="a&#10;b"></a>"#,
                Some(serde_json::json!({"shouldDecodeNewlinesForHref": true})),
                false,
            ),
            option_case(
                "modules",
                "modules",
                &["array"],
                "[]",
                "undefined",
                "undefined",
                &["module hooks"],
                &["modules"],
                &["render"],
                &["vue2-modules"],
                "base",
                "compile",
                "vue2-modules",
                r#"<div class="a"></div>"#,
                Some(serde_json::json!({"modules": ["class"]})),
                false,
            ),
            option_case(
                "directives",
                "directives",
                &["object"],
                "{}",
                "undefined",
                "undefined",
                &["custom directives"],
                &["directives"],
                &["render"],
                &["vue2-directives"],
                "base",
                "compile",
                "vue2-directives",
                r#"<div v-focus></div>"#,
                Some(serde_json::json!({"directives": {"focus": true}})),
                false,
            ),
        ],
        TargetKind::Vue27Template => vue27_template_cases(target),
        TargetKind::Vue27Sfc => vue27_sfc_cases(target),
        TargetKind::Vue3Core => vue3_core_cases(target),
        TargetKind::Vue3Dom => vue3_dom_cases(target),
        TargetKind::Vue3Ssr => vue3_ssr_cases(target),
        TargetKind::Vue3Sfc => vue3_sfc_cases(target),
    }
}

fn option_case(
    option_name: &'static str,
    option_path: &'static str,
    accepted_types: &'static [&'static str],
    default_when_missing: &'static str,
    behavior_when_undefined: &'static str,
    behavior_when_null: &'static str,
    side_effects: &'static [&'static str],
    diagnostics: &'static [&'static str],
    output_fields_affected: &'static [&'static str],
    official_fixture_ids: &'static [&'static str],
    variant: &'static str,
    method: &'static str,
    fixture_id: &'static str,
    fixture_source: &'static str,
    option_value: Option<serde_json::Value>,
    pending: bool,
) -> OptionMatrixCase {
    OptionMatrixCase {
        option_name,
        option_path,
        accepted_types,
        default_when_missing,
        behavior_when_undefined,
        behavior_when_null,
        side_effects,
        diagnostics,
        output_fields_affected,
        official_fixture_ids,
        variant,
        method,
        fixture_id,
        fixture_source,
        option_value,
        execution_mode: if pending { "pending" } else { "diff" },
        pending,
    }
}

fn vue27_template_cases(_target: TargetSpec) -> Vec<OptionMatrixCase> {
    vec![
        option_case(
            "warn",
            "warn",
            &["boolean"],
            "true",
            "false",
            "true",
            &["emits diagnostics"],
            &["warning-order"],
            &["errors", "tips", "diagnostics"],
            &["vue27-warning"],
            "base",
            "compile",
            "vue27-warning",
            r#"<script></script>"#,
            Some(serde_json::json!({"warn": true})),
            false,
        ),
        option_case(
            "modules",
            "modules",
            &["array"],
            "[]",
            "undefined",
            "undefined",
            &["module hooks"],
            &["modules"],
            &["render"],
            &["vue27-modules"],
            "base",
            "compile",
            "vue27-modules",
            r#"<div class="a"></div>"#,
            Some(serde_json::json!({"modules": ["class"]})),
            false,
        ),
        option_case(
            "directives",
            "directives",
            &["object"],
            "{}",
            "undefined",
            "undefined",
            &["custom directives"],
            &["directives"],
            &["render"],
            &["vue27-directives"],
            "base",
            "compile",
            "vue27-directives",
            r#"<div v-focus></div>"#,
            Some(serde_json::json!({"directives": {"focus": true}})),
            false,
        ),
    ]
}

fn vue27_sfc_cases(_target: TargetSpec) -> Vec<OptionMatrixCase> {
    vec![
        option_case(
            "parse",
            "parse",
            &["object"],
            "{}",
            "undefined",
            "undefined",
            &["descriptor parse"],
            &["descriptor"],
            &["template", "script", "styles"],
            &["vue27-sfc-parse"],
            "base",
            "parse",
            "vue27-sfc-parse",
            r#"<template><div>{{ msg }}</div></template><script>export default {}</script><style scoped>.a{ color: v-bind(color); }</style>"#,
            Some(serde_json::json!({"filename": "contract.vue"})),
            false,
        ),
        option_case(
            "compileTemplate",
            "compileTemplate",
            &["object"],
            "{}",
            "undefined",
            "undefined",
            &["template codegen"],
            &["render"],
            &["code", "map", "errors"],
            &["vue27-sfc-template"],
            "base",
            "compileTemplate",
            "vue27-sfc-template",
            r#"<template><div>{{ msg }}</div></template><script>export default {}</script><style scoped>.a{ color: v-bind(color); }</style>"#,
            Some(
                serde_json::json!({"id": "data-v-contract", "scopeId": "data-v-contract", "ssr": false, "slotted": false}),
            ),
            false,
        ),
        option_case(
            "compileScript",
            "compileScript",
            &["object"],
            "{}",
            "undefined",
            "undefined",
            &["script setup analysis"],
            &["bindings"],
            &["content", "bindings", "errors"],
            &["vue27-sfc-script"],
            "base",
            "compileScript",
            "vue27-sfc-script",
            r#"<template><div>{{ msg }}</div></template><script setup lang="ts">const msg = 'x'</script>"#,
            Some(serde_json::json!({"id": "data-v-contract"})),
            false,
        ),
        option_case(
            "compileStyle",
            "compileStyle",
            &["object"],
            "{}",
            "undefined",
            "undefined",
            &["style rewrite"],
            &["source map"],
            &["code", "map", "errors"],
            &["vue27-sfc-style"],
            "base",
            "compileStyle",
            "vue27-sfc-style",
            r#"<style scoped>.a{ color: v-bind(color); }</style>"#,
            Some(serde_json::json!({"id": "data-v-contract", "scoped": true, "vars": ["color"]})),
            false,
        ),
    ]
}

fn vue3_core_cases(_target: TargetSpec) -> Vec<OptionMatrixCase> {
    vec![
        option_case(
            "prefixIdentifiers",
            "prefixIdentifiers",
            &["boolean"],
            "false",
            "false",
            "true",
            &["prefixes identifiers in render code"],
            &["codegen"],
            &["code"],
            &["vue3-core-prefix"],
            "base",
            "baseCompile",
            "vue3-core-prefix",
            r#"<div>{{ msg }}</div>"#,
            Some(serde_json::json!({"prefixIdentifiers": true})),
            false,
        ),
        option_case(
            "mode",
            "mode",
            &["string"],
            "module",
            "module",
            "module",
            &["changes codegen wrapper"],
            &["codegen"],
            &["code"],
            &["vue3-core-mode"],
            "base",
            "baseCompile",
            "vue3-core-mode",
            r#"<div>{{ msg }}</div>"#,
            Some(serde_json::json!({"mode": "function"})),
            false,
        ),
        option_case(
            "hoistStatic",
            "hoistStatic",
            &["boolean"],
            "false",
            "false",
            "true",
            &["hoists static nodes"],
            &["ast"],
            &["code:contains:_cache[0]"],
            &["vue3-core-hoist"],
            "base",
            "baseCompile",
            "vue3-core-hoist",
            r#"<div><span>static</span></div>"#,
            Some(serde_json::json!({"hoistStatic": true})),
            false,
        ),
        option_case(
            "cacheHandlers",
            "cacheHandlers",
            &["boolean"],
            "false",
            "false",
            "true",
            &["caches event handlers"],
            &["codegen"],
            &["code"],
            &["vue3-core-cache"],
            "base",
            "baseCompile",
            "vue3-core-cache",
            r#"<button @click="save"></button>"#,
            Some(serde_json::json!({"cacheHandlers": true})),
            false,
        ),
        option_case(
            "scopeId",
            "scopeId",
            &["string"],
            "none",
            "undefined",
            "undefined",
            &["scopes generated code"],
            &["codegen"],
            &["code"],
            &["vue3-core-scope"],
            "base",
            "baseCompile",
            "vue3-core-scope",
            r#"<div class="a"></div>"#,
            Some(serde_json::json!({"scopeId": "data-v-x"})),
            false,
        ),
        option_case(
            "slotted",
            "slotted",
            &["boolean"],
            "false",
            "false",
            "true",
            &["marks slotted output"],
            &["codegen"],
            &["code:contains:_renderSlot"],
            &["vue3-core-slotted"],
            "base",
            "baseCompile",
            "vue3-core-slotted",
            r#"<slot></slot>"#,
            Some(serde_json::json!({"slotted": true})),
            false,
        ),
        option_case(
            "isTS",
            "isTS",
            &["boolean"],
            "false",
            "false",
            "true",
            &["parses TS expressions"],
            &["parser"],
            &["code:contains:foo as string"],
            &["vue3-core-ts"],
            "base",
            "baseCompile",
            "vue3-core-ts",
            r#"<div>{{ foo as string }}</div>"#,
            Some(serde_json::json!({"isTS": true})),
            false,
        ),
        option_case(
            "expressionPlugins",
            "expressionPlugins",
            &["array"],
            "[]",
            "undefined",
            "undefined",
            &["enables expression plugins"],
            &["parser"],
            &["code:contains:foo?.bar"],
            &["vue3-core-expression-plugins"],
            "base",
            "baseCompile",
            "vue3-core-expression-plugins",
            r#"<div>{{ foo?.bar }}</div>"#,
            Some(serde_json::json!({"expressionPlugins": ["typescript"]})),
            false,
        ),
    ]
}

fn vue3_dom_cases(_target: TargetSpec) -> Vec<OptionMatrixCase> {
    vec![
        option_case(
            "prefixIdentifiers",
            "core.prefixIdentifiers",
            &["boolean"],
            "false",
            "false",
            "true",
            &["dom codegen prefixing"],
            &["codegen"],
            &["code"],
            &["vue3-dom-prefix"],
            "dom",
            "compile",
            "vue3-dom-prefix",
            r#"<div>{{ msg }}</div>"#,
            Some(serde_json::json!({"prefixIdentifiers": true})),
            false,
        ),
        option_case(
            "transformAssetUrls",
            "transformAssetUrls",
            &["boolean"],
            "true",
            "true",
            "false",
            &["asset URL transform"],
            &["asset URLs"],
            &["code"],
            &["vue3-dom-asset"],
            "dom",
            "compile",
            "vue3-dom-asset",
            r#"<img src="./a.png">"#,
            Some(serde_json::json!({"transformAssetUrls": true})),
            false,
        ),
        option_case(
            "decodeEntities",
            "decodeEntities",
            &["boolean"],
            "true",
            "true",
            "false",
            &["decodes entities"],
            &["entity decode"],
            &["children.0.children.0.content"],
            &["vue3-dom-entity"],
            "dom",
            "parse",
            "vue3-dom-entity",
            r#"<div>&amp;</div>"#,
            Some(serde_json::json!({"decodeEntities": true})),
            false,
        ),
        option_case(
            "isCustomElement",
            "isCustomElement",
            &["array"],
            "[]",
            "undefined",
            "undefined",
            &["marks custom element"],
            &["custom element"],
            &["ast"],
            &["vue3-dom-custom"],
            "dom",
            "parse",
            "vue3-dom-custom",
            r#"<custom-el></custom-el>"#,
            Some(serde_json::json!({"isCustomElement": ["custom-el"]})),
            false,
        ),
    ]
}

fn vue3_ssr_cases(_target: TargetSpec) -> Vec<OptionMatrixCase> {
    vec![
        option_case(
            "scopeId",
            "scopeId",
            &["string"],
            "none",
            "undefined",
            "undefined",
            &["adds scope attributes"],
            &["ssr codegen"],
            &["code:contains:data-v-x"],
            &["vue3-ssr-scope"],
            "ssr",
            "compile",
            "vue3-ssr-scope",
            r#"<div class="a"></div>"#,
            Some(serde_json::json!({"scopeId": "data-v-x"})),
            false,
        ),
        option_case(
            "slotted",
            "slotted",
            &["boolean"],
            "false",
            "false",
            "true",
            &["adds slotted marker"],
            &["ssr codegen"],
            &["code:contains:ssrRenderSlot"],
            &["vue3-ssr-slotted"],
            "ssr",
            "compile",
            "vue3-ssr-slotted",
            r#"<slot></slot>"#,
            Some(serde_json::json!({"slotted": true})),
            false,
        ),
    ]
}

fn vue3_sfc_cases(_target: TargetSpec) -> Vec<OptionMatrixCase> {
    vec![
        option_case(
            "parse",
            "parse",
            &["object"],
            "{}",
            "undefined",
            "undefined",
            &["descriptor parse"],
            &["descriptor"],
            &[
                "descriptor.template.content",
                "descriptor.scriptSetup.content",
                "descriptor.styles.0.content",
                "descriptor.styles.0.scoped",
            ],
            &["vue3-sfc-parse"],
            "base",
            "parse",
            "vue3-sfc-parse",
            r#"<template><div>{{ msg }}</div></template><script setup lang="ts">const msg = 'x'</script><style scoped>.a{ color: v-bind(color); }</style>"#,
            Some(serde_json::json!({"filename": "contract.vue"})),
            false,
        ),
        option_case(
            "compileTemplate",
            "compileTemplate",
            &["object"],
            "{}",
            "undefined",
            "undefined",
            &["template codegen"],
            &["code", "map"],
            &["code", "map", "errors"],
            &["vue3-sfc-template"],
            "base",
            "compileTemplate",
            "vue3-sfc-template",
            r#"<template><div>{{ msg }}</div></template><script setup lang="ts">const msg = 'x'</script><style scoped>.a{ color: v-bind(color); }</style>"#,
            Some(
                serde_json::json!({"id": "data-v-contract", "scopeId": "data-v-contract", "ssr": false, "slotted": false}),
            ),
            false,
        ),
        option_case(
            "compileScript",
            "compileScript",
            &["object"],
            "{}",
            "undefined",
            "undefined",
            &["script setup analysis"],
            &["bindings"],
            &["content", "bindings"],
            &["vue3-sfc-script"],
            "base",
            "compileScript",
            "vue3-sfc-script",
            r#"<template><div>{{ msg }}</div></template><script setup lang="ts">const msg = 'x'</script>"#,
            Some(
                serde_json::json!({"id": "data-v-contract", "inlineTemplate": false, "refSugar": false}),
            ),
            false,
        ),
        option_case(
            "compileStyle",
            "compileStyle",
            &["object"],
            "{}",
            "undefined",
            "undefined",
            &["style rewrite"],
            &["source map"],
            &["code", "map", "errors"],
            &["vue3-sfc-style"],
            "base",
            "compileStyle",
            "vue3-sfc-style",
            r#"<style scoped>.a{ color: v-bind(color); }</style>"#,
            Some(serde_json::json!({"id": "data-v-contract", "scoped": true, "vars": ["color"]})),
            false,
        ),
    ]
}

pub fn verify_official_lock(path: &Path) -> JsonReport {
    let lock_hash = file_sha256(path).ok();
    match load_official_lock(path) {
        Ok(lock) => {
            let violations = validate_official_lock(&lock);
            let status = if violations.is_empty() {
                ReportStatus::Pass
            } else {
                ReportStatus::Fail
            };
            let mut report = JsonReport::new("verify_official_lock", status);
            report.metadata = report.metadata.with_lock_hash(lock_hash);
            report
                .with_violations(violations)
                .with_note(format!("lock: {}", path.display()))
        }
        Err(err) => {
            let mut report = JsonReport::new("verify_official_lock", ReportStatus::Fail);
            report.metadata = report.metadata.with_lock_hash(lock_hash);
            report
                .with_violations(vec![format!("failed to read/parse lock file: {err}")])
                .with_note(format!("lock: {}", path.display()))
        }
    }
}

pub fn sync_official_tests(path: &Path, locked: bool, out_dir: &Path) -> JsonReport {
    let lock_hash = file_sha256(path).ok();
    match load_official_lock(path) {
        Ok(lock) => {
            let mut created = Vec::new();
            let mut items = Vec::new();
            for (version_line, baseline) in [
                (VersionLine::Vue26, &lock.vue2_6),
                (VersionLine::Vue27, &lock.vue2_7),
                (VersionLine::Vue3, &lock.vue3),
            ] {
                let dir = out_dir.join(version_line.as_str());
                if let Err(err) = sync_git_checkout(&baseline.repo, &baseline.rev, &dir) {
                    return JsonReport::new("sync_official_tests", ReportStatus::Fail)
                        .with_violations(vec![format!(
                            "failed to sync {} into {}: {err}",
                            baseline.repo,
                            dir.display()
                        )])
                        .with_note(format!("lock: {}", path.display()));
                }
                let metadata_path = dir.join("official-revision.json");
                let metadata = serde_json::json!({
                    "version_line": version_line,
                    "repo": baseline.repo,
                    "rev": baseline.rev,
                    "npm": baseline.npm,
                    "exports": baseline.exports,
                    "lock_hash": lock_hash,
                    "locked": locked,
                });
                if let Err(err) = write_json(&metadata_path, &metadata) {
                    return JsonReport::new("sync_official_tests", ReportStatus::Fail)
                        .with_violations(vec![format!(
                            "failed to write {}: {err}",
                            metadata_path.display()
                        )])
                        .with_note(format!("lock: {}", path.display()));
                }
                created.push(metadata_path.display().to_string());
                items.push(ReportItem::new(
                    version_line.as_str(),
                    ReportStatus::Pass,
                    format!("synced {} at {}", baseline.repo, baseline.rev),
                    Some(metadata_path),
                ));
            }
            let mut report = JsonReport::new("sync_official_tests", ReportStatus::Pass);
            report.metadata = report.metadata.with_lock_hash(lock_hash);
            report
                .with_scope(&SelectionArgs {
                    all: true,
                    official: true,
                    rust: false,
                    version_line: None,
                    package: None,
                    entry: None,
                })
                .with_items(items)
                .with_created(created)
                .with_note(format!("locked={locked}, lock={}", path.display()))
        }
        Err(err) => {
            let mut report = JsonReport::new("sync_official_tests", ReportStatus::Fail);
            report.metadata = report.metadata.with_lock_hash(lock_hash);
            report
                .with_violations(vec![format!("failed to read/parse lock file: {err}")])
                .with_note(format!("lock: {}", path.display()))
        }
    }
}

pub fn export_api(scope: &SelectionArgs) -> JsonReport {
    let targets = select_targets(scope);
    let mut items = Vec::new();
    let mut created = Vec::new();
    let sides = selected_api_manifest_sides(scope);
    let lock_path = PathBuf::from("compat/official-revisions.lock");
    let lock_hash = file_sha256(&lock_path).ok();
    let lock = load_official_lock(&lock_path).ok();

    if sides.contains(&ApiManifestSide::Rust) {
        if let Err(err) = generate_rust_alias_packages(&targets) {
            let mut report = JsonReport::new("export_api", ReportStatus::Fail);
            report.metadata = report.metadata.with_lock_hash(lock_hash);
            return report.with_scope(scope).with_violations(vec![format!(
                "failed to generate Rust alias packages: {err:#}"
            )]);
        }
    }

    for target in targets {
        for side in &sides {
            let path = target.relative_api_manifest_path(side.as_str());
            let manifest_result = match side {
                ApiManifestSide::Official => {
                    export_official_api_manifest(target, lock.as_ref(), lock_hash.clone())
                }
                ApiManifestSide::Rust => export_rust_api_manifest(target, lock_hash.clone()),
            };

            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            match manifest_result {
                Ok(manifest) => {
                    let status = manifest_status(&manifest);
                    let detail = if manifest.require.success {
                        format!(
                            "{} exports captured from {}",
                            manifest.exports.len(),
                            manifest
                                .require
                                .resolved
                                .as_deref()
                                .unwrap_or(manifest.require.request.as_str())
                        )
                    } else {
                        format!(
                            "require failed: {}",
                            manifest
                                .require
                                .error_message
                                .as_deref()
                                .unwrap_or("unknown error")
                        )
                    };
                    if let Err(err) = write_json(&path, &manifest) {
                        items.push(ReportItem::new(
                            format!("{}::{}", side.as_str(), target.display()),
                            ReportStatus::Fail,
                            format!("failed to write manifest: {err}"),
                            Some(path),
                        ));
                        continue;
                    }
                    created.push(path.display().to_string());
                    items.push(ReportItem::new(
                        format!("{}::{}", side.as_str(), target.display()),
                        status,
                        detail,
                        Some(path),
                    ));
                }
                Err(err) => {
                    let manifest = failed_api_manifest(
                        target,
                        *side,
                        lock_hash.clone(),
                        lock.as_ref()
                            .and_then(|lock| baseline_for(lock, target.version_line))
                            .map(|baseline| baseline.rev.clone()),
                        format!("{err:#}"),
                    );
                    let status = manifest_status(&manifest);
                    let _ = write_json(&path, &manifest);
                    created.push(path.display().to_string());
                    items.push(ReportItem::new(
                        format!("{}::{}", side.as_str(), target.display()),
                        status,
                        format!("failed to export API manifest: {err:#}"),
                        Some(path),
                    ));
                }
            }
        }
    }
    let mut report = JsonReport::new("export_api", ReportStatus::Pending);
    report.metadata = report.metadata.with_lock_hash(lock_hash);
    report
        .with_scope(scope)
        .with_items(items)
        .with_created(created)
        .with_note("API manifest generation now probes real packages; Rust manifests require alias packages to exist")
}

fn selected_api_manifest_sides(scope: &SelectionArgs) -> Vec<ApiManifestSide> {
    match (scope.official, scope.rust) {
        (true, false) => vec![ApiManifestSide::Official],
        (false, true) => vec![ApiManifestSide::Rust],
        _ => vec![ApiManifestSide::Official, ApiManifestSide::Rust],
    }
}

pub fn diff_api(scope: &SelectionArgs) -> JsonReport {
    let targets = select_targets(scope);
    let mut items = Vec::new();
    let mut violations = Vec::new();
    let lock_hash = file_sha256(&PathBuf::from("compat/official-revisions.lock")).ok();
    let allowed = load_allowed_api_diffs(&PathBuf::from("compat/api/allowed-diff.json"));
    for target in targets {
        let official_path = target.relative_api_manifest_path(ApiManifestSide::Official.as_str());
        let rust_path = target.relative_api_manifest_path(ApiManifestSide::Rust.as_str());
        match (
            read_json::<ManifestFile>(&official_path),
            read_json::<ManifestFile>(&rust_path),
        ) {
            (Ok(official), Ok(rust)) => {
                let mut diffs = compare_api_manifests(&official, &rust);
                diffs.retain(|diff| !is_allowed_api_diff(&allowed, target, diff));
                if diffs.is_empty() {
                    items.push(ReportItem::new(
                        target.display(),
                        ReportStatus::Pass,
                        "official and Rust API manifests match",
                        Some(rust_path),
                    ));
                } else {
                    violations.extend(
                        diffs
                            .iter()
                            .map(|diff| format!("{}: {diff}", target.display())),
                    );
                    items.push(ReportItem::new(
                        target.display(),
                        ReportStatus::Fail,
                        format!("{} API manifest differences", diffs.len()),
                        Some(rust_path),
                    ));
                }
            }
            (Err(err), _) => {
                violations.push(format!(
                    "{} official manifest missing/invalid: {err}",
                    target.display()
                ));
                items.push(ReportItem::new(
                    target.display(),
                    ReportStatus::Fail,
                    "official API manifest missing or invalid",
                    Some(official_path),
                ));
            }
            (_, Err(err)) => {
                violations.push(format!(
                    "{} Rust manifest missing/invalid: {err}",
                    target.display()
                ));
                items.push(ReportItem::new(
                    target.display(),
                    ReportStatus::Fail,
                    "Rust API manifest missing or invalid",
                    Some(rust_path),
                ));
            }
        }
    }
    let mut report = JsonReport::new("diff_api", ReportStatus::Pending);
    report.metadata = report.metadata.with_lock_hash(lock_hash);
    report
        .with_scope(scope)
        .with_items(items)
        .with_violations(violations)
        .with_note("diff compares generated official and Rust alias manifests field-by-field")
}

fn export_official_api_manifest(
    target: TargetSpec,
    lock: Option<&OfficialRevisionsLock>,
    lock_hash: Option<String>,
) -> Result<ManifestFile> {
    let lock = lock.context("compat/official-revisions.lock is missing or invalid")?;
    let baseline = baseline_for(lock, target.version_line)
        .context("target version line is missing from official lock")?;
    let install_root = ensure_official_npm_install(target.version_line, baseline)?;
    let request = api_require_request(target);
    let probe = probe_api_exports(&install_root, target.package, &request)?;
    Ok(manifest_from_probe(
        target,
        ApiManifestSide::Official,
        lock_hash,
        Some(baseline.rev.clone()),
        probe,
    ))
}

fn export_rust_api_manifest(target: TargetSpec, lock_hash: Option<String>) -> Result<ManifestFile> {
    let alias_root = PathBuf::from("target")
        .join("compat")
        .join("rust-alias")
        .join(target.version_line.as_str());
    let request = api_require_request(target);
    let probe = probe_api_exports(&alias_root, target.package, &request)?;
    Ok(manifest_from_probe(
        target,
        ApiManifestSide::Rust,
        lock_hash,
        None,
        probe,
    ))
}

fn generate_rust_alias_packages(targets: &[TargetSpec]) -> Result<Vec<PathBuf>> {
    ensure_node_bridge_binary()?;
    let mut created = Vec::new();
    for target in targets {
        let official_manifest_path =
            target.relative_api_manifest_path(ApiManifestSide::Official.as_str());
        let manifest = read_json::<ManifestFile>(&official_manifest_path).with_context(|| {
            format!(
                "official API manifest {} is required before Rust alias generation; run `cargo xtask export-api --official --all`",
                official_manifest_path.display()
            )
        })?;
        let root = rust_alias_root(target.version_line);
        let package_dir = rust_alias_package_dir(*target);
        fs::create_dir_all(&package_dir)
            .with_context(|| format!("failed to create {}", package_dir.display()))?;
        write_alias_package_json(&package_dir, *target, &manifest)?;
        write_alias_index(&root, &package_dir, *target, &manifest)?;
        write_alias_types(&package_dir, *target, &manifest)?;
        created.push(package_dir);
    }
    Ok(created)
}

fn ensure_node_bridge_binary() -> Result<PathBuf> {
    run_command("cargo", &["build", "-p", "vuec_node_bridge"], None)
        .context("failed to build vuec_node_bridge")?;
    let exe_name = if cfg!(windows) {
        "vuec_node_bridge.exe"
    } else {
        "vuec_node_bridge"
    };
    Ok(PathBuf::from("target").join("debug").join(exe_name))
}

fn rust_alias_root(version_line: VersionLine) -> PathBuf {
    PathBuf::from("target")
        .join("compat")
        .join("rust-alias")
        .join(version_line.as_str())
}

fn rust_alias_package_dir(target: TargetSpec) -> PathBuf {
    let root = rust_alias_root(target.version_line).join("node_modules");
    match target.package {
        package if package.starts_with("@vue/") => {
            let package_name = package.trim_start_matches("@vue/");
            root.join("@vue").join(package_name)
        }
        "vue" => root.join("vue"),
        package => root.join(package),
    }
}

fn write_alias_package_json(
    package_dir: &Path,
    target: TargetSpec,
    manifest: &ManifestFile,
) -> Result<()> {
    let main = match target.kind {
        TargetKind::Vue3Sfc => "dist/compiler-sfc.cjs.js",
        TargetKind::Vue3Ssr => "dist/compiler-ssr.cjs.js",
        TargetKind::Vue27Sfc => "index.js",
        _ => "index.js",
    };
    let types = manifest
        .types
        .package_types
        .as_deref()
        .unwrap_or("index.d.ts");
    let package_json = serde_json::json!({
        "name": target.package,
        "version": manifest.package_version.as_deref().unwrap_or("0.0.0"),
        "private": true,
        "main": main,
        "types": types,
        "description": "Generated Rust Vue compiler compatibility alias package",
    });
    write_json(&package_dir.join("package.json"), &package_json)
}

fn write_alias_index(
    alias_root: &Path,
    package_dir: &Path,
    target: TargetSpec,
    manifest: &ManifestFile,
) -> Result<()> {
    let main_path = match target.kind {
        TargetKind::Vue3Sfc => package_dir.join("dist").join("compiler-sfc.cjs.js"),
        TargetKind::Vue3Ssr => package_dir.join("dist").join("compiler-ssr.cjs.js"),
        TargetKind::Vue27Sfc => package_dir.join("compiler-sfc").join("index.js"),
        _ => package_dir.join("index.js"),
    };
    if let Some(parent) = main_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut source = String::new();
    source.push_str("'use strict';\n\n");
    source.push_str("const cp = require('child_process');\n");
    source.push_str("const path = require('path');\n\n");
    source.push_str("const BRIDGE_BIN = process.env.VUEC_NODE_BRIDGE || path.resolve(__dirname, ");
    source.push_str(&js_string_literal(&bridge_relative_path(
        alias_root, &main_path,
    )));
    source.push_str(");\n");
    source.push('\n');
    source.push_str(ALIAS_RUNTIME_JS);
    source.push('\n');
    if target.kind == TargetKind::Vue3Core {
        source.push_str("Object.defineProperty(exports, '__vuecRuntime', { value: vue3CoreRuntime, enumerable: false });\n");
    }
    for export_name in &manifest.exports {
        let detail = manifest.export_details.get(export_name);
        source.push_str("exports[");
        source.push_str(&js_string_literal(export_name));
        source.push_str("] = ");
        source.push_str(&alias_export_expression(target, export_name, detail));
        source.push_str(";\n");
    }
    write_text(&main_path, &source)?;
    if target.kind == TargetKind::Vue27Sfc {
        write_text(
            &package_dir.join("index.js"),
            "module.exports = require('./compiler-sfc/index.js');\n",
        )?;
    }
    Ok(())
}

fn write_alias_types(
    package_dir: &Path,
    target: TargetSpec,
    manifest: &ManifestFile,
) -> Result<()> {
    let relative = manifest
        .types
        .package_types
        .as_deref()
        .unwrap_or("index.d.ts");
    let path = package_dir.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut body = String::new();
    body.push_str("// Generated compatibility alias declarations.\n");
    body.push_str("export const __vuecRustAlias: true;\n");
    if target.kind == TargetKind::Vue27Sfc {
        let root_types = package_dir.join("index.d.ts");
        write_text(&root_types, "export * from './compiler-sfc/index';\n")?;
    }
    write_text(&path, &body)
}

fn bridge_relative_path(alias_root: &Path, from_file: &Path) -> String {
    let depth = from_file
        .parent()
        .and_then(|parent| parent.strip_prefix(alias_root).ok())
        .map(|relative| relative.components().count())
        .unwrap_or(0);
    let mut path = String::new();
    for _ in 0..depth {
        path.push_str("../");
    }
    path.push_str("../../../debug/");
    path.push_str(if cfg!(windows) {
        "vuec_node_bridge.exe"
    } else {
        "vuec_node_bridge"
    });
    path
}

fn alias_export_expression(
    target: TargetSpec,
    export_name: &str,
    detail: Option<&ApiExportDetail>,
) -> String {
    let Some(detail) = detail else {
        return "undefined".into();
    };
    if target.kind == TargetKind::Vue3Core {
        if vue3_core_runtime_export(export_name, detail).is_some() {
            if detail.kind == "function" {
                return alias_runtime_function_expression("vue3CoreRuntime", export_name, detail);
            }
            return format!("vue3CoreRuntime[{}]", js_string_literal(export_name));
        }
    }
    match detail.kind.as_str() {
        "function" => alias_function_expression(target, export_name, detail),
        "symbol" => "Symbol.for('vuec.alias')".into(),
        "string" => manifest_string_value(target, export_name),
        "object" if detail.tag == "[object Array]" => {
            let entries = detail
                .own_property_names
                .iter()
                .filter(|name| name.chars().all(|ch| ch.is_ascii_digit()))
                .map(|name| js_string_literal(name))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{entries}]")
        }
        "object" if detail.tag == "[object RegExp]" => "/(?:)/".into(),
        "object" => object_from_property_names(&detail.own_property_names),
        _ => "undefined".into(),
    }
}

fn vue3_core_runtime_export(export_name: &str, detail: &ApiExportDetail) -> Option<()> {
    match export_name {
        "baseCompile" | "baseParse" => None,
        _ if detail.kind == "function" => Some(()),
        _ if detail.kind == "symbol" => Some(()),
        "BindingTypes"
        | "CompilerDeprecationTypes"
        | "ConstantTypes"
        | "ElementTypes"
        | "ErrorCodes"
        | "Namespaces"
        | "NodeTypes"
        | "TS_NODE_TYPES"
        | "errorMessages"
        | "helperNameMap"
        | "locStub"
        | "forAliasRE"
        | "validFirstIdentCharRE" => Some(()),
        _ => None,
    }
}

fn alias_function_expression(
    target: TargetSpec,
    export_name: &str,
    detail: &ApiExportDetail,
) -> String {
    let name = detail.name.as_deref().unwrap_or(export_name);
    let arity = detail.function_arity.unwrap_or(0);
    let body_arity = alias_body_arity(target, export_name, arity);
    let command = bridge_command(target, export_name);
    if detail.is_class_like.unwrap_or(false) {
        let args = (0..arity)
            .map(|index| format!("a{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut expression = format!(
            "class {} {{ constructor({args}) {{ this.args = Array.prototype.slice.call(arguments); }} }}",
            sanitize_js_identifier(name)
        );
        expression = format!("(() => {{ const cls = {expression};");
        expression.push_str(&format!(
            " Object.defineProperty(cls, 'name', {{ value: {}, configurable: true }});",
            js_string_literal(name)
        ));
        add_static_function_props(&mut expression, detail);
        expression.push_str(" return cls; })()");
        return expression;
    }
    let args = (0..arity)
        .map(|index| format!("a{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let argument_bindings = if body_arity > arity {
        (arity..body_arity)
            .map(|index| format!("const a{index} = arguments[{index}];"))
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        String::new()
    };
    let body = match command {
        Some(command) => {
            let call = if matches!(
                (target.kind, export_name),
                (TargetKind::Vue3Core, "baseParse") | (TargetKind::Vue3Dom, "parse")
            ) {
                format!(
                    "hydrateVue3Ast(callBridge({}, bridgePayloadForCall(__vuecPayload)), __vuecPayload.options)",
                    js_string_literal(command)
                )
            } else {
                format!(
                    "callBridge({}, bridgePayloadForCall(__vuecPayload))",
                    js_string_literal(command)
                )
            };
            format!(
                "{argument_bindings} const __vuecPayload = normalizeArgs({}); preflightAliasCall({}, __vuecPayload); return {call};",
                alias_argument_object(target, export_name, body_arity),
                js_string_literal(alias_preflight_name(target, export_name)),
            )
        }
        None => format!(
            "{argument_bindings} return notImplemented({});",
            js_string_literal(export_name)
        ),
    };
    if detail
        .own_property_names
        .iter()
        .any(|name| name == "prototype")
    {
        format!("function {name}({args}) {{ {body} }}")
    } else {
        let expression = format!("function {name}({args}) {{ {body} }}");
        format!(
            "namedArity({}, {}, {})",
            js_string_literal(name),
            arity,
            expression
        )
    }
}

fn alias_runtime_function_expression(
    runtime_object: &str,
    export_name: &str,
    detail: &ApiExportDetail,
) -> String {
    let name = detail.name.as_deref().unwrap_or(export_name);
    let arity = detail.function_arity.unwrap_or(0);
    let args = (0..arity)
        .map(|index| format!("a{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let apply_body = format!(
        "return {}[{}].apply(this, arguments);",
        runtime_object,
        js_string_literal(export_name)
    );
    if detail
        .own_property_names
        .iter()
        .any(|name| name == "prototype")
    {
        format!("function {name}({args}) {{ {apply_body} }}")
    } else {
        let expression = format!("function {name}({args}) {{ {apply_body} }}");
        format!(
            "namedArity({}, {}, {})",
            js_string_literal(name),
            arity,
            expression
        )
    }
}

fn alias_body_arity(target: TargetSpec, export_name: &str, arity: u32) -> u32 {
    match (target.kind, export_name) {
        (TargetKind::Vue3Core | TargetKind::Vue3Dom | TargetKind::Vue3Ssr, "baseCompile")
        | (TargetKind::Vue3Core | TargetKind::Vue3Dom | TargetKind::Vue3Ssr, "baseParse")
        | (TargetKind::Vue3Dom, "parse")
        | (TargetKind::Vue3Core | TargetKind::Vue3Dom | TargetKind::Vue3Ssr, "compile")
        | (TargetKind::Vue3Sfc, "parse")
        | (TargetKind::Vue27Sfc | TargetKind::Vue3Sfc, "compileScript") => arity.max(2),
        _ => arity,
    }
}

fn alias_preflight_name(target: TargetSpec, export_name: &str) -> &'static str {
    match (target.kind, export_name) {
        (TargetKind::Vue3Core, "baseCompile") => "vue3.core.baseCompile",
        _ => "",
    }
}

fn add_static_function_props(source: &mut String, detail: &ApiExportDetail) {
    for prop in &detail.own_property_names {
        if matches!(prop.as_str(), "length" | "name" | "prototype") {
            continue;
        }
        source.push_str(" cls[");
        source.push_str(&js_string_literal(prop));
        source.push_str("] = ");
        source.push_str(&object_value_for_property(prop));
        source.push(';');
    }
}

fn bridge_command(target: TargetSpec, export_name: &str) -> Option<&'static str> {
    match (target.kind, export_name) {
        (TargetKind::Vue26Template | TargetKind::Vue27Template, "compile") => Some("vue2.compile"),
        (TargetKind::Vue26Template | TargetKind::Vue27Template, "compileToFunctions") => {
            Some("vue2.compileToFunctions")
        }
        (TargetKind::Vue26Template | TargetKind::Vue27Template, "ssrCompile") => {
            Some("vue2.ssrCompile")
        }
        (TargetKind::Vue26Template | TargetKind::Vue27Template, "ssrCompileToFunctions") => {
            Some("vue2.ssrCompileToFunctions")
        }
        (TargetKind::Vue26Template | TargetKind::Vue27Template, "generateCodeFrame") => {
            Some("vue2.generateCodeFrame")
        }
        (TargetKind::Vue27Sfc, "parse") => Some("sfc.vue27.parse"),
        (TargetKind::Vue3Sfc, "parse") => Some("sfc.parse"),
        (TargetKind::Vue27Sfc, "compileTemplate") => Some("sfc.vue27.compileTemplate"),
        (TargetKind::Vue3Sfc, "compileTemplate") => Some("sfc.compileTemplate"),
        (TargetKind::Vue27Sfc, "compileScript") => Some("sfc.vue27.compileScript"),
        (TargetKind::Vue3Sfc, "compileScript") => Some("sfc.compileScript"),
        (TargetKind::Vue27Sfc, "compileStyle") => Some("sfc.vue27.compileStyle"),
        (TargetKind::Vue27Sfc, "compileStyleAsync") => Some("sfc.vue27.compileStyleAsync"),
        (TargetKind::Vue3Sfc, "compileStyle") => Some("sfc.compileStyle"),
        (TargetKind::Vue3Sfc, "compileStyleAsync") => Some("sfc.compileStyleAsync"),
        (TargetKind::Vue3Core, "baseCompile") => Some("vue3.core.baseCompile"),
        (TargetKind::Vue3Core, "baseParse") => Some("vue3.core.baseParse"),
        (TargetKind::Vue3Dom, "compile") => Some("vue3.dom.compile"),
        (TargetKind::Vue3Dom, "parse") => Some("vue3.dom.parse"),
        (TargetKind::Vue3Ssr, "compile") => Some("vue3.ssr.compile"),
        _ => None,
    }
}

fn alias_argument_object(target: TargetSpec, export_name: &str, _arity: u32) -> String {
    match (target.kind, export_name) {
        (TargetKind::Vue26Template | TargetKind::Vue27Template, "generateCodeFrame") => {
            "{ source: a0, start: a1, end: a2 }".into()
        }
        (TargetKind::Vue26Template | TargetKind::Vue27Template, _) => {
            "{ template: a0, options: a1 }".into()
        }
        (TargetKind::Vue27Sfc, "parse") => {
            "{ source: a0 && a0.source ? a0.source : '', filename: a0 && a0.filename, options: a0 }".into()
        }
        (TargetKind::Vue27Sfc, "compileTemplate") => {
            "{ source: a0 && a0.source ? a0.source : '', filename: a0 && (a0.filename || a0.id || 'template.vue.html'), options: a0 }"
                .into()
        }
        (TargetKind::Vue27Sfc, "compileScript") => {
            "{ source: a0 && a0.descriptor && a0.descriptor.source ? a0.descriptor.source : (a0 && a0.source ? a0.source : ''), filename: a0 && a0.descriptor && a0.descriptor.filename || (a0 && a0.filename), options: a1 || a0 }"
                .into()
        }
        (TargetKind::Vue27Sfc, "compileStyle") | (TargetKind::Vue27Sfc, "compileStyleAsync") => {
            "{ source: extractStyleSource(a0 && a0.source ? a0.source : ''), filename: a0 && a0.filename, options: a0 }"
                .into()
        }
        (TargetKind::Vue27Sfc, _) => {
            "{ source: a0 && a0.source ? a0.source : '', filename: a0 && a0.filename, options: a0 }"
                .into()
        }
        (TargetKind::Vue3Sfc, "parse") => {
            "{ source: a0, filename: a1 && a1.filename, options: a1 }".into()
        }
        (TargetKind::Vue3Sfc, "compileTemplate") => {
            "{ source: a0 && a0.source ? a0.source : '', filename: a0 && (a0.filename || 'template.vue.html'), options: a0 }"
                .into()
        }
        (TargetKind::Vue3Sfc, "compileScript") => {
            "{ source: a0 && a0.descriptor && a0.descriptor.source ? a0.descriptor.source : (a0 && a0.source ? a0.source : ''), filename: a0 && a0.descriptor && a0.descriptor.filename || (a0 && a0.filename), options: a1 || a0 }"
                .into()
        }
        (TargetKind::Vue3Sfc, "compileStyle") | (TargetKind::Vue3Sfc, "compileStyleAsync") => {
            "{ source: a0 && a0.source ? a0.source : '', filename: a0 && a0.filename, options: a0 }"
                .into()
        }
        (TargetKind::Vue3Sfc, _) => {
            "{ source: a0 && a0.source ? a0.source : '', filename: a0 && a0.filename, options: a0 }"
                .into()
        }
        (TargetKind::Vue3Dom, "parse") => {
            "vue3BridgePayload(a0 && a0.source ? a0.source : a0, undefined, a1 || (a0 && a0.options) || {})"
                .into()
        }
        (TargetKind::Vue3Core | TargetKind::Vue3Dom | TargetKind::Vue3Ssr, _) => {
            "vue3BridgePayload(a0 && a0.source ? a0.source : a0, a0 && a0.filename, a1 || (a0 && a0.options) || {})"
                .into()
        }
    }
}

fn manifest_string_value(target: TargetSpec, export_name: &str) -> String {
    if target.kind == TargetKind::Vue3Sfc && export_name == "version" {
        js_string_literal("3.5.34")
    } else {
        "''".into()
    }
}

fn object_from_property_names(properties: &[String]) -> String {
    let entries = properties
        .iter()
        .map(|prop| {
            format!(
                "{}: {}",
                js_string_literal(prop),
                object_value_for_property(prop)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{entries}}}")
}

fn object_value_for_property(prop: &str) -> String {
    if prop.chars().all(|ch| ch.is_ascii_digit()) {
        prop.to_string()
    } else {
        "undefined".into()
    }
}

fn sanitize_js_identifier(name: &str) -> String {
    let mut out = String::new();
    for (index, ch) in name.chars().enumerate() {
        let valid = ch == '_' || ch == '$' || ch.is_ascii_alphanumeric();
        if !valid || (index == 0 && ch.is_ascii_digit()) {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    if out.is_empty() {
        "_VuecAlias".into()
    } else {
        out
    }
}

fn js_string_literal(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

fn write_text(path: &Path, value: &str) -> Result<()> {
    fs::write(path, value).with_context(|| format!("failed to write {}", path.display()))
}

fn manifest_from_probe(
    target: TargetSpec,
    side: ApiManifestSide,
    lock_hash: Option<String>,
    official_revision: Option<String>,
    probe: ApiProbeOutput,
) -> ManifestFile {
    let status = match (side, probe.require.success) {
        (_, true) => "pass",
        (ApiManifestSide::Official, false) => "fail",
        (ApiManifestSide::Rust, false) => "pending",
    };
    ManifestFile {
        schema_version: 1,
        version_line: target.version_line,
        package: target.package.to_string(),
        entry: target.entry.to_string(),
        package_version: probe.package_version,
        exports: probe.exports,
        export_details: probe.export_details,
        require: probe.require,
        types: probe.types,
        status: status.to_string(),
        source: side.as_str().to_string(),
        lock_hash,
        official_revision,
    }
}

fn failed_api_manifest(
    target: TargetSpec,
    side: ApiManifestSide,
    lock_hash: Option<String>,
    official_revision: Option<String>,
    message: String,
) -> ManifestFile {
    ManifestFile {
        schema_version: 1,
        version_line: target.version_line,
        package: target.package.to_string(),
        entry: target.entry.to_string(),
        package_version: None,
        exports: Vec::new(),
        export_details: BTreeMap::new(),
        require: ApiRequireRecord {
            request: api_require_request(target),
            success: false,
            resolved: None,
            error_name: Some("XtaskError".into()),
            error_code: None,
            error_message: Some(message),
        },
        types: ApiTypesRecord::default(),
        status: if side == ApiManifestSide::Rust {
            "pending"
        } else {
            "fail"
        }
        .into(),
        source: side.as_str().to_string(),
        lock_hash,
        official_revision,
    }
}

fn manifest_status(manifest: &ManifestFile) -> ReportStatus {
    match manifest.status.as_str() {
        "pass" => ReportStatus::Pass,
        "pending" => ReportStatus::Pending,
        _ => ReportStatus::Fail,
    }
}

fn compare_api_manifests(official: &ManifestFile, rust: &ManifestFile) -> Vec<String> {
    let mut diffs = Vec::new();
    if official.version_line != rust.version_line {
        diffs.push(format!(
            "version_line differs: official={} rust={}",
            official.version_line, rust.version_line
        ));
    }
    if official.package != rust.package {
        diffs.push(format!(
            "package differs: official={} rust={}",
            official.package, rust.package
        ));
    }
    if official.entry != rust.entry {
        diffs.push(format!(
            "entry differs: official={} rust={}",
            official.entry, rust.entry
        ));
    }
    if official.require.success != rust.require.success {
        diffs.push(format!(
            "require success differs: official={} rust={}",
            official.require.success, rust.require.success
        ));
    }
    if !official.require.success {
        diffs.push(format!(
            "official manifest did not load: {}",
            official
                .require
                .error_message
                .as_deref()
                .unwrap_or("unknown error")
        ));
    }
    if !rust.require.success {
        diffs.push(format!(
            "Rust alias manifest did not load: {}",
            rust.require
                .error_message
                .as_deref()
                .unwrap_or("unknown error")
        ));
    }
    if official.package_version != rust.package_version {
        diffs.push(format!(
            "package_version differs: official={:?} rust={:?}",
            official.package_version, rust.package_version
        ));
    }
    if official.exports != rust.exports {
        diffs.push(format!(
            "exports differ: official={:?} rust={:?}",
            official.exports, rust.exports
        ));
    }
    for export_name in official
        .exports
        .iter()
        .chain(rust.exports.iter())
        .collect::<std::collections::BTreeSet<_>>()
    {
        let official_detail = official.export_details.get(export_name.as_str());
        let rust_detail = rust.export_details.get(export_name.as_str());
        if official_detail != rust_detail {
            diffs.push(format!(
                "export {export_name} detail differs: official={official_detail:?} rust={rust_detail:?}"
            ));
        }
    }
    if official.types.package_types != rust.types.package_types {
        diffs.push(format!(
            "types package path differs: official={:?} rust={:?}",
            official.types.package_types, rust.types.package_types
        ));
    }
    if official.types.exists != rust.types.exists {
        diffs.push(format!(
            "types existence differs: official={} rust={}",
            official.types.exists, rust.types.exists
        ));
    }
    diffs
}

fn load_allowed_api_diffs(path: &Path) -> AllowedApiDiffFile {
    match read_json::<AllowedApiDiffFile>(path) {
        Ok(file) => file,
        Err(_) => AllowedApiDiffFile::default(),
    }
}

fn is_allowed_api_diff(allowed: &AllowedApiDiffFile, target: TargetSpec, diff: &str) -> bool {
    allowed.entries.iter().any(|entry| {
        entry.version_line == target.version_line
            && entry.package == target.package
            && entry.entry == target.entry
            && entry.diff == diff
            && !entry.reason.trim().is_empty()
    })
}

fn api_require_request(target: TargetSpec) -> String {
    if target.entry == "index" {
        target.package.to_string()
    } else {
        target.entry.to_string()
    }
}

fn baseline_for(lock: &OfficialRevisionsLock, version_line: VersionLine) -> Option<&BaselineLock> {
    match version_line {
        VersionLine::Vue26 => Some(&lock.vue2_6),
        VersionLine::Vue27 => Some(&lock.vue2_7),
        VersionLine::Vue3 => Some(&lock.vue3),
    }
}

fn ensure_official_npm_install(
    version_line: VersionLine,
    baseline: &BaselineLock,
) -> Result<PathBuf> {
    let install_root = PathBuf::from("target")
        .join("compat")
        .join("npm")
        .join(version_line.as_str());
    let node_modules = install_root.join("node_modules");
    let specs = baseline
        .npm
        .iter()
        .map(|(package, version)| format!("{package}@{version}"))
        .collect::<Vec<_>>();
    let marker = install_root.join("official-install.json");
    if node_modules.exists() && official_install_marker_matches(&marker, &specs) {
        return Ok(install_root);
    }
    fs::create_dir_all(&install_root)
        .with_context(|| format!("failed to create {}", install_root.display()))?;
    let package_json = serde_json::json!({
        "private": true,
        "name": format!("vuec-compat-{}", version_line.as_str()),
        "version": "0.0.0",
    });
    write_json(&install_root.join("package.json"), &package_json)?;

    let npm = resolve_program("npm");
    let mut command = Command::new(npm);
    command
        .arg("install")
        .arg("--ignore-scripts")
        .arg("--no-audit")
        .arg("--no-fund")
        .arg("--package-lock=false")
        .arg("--omit=dev")
        .args(&specs)
        .current_dir(&install_root);
    let output = command
        .output()
        .with_context(|| format!("failed to spawn npm install in {}", install_root.display()))?;
    if !output.status.success() {
        anyhow::bail!(
            "`npm install {}` failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
            specs.join(" "),
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let marker_body = serde_json::json!({
        "version_line": version_line,
        "packages": specs,
        "rev": baseline.rev,
    });
    write_json(&marker, &marker_body)?;
    Ok(install_root)
}

fn official_install_marker_matches(marker: &Path, specs: &[String]) -> bool {
    let Ok(value) = read_json::<serde_json::Value>(marker) else {
        return false;
    };
    let Some(packages) = value.get("packages").and_then(|value| value.as_array()) else {
        return false;
    };
    let actual = packages
        .iter()
        .filter_map(|value| value.as_str().map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    actual == specs
}

fn ensure_official_runner_dependencies(
    spec: ConformanceSuiteSpec,
    baseline: &BaselineLock,
    vendor_dir: &Path,
) -> Result<PathBuf> {
    let install_root = ensure_official_npm_install(spec.version_line, baseline)?;
    let node_modules = install_root.join("node_modules");
    let runner_specs = runner_dependency_specs(spec, vendor_dir)?.ok_or_else(|| {
        anyhow::anyhow!(
            "{} has runner dependencies but no deterministic versions could be resolved",
            spec.name
        )
    })?;
    if runner_specs.is_empty() {
        return Ok(install_root);
    }
    let marker = install_root.join(format!("runner-install-{}.json", spec.name));
    if node_modules.exists() && official_install_marker_matches(&marker, &runner_specs) {
        return Ok(install_root);
    }

    let npm = resolve_program("npm");
    let mut command = Command::new(npm);
    command
        .arg("install")
        .arg("--ignore-scripts")
        .arg("--no-audit")
        .arg("--no-fund")
        .arg("--package-lock=false")
        .arg("--omit=dev")
        .args(&runner_specs)
        .current_dir(&install_root);
    let output = command.output().with_context(|| {
        format!(
            "failed to spawn npm runner dependency install in {}",
            install_root.display()
        )
    })?;
    if !output.status.success() {
        anyhow::bail!(
            "`npm install {}` failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
            runner_specs.join(" "),
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let marker_body = serde_json::json!({
        "version_line": spec.version_line,
        "suite": spec.name,
        "packages": runner_specs,
        "rev": baseline.rev,
    });
    write_json(&marker, &marker_body)?;
    Ok(install_root)
}

fn runner_dependency_specs(
    spec: ConformanceSuiteSpec,
    vendor_dir: &Path,
) -> Result<Option<Vec<String>>> {
    if spec.runner_dependencies.is_empty() {
        return Ok(Some(Vec::new()));
    }
    let root = vendor_dir.join(spec.version_line.as_str());
    let package_json = root.join("package.json");
    if !package_json.is_file() {
        return Ok(None);
    }
    let manifest = read_json::<serde_json::Value>(&package_json)?;
    let mut specs = Vec::new();
    for dependency in spec.runner_dependencies {
        let version = locked_runner_dependency_version(&root, dependency)
            .or_else(|| manifest_dependency_version(&manifest, dependency));
        let Some(version) = version else {
            return Ok(None);
        };
        if is_unpublished_dependency_spec(&version) {
            return Ok(None);
        }
        specs.push(format!("{dependency}@{version}"));
    }
    specs.sort();
    specs.dedup();
    Ok(Some(specs))
}

fn locked_runner_dependency_version(root: &Path, dependency: &str) -> Option<String> {
    let pnpm_lock = root.join("pnpm-lock.yaml");
    if pnpm_lock.is_file() {
        let lock = fs::read_to_string(pnpm_lock).ok()?;
        if let Some(version) = locked_pnpm_dependency_version(&lock, dependency) {
            return Some(version);
        }
    }
    let yarn_lock = root.join("yarn.lock");
    if yarn_lock.is_file() {
        let lock = fs::read_to_string(yarn_lock).ok()?;
        if let Some(version) = locked_yarn_dependency_version(&lock, dependency) {
            return Some(version);
        }
    }
    None
}

fn locked_pnpm_dependency_version(lock: &str, dependency: &str) -> Option<String> {
    for line in lock.lines() {
        let trimmed = line.trim_start();
        let candidate = trimmed
            .strip_prefix(&format!("{dependency}@"))
            .or_else(|| trimmed.strip_prefix(&format!("/{dependency}@")));
        let Some(candidate) = candidate else {
            continue;
        };
        let version_end = candidate.find(['(', ':']).unwrap_or(candidate.len());
        let version = candidate[..version_end].trim();
        if is_publishable_version(version) {
            return Some(version.to_string());
        }
    }
    None
}

fn locked_yarn_dependency_version(lock: &str, dependency: &str) -> Option<String> {
    let mut lines = lock.lines().peekable();
    while let Some(line) = lines.next() {
        if line.starts_with(char::is_whitespace)
            || !yarn_lock_key_matches_dependency(line, dependency)
        {
            continue;
        }
        while let Some(next) = lines.peek().copied() {
            if !next.starts_with("  ") {
                break;
            }
            let value = next.trim();
            if let Some(version) = value.strip_prefix("version ") {
                let version = version.trim_matches('"');
                if is_publishable_version(version) {
                    return Some(version.to_string());
                }
            }
            lines.next();
        }
    }
    None
}

fn yarn_lock_key_matches_dependency(line: &str, dependency: &str) -> bool {
    let key = line.trim().trim_end_matches(':');
    key.split(',').any(|part| {
        let part = part.trim().trim_matches('"');
        yarn_lock_package_name(part).is_some_and(|name| name == dependency)
    })
}

fn yarn_lock_package_name(spec: &str) -> Option<&str> {
    if spec.starts_with('@') {
        let slash = spec.find('/')?;
        let after_scope = &spec[slash + 1..];
        let at = after_scope.find('@')?;
        return Some(&spec[..slash + 1 + at]);
    }
    let at = spec.find('@')?;
    Some(&spec[..at])
}

fn manifest_dependency_version(manifest: &serde_json::Value, dependency: &str) -> Option<String> {
    ["dependencies", "devDependencies", "peerDependencies"]
        .into_iter()
        .find_map(|section| {
            manifest
                .get(section)
                .and_then(|value| value.get(dependency))
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        })
}

fn is_unpublished_dependency_spec(version: &str) -> bool {
    let version = version.trim();
    version.is_empty()
        || version == "catalog:"
        || version.starts_with("workspace:")
        || version == "link:"
        || version.starts_with("file:")
}

fn is_publishable_version(version: &str) -> bool {
    let first = version.chars().next();
    first.is_some_and(|ch| ch.is_ascii_digit())
}

fn probe_api_exports(root: &Path, package_name: &str, request: &str) -> Result<ApiProbeOutput> {
    let root = absolute_path(root);
    let node = resolve_program("node");
    let output = Command::new(node)
        .arg("-e")
        .arg(API_PROBE_SCRIPT)
        .env("VUEC_API_PROBE_ROOT", &root)
        .env("VUEC_API_PROBE_PACKAGE", package_name)
        .env("VUEC_API_PROBE_REQUEST", request)
        .output()
        .with_context(|| format!("failed to spawn node API probe for {request}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "node API probe failed for {} with status {:?}\nstdout:\n{}\nstderr:\n{}",
            request,
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim())
        .with_context(|| format!("failed to parse node API probe output for {request}"))
}

fn run_alias_smoke(target: TargetSpec, root: &Path) -> Result<String> {
    let root = absolute_path(root);
    let request = api_require_request(target);
    let script = alias_smoke_script(target);
    let node = resolve_program("node");
    let output = Command::new(node)
        .arg("-e")
        .arg(script)
        .env("VUEC_ALIAS_ROOT", &root)
        .env("VUEC_ALIAS_REQUEST", &request)
        .output()
        .with_context(|| format!("failed to spawn npm alias smoke for {request}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "node alias smoke failed for {} with status {:?}\nstdout:\n{}\nstderr:\n{}",
            request,
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.trim().to_string())
}

fn run_output_contract_probe(
    target: TargetSpec,
    official_root: &Path,
    rust_root: &Path,
) -> Result<serde_json::Value> {
    let official_root = absolute_path(official_root);
    let rust_root = absolute_path(rust_root);
    let request = api_require_request(target);
    let fixture = output_contract_fixture(target);
    let node = resolve_program("node");
    let output = Command::new(node)
        .arg("-e")
        .arg(OUTPUT_CONTRACT_PROBE_SCRIPT)
        .env("VUEC_OUTPUT_OFFICIAL_ROOT", &official_root)
        .env("VUEC_OUTPUT_RUST_ROOT", &rust_root)
        .env("VUEC_OUTPUT_REQUEST", &request)
        .env("VUEC_OUTPUT_KIND", output_contract_kind(target))
        .env("VUEC_OUTPUT_VERSION_LINE", target.version_line.as_str())
        .env("VUEC_OUTPUT_ENTRY", target.entry)
        .env("VUEC_OUTPUT_FIXTURE", fixture)
        .output()
        .with_context(|| format!("failed to spawn output contract probe for {request}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "node output contract probe failed for {} with status {:?}\nstdout:\n{}\nstderr:\n{}",
            request,
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim())
        .with_context(|| format!("failed to parse output contract probe for {request}"))
}

fn run_option_probe(
    side: &str,
    target: TargetSpec,
    root: &Path,
    request: &str,
    method: &str,
    fixture_source: &str,
    fixture_id: &str,
    option_name: &str,
    option_path: &str,
    input_kind: &str,
    option_value: Option<&serde_json::Value>,
) -> Result<OptionProbeOutput> {
    let root = absolute_path(root);
    let node = resolve_program("node");
    let payload = serde_json::json!({
        "request": request,
        "method": method,
        "source": fixture_source,
        "fixture_id": fixture_id,
        "option_name": option_name,
        "option_path": option_path,
        "input_kind": input_kind,
        "option_value": option_value,
        "target_version_line": target.version_line.as_str(),
        "target_package": target.package,
        "target_entry": target.entry,
    });
    let output = Command::new(node)
        .arg("-e")
        .arg(OPTION_MATRIX_PROBE_SCRIPT)
        .env("VUEC_OPTION_ROOT", &root)
        .env("VUEC_OPTION_SIDE", side)
        .env("VUEC_OPTION_PAYLOAD", serde_json::to_string(&payload)?)
        .output()
        .with_context(|| format!("failed to spawn option matrix probe for {request}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "node option matrix probe failed for {} with status {:?}\nstdout:\n{}\nstderr:\n{}",
            request,
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim())
        .with_context(|| format!("failed to parse option matrix probe for {request}"))
}

fn output_contract_kind(target: TargetSpec) -> &'static str {
    match target.kind {
        TargetKind::Vue26Template | TargetKind::Vue27Template => "vue2-template",
        TargetKind::Vue27Sfc | TargetKind::Vue3Sfc => "sfc",
        TargetKind::Vue3Core => "vue3-core",
        TargetKind::Vue3Dom => "vue3-dom",
        TargetKind::Vue3Ssr => "vue3-ssr",
    }
}

fn output_contract_fixture(target: TargetSpec) -> &'static str {
    match target.kind {
        TargetKind::Vue26Template | TargetKind::Vue27Template => {
            "<div id=\"app\"><span>{{ msg }}</span></div>"
        }
        TargetKind::Vue27Sfc | TargetKind::Vue3Sfc => {
            "<template><div class=\"a\">{{ msg }}</div></template><script>export default { props: ['msg'] }</script><style scoped>.a{ color: v-bind(color); }</style>"
        }
        TargetKind::Vue3Core | TargetKind::Vue3Dom | TargetKind::Vue3Ssr => {
            "<div class=\"a\"><span>{{ msg }}</span></div>"
        }
    }
}

fn json_usize(value: &serde_json::Value, path: &[&str]) -> usize {
    let mut cursor = value;
    for key in path {
        let Some(next) = cursor.get(*key) else {
            return 0;
        };
        cursor = next;
    }
    cursor.as_u64().unwrap_or_default() as usize
}

fn output_contract_counts_from_items(items: &[ReportItem]) -> serde_json::Value {
    serde_json::json!({
        "total": items.len(),
        "pass": items.iter().filter(|item| item.status == ReportStatus::Pass).count(),
        "pending": items.iter().filter(|item| item.status == ReportStatus::Pending).count(),
        "fail": items.iter().filter(|item| item.status == ReportStatus::Fail).count(),
    })
}

fn compare_option_probe(
    row: &OptionMatrixRow,
    official: &OptionProbeOutput,
    rust: &OptionProbeOutput,
) -> bool {
    if official.ok != rust.ok {
        return false;
    }
    if !official.ok {
        return official.error == rust.error;
    }
    let official_value = official.value.as_ref().unwrap_or(&serde_json::Value::Null);
    let rust_value = rust.value.as_ref().unwrap_or(&serde_json::Value::Null);
    for field in &row.output_fields_affected {
        if let Some(expected) = field.strip_prefix("code:contains:") {
            let official_code = json_path(official_value, "code").and_then(|value| value.as_str());
            let rust_code = json_path(rust_value, "code").and_then(|value| value.as_str());
            if official_code.map(|code| code.contains(expected)) != Some(true)
                || rust_code.map(|code| code.contains(expected)) != Some(true)
            {
                return false;
            }
            continue;
        }
        let official_field = json_path(official_value, field);
        let rust_field =
            json_path(rust_value, field).or_else(|| rust_alias_field(rust_value, field));
        if official_field != rust_field {
            return false;
        }
    }
    true
}

fn json_path<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut cursor = value;
    for segment in path.split('.') {
        if let Ok(index) = segment.parse::<usize>() {
            cursor = cursor.get(index)?;
        } else {
            cursor = cursor.get(segment)?;
        }
    }
    Some(cursor)
}

fn rust_alias_field<'a>(
    value: &'a serde_json::Value,
    field: &str,
) -> Option<&'a serde_json::Value> {
    match field {
        "staticRenderFns" => value.get("static_render_fns"),
        "template" | "script" | "styles" | "customBlocks" => value
            .get("descriptor")
            .and_then(|descriptor| descriptor.get(field)),
        "descriptor.scriptSetup.content" => value
            .get("descriptor")
            .and_then(|descriptor| descriptor.get("script_setup"))
            .and_then(|script_setup| script_setup.get("content")),
        "descriptor.styles.0.scoped" => value
            .get("descriptor")
            .and_then(|descriptor| descriptor.get("styles"))
            .and_then(|styles| styles.get(0))
            .and_then(|style| style.get("attrs"))
            .and_then(|attrs| attrs.get("scoped")),
        "ast" => value
            .get("element_ast")
            .or_else(|| value.get("ast_summary")),
        _ => None,
    }
}

fn alias_smoke_script(target: TargetSpec) -> String {
    let call = match target.kind {
        TargetKind::Vue26Template | TargetKind::Vue27Template => {
            "const result = api.compile('<div>{{ msg }}</div>', { optimize: true }); assert(result && typeof result.render === 'string', 'compile render missing');"
        }
        TargetKind::Vue27Sfc => {
            "const result = api.parse({ source: '<template><div/></template><script>export default {}</script>', filename: 'smoke.vue' }); assert(result && result.template, 'parse descriptor missing template');"
        }
        TargetKind::Vue3Sfc => {
            "const result = api.parse('<template><div/></template><script>export default {}</script>'); assert(result && result.descriptor && result.descriptor.template, 'parse descriptor missing template');"
        }
        TargetKind::Vue3Core => {
            "const result = api.baseCompile('<div>{{ msg }}</div>', {}); assert(result && typeof result.code === 'string', 'baseCompile code missing');"
        }
        TargetKind::Vue3Dom => {
            "const result = api.compile('<input v-model=\"msg\">', {}); assert(result && typeof result.code === 'string', 'dom compile code missing');"
        }
        TargetKind::Vue3Ssr => {
            "const result = api.compile('<div>{{ msg }}</div>'); assert(result && typeof result.code === 'string', 'ssr compile code missing');"
        }
    };
    format!(
        r#"
const path = require('path');
const {{ createRequire }} = require('module');
const root = process.env.VUEC_ALIAS_ROOT;
const request = process.env.VUEC_ALIAS_REQUEST;
const rootRequire = createRequire(path.join(root, 'package.json'));
function assert(value, message) {{
  if (!value) {{
    throw new Error(message);
  }}
}}
const api = rootRequire(request);
assert(api && typeof api === 'object', 'API object missing');
{call}
process.stdout.write('pass ' + request);
"#
    )
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn resolve_program(name: &str) -> String {
    if cfg!(windows) && !name.contains('.') {
        if let Some(path) = find_on_path(&format!("{name}.cmd")) {
            return path;
        }
        if let Some(path) = find_on_path(&format!("{name}.exe")) {
            return path;
        }
    }
    name.to_string()
}

fn find_on_path(name: &str) -> Option<String> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate.display().to_string());
        }
    }
    None
}

const API_PROBE_SCRIPT: &str = r#"
const fs = require('fs');
const path = require('path');
const { createRequire } = require('module');

const root = process.env.VUEC_API_PROBE_ROOT;
const packageName = process.env.VUEC_API_PROBE_PACKAGE;
const request = process.env.VUEC_API_PROBE_REQUEST;
const rootRequire = createRequire(path.join(root, 'package.json'));

function normalizePath(file) {
  if (!file) return null;
  const relative = path.relative(root, file);
  if (relative && !relative.startsWith('..') && !path.isAbsolute(relative)) {
    return '<probe-root>/' + relative.replace(/\\/g, '/');
  }
  return file.replace(/\\/g, '/');
}

function normalizeMessage(message) {
  if (!message) return null;
  const normalizedRoot = root.replace(/\\/g, '/');
  return String(message)
    .replaceAll(root, '<probe-root>')
    .replaceAll(normalizedRoot, '<probe-root>')
    .replace(/\\/g, '/');
}

function readJson(file) {
  try {
    return JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch (_) {
    return null;
  }
}

function describeExport(value) {
  const kind = typeof value;
  const tag = Object.prototype.toString.call(value);
  const detail = {
    kind,
    tag,
    name: value && value.name ? String(value.name) : null,
    function_arity: kind === 'function' ? value.length : null,
    is_async_function: kind === 'function' ? value.constructor && value.constructor.name === 'AsyncFunction' : null,
    is_class_like: kind === 'function' ? /^class\s/.test(Function.prototype.toString.call(value)) : null,
    own_property_names: []
  };
  try {
    detail.own_property_names = Object.getOwnPropertyNames(value).sort();
  } catch (_) {
    detail.own_property_names = [];
  }
  return detail;
}

function resolvePackageJson() {
  try {
    return rootRequire.resolve(path.join(packageName, 'package.json'));
  } catch (_) {
    try {
      const resolved = rootRequire.resolve(request);
      let current = path.dirname(resolved);
      while (current && current !== path.dirname(current)) {
        const candidate = path.join(current, 'package.json');
        if (fs.existsSync(candidate)) return candidate;
        current = path.dirname(current);
      }
    } catch (_) {}
  }
  return null;
}

function resolveTypesPath(packageJsonPath, packageJson) {
  if (!packageJsonPath || !packageJson) return { packageTypes: null, resolved: null };
  const packageRoot = path.dirname(packageJsonPath);
  if (packageJson.exports && request.startsWith(packageName + '/')) {
    const subpath = './' + request.slice(packageName.length + 1);
    const exportRecord = packageJson.exports[subpath];
    if (exportRecord && typeof exportRecord === 'object' && typeof exportRecord.types === 'string') {
      const resolved = path.resolve(packageRoot, exportRecord.types);
      return { packageTypes: exportRecord.types, resolved };
    }
  }
  if (typeof packageJson.types === 'string') {
    return {
      packageTypes: packageJson.types,
      resolved: path.resolve(packageRoot, packageJson.types)
    };
  }
  return { packageTypes: null, resolved: null };
}

const packageJsonPath = resolvePackageJson();
const packageJson = packageJsonPath ? readJson(packageJsonPath) : null;
const typesInfo = resolveTypesPath(packageJsonPath, packageJson);
const out = {
  package_version: packageJson && packageJson.version ? String(packageJson.version) : null,
  exports: [],
  export_details: {},
  require: {
    request,
    success: false,
    resolved: null,
    error_name: null,
    error_code: null,
    error_message: null
  },
  types: {
    package_types: typesInfo.packageTypes,
    resolved: normalizePath(typesInfo.resolved),
    exists: typesInfo.resolved ? fs.existsSync(typesInfo.resolved) : false
  }
};

try {
  out.require.resolved = normalizePath(rootRequire.resolve(request));
  const api = rootRequire(request);
  out.require.success = true;
  out.exports = Object.keys(api).sort();
  for (const key of out.exports) {
    out.export_details[key] = describeExport(api[key]);
  }
} catch (error) {
  out.require.error_name = error && error.name ? String(error.name) : null;
  out.require.error_code = error && error.code ? String(error.code) : null;
  out.require.error_message = normalizeMessage(error && error.message ? error.message : error);
}

process.stdout.write(JSON.stringify(out));
"#;

const ALIAS_RUNTIME_JS: &str = r#"
const vue3CoreRuntime = (() => {
  const enumObject = entries => {
    const out = {};
    for (const [key, value] of entries) {
      out[key] = value;
      out[value] = key;
    }
    return out;
  };
  const NodeTypes = enumObject([
    ['ROOT', 0], ['ELEMENT', 1], ['TEXT', 2], ['COMMENT', 3],
    ['SIMPLE_EXPRESSION', 4], ['INTERPOLATION', 5], ['ATTRIBUTE', 6],
    ['DIRECTIVE', 7], ['COMPOUND_EXPRESSION', 8], ['IF', 9],
    ['IF_BRANCH', 10], ['FOR', 11], ['TEXT_CALL', 12],
    ['VNODE_CALL', 13], ['JS_CALL_EXPRESSION', 14],
    ['JS_OBJECT_EXPRESSION', 15], ['JS_PROPERTY', 16],
    ['JS_ARRAY_EXPRESSION', 17], ['JS_FUNCTION_EXPRESSION', 18],
    ['JS_CONDITIONAL_EXPRESSION', 19], ['JS_CACHE_EXPRESSION', 20],
    ['JS_BLOCK_STATEMENT', 21], ['JS_TEMPLATE_LITERAL', 22],
    ['JS_IF_STATEMENT', 23], ['JS_ASSIGNMENT_EXPRESSION', 24],
    ['JS_SEQUENCE_EXPRESSION', 25], ['JS_RETURN_STATEMENT', 26],
  ]);
  const ElementTypes = enumObject([
    ['ELEMENT', 0], ['COMPONENT', 1], ['SLOT', 2], ['TEMPLATE', 3],
  ]);
  const ConstantTypes = enumObject([
    ['NOT_CONSTANT', 0], ['CAN_SKIP_PATCH', 1], ['CAN_CACHE', 2], ['CAN_STRINGIFY', 3],
  ]);
  const Namespaces = enumObject([
    ['HTML', 0], ['SVG', 1], ['MATH_ML', 2],
  ]);
  const ErrorCodes = enumObject([
    ['ABRUPT_CLOSING_OF_EMPTY_COMMENT', 0],
    ['CDATA_IN_HTML_CONTENT', 1],
    ['DUPLICATE_ATTRIBUTE', 2],
    ['END_TAG_WITH_ATTRIBUTES', 3],
    ['END_TAG_WITH_TRAILING_SOLIDUS', 4],
    ['EOF_BEFORE_TAG_NAME', 5],
    ['EOF_IN_CDATA', 6],
    ['EOF_IN_COMMENT', 7],
    ['EOF_IN_SCRIPT_HTML_COMMENT_LIKE_TEXT', 8],
    ['EOF_IN_TAG', 9],
    ['INCORRECTLY_CLOSED_COMMENT', 10],
    ['INCORRECTLY_OPENED_COMMENT', 11],
    ['INVALID_FIRST_CHARACTER_OF_TAG_NAME', 12],
    ['MISSING_ATTRIBUTE_VALUE', 13],
    ['MISSING_END_TAG_NAME', 14],
    ['MISSING_WHITESPACE_BETWEEN_ATTRIBUTES', 15],
    ['NESTED_COMMENT', 16],
    ['UNEXPECTED_CHARACTER_IN_ATTRIBUTE_NAME', 17],
    ['UNEXPECTED_CHARACTER_IN_UNQUOTED_ATTRIBUTE_VALUE', 18],
    ['UNEXPECTED_EQUALS_SIGN_BEFORE_ATTRIBUTE_NAME', 19],
    ['UNEXPECTED_NULL_CHARACTER', 20],
    ['UNEXPECTED_QUESTION_MARK_INSTEAD_OF_TAG_NAME', 21],
    ['UNEXPECTED_SOLIDUS_IN_TAG', 22],
    ['X_INVALID_END_TAG', 23],
    ['X_MISSING_END_TAG', 24],
    ['X_MISSING_INTERPOLATION_END', 25],
    ['X_MISSING_DIRECTIVE_NAME', 26],
    ['X_MISSING_DYNAMIC_DIRECTIVE_ARGUMENT_END', 27],
    ['X_V_IF_NO_EXPRESSION', 28],
    ['X_V_IF_SAME_KEY', 29],
    ['X_V_ELSE_NO_ADJACENT_IF', 30],
    ['X_V_FOR_NO_EXPRESSION', 31],
    ['X_V_FOR_MALFORMED_EXPRESSION', 32],
    ['X_V_FOR_TEMPLATE_KEY_PLACEMENT', 33],
    ['X_V_BIND_NO_EXPRESSION', 34],
    ['X_V_ON_NO_EXPRESSION', 35],
    ['X_V_SLOT_UNEXPECTED_DIRECTIVE_ON_SLOT_OUTLET', 36],
    ['X_V_SLOT_MIXED_SLOT_USAGE', 37],
    ['X_V_SLOT_DUPLICATE_SLOT_NAMES', 38],
    ['X_V_SLOT_EXTRANEOUS_DEFAULT_SLOT_CHILDREN', 39],
    ['X_V_SLOT_MISPLACED', 40],
    ['X_V_MODEL_NO_EXPRESSION', 41],
    ['X_V_MODEL_MALFORMED_EXPRESSION', 42],
    ['X_V_MODEL_ON_SCOPE_VARIABLE', 43],
    ['X_V_MODEL_ON_PROPS', 44],
    ['X_V_MODEL_ON_CONST', 45],
    ['X_INVALID_EXPRESSION', 46],
    ['X_KEEP_ALIVE_INVALID_CHILDREN', 47],
    ['X_PREFIX_ID_NOT_SUPPORTED', 48],
    ['X_MODULE_MODE_NOT_SUPPORTED', 49],
    ['X_CACHE_HANDLER_NOT_SUPPORTED', 50],
    ['X_SCOPE_ID_NOT_SUPPORTED', 51],
    ['X_VNODE_HOOKS', 52],
    ['X_V_BIND_INVALID_SAME_NAME_ARGUMENT', 53],
    ['__EXTEND_POINT__', 54],
  ]);
  const errorMessages = Object.fromEntries(
    Object.keys(ErrorCodes)
      .filter(key => /^\d+$/.test(key))
      .map(key => [Number(key), String(ErrorCodes[key] || '')])
  );
  errorMessages[23] = 'Invalid end tag.';
  errorMessages[25] = 'Interpolation end sign was not found.';
  errorMessages[46] = 'Error parsing JavaScript expression: ';
  errorMessages[50] = '"cacheHandlers" option is only supported when the "prefixIdentifiers" option is enabled.';
  errorMessages[51] = '"scopeId" option is only supported in module mode.';

  const locStub = {
    start: { line: 1, column: 1, offset: 0 },
    end: { line: 1, column: 1, offset: 0 },
    source: '',
  };

  const helperNames = [
    ['FRAGMENT', 'Fragment'],
    ['TELEPORT', 'Teleport'],
    ['SUSPENSE', 'Suspense'],
    ['KEEP_ALIVE', 'KeepAlive'],
    ['BASE_TRANSITION', 'BaseTransition'],
    ['OPEN_BLOCK', 'openBlock'],
    ['CREATE_BLOCK', 'createBlock'],
    ['CREATE_ELEMENT_BLOCK', 'createElementBlock'],
    ['CREATE_VNODE', 'createVNode'],
    ['CREATE_ELEMENT_VNODE', 'createElementVNode'],
    ['CREATE_COMMENT', 'createCommentVNode'],
    ['CREATE_TEXT', 'createTextVNode'],
    ['CREATE_STATIC', 'createStaticVNode'],
    ['RESOLVE_COMPONENT', 'resolveComponent'],
    ['RESOLVE_DYNAMIC_COMPONENT', 'resolveDynamicComponent'],
    ['RESOLVE_DIRECTIVE', 'resolveDirective'],
    ['RESOLVE_FILTER', 'resolveFilter'],
    ['WITH_DIRECTIVES', 'withDirectives'],
    ['RENDER_LIST', 'renderList'],
    ['RENDER_SLOT', 'renderSlot'],
    ['CREATE_SLOTS', 'createSlots'],
    ['TO_DISPLAY_STRING', 'toDisplayString'],
    ['MERGE_PROPS', 'mergeProps'],
    ['NORMALIZE_CLASS', 'normalizeClass'],
    ['NORMALIZE_STYLE', 'normalizeStyle'],
    ['NORMALIZE_PROPS', 'normalizeProps'],
    ['GUARD_REACTIVE_PROPS', 'guardReactiveProps'],
    ['TO_HANDLERS', 'toHandlers'],
    ['CAMELIZE', 'camelize'],
    ['CAPITALIZE', 'capitalize'],
    ['TO_HANDLER_KEY', 'toHandlerKey'],
    ['SET_BLOCK_TRACKING', 'setBlockTracking'],
    ['PUSH_SCOPE_ID', 'pushScopeId'],
    ['POP_SCOPE_ID', 'popScopeId'],
    ['WITH_CTX', 'withCtx'],
    ['UNREF', 'unref'],
    ['IS_REF', 'isRef'],
    ['WITH_MEMO', 'withMemo'],
    ['IS_MEMO_SAME', 'isMemoSame'],
  ];
  const runtime = {
    NodeTypes,
    ElementTypes,
    ConstantTypes,
    Namespaces,
    ErrorCodes,
    BindingTypes: {
      DATA: 'data',
      PROPS: 'props',
      PROPS_ALIASED: 'props-aliased',
      SETUP_LET: 'setup-let',
      SETUP_CONST: 'setup-const',
      SETUP_REACTIVE_CONST: 'setup-reactive-const',
      SETUP_MAYBE_REF: 'setup-maybe-ref',
      SETUP_REF: 'setup-ref',
      OPTIONS: 'options',
      LITERAL_CONST: 'literal-const',
    },
    CompilerDeprecationTypes: {
      COMPILER_IS_ON_ELEMENT: 'COMPILER_IS_ON_ELEMENT',
      COMPILER_V_BIND_SYNC: 'COMPILER_V_BIND_SYNC',
      COMPILER_V_BIND_OBJECT_ORDER: 'COMPILER_V_BIND_OBJECT_ORDER',
      COMPILER_V_ON_NATIVE: 'COMPILER_V_ON_NATIVE',
      COMPILER_V_IF_V_FOR_PRECEDENCE: 'COMPILER_V_IF_V_FOR_PRECEDENCE',
      COMPILER_NATIVE_TEMPLATE: 'COMPILER_NATIVE_TEMPLATE',
      COMPILER_INLINE_TEMPLATE: 'COMPILER_INLINE_TEMPLATE',
      COMPILER_FILTERS: 'COMPILER_FILTERS',
    },
    TS_NODE_TYPES: [
      'TSAsExpression',
      'TSTypeAssertion',
      'TSNonNullExpression',
      'TSInstantiationExpression',
      'TSSatisfiesExpression',
    ],
    locStub,
    errorMessages,
    helperNameMap: {},
    forAliasRE: /([\s\S]*?)\s+(?:in|of)\s+(\S[\s\S]*)/,
    validFirstIdentCharRE: /[A-Za-z_$]/,
  };
  for (const [key, name] of helperNames) {
    const symbol = Symbol(name);
    runtime[key] = symbol;
    runtime.helperNameMap[symbol] = name;
  }

  runtime.advancePositionWithClone = function advancePositionWithClone(pos, source, numberOfCharacters) {
    return runtime.advancePositionWithMutation(
      { offset: pos.offset, line: pos.line, column: pos.column },
      source,
      numberOfCharacters === undefined ? String(source).length : numberOfCharacters,
    );
  };
  runtime.advancePositionWithMutation = function advancePositionWithMutation(pos, source, numberOfCharacters) {
    source = String(source || '');
    const count = numberOfCharacters === undefined ? source.length : numberOfCharacters;
    let linesCount = 0;
    let lastNewLinePos = -1;
    for (let i = 0; i < count; i++) {
      if (source.charCodeAt(i) === 10) {
        linesCount++;
        lastNewLinePos = i;
      }
    }
    pos.offset += count;
    pos.line += linesCount;
    pos.column = lastNewLinePos === -1 ? pos.column + count : count - lastNewLinePos;
    return pos;
  };
  runtime.assert = function assert(condition, msg) {
    if (!condition) throw new Error(msg || 'unexpected compiler condition');
  };
  runtime.createRoot = function createRoot(children, source = '') {
    return hydrateVue3Ast({
      type: NodeTypes.ROOT,
      source,
      children,
      helpers: [],
      components: [],
      directives: [],
      hoists: [],
      imports: [],
      cached: [],
      temps: 0,
      codegenNode: null,
      loc: locStub,
    });
  };
  runtime.createSimpleExpression = function createSimpleExpression(content, isStatic = false, loc = locStub, constType = ConstantTypes.NOT_CONSTANT) {
    return {
      type: NodeTypes.SIMPLE_EXPRESSION,
      loc,
      content,
      isStatic,
      constType: isStatic ? ConstantTypes.CAN_STRINGIFY : constType,
    };
  };
  runtime.createInterpolation = function createInterpolation(content, loc) {
    return {
      type: NodeTypes.INTERPOLATION,
      loc,
      content: typeof content === 'string' ? runtime.createSimpleExpression(content, false, loc) : content,
    };
  };
  runtime.createCompoundExpression = function createCompoundExpression(children, loc = locStub) {
    return { type: NodeTypes.COMPOUND_EXPRESSION, loc, children };
  };
  runtime.createArrayExpression = function createArrayExpression(elements, loc = locStub) {
    return { type: NodeTypes.JS_ARRAY_EXPRESSION, loc, elements };
  };
  runtime.createObjectExpression = function createObjectExpression(properties, loc = locStub) {
    return { type: NodeTypes.JS_OBJECT_EXPRESSION, loc, properties };
  };
  runtime.createObjectProperty = function createObjectProperty(key, value) {
    return {
      type: NodeTypes.JS_PROPERTY,
      loc: locStub,
      key: typeof key === 'string' ? runtime.createSimpleExpression(key, true) : key,
      value,
    };
  };
  runtime.createCallExpression = function createCallExpression(callee, args = [], loc = locStub) {
    return { type: NodeTypes.JS_CALL_EXPRESSION, loc, callee, arguments: args };
  };
  runtime.createFunctionExpression = function createFunctionExpression(params, returns = undefined, newline = false, isSlot = false, loc = locStub) {
    return { type: NodeTypes.JS_FUNCTION_EXPRESSION, params, returns, newline, isSlot, loc };
  };
  runtime.createConditionalExpression = function createConditionalExpression(test, consequent, alternate, newline = true) {
    return { type: NodeTypes.JS_CONDITIONAL_EXPRESSION, test, consequent, alternate, newline, loc: locStub };
  };
  runtime.createCacheExpression = function createCacheExpression(index, value, needPauseTracking = false, inVOnce = false) {
    return { type: NodeTypes.JS_CACHE_EXPRESSION, index, value, needPauseTracking, inVOnce, needArraySpread: false, loc: locStub };
  };
  runtime.createBlockStatement = function createBlockStatement(body) {
    return { type: NodeTypes.JS_BLOCK_STATEMENT, body, loc: locStub };
  };
  runtime.createTemplateLiteral = function createTemplateLiteral(elements) {
    return { type: NodeTypes.JS_TEMPLATE_LITERAL, elements, loc: locStub };
  };
  runtime.createIfStatement = function createIfStatement(test, consequent, alternate) {
    return { type: NodeTypes.JS_IF_STATEMENT, test, consequent, alternate, loc: locStub };
  };
  runtime.createAssignmentExpression = function createAssignmentExpression(left, right) {
    return { type: NodeTypes.JS_ASSIGNMENT_EXPRESSION, left, right, loc: locStub };
  };
  runtime.createSequenceExpression = function createSequenceExpression(expressions) {
    return { type: NodeTypes.JS_SEQUENCE_EXPRESSION, expressions, loc: locStub };
  };
  runtime.createReturnStatement = function createReturnStatement(returns) {
    return { type: NodeTypes.JS_RETURN_STATEMENT, returns, loc: locStub };
  };
  runtime.createVNodeCall = function createVNodeCall(context, tag, props, children, patchFlag, dynamicProps, directives, isBlock = false, disableTracking = false, isComponent = false, loc = locStub) {
    if (context) {
      if (isBlock) {
        context.helper(runtime.OPEN_BLOCK);
        context.helper(runtime.getVNodeBlockHelper(context.inSSR, isComponent));
      } else {
        context.helper(runtime.getVNodeHelper(context.inSSR, isComponent));
      }
      if (directives) context.helper(runtime.WITH_DIRECTIVES);
    }
    return {
      type: NodeTypes.VNODE_CALL,
      tag,
      props,
      children,
      patchFlag,
      dynamicProps,
      directives,
      isBlock,
      disableTracking,
      isComponent,
      loc,
    };
  };
  runtime.getVNodeHelper = function getVNodeHelper(ssr, isComponent) {
    return ssr || isComponent ? runtime.CREATE_VNODE : runtime.CREATE_ELEMENT_VNODE;
  };
  runtime.getVNodeBlockHelper = function getVNodeBlockHelper(ssr, isComponent) {
    return ssr || isComponent ? runtime.CREATE_BLOCK : runtime.CREATE_ELEMENT_BLOCK;
  };
  runtime.convertToBlock = function convertToBlock(node, context) {
    if (!node.isBlock) {
      node.isBlock = true;
      context.removeHelper(runtime.getVNodeHelper(context.inSSR, node.isComponent));
      context.helper(runtime.OPEN_BLOCK);
      context.helper(runtime.getVNodeBlockHelper(context.inSSR, node.isComponent));
    }
  };
  runtime.createCompilerError = function createCompilerError(code, loc, messages, additionalMessage) {
    const error = new SyntaxError(String((messages || errorMessages)[code] || '') + (additionalMessage || ''));
    error.code = code;
    error.loc = loc;
    return error;
  };
  runtime.registerRuntimeHelpers = function registerRuntimeHelpers(helpers) {
    Object.getOwnPropertySymbols(helpers).forEach(symbol => {
      runtime.helperNameMap[symbol] = helpers[symbol];
    });
  };
  runtime.stringifyExpression = function stringifyExpression(exp) {
    return typeof exp === 'string' ? exp : exp && exp.type === NodeTypes.SIMPLE_EXPRESSION ? exp.content : exp && exp.loc ? exp.loc.source : '';
  };
  runtime.isStaticExp = function isStaticExp(p) {
    return !!(p && p.type === NodeTypes.SIMPLE_EXPRESSION && p.isStatic);
  };
  runtime.isText = function isText$1(node) {
    return !!node && (node.type === NodeTypes.INTERPOLATION || node.type === NodeTypes.TEXT);
  };
  runtime.isAllWhitespace = function isAllWhitespace(str) {
    return /^[\t\r\n\f ]*$/.test(String(str || ''));
  };
  runtime.isWhitespaceText = function isWhitespaceText(node) {
    return !!node && ((node.type === NodeTypes.TEXT && runtime.isAllWhitespace(node.content)) || (node.type === NodeTypes.TEXT_CALL && runtime.isWhitespaceText(node.content)));
  };
  runtime.isCommentOrWhitespace = function isCommentOrWhitespace(node) {
    return !!node && (node.type === NodeTypes.COMMENT || runtime.isWhitespaceText(node));
  };
  runtime.findDir = function findDir(node, name, allowEmpty = false) {
    const matches = typeof name === 'string' ? n => n === name : n => name.test(n);
    return node.props && node.props.find(p => p.type === NodeTypes.DIRECTIVE && (allowEmpty || p.exp) && matches(p.name));
  };
  runtime.findProp = function findProp(node, name, dynamicOnly = false, allowEmpty = false) {
    if (!node.props) return undefined;
    for (const p of node.props) {
      if (p.type === NodeTypes.ATTRIBUTE) {
        if (!dynamicOnly && p.name === name && (p.value || allowEmpty)) return p;
      } else if (p.name === 'bind' && (p.exp || allowEmpty) && runtime.isStaticArgOf(p.arg, name)) {
        return p;
      }
    }
    return undefined;
  };
  runtime.isStaticArgOf = function isStaticArgOf(arg, name) {
    return !!(arg && runtime.isStaticExp(arg) && arg.content === name);
  };
  runtime.hasDynamicKeyVBind = function hasDynamicKeyVBind(node) {
    return !!(node.props && node.props.some(p => p.type === NodeTypes.DIRECTIVE && p.name === 'bind' && (!p.arg || p.arg.type !== NodeTypes.SIMPLE_EXPRESSION || !p.arg.isStatic)));
  };
  runtime.isVPre = function isVPre(p) { return !!p && p.type === NodeTypes.DIRECTIVE && p.name === 'pre'; };
  runtime.isVSlot = function isVSlot(p) { return !!p && p.type === NodeTypes.DIRECTIVE && p.name === 'slot'; };
  runtime.isTemplateNode = function isTemplateNode(node) { return !!node && node.type === NodeTypes.ELEMENT && node.tagType === ElementTypes.TEMPLATE; };
  runtime.isSlotOutlet = function isSlotOutlet(node) { return !!node && node.type === NodeTypes.ELEMENT && node.tagType === ElementTypes.SLOT; };
  runtime.toValidAssetId = function toValidAssetId(name, type) {
    return `_${type}_${String(name).replace(/[^\w]/g, (searchValue, replaceValue) => searchValue === '-' ? '_' : String(name).charCodeAt(replaceValue).toString())}`;
  };
  runtime.injectProp = function injectProp(node, prop) {
    const target = node.type === NodeTypes.VNODE_CALL ? node : { props: node.arguments && node.arguments[2] };
    if (!target.props || typeof target.props === 'string') {
      target.props = runtime.createObjectExpression([prop]);
    } else if (target.props.type === NodeTypes.JS_OBJECT_EXPRESSION) {
      target.props.properties.unshift(prop);
    }
    if (node.type !== NodeTypes.VNODE_CALL && node.arguments) node.arguments[2] = target.props;
  };
  runtime.hasScopeRef = function hasScopeRef() { return false; };
  runtime.getMemoedVNodeCall = function getMemoedVNodeCall(node) {
    return node && node.type === NodeTypes.JS_CALL_EXPRESSION && node.callee === runtime.WITH_MEMO ? node.arguments[1].returns : node;
  };
  runtime.isCoreComponent = function isCoreComponent(tag) {
    return tag === 'Teleport' ? runtime.TELEPORT
      : tag === 'Suspense' ? runtime.SUSPENSE
      : tag === 'KeepAlive' ? runtime.KEEP_ALIVE
      : tag === 'BaseTransition' ? runtime.BASE_TRANSITION
      : undefined;
  };
  runtime.isSimpleIdentifier = function isSimpleIdentifier(name) {
    return /^[A-Za-z_$][\w$]*$/.test(String(name || ''));
  };
  runtime.isGloballyAllowed = function isGloballyAllowed(name) {
    return new Set([
      'Infinity', 'NaN', 'undefined', 'parseInt', 'parseFloat', 'isNaN', 'isFinite',
      'decodeURI', 'decodeURIComponent', 'encodeURI', 'encodeURIComponent',
      'Math', 'Number', 'Date', 'Array', 'Object', 'Boolean', 'String', 'RegExp',
      'Map', 'Set', 'WeakMap', 'WeakSet', 'JSON', 'Intl', 'BigInt', 'console',
      'Error', 'TypeError', 'Symbol', 'Promise', 'Reflect', 'globalThis',
    ]).has(String(name || ''));
  };
  runtime.getBabelParser = function getBabelParser() {
    if (runtime._babelParser !== undefined) return runtime._babelParser;
    try {
      runtime._babelParser = require('@babel/parser');
    } catch (_error) {
      try {
        runtime._babelParser = process.env.VUEC_OFFICIAL_NPM_ROOT
          ? require(path.join(process.env.VUEC_OFFICIAL_NPM_ROOT, 'node_modules/@babel/parser'))
          : null;
      } catch (_fallbackError) {
        runtime._babelParser = null;
      }
    }
    return runtime._babelParser;
  };
  runtime.isMemberExpressionBrowser = function isMemberExpressionBrowser(path) {
    return /^[A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*|\[[^\]]+\])*$/.test(String(path || '').trim());
  };
  runtime.isMemberExpressionNode = function isMemberExpressionNode(path) {
    return runtime.isMemberExpressionBrowser(path);
  };
  runtime.isMemberExpression = runtime.isMemberExpressionNode;
  runtime.isFnExpressionBrowser = function isFnExpressionBrowser(exp) {
    const content = typeof exp === 'string' ? exp : exp && exp.content;
    return /^\s*(?:async\s*)?(?:\([^)]*\)|[A-Za-z_$][\w$]*)\s*=>/.test(String(content || '')) || /^\s*function\b/.test(String(content || ''));
  };
  runtime.isFnExpressionNode = function isFnExpressionNode(exp) { return runtime.isFnExpressionBrowser(exp); };
  runtime.isFnExpression = runtime.isFnExpressionNode;
  runtime.isFunctionType = function isFunctionType(node) { return !!node && /Function/.test(String(node.type || '')); };
  runtime.isStaticProperty = function isStaticProperty(node) { return !!node && node.type === 'Property' && !node.computed; };
  runtime.isStaticPropertyKey = function isStaticPropertyKey(node, parent) { return !!parent && runtime.isStaticProperty(parent) && parent.key === node; };
  runtime.unwrapTSNode = function unwrapTSNode(node) {
    while (node && runtime.TS_NODE_TYPES.includes(node.type)) node = node.expression;
    return node;
  };
  runtime.isReferencedIdentifier = function isReferencedIdentifier() { return true; };
  runtime.isInDestructureAssignment = function isInDestructureAssignment() { return false; };
  runtime.isInNewExpression = function isInNewExpression() { return false; };
  runtime.walkIdentifiers = function walkIdentifiers(root, onIdentifier) {
    if (root && root.type === NodeTypes.SIMPLE_EXPRESSION && runtime.isSimpleIdentifier(root.content)) onIdentifier(root);
  };
  runtime.extractIdentifiers = function extractIdentifiers(param) {
    if (!param) return [];
    if (typeof param === 'string') return param.split(',').map(s => s.trim()).filter(Boolean).map(content => runtime.createSimpleExpression(content, false));
    if (param.type === NodeTypes.SIMPLE_EXPRESSION) return [param];
    return [];
  };
  runtime.walkFunctionParams = function walkFunctionParams(node, onIdent) {
    for (const ident of runtime.extractIdentifiers(node && node.params)) onIdent(ident);
  };
  runtime.walkBlockDeclarations = function walkBlockDeclarations() {};
  runtime.createTransformContext = function createTransformContext(root, options = {}) {
    const context = {
      filename: options.filename || '',
      selfName: options.filename ? selfNameFromFilename(options.filename) : null,
      prefixIdentifiers: !!options.prefixIdentifiers,
      hoistStatic: !!options.hoistStatic,
      hmr: !!options.hmr,
      cacheHandlers: !!options.cacheHandlers,
      nodeTransforms: options.nodeTransforms || [],
      directiveTransforms: options.directiveTransforms || {},
      transformHoist: options.transformHoist || null,
      isBuiltInComponent: options.isBuiltInComponent || (() => false),
      isCustomElement: options.isCustomElement || (() => false),
      expressionPlugins: options.expressionPlugins || [],
      scopeId: options.scopeId || null,
      slotted: options.slotted !== undefined ? options.slotted : true,
      ssr: !!options.ssr,
      inSSR: !!(options.inSSR || options.ssr),
      ssrCssVars: options.ssrCssVars || '',
      bindingMetadata: options.bindingMetadata || {},
      inline: !!options.inline,
      isTS: !!options.isTS,
      onError: options.onError || (error => { throw error; }),
      onWarn: options.onWarn || (() => {}),
      compatConfig: options.compatConfig,
      root,
      helpers: new Map(),
      components: new Set(),
      directives: new Set(),
      hoists: [],
      imports: [],
      cached: [],
      constantCache: new WeakMap(),
      temps: 0,
      identifiers: Object.create(null),
      scopes: { vFor: 0, vSlot: 0, vPre: 0, vOnce: 0 },
      parent: null,
      grandParent: null,
      currentNode: root,
      childIndex: 0,
      inVOnce: false,
      helper(name) {
        context.helpers.set(name, (context.helpers.get(name) || 0) + 1);
        return name;
      },
      removeHelper(name) {
        const count = context.helpers.get(name);
        if (count === 1) context.helpers.delete(name);
        else if (count) context.helpers.set(name, count - 1);
      },
      helperString(name) {
        return `_${runtime.helperNameMap[context.helper(name)]}`;
      },
      replaceNode(node) {
        if (!context.currentNode) throw new Error('Node being replaced is already removed.');
        if (!context.parent) throw new Error('Cannot replace root node.');
        context.parent.children[context.childIndex] = context.currentNode = node;
      },
      removeNode(node) {
        if (!context.parent) throw new Error('Cannot remove root node.');
        const list = context.parent.children;
        const removalIndex = node ? list.indexOf(node) : context.currentNode ? context.childIndex : -1;
        if (removalIndex < 0) throw new Error('node being removed is not a child of current parent');
        if (!node || node === context.currentNode) {
          context.currentNode = null;
          context.onNodeRemoved();
        } else if (context.childIndex > removalIndex) {
          context.childIndex--;
          context.onNodeRemoved();
        }
        list.splice(removalIndex, 1);
      },
      onNodeRemoved() {},
      addIdentifiers() {},
      removeIdentifiers() {},
      hoist(exp) {
        if (typeof exp === 'string') exp = runtime.createSimpleExpression(exp);
        context.hoists.push(exp);
        const identifier = runtime.createSimpleExpression(`_hoisted_${context.hoists.length}`, false, exp.loc, ConstantTypes.CAN_CACHE);
        identifier.hoisted = exp;
        return identifier;
      },
      cache(exp, isVNode = false, inVOnce = false) {
        const cacheExp = runtime.createCacheExpression(context.cached.length, exp, isVNode, inVOnce);
        context.cached.push(cacheExp);
        return cacheExp;
      },
      filters: new Set(),
    };
    return context;
  };
  runtime.traverseNode = function traverseNode(node, context) {
    context.currentNode = node;
    const exitFns = [];
    for (const transform of context.nodeTransforms || []) {
      const onExit = transform(node, context);
      if (Array.isArray(onExit)) exitFns.push(...onExit);
      else if (onExit) exitFns.push(onExit);
      if (!context.currentNode) return;
      node = context.currentNode;
    }
    switch (node.type) {
      case NodeTypes.COMMENT:
        if (!context.ssr) context.helper(runtime.CREATE_COMMENT);
        break;
      case NodeTypes.INTERPOLATION:
        if (!context.ssr) context.helper(runtime.TO_DISPLAY_STRING);
        break;
      case NodeTypes.IF:
        for (const branch of node.branches || []) runtime.traverseNode(branch, context);
        break;
      case NodeTypes.IF_BRANCH:
      case NodeTypes.FOR:
      case NodeTypes.ELEMENT:
      case NodeTypes.ROOT:
        runtime.traverseChildren(node, context);
        break;
    }
    context.currentNode = node;
    for (let i = exitFns.length - 1; i >= 0; i--) exitFns[i]();
  };
  runtime.traverseChildren = function traverseChildren(parent, context) {
    let i = 0;
    const nodeRemoved = () => { i--; };
    for (; i < parent.children.length; i++) {
      const child = parent.children[i];
      if (typeof child === 'string') continue;
      context.grandParent = context.parent;
      context.parent = parent;
      context.childIndex = i;
      context.onNodeRemoved = nodeRemoved;
      runtime.traverseNode(child, context);
    }
  };
  runtime.transform = function transform(root, options = {}) {
    const context = runtime.createTransformContext(root, options);
    runtime.traverseNode(root, context);
    if (!options.ssr) createRootCodegen(root, context);
    root.helpers = new Set([...context.helpers.keys()]);
    root.components = [...context.components];
    root.directives = [...context.directives];
    root.imports = context.imports;
    root.hoists = context.hoists;
    root.temps = context.temps;
    root.cached = context.cached;
    root.transformed = true;
    root.filters = [...context.filters];
  };
  runtime.generate = function generate(ast, options = {}) {
    ast = hydrateVue3Ast(ast);
    const mode = options.mode || 'function';
    const prefixIdentifiers = options.prefixIdentifiers !== undefined ? options.prefixIdentifiers : mode === 'module';
    const ssr = !!options.ssr;
    const helpers = Array.from(ast.helpers || []);
    const useWithBlock = !prefixIdentifiers && mode !== 'module';
    const runtimeModuleName = options.runtimeModuleName || 'vue';
    const runtimeGlobalName = options.runtimeGlobalName || 'Vue';
    const ssrRuntimeModuleName = options.ssrRuntimeModuleName || 'vue/server-renderer';
    let code = '';
    let indentLevel = 0;
    let preamble = '';
    const push = value => { code += String(value); };
    const currentIndent = () => '  '.repeat(indentLevel);
    const newline = () => { code += `\n${currentIndent()}`; };
    const indent = () => { indentLevel++; newline(); };
    const deindent = (withoutNewline = false) => {
      indentLevel = Math.max(0, indentLevel - 1);
      if (!withoutNewline) newline();
    };
    const helperAlias = (symbol, asImport = false) => {
      const name = helperName(symbol);
      return asImport ? `${name} as _${name}` : `${name}: _${name}`;
    };

    if (mode === 'module') {
      if (helpers.length) {
        if (options.optimizeImports) {
          push(`import { ${helpers.map(helperName).join(', ')} } from ${JSON.stringify(runtimeModuleName)}`);
          newline();
          newline();
          push(`// Binding optimization for webpack code-split`);
          newline();
          push(`const ${helpers.map(s => `_${helperName(s)} = ${helperName(s)}`).join(', ')}`);
          newline();
        } else {
          push(`import { ${helpers.map(s => helperAlias(s, true)).join(', ')} } from ${JSON.stringify(runtimeModuleName)}`);
          newline();
        }
      }
      if (ast.ssrHelpers && ast.ssrHelpers.length) {
        push(`import { ${ast.ssrHelpers.map(s => helperAlias(s, true)).join(', ')} } from ${JSON.stringify(ssrRuntimeModuleName)}`);
        newline();
      }
      genHoists(ast.hoists || []);
      if (!code) push(`\n`);
      else {
        if (!code.endsWith('\n')) newline();
        if (!code.endsWith('\n\n')) newline();
      }
      push(`export `);
    } else {
      const vueBinding = ssr ? `require(${JSON.stringify(runtimeModuleName)})` : runtimeGlobalName;
      if (helpers.length) {
        if (prefixIdentifiers) {
          push(`const { ${helpers.map(s => helperAlias(s)).join(', ')} } = ${vueBinding}`);
          newline();
        } else {
          push(`const _Vue = ${vueBinding}`);
          newline();
          if ((ast.hoists || []).length) {
            const staticHelpers = [runtime.CREATE_VNODE, runtime.CREATE_ELEMENT_VNODE, runtime.CREATE_COMMENT, runtime.CREATE_TEXT, runtime.CREATE_STATIC]
              .filter(symbol => helpers.includes(symbol))
              .map(s => helperAlias(s))
              .join(', ');
            if (staticHelpers) {
              push(`const { ${staticHelpers} } = _Vue`);
              newline();
            }
          }
        }
      }
      if (ast.ssrHelpers && ast.ssrHelpers.length) {
        push(`const { ${ast.ssrHelpers.map(s => helperAlias(s)).join(', ')} } = require(${JSON.stringify(ssrRuntimeModuleName)})`);
        newline();
      }
      if (!code && (ast.hoists || []).length) push(`\n`);
      genHoists(ast.hoists || []);
      if (!code) push(`\n`);
      else newline();
      push(`return `);
    }

    const functionName = ssr ? 'ssrRender' : 'render';
    const args = ssr ? ['_ctx', '_push', '_parent', '_attrs'] : ['_ctx', '_cache'];
    if (options.bindingMetadata && !options.inline) args.push('$props', '$setup', '$data', '$options');
    push(`function ${functionName}(${args.join(', ')}) {`);
    indent();

    if (useWithBlock) {
      push(`with (_ctx) {`);
      indent();
      if (helpers.length) {
        push(`const { ${helpers.map(s => helperAlias(s)).join(', ')} } = _Vue`);
        push(`\n\n${currentIndent()}`);
      }
    }

    genAssets(ast.components || [], 'component');
    if ((ast.components || []).length && ((ast.directives || []).length || ast.temps > 0)) {
      newline();
    }
    genAssets(ast.directives || [], 'directive');
    if ((ast.directives || []).length && ast.temps > 0) {
      newline();
    }
    if (ast.temps > 0) {
      push(`let ${Array.from({ length: ast.temps }, (_, i) => `_temp${i}`).join(', ')}`);
    }
    if ((ast.components || []).length || (ast.directives || []).length || ast.temps > 0) {
      push(`\n\n${currentIndent()}`);
    }

    if (!ssr) push(`return `);
    genNode(ast.codegenNode || null);

    if (useWithBlock) {
      deindent();
      push(`}`);
    }
    deindent();
    push(`}`);
    return { ast, code, preamble, map: undefined };

    function genNode(node) {
      if (node == null) {
        push('null');
        return;
      }
      if (typeof node === 'string') {
        push(node);
        return;
      }
      if (typeof node === 'symbol') {
        push(helper(node));
        return;
      }
      if (Array.isArray(node)) {
        genNodeListAsArray(node);
        return;
      }
      switch (node.type) {
        case NodeTypes.ELEMENT:
        case NodeTypes.IF:
        case NodeTypes.FOR:
          if (node.codegenNode) genNode(node.codegenNode);
          else push('null');
          break;
        case NodeTypes.TEXT:
          push(JSON.stringify(node.content));
          break;
        case NodeTypes.COMMENT:
          push(`${helper(runtime.CREATE_COMMENT)}(${JSON.stringify(node.content)})`);
          break;
        case NodeTypes.SIMPLE_EXPRESSION:
          push(node.isStatic ? JSON.stringify(node.content) : node.content);
          break;
        case NodeTypes.INTERPOLATION:
          push(`${helper(runtime.TO_DISPLAY_STRING)}(`);
          genNode(node.content);
          push(`)`);
          break;
        case NodeTypes.COMPOUND_EXPRESSION:
          for (const child of node.children || []) genNode(child);
          break;
        case NodeTypes.TEXT_CALL:
          genNode(node.codegenNode);
          break;
        case NodeTypes.VNODE_CALL:
          genVNodeCall(node);
          break;
        case NodeTypes.JS_CALL_EXPRESSION:
          genCallExpression(node);
          break;
        case NodeTypes.JS_OBJECT_EXPRESSION:
          genObjectExpression(node);
          break;
        case NodeTypes.JS_ARRAY_EXPRESSION:
          genArrayExpression(node);
          break;
        case NodeTypes.JS_FUNCTION_EXPRESSION:
          genFunctionExpression(node);
          break;
        case NodeTypes.JS_CONDITIONAL_EXPRESSION:
          genConditionalExpression(node);
          break;
        case NodeTypes.JS_CACHE_EXPRESSION:
          genCacheExpression(node);
          break;
        case NodeTypes.JS_BLOCK_STATEMENT:
          genNodeList(node.body || [], true, false);
          break;
        case NodeTypes.JS_TEMPLATE_LITERAL:
          genTemplateLiteral(node);
          break;
        case NodeTypes.JS_IF_STATEMENT:
          genIfStatement(node);
          break;
        case NodeTypes.JS_ASSIGNMENT_EXPRESSION:
          genNode(node.left);
          push(` = `);
          genNode(node.right);
          break;
        case NodeTypes.JS_SEQUENCE_EXPRESSION:
          push(`(`);
          genNodeList(node.expressions || [], false, true);
          push(`)`);
          break;
        case NodeTypes.JS_RETURN_STATEMENT:
          push(`return `);
          Array.isArray(node.returns) ? genNodeListAsArray(node.returns) : genNode(node.returns);
          break;
        default:
          push('null');
      }
    }

    function genNodeToString(node) {
      const previous = code;
      code = '';
      genNode(node);
      const out = code;
      code = previous;
      return out;
    }

    function genNodeList(nodes, multilines = false, comma = true) {
      nodes = nodes || [];
      for (let i = 0; i < nodes.length; i++) {
        genNode(nodes[i]);
        if (i < nodes.length - 1) {
          if (multilines) {
            if (comma) push(',');
            newline();
          } else if (comma) {
            push(', ');
          }
        }
      }
    }

    function genNodeListAsArray(nodes) {
      nodes = nodes || [];
      const multilines = nodes.length > 3 || nodes.some(n => Array.isArray(n) || !isTextLike(n));
      push(`[`);
      if (multilines) indent();
      genNodeList(nodes, multilines, true);
      if (multilines) deindent();
      push(`]`);
    }

    function genVNodeCall(node) {
      const call = node.isBlock ? helper(runtime.getVNodeBlockHelper(false, node.isComponent)) : helper(runtime.getVNodeHelper(false, node.isComponent));
      const args = genNullableArgs([node.tag, node.props, node.children, patchFlagText(node.patchFlag), node.dynamicProps]);
      if (node.directives) push(`${helper(runtime.WITH_DIRECTIVES)}(`);
      if (node.isBlock) push(`(${helper(runtime.OPEN_BLOCK)}(${node.disableTracking ? 'true' : ''}), `);
      push(`${call}(`);
      genNodeList(args, false, true);
      push(`)`);
      if (node.isBlock) push(`)`);
      if (node.directives) {
        push(`, `);
        genNode(node.directives);
        push(`)`);
      }
    }

    function genNullableArgs(args) {
      let i = args.length;
      while (i--) {
        if (args[i] != null) break;
      }
      return args.slice(0, i + 1).map(arg => arg || 'null');
    }

    function genCallExpression(node) {
      const callee = typeof node.callee === 'symbol' ? helper(node.callee) : String(node.callee);
      push(`${callee}(`);
      genNodeList(node.arguments || [], false, true);
      push(`)`);
    }

    function genObjectExpression(node) {
      const properties = node.properties || [];
      if (!properties.length) {
        push(`{}`);
        return;
      }
      const multilines = properties.length > 1 || properties.some(prop => prop.value && prop.value.type !== NodeTypes.SIMPLE_EXPRESSION);
      push(multilines ? `{` : `{ `);
      if (multilines) indent();
      for (let i = 0; i < properties.length; i++) {
        genPropertyKey(properties[i].key);
        push(`: `);
        genNode(properties[i].value);
        if (i < properties.length - 1) {
          push(`,`);
          newline();
        }
      }
      if (multilines) deindent();
      push(multilines ? `}` : ` }`);
    }

    function genPropertyKey(key) {
      if (!key) {
        push('undefined');
      } else if (key.type === NodeTypes.COMPOUND_EXPRESSION) {
        push(`[`);
        genNode(key);
        push(`]`);
      } else if (key.type === NodeTypes.SIMPLE_EXPRESSION && key.isStatic) {
        push(runtime.isSimpleIdentifier(key.content) ? key.content : JSON.stringify(key.content));
      } else if (key.type === NodeTypes.SIMPLE_EXPRESSION) {
        push(`[${key.content}]`);
      } else {
        push(`[`);
        genNode(key);
        push(`]`);
      }
    }

    function genArrayExpression(node) {
      genNodeListAsArray(node.elements || []);
    }

    function genFunctionExpression(node) {
      if (node.isSlot) push(`${helper(runtime.WITH_CTX)}(`);
      push(`(`);
      if (Array.isArray(node.params)) genNodeList(node.params, false, true);
      else if (node.params) genNode(node.params);
      push(`) => `);
      if (node.newline || node.body) {
        push(`{`);
        indent();
      }
      if (node.returns) {
        if (node.newline) push(`return `);
        Array.isArray(node.returns) ? genNodeListAsArray(node.returns) : genNode(node.returns);
      } else if (node.body) {
        genNode(node.body);
      }
      if (node.newline || node.body) {
        deindent();
        push(`}`);
      }
      if (node.isSlot) push(`)`);
    }

    function genConditionalExpression(node) {
      const nested = node.alternate && node.alternate.type === NodeTypes.JS_CONDITIONAL_EXPRESSION;
      genNode(node.test);
      if (node.newline === false) {
        push(` ? `);
        genNode(node.consequent);
        push(` : `);
        genNode(node.alternate);
        return;
      }
      indentLevel++;
      newline();
      push(`? `);
      genNode(node.consequent);
      newline();
      push(`: `);
      if (!nested) indentLevel++;
      genNode(node.alternate);
      if (!nested) indentLevel--;
      indentLevel--;
    }

    function genCacheExpression(node) {
      if (node.needArraySpread) push(`[...(`);
      push(`_cache[${node.index}] || (`);
      if (node.needPauseTracking) {
        indent();
        push(`${helper(runtime.SET_BLOCK_TRACKING)}(-1${node.inVOnce ? ', true' : ''}),`);
        newline();
        push(`(_cache[${node.index}] = `);
        genNode(node.value);
        push(`).cacheIndex = ${node.index},`);
        newline();
        push(`${helper(runtime.SET_BLOCK_TRACKING)}(1),`);
        newline();
        push(`_cache[${node.index}]`);
        deindent();
      } else {
        push(`_cache[${node.index}] = `);
        genNode(node.value);
      }
      push(`)`);
      if (node.needArraySpread) push(`)]`);
    }

    function genTemplateLiteral(node) {
      push('`');
      for (const element of node.elements || []) {
        if (typeof element === 'string') {
          push(element.replace(/(`|\$|\\)/g, '\\$1'));
        } else {
          push('${');
          genNode(element);
          push('}');
        }
      }
      push('`');
    }

    function genIfStatement(node) {
      push(`if (`);
      genNode(node.test);
      push(`) {`);
      indent();
      genNode(node.consequent);
      deindent();
      push(`}`);
      if (node.alternate) {
        push(` else `);
        if (node.alternate.type === NodeTypes.JS_IF_STATEMENT) {
          genIfStatement(node.alternate);
        } else {
          push(`{`);
          indent();
          genNode(node.alternate);
          deindent();
          push(`}`);
        }
      }
    }

    function genAssets(assets, type) {
      if (!assets.length) return;
      const resolver = helper(type === 'component' ? runtime.RESOLVE_COMPONENT : runtime.RESOLVE_DIRECTIVE);
      for (let i = 0; i < assets.length; i++) {
        let id = assets[i];
        const maybeSelfReference = String(id).endsWith('__self');
        if (maybeSelfReference) id = id.slice(0, -6);
        push(`const ${runtime.toValidAssetId(id, type)} = ${resolver}(${JSON.stringify(id)}${maybeSelfReference ? ', true' : ''})`);
        if (i < assets.length - 1) newline();
      }
    }

    function genHoists(hoists) {
      if (!hoists.length) return;
      for (let i = 0; i < hoists.length; i++) {
        const exp = hoists[i];
        if (!exp) continue;
        push(`const _hoisted_${i + 1} = `);
        genNode(exp);
        newline();
      }
    }

    function patchFlagText(flag) {
      if (flag == null) return flag;
      if (typeof flag === 'string' && /\/\*/.test(flag)) return flag;
      const value = Number(flag);
      if (!Number.isFinite(value) || value === 0) return flag;
      const names = {
        1: 'TEXT', 2: 'CLASS', 4: 'STYLE', 8: 'PROPS', 16: 'FULL_PROPS',
        32: 'HYDRATE_EVENTS', 64: 'STABLE_FRAGMENT', 128: 'KEYED_FRAGMENT',
        256: 'UNKEYED_FRAGMENT', 512: 'NEED_PATCH', 1024: 'DYNAMIC_SLOTS',
        2048: 'DEV_ROOT_FRAGMENT', [-1]: 'HOISTED', [-2]: 'BAIL',
      };
      const text = value < 0 ? names[value] : Object.keys(names)
        .map(Number)
        .filter(n => n > 0 && (value & n))
        .map(n => names[n])
        .join(', ');
      return text ? `${value} /* ${text} */` : String(flag);
    }

    function isTextLike(node) {
      return typeof node === 'string'
        || (node && [NodeTypes.SIMPLE_EXPRESSION, NodeTypes.TEXT, NodeTypes.INTERPOLATION, NodeTypes.COMPOUND_EXPRESSION].includes(node.type));
    }

    function helper(symbol) {
      return `_${helperName(symbol)}`;
    }

    function helperName(symbol) {
      return runtime.helperNameMap[symbol] || String(symbol || '').replace(/^_/, '');
    }
  };
  runtime.createStructuralDirectiveTransform = function createStructuralDirectiveTransform(name, fn) {
    const matches = typeof name === 'string' ? n => n === name : n => name.test(n);
    return (node, context) => {
      if (node.type !== NodeTypes.ELEMENT) return;
      const exitFns = [];
      for (let i = 0; i < node.props.length; i++) {
        const prop = node.props[i];
        if (prop.type === NodeTypes.DIRECTIVE && matches(prop.name)) {
          node.props.splice(i, 1);
          i--;
          const onExit = fn(node, prop, context);
          if (onExit) exitFns.push(onExit);
        }
      }
      return exitFns;
    };
  };
  runtime.noopDirectiveTransform = () => ({ props: [] });
  runtime.processExpression = function processExpression(node, context, asParams = false, asRawStatements = false, localVars) {
    if (!node || node.type !== NodeTypes.SIMPLE_EXPRESSION) return node;
    const rawExp = String(node.content || '');
    if (!context || !context.prefixIdentifiers || !rawExp.trim()) return node;
    const identifiers = localVars || Object.create(context.identifiers || null);
    const bindingMetadata = context.bindingMetadata || {};
    const literalAllowed = raw => raw === 'true' || raw === 'false' || raw === 'null' || raw === 'this';

    const rewriteIdentifier = (raw, parent, id) => {
      const type = Object.prototype.hasOwnProperty.call(bindingMetadata, raw) && bindingMetadata[raw];
      if (context.inline) {
        const isAssignmentLVal = parent && parent.type === 'AssignmentExpression' && parent.left === id;
        const isUpdateArg = parent && parent.type === 'UpdateExpression' && parent.argument === id;
        if (type === runtime.BindingTypes.SETUP_REF) return `${raw}.value`;
        if (type === runtime.BindingTypes.SETUP_MAYBE_REF) {
          return isAssignmentLVal || isUpdateArg ? `${raw}.value` : `${context.helperString(runtime.UNREF)}(${raw})`;
        }
        if (type === runtime.BindingTypes.SETUP_CONST || type === runtime.BindingTypes.LITERAL_CONST || type === runtime.BindingTypes.SETUP_REACTIVE_CONST) return raw;
        if (type === runtime.BindingTypes.PROPS) return `__props.${raw}`;
        if (type === runtime.BindingTypes.PROPS_ALIASED) return `__props[${JSON.stringify((bindingMetadata.__propsAliases || {})[raw] || raw)}]`;
      } else if (type) {
        if (String(type).startsWith('setup') || type === runtime.BindingTypes.LITERAL_CONST) return `$setup.${raw}`;
        if (type === runtime.BindingTypes.PROPS_ALIASED) return `$props[${JSON.stringify((bindingMetadata.__propsAliases || {})[raw] || raw)}]`;
        return `$${type}.${raw}`;
      }
      return `_ctx.${raw}`;
    };

    if (runtime.isSimpleIdentifier(rawExp)) {
      const isLocal = identifiers[rawExp];
      const isGlobal = runtime.isGloballyAllowed(rawExp);
      const isLiteral = literalAllowed(rawExp);
      if (!asParams && !isLocal && !isLiteral && (!isGlobal || bindingMetadata[rawExp])) {
        node.content = rewriteIdentifier(rawExp);
        if (bindingMetadata[rawExp] === runtime.BindingTypes.SETUP_CONST || bindingMetadata[rawExp] === runtime.BindingTypes.LITERAL_CONST) {
          node.constType = ConstantTypes.CAN_SKIP_PATCH;
        }
      } else if (!isLocal) {
        node.constType = isLiteral ? ConstantTypes.CAN_STRINGIFY : ConstantTypes.CAN_CACHE;
      }
      return node;
    }

    const parser = runtime.getBabelParser();
    if (!parser) return processExpressionByLexer(node, context, rawExp, rewriteIdentifier, identifiers, literalAllowed);
    let ast;
    const source = asRawStatements ? ` ${rawExp} ` : `(${rawExp})${asParams ? `=>{}` : ``}`;
    try {
      ast = parser.parseExpression(source, {
        sourceType: 'module',
        plugins: context.expressionPlugins || [],
      });
      if (!asRawStatements && ast && ast.type === 'AssignmentExpression' && ast.left && ast.left.type === 'ObjectPattern') {
        ast.left.type = 'ObjectExpression';
      }
    } catch (error) {
      context.onError(runtime.createCompilerError(ErrorCodes.X_INVALID_EXPRESSION, node.loc, undefined, error && error.message ? error.message : String(error)));
      return node;
    }

    const ids = [];
    const knownIds = Object.create(identifiers || null);
    const mark = name => {
      if (!name) return;
      knownIds[name] = (knownIds[name] || 0) + 1;
    };
    const extractPatternIds = pattern => {
      const out = [];
      const visitPattern = p => {
        if (!p) return;
        if (p.type === 'Identifier') out.push(p);
        else if (p.type === 'RestElement') visitPattern(p.argument);
        else if (p.type === 'AssignmentPattern') visitPattern(p.left);
        else if (p.type === 'ArrayPattern') (p.elements || []).forEach(visitPattern);
        else if (p.type === 'ObjectPattern') {
          for (const prop of p.properties || []) {
            if (prop.type === 'RestElement') visitPattern(prop.argument);
            else visitPattern(prop.value || prop.argument || prop.key);
          }
        }
      };
      visitPattern(pattern);
      return out;
    };
    const collectParamDefaultExpressions = (param, locals) => {
      if (!param) return;
      if (param.type === 'AssignmentPattern') visit(param.right, param, locals);
      else if (param.type === 'ObjectPattern') {
        for (const prop of param.properties || []) {
          if (prop.type !== 'RestElement') collectParamDefaultExpressions(prop.value, locals);
        }
      } else if (param.type === 'ArrayPattern') {
        (param.elements || []).forEach(item => collectParamDefaultExpressions(item, locals));
      }
    };
    const addIdentifier = (id, replacement, prefix, isConstant) => {
      if (id.start == null || id.end == null) return;
      ids.push({ start: id.start - 1, end: id.end - 1, name: replacement || id.name, prefix: prefix || '', isConstant: !!isConstant });
    };
    const canPrefix = name => name !== 'require' && !runtime.isGloballyAllowed(name);
    const visit = (n, parent, locals) => {
      if (!n || typeof n.type !== 'string') return;
      switch (n.type) {
        case 'Identifier': {
          const role = identifierRole(n, parent);
          if (role.skip) return;
          const isLocal = !!locals[n.name];
          const isRef = role.reference;
          if (isRef && !isLocal && canPrefix(n.name)) {
            addIdentifier(n, rewriteIdentifier(n.name, parent, n), role.shorthand ? `${n.name}: ` : '', false);
          } else {
            const constant = !isLocal && !['CallExpression', 'NewExpression', 'MemberExpression', 'OptionalMemberExpression'].includes(parent && parent.type);
            addIdentifier(n, n.name, '', constant);
          }
          return;
        }
        case 'FunctionExpression':
        case 'FunctionDeclaration':
        case 'ArrowFunctionExpression': {
          if (n.id) addIdentifier(n.id, n.id.name, '', true);
          for (const param of n.params || []) collectParamDefaultExpressions(param, locals);
          const childLocals = Object.create(locals || null);
          for (const param of n.params || []) {
            for (const id of extractPatternIds(param)) {
              addIdentifier(id, id.name, '', true);
              markKnown(childLocals, id.name);
            }
          }
          if (n.body) visit(n.body, n, childLocals);
          return;
        }
        case 'ObjectProperty':
        case 'Property': {
          if (n.computed) visit(n.key, n, locals);
          else if (n.key && n.key.type === 'Identifier') {
            if (n.shorthand) {
              const isLocal = !!locals[n.key.name];
              const replacement = !isLocal && canPrefix(n.key.name) ? rewriteIdentifier(n.key.name, n, n.key) : n.key.name;
              addIdentifier(n.key, replacement, `${n.key.name}: `, isLocal);
              return;
            }
          }
          visit(n.value, n, locals);
          return;
        }
        case 'ObjectMethod': {
          if (n.computed) visit(n.key, n, locals);
          else if (n.key && n.key.type === 'Identifier') addIdentifier(n.key, n.key.name, '', true);
          const childLocals = Object.create(locals || null);
          for (const param of n.params || []) {
            for (const id of extractPatternIds(param)) markKnown(childLocals, id.name);
          }
          visit(n.body, n, childLocals);
          return;
        }
        case 'MemberExpression':
        case 'OptionalMemberExpression':
          visit(n.object, n, locals);
          if (n.computed) visit(n.property, n, locals);
          else if (n.property && n.property.type === 'Identifier') addIdentifier(n.property, n.property.name, '', false);
          return;
        case 'VariableDeclarator':
          for (const id of extractPatternIds(n.id)) markKnown(locals, id.name);
          visit(n.init, n, locals);
          return;
        case 'CatchClause': {
          const childLocals = Object.create(locals || null);
          for (const id of extractPatternIds(n.param)) markKnown(childLocals, id.name);
          visit(n.body, n, childLocals);
          return;
        }
      }
      for (const key of Object.keys(n)) {
        if (key === 'loc' || key === 'start' || key === 'end' || key === 'extra') continue;
        const value = n[key];
        if (Array.isArray(value)) value.forEach(child => visit(child, n, locals));
        else if (value && typeof value.type === 'string') visit(value, n, locals);
      }
    };
    const markKnown = (locals, name) => { if (name) locals[name] = (locals[name] || 0) + 1; };
    const identifierRole = (id, parent) => {
      if (!parent) return { reference: true };
      if ((parent.type === 'ObjectProperty' || parent.type === 'Property') && parent.key === id && !parent.computed) {
        return parent.shorthand ? { reference: true, shorthand: true } : { skip: true };
      }
      if ((parent.type === 'ObjectMethod' || parent.type === 'ClassMethod' || parent.type === 'ClassProperty') && parent.key === id && !parent.computed) return { skip: true };
      if ((parent.type === 'MemberExpression' || parent.type === 'OptionalMemberExpression') && parent.property === id && !parent.computed) return { reference: false };
      if ((parent.type === 'FunctionExpression' || parent.type === 'FunctionDeclaration') && parent.id === id) return { reference: false };
      if (parent.type === 'LabeledStatement' && parent.label === id) return { skip: true };
      if (parent.type === 'BreakStatement' || parent.type === 'ContinueStatement') return { skip: true };
      if (parent.type === 'ImportSpecifier' || parent.type === 'ImportDefaultSpecifier' || parent.type === 'ImportNamespaceSpecifier') return { skip: true };
      return { reference: true };
    };
    visit(unwrapExpressionAst(ast), null, knownIds);
    ids.sort((a, b) => a.start - b.start || a.end - b.end);
    const filtered = [];
    for (const id of ids) {
      if (id.start < 0 || id.end > rawExp.length || id.end <= id.start) continue;
      const prev = filtered[filtered.length - 1];
      if (prev && id.start < prev.end) continue;
      filtered.push(id);
    }
    if (!filtered.length) {
      node.constType = ConstantTypes.CAN_STRINGIFY;
      return node;
    }
    const children = [];
    for (let i = 0; i < filtered.length; i++) {
      const id = filtered[i];
      const last = filtered[i - 1];
      const leadingText = rawExp.slice(last ? last.end : 0, id.start);
      if (leadingText.length || id.prefix) children.push(leadingText + (id.prefix || ''));
      const source = rawExp.slice(id.start, id.end);
      children.push(runtime.createSimpleExpression(
        id.name,
        false,
        {
          start: runtime.advancePositionWithClone(node.loc.start, rawExp, id.start),
          end: runtime.advancePositionWithClone(node.loc.start, rawExp, id.end),
          source,
        },
        id.isConstant ? ConstantTypes.CAN_STRINGIFY : ConstantTypes.NOT_CONSTANT,
      ));
      if (i === filtered.length - 1 && id.end < rawExp.length) children.push(rawExp.slice(id.end));
    }
    const ret = runtime.createCompoundExpression(children, node.loc);
    ret.ast = ast;
    ret.identifiers = Object.keys(knownIds);
    return ret;

    function unwrapExpressionAst(parsed) {
      if (!parsed) return parsed;
      if (asRawStatements) return parsed;
      if (asParams && parsed.type === 'ArrowFunctionExpression') return parsed;
      if (parsed.type === 'SequenceExpression' && parsed.expressions && parsed.expressions.length === 1) return parsed.expressions[0];
      if (parsed.type === 'ParenthesizedExpression') return parsed.expression;
      return parsed;
    }

    function processExpressionByLexer(simpleNode, ctx, sourceText, rewrite, locals, isLiteral) {
      const children = [];
      const re = /[A-Za-z_$][\w$]*/g;
      let last = 0;
      let match;
      while ((match = re.exec(sourceText))) {
        const raw = match[0];
        if (isLiteral(raw) || runtime.isGloballyAllowed(raw) || locals[raw]) continue;
        children.push(sourceText.slice(last, match.index));
        children.push(runtime.createSimpleExpression(rewrite(raw), false, simpleNode.loc));
        last = match.index + raw.length;
      }
      if (!children.length) return simpleNode;
      children.push(sourceText.slice(last));
      return runtime.createCompoundExpression(children, simpleNode.loc);
    }
  };
  runtime.transformExpression = function transformExpression(node, context) {
    if (node.type === NodeTypes.INTERPOLATION) {
      node.content = runtime.processExpression(node.content, context);
    } else if (node.type === NodeTypes.ELEMENT) {
      const memo = runtime.findDir(node, 'memo');
      for (const dir of node.props || []) {
        if (dir.type !== NodeTypes.DIRECTIVE || dir.name === 'for') continue;
        const exp = dir.exp;
        const arg = dir.arg;
        if (exp && exp.type === NodeTypes.SIMPLE_EXPRESSION && !(dir.name === 'on' && arg) && !(memo && arg && arg.type === NodeTypes.SIMPLE_EXPRESSION && arg.content === 'key')) {
          dir.exp = runtime.processExpression(exp, context, dir.name === 'slot');
        }
        if (arg && arg.type === NodeTypes.SIMPLE_EXPRESSION && !arg.isStatic) {
          dir.arg = runtime.processExpression(arg, context);
        }
      }
    }
  };
  runtime.transformBind = function transformBind(dir) {
    return { props: [runtime.createObjectProperty(dir.arg || runtime.createSimpleExpression('dynamic', true), dir.exp || runtime.createSimpleExpression('', true))] };
  };
  runtime.transformOn = function transformOn(dir) {
    const arg = dir.arg || runtime.createSimpleExpression('on', true);
    return { props: [runtime.createObjectProperty(arg, dir.exp || runtime.createSimpleExpression('() => {}', false))] };
  };
  runtime.transformModel = function transformModel(dir) {
    return { props: [runtime.createObjectProperty('modelValue', dir.exp || runtime.createSimpleExpression('', false))] };
  };
  runtime.transformVBindShorthand = () => {};
  runtime.transformElement = function transformElement(node, context) {
    return () => {
      node = context.currentNode;
      if (!node || node.type !== NodeTypes.ELEMENT) return;
      if (node.tagType !== ElementTypes.ELEMENT && node.tagType !== ElementTypes.COMPONENT) return;
      const isComponent = node.tagType === ElementTypes.COMPONENT;
      const tag = isComponent ? node.tag : `"${node.tag}"`;
      let patchFlag;
      let props;
      const dynamicProps = [];
      if (node.props && node.props.length) {
        const objectProps = [];
        const directives = [];
        for (const prop of node.props) {
          if (prop.type === NodeTypes.ATTRIBUTE) {
            objectProps.push(runtime.createObjectProperty(prop.name, runtime.createSimpleExpression(prop.value ? prop.value.content : '', true)));
          } else if (prop.name === 'bind' && prop.arg) {
            objectProps.push(runtime.createObjectProperty(prop.arg, prop.exp || runtime.createSimpleExpression('', false)));
            if (prop.arg.isStatic) dynamicProps.push(prop.arg.content);
          } else if (prop.name === 'on' && prop.arg) {
            objectProps.push(runtime.createObjectProperty(runtime.createSimpleExpression(`on${capitalize(prop.arg.content)}`, true), prop.exp || runtime.createSimpleExpression('() => {}', false)));
          } else {
            directives.push(prop);
          }
        }
        if (objectProps.length) props = runtime.createObjectExpression(objectProps);
        if (dynamicProps.length) patchFlag = 8;
        if (directives.length) {
          node.codegenNode = runtime.createVNodeCall(context, tag, props, node.children && node.children.length ? node.children : undefined, patchFlag, dynamicProps.length ? JSON.stringify(dynamicProps) : undefined, runtime.createArrayExpression(directives.map(d => runtime.createArrayExpression([d.name]))), false, false, isComponent, node.loc);
          return;
        }
      }
      const children = node.children && node.children.length === 1 ? node.children[0] : node.children && node.children.length ? node.children : undefined;
      if (!patchFlag && children && (children.type === NodeTypes.INTERPOLATION || children.type === NodeTypes.COMPOUND_EXPRESSION)) patchFlag = 1;
      node.codegenNode = runtime.createVNodeCall(context, tag, props, children, patchFlag, dynamicProps.length ? JSON.stringify(dynamicProps) : undefined, undefined, false, false, isComponent, node.loc);
    };
  };
  runtime.processSlotOutlet = function processSlotOutlet(node) {
    let slotName = '"default"';
    const nameProp = runtime.findProp(node, 'name', false, true);
    if (nameProp && nameProp.type === NodeTypes.ATTRIBUTE && nameProp.value) slotName = JSON.stringify(nameProp.value.content);
    return { slotName, slotProps: undefined };
  };
  runtime.transformSlotOutlet = function transformSlotOutlet(node, context) {
    if (node.type === NodeTypes.ELEMENT && node.tagType === ElementTypes.SLOT) {
      return () => {
        const { slotName, slotProps } = runtime.processSlotOutlet(node, context);
        node.codegenNode = runtime.createCallExpression(context.helper(runtime.RENDER_SLOT), ['$slots', slotName, slotProps]);
      };
    }
  };
  runtime.transformText = function transformText(node, context) {
    if (![NodeTypes.ROOT, NodeTypes.ELEMENT, NodeTypes.FOR, NodeTypes.IF_BRANCH].includes(node.type)) return;
    return () => {
      const children = node.children || [];
      for (let i = 0; i < children.length - 1; i++) {
        if (runtime.isText(children[i]) && runtime.isText(children[i + 1])) {
          const compound = runtime.createCompoundExpression([children[i]], children[i].loc);
          while (runtime.isText(children[i + 1])) {
            compound.children.push(' + ', children[i + 1]);
            children.splice(i + 1, 1);
          }
          children[i] = compound;
        }
      }
      if (children.length > 1 || node.type === NodeTypes.ROOT) {
        for (let i = 0; i < children.length; i++) {
          const child = children[i];
          if (runtime.isText(child) || child.type === NodeTypes.COMPOUND_EXPRESSION) {
            const callArgs = child.type === NodeTypes.TEXT && child.content === ' ' ? [] : [child];
            if (child.type !== NodeTypes.TEXT) callArgs.push('1 /* TEXT */');
            children[i] = { type: NodeTypes.TEXT_CALL, content: child, loc: child.loc, codegenNode: runtime.createCallExpression(context.helper(runtime.CREATE_TEXT), callArgs) };
          }
        }
      }
    };
  };
  runtime.processIf = function processIf(node, dir, context, processCodegen) {
    const branch = {
      type: NodeTypes.IF_BRANCH,
      loc: node.loc,
      condition: dir.name === 'else' ? undefined : dir.exp,
      children: [node],
      userKey: runtime.findProp(node, 'key'),
      isTemplateIf: node.tagType === ElementTypes.TEMPLATE,
    };
    const ifNode = { type: NodeTypes.IF, loc: node.loc, branches: [branch], codegenNode: undefined };
    context.replaceNode(ifNode);
    if (processCodegen) return processCodegen(ifNode, branch, false);
  };
  runtime.transformIf = runtime.createStructuralDirectiveTransform(/^(if|else|else-if)$/, runtime.processIf);
  runtime.processFor = function processFor(node, dir, context, processCodegen) {
    const parsed = parseForExpression(dir.exp && dir.exp.content || '');
    const forNode = {
      type: NodeTypes.FOR,
      loc: node.loc,
      source: parsed.source,
      valueAlias: parsed.value,
      keyAlias: parsed.key,
      objectIndexAlias: parsed.index,
      parseResult: parsed,
      children: [node],
      codegenNode: undefined,
    };
    context.replaceNode(forNode);
    if (processCodegen) return processCodegen(forNode);
  };
  runtime.transformFor = runtime.createStructuralDirectiveTransform('for', runtime.processFor);
  runtime.createForLoopParams = function createForLoopParams(parseResult) {
    return [parseResult.value, parseResult.key, parseResult.index].filter(Boolean);
  };
  runtime.trackSlotScopes = () => {};
  runtime.trackVForSlotScopes = () => {};
  runtime.buildProps = function buildProps() {
    return { props: undefined, directives: [], patchFlag: 0, dynamicPropNames: [], shouldUseBlock: false };
  };
  runtime.buildDirectiveArgs = function buildDirectiveArgs(dir) {
    return runtime.createArrayExpression([dir.name, dir.exp, dir.arg].filter(Boolean));
  };
  runtime.buildSlots = function buildSlots() {
    return { slots: runtime.createObjectExpression([]), hasDynamicSlots: false };
  };
  runtime.resolveComponentType = function resolveComponentType(node) { return node.tag; };
  runtime.getBaseTransformPreset = function getBaseTransformPreset() {
    return [[runtime.transformOnce, runtime.transformIf, runtime.transformMemo, runtime.transformFor, runtime.transformExpression, runtime.transformSlotOutlet, runtime.transformElement, runtime.trackSlotScopes, runtime.transformText], { on: runtime.transformOn, bind: runtime.transformBind, model: runtime.transformModel }];
  };
  runtime.getConstantType = function getConstantType(node) {
    return node && node.type === NodeTypes.SIMPLE_EXPRESSION && node.isStatic ? ConstantTypes.CAN_STRINGIFY : ConstantTypes.NOT_CONSTANT;
  };
  runtime.transformOnce = () => {};
  runtime.transformMemo = () => {};
  runtime.checkCompatEnabled = () => false;
  runtime.warnDeprecation = () => {};
  return runtime;
})();

function capitalize(value) {
  value = String(value || '');
  return value ? value.charAt(0).toUpperCase() + value.slice(1) : value;
}

function selfNameFromFilename(filename) {
  const match = String(filename).replace(/\?.*$/, '').match(/([^/\\]+)\.\w+$/);
  if (!match) return null;
  return match[1].replace(/(^|[-_])(\w)/g, (_, _sep, ch) => ch.toUpperCase());
}

function parseForExpression(source) {
  const match = String(source || '').match(vue3CoreRuntime.forAliasRE);
  const iterable = match ? match[2].trim() : '';
  const aliases = match ? match[1].trim().replace(/^\(|\)$/g, '') : '';
  const parts = aliases.split(',').map(part => part.trim()).filter(Boolean);
  return {
    source: vue3CoreRuntime.createSimpleExpression(iterable, false),
    value: parts[0] ? vue3CoreRuntime.createSimpleExpression(parts[0], false) : undefined,
    key: parts[1] ? vue3CoreRuntime.createSimpleExpression(parts[1], false) : undefined,
    index: parts[2] ? vue3CoreRuntime.createSimpleExpression(parts[2], false) : undefined,
    finalized: false,
  };
}

function createRootCodegen(root, context) {
  const children = root.children || [];
  if (children.length === 1) {
    const child = children[0];
    if (child && child.codegenNode) {
      const codegenNode = child.codegenNode;
      if (codegenNode.type === vue3CoreRuntime.NodeTypes.VNODE_CALL) {
        vue3CoreRuntime.convertToBlock(codegenNode, context);
      }
      root.codegenNode = codegenNode;
    } else {
      root.codegenNode = child;
    }
  } else if (children.length > 1) {
    root.codegenNode = vue3CoreRuntime.createVNodeCall(
      context,
      context.helper(vue3CoreRuntime.FRAGMENT),
      undefined,
      children,
      64,
      undefined,
      undefined,
      true,
      undefined,
      false,
    );
  }
}

function hydrateVue3Ast(ast, options) {
  emitVue3ParseDiagnostics(ast, options);
  hydrateVue3Node(ast);
  return ast;
}

function emitVue3ParseDiagnostics(ast, options) {
  if (!ast || !Array.isArray(ast.__vuecDiagnostics)) return;
  const onError = options && typeof options.onError === 'function'
    ? options.onError
    : error => { throw error; };
  for (const diagnostic of ast.__vuecDiagnostics) {
    const error = new SyntaxError(vue3CoreRuntime.errorMessages[diagnostic.code] || 'Vue compiler parse error');
    error.code = diagnostic.code;
    error.loc = diagnostic.loc;
    onError(error);
  }
  delete ast.__vuecDiagnostics;
}

function hydrateVue3Node(node) {
  if (!node || typeof node !== 'object') return node;
  if (node.type === vue3CoreRuntime.NodeTypes.ROOT) {
    node.helpers = new Set(node.helpers || []);
    node.components = node.components || [];
    node.directives = node.directives || [];
    node.hoists = node.hoists || [];
    node.imports = node.imports || [];
    node.cached = node.cached || [];
    node.temps = node.temps || 0;
    if (node.codegenNode === null) node.codegenNode = undefined;
  }
  if (node.type === vue3CoreRuntime.NodeTypes.ELEMENT) {
    if (node.codegenNode === null) node.codegenNode = undefined;
    if (node.isSelfClosing === null) delete node.isSelfClosing;
  }
  if (node.type === vue3CoreRuntime.NodeTypes.ATTRIBUTE) {
    if (node.value === null) node.value = undefined;
  }
  if (node.type === vue3CoreRuntime.NodeTypes.DIRECTIVE) {
    if (node.exp === null) node.exp = undefined;
    if (node.arg === null) node.arg = undefined;
  }
  if (Array.isArray(node.children)) node.children.forEach(hydrateVue3Node);
  if (Array.isArray(node.props)) node.props.forEach(hydrateVue3Node);
  if (Array.isArray(node.modifiers)) node.modifiers.forEach(hydrateVue3Node);
  if (node.content && typeof node.content === 'object') hydrateVue3Node(node.content);
  if (node.exp && typeof node.exp === 'object') hydrateVue3Node(node.exp);
  if (node.arg && typeof node.arg === 'object') hydrateVue3Node(node.arg);
  return node;
}

function callBridge(command, payload) {
  const result = cp.spawnSync(BRIDGE_BIN, [command], {
    input: JSON.stringify(payload || {}),
    encoding: 'utf8'
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    const error = new Error(result.stderr || result.stdout || `vuec bridge command failed: ${command}`);
    error.code = 'VUEC_BRIDGE_FAILED';
    throw error;
  }
  return result.stdout.trim() ? JSON.parse(result.stdout) : undefined;
}

function normalizeArgs(payload) {
  return payload || {};
}

function bridgePayloadForCall(payload) {
  if (!payload || !Object.prototype.hasOwnProperty.call(payload, 'bridgeOptions')) return payload || {};
  const bridgePayload = {};
  for (const key of Object.keys(payload)) {
    if (key === 'options') {
      bridgePayload.options = payload.bridgeOptions;
    } else if (key !== 'bridgeOptions') {
      bridgePayload[key] = payload[key];
    }
  }
  return bridgePayload;
}

function vue3BridgePayload(source, filename, options) {
  return {
    source,
    filename,
    options,
    bridgeOptions: normalizeVue3OptionsForBridge(options, source),
  };
}

function normalizeVue3OptionsForBridge(options, source) {
  if (!options || typeof options !== 'object') return {};
  const normalized = {};
  for (const key of Object.keys(options)) {
    if (typeof options[key] !== 'function') normalized[key] = options[key];
  }
  const tags = extractVueTemplateTags(String(source || ''));
  normalized.__vuecVoidTags = collectVuePredicateHits(options.isVoidTag, tags);
  if (typeof options.isNativeTag === 'function') {
    normalized.__vuecNativeTags = collectVuePredicateHits(options.isNativeTag, tags);
  }
  normalized.__vuecCustomElements = collectVuePredicateHits(options.isCustomElement, tags);
  normalized.__vuecBuiltInComponents = collectVuePredicateHits(options.isBuiltInComponent, tags);
  return normalized;
}

function extractVueTemplateTags(source) {
  const tags = [];
  const seen = new Set();
  const pattern = /<\/?\s*([A-Za-z][A-Za-z0-9._:-]*)/g;
  let match;
  while ((match = pattern.exec(source))) {
    const tag = match[1];
    if (!seen.has(tag)) {
      seen.add(tag);
      tags.push(tag);
    }
  }
  return tags;
}

function collectVuePredicateHits(predicate, values) {
  if (Array.isArray(predicate)) return predicate.map(String);
  if (typeof predicate !== 'function') return [];
  const hits = [];
  for (const value of values) {
    try {
      if (predicate(value)) hits.push(value);
    } catch (_) {}
  }
  return hits;
}

function preflightAliasCall(name, payload) {
  if (name === 'vue3.core.baseCompile') {
    const options = payload && payload.options ? payload.options : {};
    const isModuleMode = options.mode === 'module';
    const prefixIdentifiers = options.prefixIdentifiers === true || isModuleMode;
    if (!prefixIdentifiers && options.cacheHandlers) {
      throwCompilerSyntaxError(50, '"cacheHandlers" option is only supported when the "prefixIdentifiers" option is enabled.');
    }
    if (options.scopeId && !isModuleMode) {
      throwCompilerSyntaxError(51, '"scopeId" option is only supported in module mode.');
    }
  }
}

function throwCompilerSyntaxError(code, message) {
  const error = new SyntaxError(message);
  error.code = code;
  error.loc = undefined;
  throw error;
}

function extractStyleSource(source) {
  const match = String(source || '').match(/<style[^>]*>([\s\S]*?)<\/style>/i);
  return match ? match[1] : String(source || '');
}

function notImplemented(name) {
  const error = new Error(`Rust Vue compiler alias export ${name} is not implemented yet`);
  error.code = 'VUEC_NOT_IMPLEMENTED';
  throw error;
}

function namedArity(name, arity, fn) {
  const bound = fn.bind(null);
  Object.defineProperty(bound, 'name', { value: name, configurable: true });
  Object.defineProperty(bound, 'length', { value: arity, configurable: true });
  return bound;
}
"#;

const OUTPUT_CONTRACT_PROBE_SCRIPT: &str = r#"
const path = require('path');
const { createRequire } = require('module');

const officialRoot = process.env.VUEC_OUTPUT_OFFICIAL_ROOT;
const rustRoot = process.env.VUEC_OUTPUT_RUST_ROOT;
const request = process.env.VUEC_OUTPUT_REQUEST;
const kind = process.env.VUEC_OUTPUT_KIND;
const fixture = process.env.VUEC_OUTPUT_FIXTURE || '';
const versionLine = process.env.VUEC_OUTPUT_VERSION_LINE || '';
const entry = process.env.VUEC_OUTPUT_ENTRY || '';

const officialRequire = createRequire(path.join(officialRoot, 'package.json'));
const rustRequire = createRequire(path.join(rustRoot, 'package.json'));

function load(rootRequire) {
  return rootRequire(request);
}

function isVue27Sfc() {
  return versionLine === 'vue2_7' && entry === 'vue/compiler-sfc';
}

function extractStyleSource(source) {
  const match = String(source).match(/<style[^>]*>([\s\S]*?)<\/style>/i);
  return match ? match[1] : source;
}

function extractTemplateSource(source) {
  const match = String(source).match(/<template[^>]*>([\s\S]*?)<\/template>/i);
  return match ? match[1] : source;
}

function parseSfc(api) {
  return isVue27Sfc()
    ? api.parse({ source: fixture, filename: 'contract.vue' })
    : api.parse(fixture, { filename: 'contract.vue' });
}

function invoke(api) {
  switch (kind) {
    case 'vue2-template': {
      const compile = api.compile(fixture, { outputSourceRange: true, comments: true });
      const functions = api.compileToFunctions(fixture, {}, {});
      return { compile, compileToFunctions: functions };
    }
    case 'sfc': {
      const parsed = parseSfc(api);
      const descriptor = parsed && parsed.descriptor ? parsed.descriptor : parsed;
      const templateSource = descriptor && descriptor.template && descriptor.template.content ? descriptor.template.content : '';
      const styleSource = descriptor && descriptor.styles && descriptor.styles[0] && descriptor.styles[0].content ? descriptor.styles[0].content : '';
      const template = api.compileTemplate({
        source: isVue27Sfc() ? extractTemplateSource(fixture) : templateSource,
        filename: 'contract.vue',
        id: 'data-v-contract',
        scoped: true
      });
      const script = api.compileScript(descriptor, { id: 'data-v-contract' });
      const style = api.compileStyle({
        source: isVue27Sfc() ? extractStyleSource(fixture) : styleSource,
        filename: 'contract.vue',
        id: 'data-v-contract',
        scoped: true
      });
      return { parse: parsed, compileTemplate: template, compileScript: script, compileStyle: style };
    }
    case 'vue3-core':
      return {
        baseCompile: api.baseCompile(fixture, { mode: 'function' }),
        baseParse: api.baseParse(fixture, {})
      };
    case 'vue3-dom':
      return {
        compile: api.compile(fixture, { mode: 'function' }),
        parse: api.parse(fixture, {})
      };
    case 'vue3-ssr':
      return { compile: api.compile(fixture, {}) };
    default:
      throw new Error(`unknown output contract kind ${kind}`);
  }
}

function capture(side, fn) {
  try {
    return { side, ok: true, value: normalize(fn()) };
  } catch (error) {
    return {
      side,
      ok: false,
      error: {
        name: error && error.name ? String(error.name) : null,
        code: error && error.code ? String(error.code) : null,
        message: error && error.message ? normalizeMessage(error.message) : String(error)
      }
    };
  }
}

function normalizeMessage(message) {
  return String(message)
    .replaceAll(officialRoot.replace(/\\/g, '/'), '<official-root>')
    .replaceAll(rustRoot.replace(/\\/g, '/'), '<rust-root>')
    .replaceAll(officialRoot, '<official-root>')
    .replaceAll(rustRoot, '<rust-root>')
    .replace(/\\/g, '/');
}

function normalize(value, seen = new WeakSet()) {
  if (value === undefined) return { __type: 'undefined' };
  if (value === null) return null;
  if (typeof value === 'function') {
    return { __type: 'function', name: value.name, length: value.length };
  }
  if (typeof value === 'symbol') {
    return { __type: 'symbol', description: value.description || null };
  }
  if (typeof value !== 'object') return value;
  if (seen.has(value)) return { __type: 'cycle' };
  seen.add(value);
  if (Array.isArray(value)) return value.map(item => normalize(item, seen));
  if (value instanceof Set) {
    return Array.from(value).map(item => normalize(item, seen));
  }
  if (value instanceof Map) {
    return Array.from(value.entries()).map(([key, item]) => [normalize(key, seen), normalize(item, seen)]);
  }
  const out = {};
  for (const key of Object.keys(value).sort()) {
    if (key === 'ast' || key === 'element_ast' || key === 'source' || key === 'source_file') continue;
    out[key] = normalize(value[key], seen);
  }
  return out;
}

function objectShape(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return [];
  return Object.keys(value).sort();
}

function codeFields(value, prefix = '') {
  const out = {};
  collectCodeFields(value, prefix, out, new WeakSet());
  return out;
}

function collectCodeFields(value, prefix, out, seen) {
  if (!value || typeof value !== 'object') return;
  if (seen.has(value)) return;
  seen.add(value);
  if (Array.isArray(value)) {
    value.forEach((item, index) => collectCodeFields(item, `${prefix}[${index}]`, out, seen));
    return;
  }
  for (const key of Object.keys(value).sort()) {
    const next = prefix ? `${prefix}.${key}` : key;
    if (['code', 'render', 'ssrRender'].includes(key) && typeof value[key] === 'string') {
      out[next] = value[key];
    } else if (key === 'staticRenderFns' && Array.isArray(value[key])) {
      out[next] = value[key];
    } else {
      collectCodeFields(value[key], next, out, seen);
    }
  }
}

function diagnosticFields(value, prefix = '') {
  const out = {};
  collectDiagnosticFields(value, prefix, out, new WeakSet());
  return out;
}

function collectDiagnosticFields(value, prefix, out, seen) {
  if (!value || typeof value !== 'object') return;
  if (seen.has(value)) return;
  seen.add(value);
  if (Array.isArray(value)) {
    value.forEach((item, index) => collectDiagnosticFields(item, `${prefix}[${index}]`, out, seen));
    return;
  }
  for (const key of Object.keys(value).sort()) {
    const next = prefix ? `${prefix}.${key}` : key;
    if (['errors', 'warnings', 'tips', 'diagnostics'].includes(key) && Array.isArray(value[key])) {
      out[next] = normalize(value[key]);
    } else {
      collectDiagnosticFields(value[key], next, out, seen);
    }
  }
}

function sourceMapFields(value, prefix = '') {
  const out = {};
  collectSourceMapFields(value, prefix, out, new WeakSet());
  return out;
}

function collectSourceMapFields(value, prefix, out, seen) {
  if (!value || typeof value !== 'object') return;
  if (seen.has(value)) return;
  seen.add(value);
  if (Array.isArray(value)) {
    value.forEach((item, index) => collectSourceMapFields(item, `${prefix}[${index}]`, out, seen));
    return;
  }
  for (const key of Object.keys(value).sort()) {
    const next = prefix ? `${prefix}.${key}` : key;
    if ((key === 'map' || key === 'sourceMap') && value[key] != null) {
      out[next] = normalize(value[key]);
    } else {
      collectSourceMapFields(value[key], next, out, seen);
    }
  }
}

function compareJson(mode, official, rust, extractor) {
  const officialValue = official.ok ? extractor(official.value) : official.error;
  const rustValue = rust.ok ? extractor(rust.value) : rust.error;
  const equal = JSON.stringify(officialValue) === JSON.stringify(rustValue);
  return {
    mode,
    status: equal ? 'pass' : 'fail',
    official: officialValue,
    rust: rustValue
  };
}

const official = capture('official', () => invoke(load(officialRequire)));
const rust = capture('rust', () => invoke(load(rustRequire)));
function topLevelShape(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return [];
  if ('descriptor' in value && 'errors' in value) {
    return objectShape(value);
  }
  if ('ast' in value && 'code' in value && 'map' in value) {
    return objectShape(value);
  }
  return objectShape(value);
}

function topLevelCodeFields(value) {
  const out = {};
  if (!value || typeof value !== 'object') return out;
  if (value.code && typeof value.code === 'string') {
    out.code = value.code;
  }
  if (value.render && typeof value.render === 'string') {
    out.render = value.render;
  }
  if (value.ssrRender && typeof value.ssrRender === 'string') {
    out.ssrRender = value.ssrRender;
  }
  return out;
}

function topLevelSourceMap(value) {
  const out = {};
  if (!value || typeof value !== 'object') return out;
  if (value.map !== undefined) {
    out.map = normalize(value.map);
  }
  if (value.rawResult !== undefined) {
    out.rawResult = { keys: Object.keys(value.rawResult).sort() };
  }
  return out;
}

function topLevelRuntime(value) {
  const out = {};
  if (!value || typeof value !== 'object') return out;
  out.runtime = normalize(executeRuntime(value));
  return out;
}

function executeRuntime(value) {
  if (kind === 'vue2-template') {
    return executeVue2Runtime(value.compile || value);
  }
  if (kind === 'sfc' && isVue27Sfc()) {
    return executeVue2Runtime(value.compileTemplate || value);
  }
  if (kind === 'vue3-ssr') {
    return executeVue3SsrRuntime(value.compile || value);
  }
  if (kind === 'vue3-core') {
    return executeVue3Runtime(value.baseCompile || value);
  }
  if (kind === 'vue3-dom') {
    return executeVue3Runtime(value.compile || value);
  }
  if (kind === 'sfc') {
    return executeVue3Runtime(value.compileTemplate || value);
  }
  throw new Error(`unsupported runtime kind ${kind}`);
}

function pickCodeSource(entry, keys) {
  if (!entry || typeof entry !== 'object') {
    return null;
  }
  for (const key of keys) {
    const value = entry[key];
    if (typeof value === 'string' && value.trim()) {
      return value;
    }
  }
  return null;
}

function runtimeFixtureContext() {
  return {
    msg: 'hello',
    a: 'alpha',
    b: 'beta',
    c: 'gamma',
    d: 'delta',
    color: 'rebeccapurple',
    checked: true,
    value: 'value',
    item: { id: 1, name: 'one', uid: 1, ok: true },
    items: [
      { id: 1, name: 'one', uid: 1, ok: true },
      { id: 2, name: 'two', uid: 2, ok: false },
    ],
    list: [
      { id: 1, name: 'one', uid: 1, ok: true },
      { id: 2, name: 'two', uid: 2, ok: true },
    ],
    save: () => 'saved',
    $slots: {},
    _ssrInterpolate: (value) => ssrInterpolate(value),
    _ssrRenderAttrs: (value) => ssrRenderAttrs(value),
    _mergeProps: (...args) => Object.assign({}, ...args),
  };
}

function executeVue2Runtime(entry) {
  if (entry && typeof entry.render === 'function') {
    return executeVue2Render(entry.render, entry.staticRenderFns || []);
  }
  const source = pickCodeSource(entry, ['code', 'render']);
  return executeVue2RenderSource(source);
}

function executeVue2RenderSource(source) {
  if (!source) throw new Error('missing Vue 2 render source');
  const compiled = new Function(`var render = function render() { ${source} };\nreturn { render, staticRenderFns: [] };`)();
  return executeVue2Render(compiled.render, compiled.staticRenderFns || []);
}

function executeVue2Render(render, staticRenderFns) {
  if (typeof render !== 'function') {
    throw new Error('Vue 2 render did not evaluate to a function');
  }
  const context = createVue2RuntimeContext(staticRenderFns);
  return render.call(context);
}

function createVue2RuntimeContext(staticRenderFns) {
  const context = runtimeFixtureContext();
  context._self = context;
  context.$options = {
    staticRenderFns,
    filters: {},
  };
  context.$slots = {};
  context.$scopedSlots = {};
  context._c = function(tag, data, children) {
    return {
      kind: 'vue2-element',
      tag,
      data: normalize(data),
      children: normalize(children),
    };
  };
  context._v = function(text) {
    return {
      kind: 'vue2-text',
      text: String(text),
    };
  };
  context._s = function(value) {
    if (value == null) return '';
    if (typeof value === 'object') return JSON.stringify(normalize(value));
    return String(value);
  };
  context._l = function(list, fn) {
    const source = Array.isArray(list) ? list : list == null ? [] : [list];
    return source.map((item, index) => normalize(fn.call(context, item, index)));
  };
  context._e = function() {
    return {
      kind: 'vue2-comment',
      text: '',
    };
  };
  context._m = function(index) {
    const renderFn = staticRenderFns[index];
    return typeof renderFn === 'function' ? normalize(renderFn.call(context)) : null;
  };
  context._f = function(name) {
    return context.$options.filters[name] || ((value) => value);
  };
  context._o = function(value) {
    return value;
  };
  context._n = function(value) {
    return value;
  };
  context._t = function(name, fallback) {
    return typeof fallback === 'function' ? fallback() : fallback;
  };
  context._u = function(value) {
    return value;
  };
  context._g = function(data, value) {
    return Object.assign({}, data, value);
  };
  context._d = function(list, value) {
    return value;
  };
  context._b = function(data, tag, value) {
    return Object.assign({}, data, value);
  };
  context._k = function() {
    return false;
  };
  return context;
}

function executeVue3Runtime(entry) {
  if (entry && typeof entry.render === 'function') {
    return entry.render(runtimeFixtureContext(), []);
  }
  const source = pickCodeSource(entry, ['code', 'render']);
  return instantiateVue3Render(source)(runtimeFixtureContext(), []);
}

function instantiateVue3Render(source) {
  if (!source) throw new Error('missing Vue 3 render source');
  const transformed = transformVue3ModuleSource(source);
  const factory = new Function(
    'Vue',
    'require',
    '__ctx',
    `with (__ctx) { ${transformed}\nreturn typeof render === 'function' ? render : undefined; }`
  );
  const render = factory(createVue3Runtime(), createVue3SsrRequire(), runtimeFixtureContext());
  if (typeof render !== 'function') {
    throw new Error('Vue 3 render did not evaluate to a function');
  }
  return render;
}

function createVue3Runtime() {
  return {
    mergeProps: (...args) => Object.assign({}, ...args),
    openBlock: () => null,
    createElementVNode: (type, props, children) => ({
      kind: 'vue3-node',
      type,
      props: normalize(props),
      children: normalize(children),
    }),
    createElementBlock: (type, props, children) => ({
      kind: 'vue3-node',
      type,
      props: normalize(props),
      children: normalize(children),
    }),
    createVNode: (type, props, children) => ({
      kind: 'vue3-node',
      type,
      props: normalize(props),
      children: normalize(children),
    }),
    createBlock: (type, props, children) => ({
      kind: 'vue3-node',
      type,
      props: normalize(props),
      children: normalize(children),
    }),
    createTextVNode: (text) => ({
      kind: 'vue3-text',
      text: String(text),
    }),
    createCommentVNode: (text) => ({
      kind: 'vue3-comment',
      text: String(text),
    }),
    toDisplayString: (value) => (value == null ? '' : String(value)),
    renderSlot: (slots, name, props, fallback) => {
      const slot = slots && slots[name];
      if (typeof slot === 'function') {
        return slot(props || {});
      }
      if (typeof fallback === 'function') {
        return fallback();
      }
      return {
        kind: 'vue3-slot',
        name,
        props: normalize(props),
      };
    },
    resolveComponent: (name) => name,
    withCtx: (fn) => fn,
    Fragment: 'Fragment',
    Text: 'Text',
    Comment: 'Comment',
  };
}

function instantiateVue3SsrRender(source) {
  if (!source) throw new Error('missing Vue 3 SSR source');
  const transformed = transformVue3ModuleSource(source);
  const factory = new Function(
    'require',
    '__ctx',
    `with (__ctx) { ${transformed}\nreturn typeof ssrRender === 'function' ? ssrRender : undefined; }`
  );
  const ssrRender = factory(createVue3SsrRequire(), runtimeFixtureContext());
  if (typeof ssrRender !== 'function') {
    throw new Error('Vue 3 SSR render did not evaluate to a function');
  }
  return ssrRender;
}

function executeVue3SsrRuntime(entry) {
  const ssrRender = entry && typeof entry.ssrRender === 'function'
    ? entry.ssrRender
    : instantiateVue3SsrRender(pickCodeSource(entry, ['code', 'ssrRender']));
  const chunks = [];
  const push = (chunk) => {
    if (chunk == null) return;
    chunks.push(String(chunk));
  };
  ssrRender(runtimeFixtureContext(), push, null, {});
  return chunks.join('');
}

function createVue3SsrRequire() {
  const serverRenderer = {
    ssrRenderAttrs: (props) => ssrRenderAttrs(props),
    ssrInterpolate: (value) => ssrInterpolate(value),
    ssrRenderInterpolate: (value) => ssrInterpolate(value),
    ssrRenderSlot: (slots, name, props, fallbackRenderFn, push) => {
      const slot = slots && slots[name];
      if (typeof slot === 'function') {
        const result = slot(props || {});
        if (Array.isArray(result)) {
          for (const item of result) {
            push(String(item));
          }
        } else if (result != null) {
          push(String(result));
        }
      } else if (typeof fallbackRenderFn === 'function') {
        fallbackRenderFn();
      }
    },
    ssrRenderList: (source, renderItem) => {
      const list = Array.isArray(source) ? source : source == null ? [] : [source];
      return list.map((item, index) => renderItem(item, index));
    },
    ssrRenderComponent: () => '',
    ssrRenderTeleport: () => {},
    ssrRenderSuspense: (push, slots) => {
      if (slots && typeof slots.default === 'function') {
        slots.default();
      }
      return Promise.resolve();
    },
    ssrRenderDynamicModel: () => '',
    ssrRenderAttr: (key, value) => ssrRenderAttrs({ [key]: value }),
    ssrRenderClass: (raw) => (raw == null ? '' : ssrEscape(String(raw))),
    ssrRenderStyle: (raw) => (raw == null ? '' : ssrEscape(String(raw))),
  };
  return (id) => {
    if (id === 'vue') {
      return {
        mergeProps: (...args) => Object.assign({}, ...args),
      };
    }
    if (id === 'vue/server-renderer') {
      return serverRenderer;
    }
    throw new Error(`unsupported runtime require ${id}`);
  };
}

function transformVue3ModuleSource(source) {
  return String(source)
    .replace(/import\s+\{([\s\S]*?)\}\s+from\s+["']vue["'];?\s*/g, (_, specifiers) => {
      return `const { ${specifiers.replace(/\s+as\s+/g, ': ')} } = Vue;\n`;
    })
    .replace(/import\s+\{([\s\S]*?)\}\s+from\s+["']vue\/server-renderer["'];?\s*/g, (_, specifiers) => {
      return `const { ${specifiers.replace(/\s+as\s+/g, ': ')} } = require("vue/server-renderer");\n`;
    })
    .replace(/export\s+function\s+(render|ssrRender)/g, 'function $1');
}

function ssrRenderAttrs(props) {
  if (!props || typeof props !== 'object') return '';
  const attrs = [];
  for (const key of Object.keys(props).sort()) {
    const value = props[key];
    if (value == null || value === false) continue;
    if (value === true) {
      attrs.push(` ${key}`);
    } else {
      attrs.push(` ${key}="${ssrEscape(String(value))}"`);
    }
  }
  return attrs.join('');
}

function ssrInterpolate(value) {
  return ssrEscape(value == null ? '' : String(value));
}

function ssrEscape(value) {
  return String(value)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function topLevelDiagnostics(value) {
  const out = {};
  if (!value || typeof value !== 'object') return out;
  if (Array.isArray(value.errors)) out.errors = normalize(value.errors);
  if (Array.isArray(value.tips)) out.tips = normalize(value.tips);
  if (Array.isArray(value.warnings)) out.warnings = normalize(value.warnings);
  if (Array.isArray(value.diagnostics)) out.diagnostics = normalize(value.diagnostics);
  return out;
}

const checks = [
  compareJson('schema-parity', official, rust, value => topLevelShape(value)),
  compareJson('exact-js-output', official, rust, value => topLevelCodeFields(value)),
  compareJson('diagnostic-parity', official, rust, value => topLevelDiagnostics(value)),
  compareJson('source-map-parity', official, rust, value => topLevelSourceMap(value)),
  compareJson('runtime-parity', official, rust, value => topLevelRuntime(value))
];
const counts = {
  total: checks.length,
  pass: checks.filter(check => check.status === 'pass').length,
  fail: checks.filter(check => check.status === 'fail').length,
  pending: checks.filter(check => check.status === 'pending').length
};
process.stdout.write(JSON.stringify({ request, kind, fixture, counts, checks }));
"#;

const OPTION_MATRIX_PROBE_SCRIPT: &str = r#"
const path = require('path');
const { createRequire } = require('module');

const root = process.env.VUEC_OPTION_ROOT;
const side = process.env.VUEC_OPTION_SIDE;
const payload = JSON.parse(process.env.VUEC_OPTION_PAYLOAD || '{}');
const rootRequire = createRequire(path.join(root, 'package.json'));
const request = payload.request;

function load() {
  return rootRequire(request);
}

function capture(fn) {
  try {
    return { ok: true, value: normalize(fn()) };
  } catch (error) {
    return {
      ok: false,
      error: {
        name: error && error.name ? String(error.name) : null,
        code: error && error.code ? String(error.code) : null,
        message: normalizeMessage(error && error.message ? error.message : String(error))
      }
    };
  }
}

function normalizeMessage(message) {
  return String(message)
    .replaceAll(root.replace(/\\/g, '/'), '<option-root>')
    .replace(/\\/g, '/');
}

function normalize(value, seen = new WeakSet()) {
  if (value === undefined) return { __type: 'undefined' };
  if (value === null) return null;
  if (typeof value === 'symbol') return { __type: 'symbol', description: value.description || null };
  if (typeof value === 'function') return { __type: 'function', name: value.name, length: value.length };
  if (value instanceof Set) {
    return Array.from(value)
      .map(item => normalize(item, seen))
      .sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b)));
  }
  if (typeof value !== 'object') return value;
  if (seen.has(value)) return { __type: 'cycle' };
  seen.add(value);
  if (Array.isArray(value)) return value.map(item => normalize(item, seen));
  const out = {};
  for (const key of Object.keys(value).sort()) {
    out[key] = normalize(value[key], seen);
  }
  return out;
}

function pathValue(value, optionPath) {
  if (!optionPath) return value;
  const segments = optionPath.split('.');
  let cursor = value;
  for (const segment of segments) {
    if (cursor == null || typeof cursor !== 'object') return undefined;
    cursor = cursor[segment];
  }
  return cursor;
}

function cloneOptionValue(optionValue) {
  if (optionValue === null || optionValue === undefined) return optionValue;
  return JSON.parse(JSON.stringify(optionValue));
}

function normalizeOptionValue(optionValue) {
  const value = cloneOptionValue(optionValue);
  if (
    side === 'official' &&
    payload.target_package === '@vue/compiler-dom' &&
    payload.option_name === 'isCustomElement' &&
    value &&
    Array.isArray(value.isCustomElement)
  ) {
    const customElements = new Set(value.isCustomElement);
    value.isCustomElement = tag => customElements.has(tag);
  }
  if (
    side === 'official' &&
    payload.target_package === 'vue-template-compiler' &&
    payload.option_name === 'directives' &&
    value &&
    value.directives &&
    typeof value.directives === 'object'
  ) {
    for (const key of Object.keys(value.directives)) {
      if (value.directives[key] === true) {
        value.directives[key] = () => true;
      }
    }
  }
  return value;
}

function optionsArg() {
  switch (payload.input_kind || 'value') {
    case 'missing':
      return { present: false, value: undefined };
    case 'undefined':
      return { present: true, value: undefined };
    case 'null':
      return { present: true, value: null };
    default:
      return { present: true, value: normalizeOptionValue(payload.option_value) };
  }
}

function optionObjectWithSource(baseSource) {
  const arg = optionsArg();
  const objectValue = arg.value && typeof arg.value === 'object' ? arg.value : {};
  return Object.assign({ source: baseSource }, objectValue);
}

function extractStyleSource(fixture) {
  const match = String(fixture).match(/<style[^>]*>([\s\S]*?)<\/style>/i);
  return match ? match[1] : fixture;
}

function extractTemplateSource(fixture) {
  const match = String(fixture).match(/<template[^>]*>([\s\S]*?)<\/template>/i);
  return match ? match[1] : fixture;
}

function isVue27Sfc() {
  return payload.target_version_line === 'vue2_7' && payload.target_entry === 'vue/compiler-sfc';
}

function normalizeSfcStyleResult(result) {
  if (!result || typeof result !== 'object') return result;
  const out = Object.assign({}, result);
  if (out.rawResult && !Array.isArray(out.rawResult)) {
    out.rawResult = ['postcss-result'];
  }
  if (out.map === undefined) {
    out.map = null;
  }
  if (out.dependencies instanceof Set) {
    out.dependencies = Array.from(out.dependencies).sort();
  }
  return out;
}

function invoke(api) {
  const method = payload.method;
  const fixture = payload.source;
  const arg = optionsArg();
  switch (method) {
    case 'compile':
      return capture(() => arg.present ? api.compile(fixture, arg.value) : api.compile(fixture));
    case 'compileToFunctions':
      return capture(() => arg.present ? api.compileToFunctions(fixture, arg.value, {}) : api.compileToFunctions(fixture));
    case 'parse':
      if (isVue27Sfc()) {
        const value = Object.assign({ source: fixture }, arg.value && typeof arg.value === 'object' ? arg.value : {});
        return capture(() => arg.present ? api.parse(value) : api.parse({ source: fixture }));
      }
      return capture(() => arg.present ? api.parse(fixture, arg.value) : api.parse(fixture));
    case 'compileTemplate':
      if (isVue27Sfc()) {
        return capture(() => api.compileTemplate(Object.assign(optionObjectWithSource(extractTemplateSource(fixture)), arg.value && typeof arg.value === 'object' ? arg.value : {})));
      }
      return capture(() => api.compileTemplate(optionObjectWithSource(fixture)));
    case 'compileScript': {
      return capture(() => {
        const parsed = isVue27Sfc()
          ? api.parse({ source: fixture, filename: 'contract.vue' })
          : api.parse(fixture, { filename: 'contract.vue' });
        const descriptor = parsed && parsed.descriptor ? parsed.descriptor : parsed;
        return arg.present ? api.compileScript(descriptor, arg.value) : api.compileScript(descriptor);
      });
    }
    case 'compileStyle':
      return capture(() => normalizeSfcStyleResult(api.compileStyle(optionObjectWithSource(side === 'official' ? extractStyleSource(fixture) : fixture))));
    case 'baseCompile':
      return capture(() => arg.present ? api.baseCompile(fixture, arg.value) : api.baseCompile(fixture));
    case 'baseParse':
      return capture(() => arg.present ? api.baseParse(fixture, arg.value) : api.baseParse(fixture));
    default:
      throw new Error(`unknown option matrix method ${method}`);
  }
}

const api = load();
const result = invoke(api);
const normalized = {
  side,
  request,
  method: payload.method,
  fixture_id: payload.fixture_id,
  option_name: payload.option_name,
  option_path: payload.option_path,
  ok: result.ok,
  value: result.ok ? result.value : null,
  error: result.ok ? null : result.error,
};
process.stdout.write(JSON.stringify(normalized));
"#;

pub fn generate_option_matrix(scope: &SelectionArgs) -> JsonReport {
    let targets = select_targets(scope);
    let mut created = Vec::new();
    let mut items = Vec::new();
    for target in targets {
        let path = target.relative_option_matrix_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let rows = option_matrix_cases(target)
            .into_iter()
            .map(|case| OptionMatrixRow {
                option_name: case.option_name.to_string(),
                option_path: case.option_path.to_string(),
                entry: target.entry.to_string(),
                version_line: target.version_line,
                accepted_types: case
                    .accepted_types
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
                default_when_missing: case.default_when_missing.to_string(),
                behavior_when_undefined: case.behavior_when_undefined.to_string(),
                behavior_when_null: case.behavior_when_null.to_string(),
                side_effects: case.side_effects.iter().map(|s| (*s).to_string()).collect(),
                diagnostics: case.diagnostics.iter().map(|s| (*s).to_string()).collect(),
                output_fields_affected: case
                    .output_fields_affected
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
                official_fixture_ids: case
                    .official_fixture_ids
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
                variant: case.variant.to_string(),
                input_kind: if case.option_value.is_some() {
                    "value".into()
                } else {
                    "missing".into()
                },
                method: case.method.to_string(),
                fixture_id: case.fixture_id.to_string(),
                fixture_source: case.fixture_source.to_string(),
                option_value: case.option_value.clone(),
                execution_mode: case.execution_mode.to_string(),
                status: if case.pending { "pending" } else { "pass" }.into(),
            })
            .collect::<Vec<_>>();
        let matrix = OptionMatrixFile {
            schema_version: 2,
            version_line: target.version_line,
            package: target.package.to_string(),
            entry: target.entry.to_string(),
            status: if rows.iter().any(|row| row.status == "pending") {
                "pending".into()
            } else {
                "pass".into()
            },
            rows,
        };
        let _ = write_json(&path, &matrix);
        created.push(path.display().to_string());
        items.push(ReportItem::new(
            target.display(),
            if matrix.status == "pass" {
                ReportStatus::Pass
            } else {
                ReportStatus::Pending
            },
            format!("{} option cases seeded", matrix.rows.len()),
            Some(path),
        ));
    }
    JsonReport::new("generate_option_matrix", ReportStatus::Pass)
        .with_scope(scope)
        .with_items(items)
        .with_created(created)
}

pub fn audit_option_matrix(scope: &SelectionArgs) -> JsonReport {
    let targets = select_targets(scope);
    let mut items = Vec::new();
    let mut violations = Vec::new();
    for target in targets {
        let path = target.relative_option_matrix_path();
        match read_json::<OptionMatrixFile>(&path) {
            Ok(matrix) => {
                let expected = option_matrix_cases(target);
                let has_expected_version = matrix.version_line == target.version_line;
                let has_expected_entry = matrix.entry == target.entry;
                let has_cases = matrix.rows.len() == expected.len();
                let has_schema = matrix.schema_version >= 2;
                if has_expected_version && has_expected_entry && has_cases && has_schema {
                    items.push(ReportItem::new(
                        target.display(),
                        ReportStatus::Pass,
                        "option matrix shape matches compatibility spec",
                        Some(path),
                    ));
                } else {
                    violations.push(format!(
                        "{} matrix contents do not match spec",
                        path.display()
                    ));
                    items.push(ReportItem::new(
                        target.display(),
                        ReportStatus::Fail,
                        "option matrix contents do not match compatibility spec",
                        Some(path),
                    ));
                }
            }
            Err(err) => {
                violations.push(format!("{}: {err}", path.display()));
                items.push(ReportItem::new(
                    target.display(),
                    ReportStatus::Fail,
                    "option matrix file missing or invalid",
                    Some(path),
                ));
            }
        }
    }
    let status = if violations.is_empty() {
        ReportStatus::Pass
    } else {
        ReportStatus::Fail
    };
    JsonReport::new("audit_option_matrix", status)
        .with_scope(scope)
        .with_items(items)
        .with_violations(violations)
}

pub fn run_option_matrix(scope: &SelectionArgs) -> JsonReport {
    let targets = select_targets(scope);
    let lock_path = PathBuf::from("compat/official-revisions.lock");
    let lock_hash = file_sha256(&lock_path).ok();
    let lock = match load_official_lock(&lock_path) {
        Ok(lock) => lock,
        Err(err) => {
            let mut report = JsonReport::new("run_option_matrix", ReportStatus::Fail);
            report.metadata = report.metadata.with_lock_hash(lock_hash);
            return report
                .with_scope(scope)
                .with_violations(vec![format!("failed to load official lock: {err}")]);
        }
    };
    let mut items = Vec::new();
    let mut violations = Vec::new();
    let mut target_reports = Vec::new();
    if let Err(err) = generate_rust_alias_packages(&targets) {
        violations.push(format!("failed to generate Rust alias packages: {err:#}"));
    }
    for target in targets {
        let path = target.relative_option_matrix_path();
        let matrix = match read_json::<OptionMatrixFile>(&path) {
            Ok(matrix) => matrix,
            Err(err) => {
                violations.push(format!(
                    "{} option matrix missing/invalid: {err}",
                    path.display()
                ));
                items.push(ReportItem::new(
                    target.display(),
                    ReportStatus::Fail,
                    "option matrix missing or invalid",
                    Some(path),
                ));
                continue;
            }
        };
        let Some(baseline) = baseline_for(&lock, target.version_line) else {
            violations.push(format!("{} has no official baseline", target.display()));
            items.push(ReportItem::new(
                target.display(),
                ReportStatus::Fail,
                "official baseline missing",
                Some(path),
            ));
            continue;
        };
        let official_root = match ensure_official_npm_install(target.version_line, baseline) {
            Ok(root) => root,
            Err(err) => {
                violations.push(format!(
                    "{} official npm install failed: {err:#}",
                    target.display()
                ));
                items.push(ReportItem::new(
                    target.display(),
                    ReportStatus::Fail,
                    "official npm install failed",
                    Some(path),
                ));
                continue;
            }
        };
        let rust_root = rust_alias_root(target.version_line);
        let mut row_reports = Vec::new();
        for row in &matrix.rows {
            if row.status == "pending" {
                row_reports.push(serde_json::json!({
                    "option_name": row.option_name,
                    "option_path": row.option_path,
                    "fixture_id": row.fixture_id,
                    "method": row.method,
                    "status": "pending",
                    "detail": "option case is intentionally pending because the current Rust surface does not yet fully wire this option",
                }));
                continue;
            }
            let official_probe = run_option_probe(
                "official",
                target,
                &official_root,
                &api_require_request(target),
                &row.method,
                &row.fixture_source,
                &row.fixture_id,
                &row.option_name,
                &row.option_path,
                &row.input_kind,
                row.option_value.as_ref(),
            );
            let rust_probe = run_option_probe(
                "rust",
                target,
                &rust_root,
                &api_require_request(target),
                &row.method,
                &row.fixture_source,
                &row.fixture_id,
                &row.option_name,
                &row.option_path,
                &row.input_kind,
                row.option_value.as_ref(),
            );
            match (official_probe, rust_probe) {
                (Ok(official), Ok(rust)) => {
                    let equal = compare_option_probe(row, &official, &rust);
                    let status = if equal { "pass" } else { "fail" };
                    if !equal {
                        violations.push(format!(
                            "{} {}:{} option case diverged",
                            target.display(),
                            row.option_name,
                            row.fixture_id
                        ));
                    }
                    row_reports.push(serde_json::json!({
                        "option_name": row.option_name,
                        "option_path": row.option_path,
                        "fixture_id": row.fixture_id,
                        "method": row.method,
                        "status": status,
                        "official": official,
                        "rust": rust,
                    }));
                }
                (Err(err), _) | (_, Err(err)) => {
                    violations.push(format!(
                        "{} {}:{} option execution failed: {err:#}",
                        target.display(),
                        row.option_name,
                        row.fixture_id
                    ));
                    row_reports.push(serde_json::json!({
                        "option_name": row.option_name,
                        "option_path": row.option_path,
                        "fixture_id": row.fixture_id,
                        "method": row.method,
                        "status": "fail",
                        "error": format!("{err:#}"),
                    }));
                }
            }
        }
        let pass = row_reports
            .iter()
            .filter(|row| row.get("status").and_then(|s| s.as_str()) == Some("pass"))
            .count();
        let fail = row_reports
            .iter()
            .filter(|row| row.get("status").and_then(|s| s.as_str()) == Some("fail"))
            .count();
        let pending = row_reports
            .iter()
            .filter(|row| row.get("status").and_then(|s| s.as_str()) == Some("pending"))
            .count();
        let total = row_reports.len();
        let status = if fail > 0 {
            ReportStatus::Fail
        } else if pending > 0 {
            ReportStatus::Pending
        } else {
            ReportStatus::Pass
        };
        items.push(ReportItem::new(
            target.display(),
            status,
            format!("{pass}/{total} option rows passed, {fail} failed, {pending} pending"),
            Some(path),
        ));
        target_reports.push(serde_json::json!({
            "target": target.display(),
            "version_line": target.version_line,
            "package": target.package,
            "entry": target.entry,
            "rows": row_reports,
        }));
    }
    let report_path = PathBuf::from("target")
        .join("conformance")
        .join(lock_hash.as_deref().unwrap_or("unknown-lock"))
        .join("option-matrix.json");
    if let Some(parent) = report_path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            violations.push(format!("failed to create {}: {err}", parent.display()));
        }
    }
    let report_body = serde_json::json!({
        "command": "run_option_matrix",
        "lock_hash": lock_hash,
        "targets": target_reports,
        "counts": output_contract_counts_from_items(&items),
    });
    if let Err(err) = write_json(&report_path, &report_body) {
        violations.push(format!("failed to write {}: {err}", report_path.display()));
    }
    let mut report = JsonReport::new("run_option_matrix", aggregate_status(&items));
    report.metadata = report.metadata.with_lock_hash(lock_hash);
    report
        .with_scope(scope)
        .with_items(items)
        .with_violations(violations)
        .with_created(vec![report_path.display().to_string()])
        .with_note(
            "option matrix now executes official vs Rust probe cases and records per-row results",
        )
}

pub fn run_conformance(args: &ConformanceArgs) -> JsonReport {
    let lock_hash = file_sha256(&args.lock).ok();
    let lock = load_official_lock(&args.lock).ok();
    let requested = select_conformance_suites(args);
    let mut items = Vec::new();
    let mut violations = Vec::new();
    let mut created = Vec::new();
    let conformance_targets = conformance_targets(&requested);
    if let Err(err) = generate_rust_alias_packages(&conformance_targets) {
        violations.push(format!(
            "failed to generate Rust alias packages for conformance: {err:#}"
        ));
    }

    for suite in requested {
        let spec = suite_spec(suite);
        let root = args.vendor_dir.join(spec.version_line.as_str());
        let metadata = root.join("official-revision.json");
        if !metadata.exists() {
            violations.push(format!(
                "{} is missing; run `cargo xtask sync-official-tests --locked` first",
                metadata.display()
            ));
            items.push(ReportItem::new(
                spec.name,
                ReportStatus::Fail,
                "official checkout metadata is missing",
                Some(metadata),
            ));
            continue;
        }

        if let Some(lock) = lock.as_ref() {
            if let Some(baseline) = baseline_for(lock, spec.version_line) {
                if let Err(err) =
                    ensure_official_runner_dependencies(spec, baseline, &args.vendor_dir)
                {
                    violations.push(format!(
                        "{} official runner dependency install failed: {err:#}",
                        spec.name
                    ));
                }
            } else {
                violations.push(format!(
                    "{} has no baseline lock entry for {}",
                    spec.name,
                    spec.version_line.as_str()
                ));
            }
        } else {
            violations.push(format!(
                "failed to read {}; runner dependencies cannot be provisioned",
                args.lock.display()
            ));
        }

        let mut discovered = Vec::new();
        for relative_dir in spec.relative_test_dirs {
            let dir = root.join(relative_dir);
            if !dir.exists() {
                violations.push(format!("{} test directory is missing", dir.display()));
                continue;
            }
            discover_test_files(&dir, &mut discovered);
        }
        discovered.sort();
        let readiness = conformance_readiness(spec);
        let smoke_results = run_conformance_smokes(spec);
        let smoke_failures = smoke_results
            .iter()
            .filter(|result| result.status == "fail")
            .count();
        let ready_to_execute =
            !discovered.is_empty() && readiness.alias_ready && readiness.runner_ready;
        let execution_result = if ready_to_execute {
            match run_conformance_execution(spec, &root, &discovered, lock_hash.as_deref()) {
                Ok(result) => Some(result),
                Err(err) => {
                    violations.push(format!(
                        "{} official conformance execution failed to start: {err:#}",
                        spec.name
                    ));
                    None
                }
            }
        } else {
            None
        };
        let counts = execution_result
            .as_ref()
            .map(|result| result.counts)
            .unwrap_or(ConformanceExecutionCounts {
                total: discovered.len(),
                pass: 0,
                fail: 0,
                skip: 0,
                pending: discovered.len(),
            });
        let execution_status = execution_result
            .as_ref()
            .map(|result| result.status.as_str())
            .unwrap_or(if ready_to_execute { "ready" } else { "blocked" });
        let report_path = PathBuf::from("target")
            .join("conformance")
            .join(lock_hash.as_deref().unwrap_or("unknown-lock"))
            .join(format!("{}.json", spec.name));
        let report_body = serde_json::json!({
            "suite": spec.name,
            "version_line": spec.version_line,
            "lock_hash": lock_hash,
            "test_files": discovered,
            "counts": counts,
            "execution": execution_status,
            "execution_result": execution_result,
            "readiness": readiness,
            "smoke": smoke_results,
        });
        if let Some(parent) = report_path.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                violations.push(format!("failed to create {}: {err}", parent.display()));
            }
        }
        if let Err(err) = write_json(&report_path, &report_body) {
            violations.push(format!("failed to write {}: {err}", report_path.display()));
        } else {
            created.push(report_path.display().to_string());
        }
        let status = if discovered.is_empty()
            || smoke_failures > 0
            || counts.fail > 0
            || (ready_to_execute && counts.total == 0)
        {
            ReportStatus::Fail
        } else if counts.pending > 0 {
            ReportStatus::Pending
        } else {
            ReportStatus::Pass
        };
        if discovered.is_empty() {
            violations.push(format!("{} discovered no official test files", spec.name));
        }
        if smoke_failures > 0 {
            violations.push(format!(
                "{} conformance smoke failed for {smoke_failures} alias package(s)",
                spec.name
            ));
        }
        if counts.fail > 0 {
            violations.push(format!(
                "{} official conformance has {} failing tests",
                spec.name, counts.fail
            ));
        }
        if ready_to_execute && counts.total == 0 {
            violations.push(format!(
                "{} official conformance runner executed zero tests",
                spec.name
            ));
        }
        items.push(ReportItem::new(
            spec.name,
            status,
            conformance_item_detail(discovered.len(), &readiness, execution_result.as_ref()),
            Some(report_path),
        ));
    }

    let mut report = JsonReport::new("run_conformance", ReportStatus::Pending);
    report.metadata = report.metadata.with_lock_hash(lock_hash);
    report
        .with_items(items)
        .with_violations(violations)
        .with_created(created)
}

pub fn generate_output_contract(scope: &SelectionArgs) -> JsonReport {
    let targets = select_targets(scope);
    let mut created = Vec::new();
    let mut items = Vec::new();
    for target in targets {
        let path = target.relative_output_contract_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let contract = OutputContractFile {
            version_line: target.version_line,
            package: target.package.to_string(),
            entry: target.entry.to_string(),
            required_modes: target
                .output_modes()
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            status: "pending".into(),
        };
        let _ = write_json(&path, &contract);
        created.push(path.display().to_string());
        items.push(ReportItem::new(
            target.display(),
            ReportStatus::Pass,
            format!(
                "{} output contract modes seeded",
                contract.required_modes.len()
            ),
            Some(path),
        ));
    }
    JsonReport::new("generate_output_contract", ReportStatus::Pass)
        .with_scope(scope)
        .with_items(items)
        .with_created(created)
}

pub fn run_output_contract(scope: &SelectionArgs) -> JsonReport {
    let targets = select_targets(scope);
    let mut items = Vec::new();
    let mut violations = Vec::new();
    let lock_path = PathBuf::from("compat/official-revisions.lock");
    let lock_hash = file_sha256(&lock_path).ok();
    let lock = match load_official_lock(&lock_path) {
        Ok(lock) => lock,
        Err(err) => {
            let mut report = JsonReport::new("run_output_contract", ReportStatus::Fail);
            report.metadata = report.metadata.with_lock_hash(lock_hash);
            return report
                .with_scope(scope)
                .with_violations(vec![format!("failed to load official lock: {err}")]);
        }
    };
    let report_path = PathBuf::from("target")
        .join("conformance")
        .join(lock_hash.as_deref().unwrap_or("unknown-lock"))
        .join("output-contract.json");
    let mut target_reports = Vec::new();

    if let Err(err) = generate_rust_alias_packages(&targets) {
        violations.push(format!("failed to generate Rust alias packages: {err:#}"));
    }
    for target in targets {
        let path = target.relative_output_contract_path();
        let contract = match read_json::<OutputContractFile>(&path) {
            Ok(contract) => contract,
            Err(err) => {
                violations.push(format!(
                    "{} output contract missing/invalid: {err}",
                    target.display()
                ));
                items.push(ReportItem::new(
                    target.display(),
                    ReportStatus::Fail,
                    "output contract file missing or invalid",
                    Some(path),
                ));
                continue;
            }
        };
        let expected_modes = target
            .output_modes()
            .iter()
            .map(|mode| (*mode).to_string())
            .collect::<Vec<_>>();
        if contract.required_modes != expected_modes {
            violations.push(format!(
                "{} output contract modes do not match target spec",
                target.display()
            ));
            items.push(ReportItem::new(
                target.display(),
                ReportStatus::Fail,
                "output contract modes do not match target spec",
                Some(path),
            ));
            continue;
        }
        let Some(baseline) = baseline_for(&lock, target.version_line) else {
            violations.push(format!("{} has no official baseline", target.display()));
            items.push(ReportItem::new(
                target.display(),
                ReportStatus::Fail,
                "official baseline missing",
                Some(path),
            ));
            continue;
        };
        let official_root = match ensure_official_npm_install(target.version_line, baseline) {
            Ok(root) => root,
            Err(err) => {
                violations.push(format!(
                    "{} official npm install failed: {err:#}",
                    target.display()
                ));
                items.push(ReportItem::new(
                    target.display(),
                    ReportStatus::Fail,
                    "official npm install failed",
                    Some(path),
                ));
                continue;
            }
        };
        let rust_root = rust_alias_root(target.version_line);
        match run_output_contract_probe(target, &official_root, &rust_root) {
            Ok(target_report) => {
                let failed = json_usize(&target_report, &["counts", "fail"]);
                let pending = json_usize(&target_report, &["counts", "pending"]);
                let passed = json_usize(&target_report, &["counts", "pass"]);
                let total = json_usize(&target_report, &["counts", "total"]);
                let status = if failed > 0 {
                    ReportStatus::Fail
                } else if pending > 0 {
                    ReportStatus::Pending
                } else {
                    ReportStatus::Pass
                };
                if failed > 0 {
                    violations.push(format!(
                        "{} output contract has {failed} failing checks",
                        target.display()
                    ));
                }
                items.push(ReportItem::new(
                    target.display(),
                    status,
                    format!("{passed}/{total} checks passed, {failed} failed, {pending} pending"),
                    Some(path),
                ));
                target_reports.push(target_report);
            }
            Err(err) => {
                violations.push(format!(
                    "{} output contract execution failed: {err:#}",
                    target.display()
                ));
                items.push(ReportItem::new(
                    target.display(),
                    ReportStatus::Fail,
                    format!("output contract execution failed: {err:#}"),
                    Some(path),
                ));
            }
        }
    }
    let aggregate = serde_json::json!({
        "command": "run_output_contract",
        "lock_hash": lock_hash,
        "targets": target_reports,
        "counts": output_contract_counts_from_items(&items),
    });
    if let Some(parent) = report_path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            violations.push(format!("failed to create {}: {err}", parent.display()));
        }
    }
    if let Err(err) = write_json(&report_path, &aggregate) {
        violations.push(format!("failed to write {}: {err}", report_path.display()));
    }
    let mut report = JsonReport::new("run_output_contract", ReportStatus::Pending);
    report.metadata = report.metadata.with_lock_hash(lock_hash);
    report
        .with_scope(scope)
        .with_items(items)
        .with_violations(violations)
        .with_created(vec![report_path.display().to_string()])
        .with_note("output contract executes official npm packages and generated Rust alias packages against representative fixtures")
}

pub fn verify_npm_alias(scope: &SelectionArgs) -> JsonReport {
    let targets = select_targets(scope);
    let lock_hash = file_sha256(&PathBuf::from("compat/official-revisions.lock")).ok();
    let mut items = Vec::new();
    let mut violations = Vec::new();
    if let Err(err) = generate_rust_alias_packages(&targets) {
        violations.push(format!("failed to generate Rust alias packages: {err:#}"));
    }
    for target in targets {
        let root = rust_alias_root(target.version_line);
        match run_alias_smoke(target, &root) {
            Ok(detail) => items.push(ReportItem::new(
                target.display(),
                ReportStatus::Pass,
                detail,
                Some(root),
            )),
            Err(err) => {
                violations.push(format!("{}: {err:#}", target.display()));
                items.push(ReportItem::new(
                    target.display(),
                    ReportStatus::Fail,
                    format!("npm alias smoke failed: {err:#}"),
                    Some(root),
                ));
            }
        }
    }
    let mut report = JsonReport::new("verify_npm_alias", ReportStatus::Pending);
    report.metadata = report.metadata.with_lock_hash(lock_hash);
    report
        .with_scope(scope)
        .with_items(items)
        .with_violations(violations)
}

pub fn summarize_compat(locked: bool, path: &Path) -> JsonReport {
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    summarize_compat_at_root(locked, path, &root)
}

fn summarize_compat_at_root(locked: bool, path: &Path, root: &Path) -> JsonReport {
    let lock_path = resolve_path(root, path);
    let lock_hash = file_sha256(&lock_path).ok();
    let mut items = Vec::new();
    let mut violations = Vec::new();
    let lock = load_official_lock(&lock_path).ok();
    let conformance_root = root
        .join("target")
        .join("conformance")
        .join(lock_hash.as_deref().unwrap_or("unknown-lock"));

    for target in all_targets() {
        let official_api =
            root.join(target.relative_api_manifest_path(ApiManifestSide::Official.as_str()));
        let rust_api = root.join(target.relative_api_manifest_path(ApiManifestSide::Rust.as_str()));
        let option_report = conformance_root.join("option-matrix.json");
        let output_report = conformance_root.join("output-contract.json");
        let conformance_report = conformance_root.join(conformance_report_name(*target));

        let api_status = combine_report_statuses([
            report_file_status(&official_api),
            report_file_status(&rust_api),
        ]);
        let option_status = report_file_status(&option_report);
        let output_status = report_file_status(&output_report);
        let conformance_status = report_file_status(&conformance_report);
        let lock_status = if locked {
            match &lock {
                Some(lock) => {
                    combine_report_statuses([if validate_official_lock(lock).is_empty() {
                        ReportStatus::Pass
                    } else {
                        ReportStatus::Fail
                    }])
                }
                None => ReportStatus::Fail,
            }
        } else {
            ReportStatus::Pass
        };

        let target_status = combine_report_statuses([
            api_status,
            option_status,
            output_status,
            conformance_status,
            lock_status,
        ]);
        if target_status == ReportStatus::Fail {
            if api_status == ReportStatus::Fail {
                violations.push(format!("{} missing API manifest(s)", target.display()));
            }
            if option_status == ReportStatus::Fail {
                violations.push(format!("{} missing option report", target.display()));
            }
            if output_status == ReportStatus::Fail {
                violations.push(format!("{} missing output report", target.display()));
            }
            if conformance_status == ReportStatus::Fail {
                if conformance_report.exists() {
                    violations.push(format!("{} conformance failed", target.display()));
                } else {
                    violations.push(format!("{} missing conformance report", target.display()));
                }
            }
            if lock_status == ReportStatus::Fail {
                violations.push(format!(
                    "{} official lock validation failed",
                    target.display()
                ));
            }
        }

        items.push(ReportItem::new(
            target.display(),
            target_status,
            format!(
                "api={}, options={}, output={}, conformance={}, lock={}",
                api_status.as_str(),
                option_status.as_str(),
                output_status.as_str(),
                conformance_status.as_str(),
                lock_status.as_str()
            ),
            Some(conformance_report),
        ));
    }

    let mut report = JsonReport::new("summarize_compat", aggregate_status(&items));
    report.metadata = report.metadata.with_lock_hash(lock_hash);
    report
        .with_items(items)
        .with_violations(violations)
        .with_note(if locked {
            "summary aggregates lock validation plus API, option, output, and conformance artifacts"
        } else {
            "summary aggregates API, option, output, and conformance artifacts"
        })
}

fn report_file_status(path: &Path) -> ReportStatus {
    let Ok(data) = fs::read_to_string(path) else {
        return ReportStatus::Pending;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&data) else {
        return ReportStatus::Fail;
    };
    report_value_status(&value)
}

fn report_value_status(value: &serde_json::Value) -> ReportStatus {
    let mut seen = false;
    let mut status = ReportStatus::Pass;

    fn merge_status(seen: &mut bool, status: &mut ReportStatus, next: ReportStatus) {
        *seen = true;
        match (*status, next) {
            (ReportStatus::Fail, _) => {}
            (_, ReportStatus::Fail) => *status = ReportStatus::Fail,
            (ReportStatus::Pending, _) => {}
            (_, ReportStatus::Pending) => *status = ReportStatus::Pending,
            _ => {}
        }
    }

    if let Some(value) = value.get("status").and_then(|value| value.as_str()) {
        merge_status(
            &mut seen,
            &mut status,
            match value {
                "pass" => ReportStatus::Pass,
                "pending" => ReportStatus::Pending,
                _ => ReportStatus::Fail,
            },
        );
    }

    if let Some(counts) = value.get("counts").and_then(|value| value.as_object()) {
        seen = true;
        if counts
            .get("fail")
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
            > 0
        {
            status = ReportStatus::Fail;
        } else if counts
            .get("pending")
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
            > 0
            && status == ReportStatus::Pass
        {
            status = ReportStatus::Pending;
        }
    }

    if let Some(checks) = value.get("checks").and_then(|value| value.as_array()) {
        for check in checks {
            seen = true;
            match check.get("status").and_then(|value| value.as_str()) {
                Some("pass") => {}
                Some("pending") => merge_status(&mut seen, &mut status, ReportStatus::Pending),
                Some(_) | None => merge_status(&mut seen, &mut status, ReportStatus::Fail),
            }
        }
    }

    if let Some(rows) = value.get("rows").and_then(|value| value.as_array()) {
        for row in rows {
            seen = true;
            match row.get("status").and_then(|value| value.as_str()) {
                Some("pass") => {}
                Some("pending") => merge_status(&mut seen, &mut status, ReportStatus::Pending),
                Some(_) | None => merge_status(&mut seen, &mut status, ReportStatus::Fail),
            }
        }
    }

    if let Some(smokes) = value.get("smoke").and_then(|value| value.as_array()) {
        for smoke in smokes {
            seen = true;
            match smoke.get("status").and_then(|value| value.as_str()) {
                Some("pass") => {}
                Some("pending") => merge_status(&mut seen, &mut status, ReportStatus::Pending),
                Some(_) | None => merge_status(&mut seen, &mut status, ReportStatus::Fail),
            }
        }
    }

    if let Some(targets) = value.get("targets").and_then(|value| value.as_array()) {
        for target in targets {
            merge_status(&mut seen, &mut status, report_value_status(target));
        }
    }

    if seen {
        status
    } else {
        ReportStatus::Fail
    }
}

fn combine_report_statuses<const N: usize>(statuses: [ReportStatus; N]) -> ReportStatus {
    if statuses.iter().any(|status| *status == ReportStatus::Fail) {
        ReportStatus::Fail
    } else if statuses
        .iter()
        .any(|status| *status == ReportStatus::Pending)
    {
        ReportStatus::Pending
    } else {
        ReportStatus::Pass
    }
}

fn conformance_report_name(target: TargetSpec) -> &'static str {
    match target.kind {
        TargetKind::Vue26Template => "vue2-compiler.json",
        TargetKind::Vue27Template => "vue27-compiler.json",
        TargetKind::Vue27Sfc => "vue27-sfc.json",
        TargetKind::Vue3Core => "vue3-core.json",
        TargetKind::Vue3Dom => "vue3-dom.json",
        TargetKind::Vue3Ssr => "vue3-ssr.json",
        TargetKind::Vue3Sfc => "vue3-sfc.json",
    }
}

fn resolve_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

pub fn load_official_lock(path: &Path) -> Result<OfficialRevisionsLock> {
    let data = fs::read_to_string(path)
        .with_context(|| format!("failed to read lock file {}", path.display()))?;
    toml::from_str(&data).with_context(|| format!("failed to parse lock file {}", path.display()))
}

pub fn validate_official_lock(lock: &OfficialRevisionsLock) -> Vec<String> {
    let mut violations = Vec::new();
    validate_baseline(
        "vue2_6",
        &lock.vue2_6,
        &["vue", "vue-template-compiler"],
        &[],
        &mut violations,
    );
    validate_baseline(
        "vue2_7",
        &lock.vue2_7,
        &["vue", "vue-template-compiler"],
        &["vue/compiler-sfc"],
        &mut violations,
    );
    validate_baseline(
        "vue3",
        &lock.vue3,
        &[
            "vue",
            "@vue/compiler-core",
            "@vue/compiler-dom",
            "@vue/compiler-ssr",
            "@vue/compiler-sfc",
        ],
        &[],
        &mut violations,
    );
    violations
}

#[derive(Debug, Deserialize)]
pub struct OfficialRevisionsLock {
    pub vue2_6: BaselineLock,
    pub vue2_7: BaselineLock,
    pub vue3: BaselineLock,
}

#[derive(Debug, Deserialize)]
pub struct BaselineLock {
    pub repo: String,
    pub rev: String,
    #[serde(default)]
    pub npm: BTreeMap<String, String>,
    #[serde(default)]
    pub exports: BTreeMap<String, String>,
}

fn validate_baseline(
    label: &str,
    baseline: &BaselineLock,
    required_npm: &[&str],
    required_exports: &[&str],
    violations: &mut Vec<String>,
) {
    if baseline.repo.trim().is_empty() {
        violations.push(format!("{label}.repo is empty"));
    }
    if !is_commit_sha(&baseline.rev) {
        violations.push(format!("{label}.rev is not a 40-character commit SHA"));
    }
    for key in required_npm {
        match baseline.npm.get(*key) {
            Some(value) if !value.trim().is_empty() => {}
            Some(_) => violations.push(format!("{label}.npm.{key} is empty")),
            None => violations.push(format!("{label}.npm.{key} is missing")),
        }
    }
    for key in required_exports {
        match baseline.exports.get(*key) {
            Some(value) if !value.trim().is_empty() => {}
            Some(_) => violations.push(format!("{label}.exports[{key:?}] is empty")),
            None => violations.push(format!("{label}.exports[{key:?}] is missing")),
        }
    }
}

fn is_commit_sha(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn sanitize_segment(segment: &str) -> String {
    segment
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | '@' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => ch,
        })
        .collect()
}

fn select_targets(scope: &SelectionArgs) -> Vec<TargetSpec> {
    let mut targets = Vec::new();
    for target in all_targets() {
        if let Some(version_line) = scope.version_line {
            if target.version_line != version_line {
                continue;
            }
        }
        if let Some(package) = scope.package.as_deref() {
            if target.package != package {
                continue;
            }
        }
        if let Some(entry) = scope.entry.as_deref() {
            if target.entry != entry {
                continue;
            }
        }
        targets.push(*target);
    }
    if targets.is_empty() && scope.all {
        targets.extend_from_slice(all_targets());
    }
    if targets.is_empty() {
        targets.extend_from_slice(all_targets());
    }
    targets
}

#[derive(Clone, Copy)]
struct ConformanceSuiteSpec {
    name: &'static str,
    version_line: VersionLine,
    relative_test_dirs: &'static [&'static str],
    package_requests: &'static [&'static str],
    runner_dependencies: &'static [&'static str],
}

#[derive(Clone, Debug, Serialize)]
struct ConformanceReadiness {
    alias_ready: bool,
    runner_ready: bool,
    package_requests: Vec<String>,
    runner_dependencies: Vec<String>,
    missing_alias_packages: Vec<String>,
    missing_runner_dependencies: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct ConformanceSmokeResult {
    request: String,
    status: String,
    detail: String,
}

#[derive(Clone, Debug, Serialize)]
struct ConformanceExecutionResult {
    status: String,
    runner: String,
    prepared_root: String,
    output_file: String,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    counts: ConformanceExecutionCounts,
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
struct ConformanceExecutionCounts {
    total: usize,
    pass: usize,
    fail: usize,
    skip: usize,
    pending: usize,
}

fn run_conformance_smokes(spec: ConformanceSuiteSpec) -> Vec<ConformanceSmokeResult> {
    conformance_smoke_targets(spec)
        .into_iter()
        .map(|target| {
            let request = api_require_request(target);
            match run_alias_smoke(target, &rust_alias_root(target.version_line)) {
                Ok(detail) => ConformanceSmokeResult {
                    request,
                    status: "pass".into(),
                    detail,
                },
                Err(err) => ConformanceSmokeResult {
                    request,
                    status: "fail".into(),
                    detail: format!("{err:#}"),
                },
            }
        })
        .collect()
}

fn conformance_smoke_targets(spec: ConformanceSuiteSpec) -> Vec<TargetSpec> {
    all_targets()
        .iter()
        .copied()
        .filter(|target| {
            target.version_line == spec.version_line
                && spec
                    .package_requests
                    .iter()
                    .any(|request| api_require_request(*target) == *request)
        })
        .collect()
}

fn run_conformance_execution(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    discovered: &[String],
    lock_hash: Option<&str>,
) -> Result<ConformanceExecutionResult> {
    match spec.name {
        "vue3-core" => run_vue3_core_conformance(spec, official_root, discovered, lock_hash),
        _ => Ok(ConformanceExecutionResult {
            status: "pending".into(),
            runner: "not-wired".into(),
            prepared_root: String::new(),
            output_file: String::new(),
            exit_code: None,
            stdout: String::new(),
            stderr: format!("{} official execution is not wired yet", spec.name),
            counts: ConformanceExecutionCounts {
                total: discovered.len(),
                pending: discovered.len(),
                ..ConformanceExecutionCounts::default()
            },
        }),
    }
}

fn run_vue3_core_conformance(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    discovered: &[String],
    lock_hash: Option<&str>,
) -> Result<ConformanceExecutionResult> {
    let prepared_root = prepare_vue3_core_conformance_suite(spec, official_root, lock_hash)?;
    let output_file = prepared_root.join("vitest-report.json");
    let npm_root = PathBuf::from("target")
        .join("compat")
        .join("npm")
        .join(spec.version_line.as_str());
    let alias_root = rust_alias_root(spec.version_line);
    let absolute_npm_root = absolute_path(&npm_root);
    let absolute_alias_root = absolute_path(&alias_root);
    let absolute_prepared_root = absolute_path(&prepared_root);
    let absolute_output_file = absolute_path(&output_file);
    let node_modules = absolute_npm_root.join("node_modules");
    let vitest_bin = node_modules
        .join("vitest")
        .join("vitest.mjs")
        .display()
        .to_string();
    let node = resolve_program("node");
    let output = Command::new(node)
        .arg(vitest_bin)
        .arg("run")
        .arg("--globals")
        .arg("--reporter=json")
        .arg(format!("--outputFile={}", absolute_output_file.display()))
        .env("VUEC_RUST_ALIAS_ROOT", &absolute_alias_root)
        .env("VUEC_OFFICIAL_NPM_ROOT", &absolute_npm_root)
        .env(
            "NODE_PATH",
            conformance_node_path(&absolute_alias_root, &absolute_npm_root),
        )
        .current_dir(&absolute_prepared_root)
        .output()
        .with_context(|| format!("failed to spawn Vitest for {}", spec.name))?;
    let stdout = normalize_conformance_output(&String::from_utf8_lossy(&output.stdout));
    let stderr = normalize_conformance_output(&String::from_utf8_lossy(&output.stderr));
    let counts = read_vitest_counts(&output_file)
        .or_else(|_| read_vitest_counts(&absolute_output_file))
        .unwrap_or_else(|_| ConformanceExecutionCounts {
            total: discovered.len(),
            pending: discovered.len(),
            ..ConformanceExecutionCounts::default()
        });
    let status = if counts.fail > 0 || !output.status.success() {
        "failed"
    } else {
        "executed"
    };
    Ok(ConformanceExecutionResult {
        status: status.into(),
        runner: "vitest".into(),
        prepared_root: prepared_root.display().to_string(),
        output_file: output_file.display().to_string(),
        exit_code: output.status.code(),
        stdout,
        stderr,
        counts,
    })
}

fn prepare_vue3_core_conformance_suite(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    lock_hash: Option<&str>,
) -> Result<PathBuf> {
    let prepared_root = PathBuf::from("target")
        .join("conformance")
        .join(lock_hash.unwrap_or("unknown-lock"))
        .join("prepared")
        .join(spec.name);
    if prepared_root.exists() {
        fs::remove_dir_all(&prepared_root)
            .with_context(|| format!("failed to remove {}", prepared_root.display()))?;
    }
    let official_tests = official_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__");
    let prepared_tests = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__");
    copy_dir_recursive(&official_tests, &prepared_tests)?;
    write_vue3_core_conformance_shims(&prepared_root)?;
    Ok(prepared_root)
}

fn write_vue3_core_conformance_shims(prepared_root: &Path) -> Result<()> {
    let core_src = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("src");
    let transforms = core_src.join("transforms");
    fs::create_dir_all(&transforms)
        .with_context(|| format!("failed to create {}", transforms.display()))?;
    for module in [
        "index",
        "ast",
        "codegen",
        "compile",
        "errors",
        "options",
        "parser",
        "runtimeHelpers",
        "transform",
        "utils",
    ] {
        write_reexport_module(&core_src.join(format!("{module}.ts")), "@vue/compiler-core")?;
    }
    for module in [
        "transformElement",
        "transformExpression",
        "transformSlotOutlet",
        "transformText",
        "transformVBindShorthand",
        "vBind",
        "vFor",
        "vIf",
        "vMemo",
        "vModel",
        "vOn",
        "vOnce",
        "vSlot",
    ] {
        write_vue3_core_transform_shim(&transforms.join(format!("{module}.ts")), module)?;
    }

    let dom_transform = prepared_root
        .join("packages")
        .join("compiler-dom")
        .join("src")
        .join("transforms");
    fs::create_dir_all(&dom_transform)
        .with_context(|| format!("failed to create {}", dom_transform.display()))?;
    write_reexport_module(
        &dom_transform.join("transformStyle.ts"),
        "@vue/compiler-dom",
    )?;

    let shared_src = prepared_root.join("packages").join("shared").join("src");
    fs::create_dir_all(&shared_src)
        .with_context(|| format!("failed to create {}", shared_src.display()))?;
    write_reexport_module(&shared_src.join("index.ts"), "@vue/shared")?;

    let config = r#"
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.dirname(fileURLToPath(import.meta.url))
const aliasRoot = process.env.VUEC_RUST_ALIAS_ROOT
const npmRoot = process.env.VUEC_OFFICIAL_NPM_ROOT

export default {
  define: {
    __DEV__: true,
    __TEST__: true,
    __VERSION__: '"test"',
    __BROWSER__: false,
    __GLOBAL__: false,
    __ESM_BUNDLER__: true,
    __ESM_BROWSER__: false,
    __CJS__: true,
    __SSR__: true,
    __FEATURE_OPTIONS_API__: true,
    __FEATURE_SUSPENSE__: true,
    __FEATURE_PROD_DEVTOOLS__: false,
    __FEATURE_PROD_HYDRATION_MISMATCH_DETAILS__: false,
    __COMPAT__: true,
  },
  resolve: {
    alias: {
      '@vue/compiler-core': path.resolve(aliasRoot, 'node_modules/@vue/compiler-core/index.js'),
      '@vue/compiler-dom': path.resolve(aliasRoot, 'node_modules/@vue/compiler-dom/index.js'),
      '@vue/compiler-sfc': path.resolve(aliasRoot, 'node_modules/@vue/compiler-sfc/dist/compiler-sfc.cjs.js'),
      '@vue/shared': path.resolve(npmRoot, 'node_modules/@vue/shared/index.js'),
      'source-map-js': path.resolve(npmRoot, 'node_modules/source-map-js/source-map.js'),
    },
  },
  test: {
    globals: true,
    pool: 'forks',
    include: ['packages/compiler-core/__tests__/**/*.spec.ts'],
  },
}
"#;
    write_text(&prepared_root.join("vitest.config.ts"), config)?;
    Ok(())
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

fn copy_dir_recursive(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).with_context(|| format!("failed to create {}", to.display()))?;
    for entry in fs::read_dir(from).with_context(|| format!("failed to read {}", from.display()))? {
        let entry = entry?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        if source.is_dir() {
            copy_dir_recursive(&source, &target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::copy(&source, &target).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source.display(),
                    target.display()
                )
            })?;
        }
    }
    Ok(())
}

fn conformance_node_path(alias_root: &Path, npm_root: &Path) -> String {
    std::env::join_paths([
        alias_root.join("node_modules"),
        npm_root.join("node_modules"),
    ])
    .map(|value| value.to_string_lossy().to_string())
    .unwrap_or_else(|_| {
        format!(
            "{}{}{}",
            alias_root.join("node_modules").display(),
            if cfg!(windows) { ";" } else { ":" },
            npm_root.join("node_modules").display()
        )
    })
}

fn normalize_conformance_output(output: &str) -> String {
    output
        .replace('\\', "/")
        .lines()
        .take(200)
        .collect::<Vec<_>>()
        .join("\n")
}

fn read_vitest_counts(path: &Path) -> Result<ConformanceExecutionCounts> {
    let value = read_json::<serde_json::Value>(path)?;
    let total = json_usize(&value, &["numTotalTests"]);
    let failed_suites = json_usize(&value, &["numFailedTestSuites"]);
    let fail =
        json_usize(&value, &["numFailedTests"]).max(if total == 0 { failed_suites } else { 0 });
    let skip = json_usize(&value, &["numPendingTests"]) + json_usize(&value, &["numTodoTests"]);
    let pass = json_usize(&value, &["numPassedTests"]);
    let pending = total.saturating_sub(pass + fail + skip);
    Ok(ConformanceExecutionCounts {
        total,
        pass,
        fail,
        skip,
        pending,
    })
}

fn conformance_targets(suites: &[ConformanceSuite]) -> Vec<TargetSpec> {
    let mut targets = Vec::new();
    for suite in suites {
        for target in conformance_smoke_targets(suite_spec(*suite)) {
            if !targets.contains(&target) {
                targets.push(target);
            }
        }
    }
    targets
}

fn conformance_readiness(spec: ConformanceSuiteSpec) -> ConformanceReadiness {
    let alias_root = rust_alias_root(spec.version_line);
    let npm_root = PathBuf::from("target")
        .join("compat")
        .join("npm")
        .join(spec.version_line.as_str())
        .join("node_modules");
    let missing_alias_packages = spec
        .package_requests
        .iter()
        .filter(|request| !alias_package_available(&alias_root, request))
        .map(|request| (*request).to_string())
        .collect::<Vec<_>>();
    let missing_runner_dependencies = spec
        .runner_dependencies
        .iter()
        .filter(|dependency| !node_dependency_available(&npm_root, dependency))
        .map(|dependency| (*dependency).to_string())
        .collect::<Vec<_>>();
    ConformanceReadiness {
        alias_ready: missing_alias_packages.is_empty(),
        runner_ready: missing_runner_dependencies.is_empty(),
        package_requests: spec
            .package_requests
            .iter()
            .map(|request| (*request).to_string())
            .collect(),
        runner_dependencies: spec
            .runner_dependencies
            .iter()
            .map(|dependency| (*dependency).to_string())
            .collect(),
        missing_alias_packages,
        missing_runner_dependencies,
    }
}

fn alias_package_available(alias_root: &Path, request: &str) -> bool {
    if request == "vue/compiler-sfc" {
        return alias_root
            .join("node_modules")
            .join("vue")
            .join("compiler-sfc")
            .join("index.js")
            .is_file();
    }
    node_dependency_available(&alias_root.join("node_modules"), request)
}

fn node_dependency_available(node_modules: &Path, request: &str) -> bool {
    let segments = request.split('/').collect::<Vec<_>>();
    let package_dir = if request.starts_with('@') && segments.len() >= 2 {
        node_modules.join(segments[0]).join(segments[1])
    } else {
        node_modules.join(segments[0])
    };
    package_dir.join("package.json").is_file() || package_dir.join("index.js").is_file()
}

fn conformance_item_detail(
    test_count: usize,
    readiness: &ConformanceReadiness,
    execution: Option<&ConformanceExecutionResult>,
) -> String {
    if let Some(execution) = execution {
        return format!(
            "{}/{} official tests passed, {} failed, {} skipped, {} pending",
            execution.counts.pass,
            execution.counts.total,
            execution.counts.fail,
            execution.counts.skip,
            execution.counts.pending
        );
    }
    if readiness.alias_ready && readiness.runner_ready {
        return format!("{test_count} official test files discovered; runner is ready to execute");
    }
    let mut missing = Vec::new();
    if !readiness.alias_ready {
        missing.push(format!(
            "missing alias packages: {}",
            readiness.missing_alias_packages.join(", ")
        ));
    }
    if !readiness.runner_ready {
        missing.push(format!(
            "missing runner dependencies: {}",
            readiness.missing_runner_dependencies.join(", ")
        ));
    }
    format!(
        "{test_count} official test files discovered; execution blocked by {}",
        missing.join("; ")
    )
}

fn suite_spec(suite: ConformanceSuite) -> ConformanceSuiteSpec {
    match suite {
        ConformanceSuite::Vue2Compiler => ConformanceSuiteSpec {
            name: "vue2-compiler",
            version_line: VersionLine::Vue26,
            relative_test_dirs: &["test/unit/modules/compiler"],
            package_requests: &["vue-template-compiler"],
            runner_dependencies: &["@babel/register", "jasmine"],
        },
        ConformanceSuite::Vue27Compiler => ConformanceSuiteSpec {
            name: "vue27-compiler",
            version_line: VersionLine::Vue27,
            relative_test_dirs: &["test/unit/modules/compiler"],
            package_requests: &["vue-template-compiler"],
            runner_dependencies: &["vitest", "esbuild", "typescript"],
        },
        ConformanceSuite::Vue27Sfc => ConformanceSuiteSpec {
            name: "vue27-sfc",
            version_line: VersionLine::Vue27,
            relative_test_dirs: &["packages/compiler-sfc/test"],
            package_requests: &["vue/compiler-sfc"],
            runner_dependencies: &["vitest", "esbuild", "typescript"],
        },
        ConformanceSuite::Vue3Core => ConformanceSuiteSpec {
            name: "vue3-core",
            version_line: VersionLine::Vue3,
            relative_test_dirs: &["packages/compiler-core/__tests__"],
            package_requests: &["@vue/compiler-core"],
            runner_dependencies: &["vitest", "esbuild", "source-map-js"],
        },
        ConformanceSuite::Vue3Dom => ConformanceSuiteSpec {
            name: "vue3-dom",
            version_line: VersionLine::Vue3,
            relative_test_dirs: &["packages/compiler-dom/__tests__"],
            package_requests: &["@vue/compiler-dom", "@vue/compiler-core"],
            runner_dependencies: &["vitest", "esbuild", "source-map-js"],
        },
        ConformanceSuite::Vue3Sfc => ConformanceSuiteSpec {
            name: "vue3-sfc",
            version_line: VersionLine::Vue3,
            relative_test_dirs: &["packages/compiler-sfc/__tests__"],
            package_requests: &["@vue/compiler-sfc"],
            runner_dependencies: &["vitest", "esbuild", "typescript"],
        },
        ConformanceSuite::Vue3Ssr => ConformanceSuiteSpec {
            name: "vue3-ssr",
            version_line: VersionLine::Vue3,
            relative_test_dirs: &["packages/compiler-ssr/__tests__"],
            package_requests: &["@vue/compiler-ssr", "@vue/compiler-core"],
            runner_dependencies: &["vitest", "esbuild"],
        },
    }
}

fn select_conformance_suites(args: &ConformanceArgs) -> Vec<ConformanceSuite> {
    if args.all {
        return vec![
            ConformanceSuite::Vue2Compiler,
            ConformanceSuite::Vue27Compiler,
            ConformanceSuite::Vue27Sfc,
            ConformanceSuite::Vue3Core,
            ConformanceSuite::Vue3Dom,
            ConformanceSuite::Vue3Sfc,
            ConformanceSuite::Vue3Ssr,
        ];
    }
    args.suite
        .map(|suite| vec![suite])
        .unwrap_or_else(|| vec![ConformanceSuite::Vue3Core])
}

fn discover_test_files(dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            discover_test_files(&path, out);
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let is_test = file_name.ends_with(".spec.ts")
            || file_name.ends_with(".spec.js")
            || file_name.ends_with(".test.ts")
            || file_name.ends_with(".test.js");
        if is_test {
            out.push(path.display().to_string());
        }
    }
}

fn aggregate_status(items: &[ReportItem]) -> ReportStatus {
    if items.iter().any(|item| item.status == ReportStatus::Fail) {
        ReportStatus::Fail
    } else if items
        .iter()
        .any(|item| item.status == ReportStatus::Pending)
    {
        ReportStatus::Pending
    } else {
        ReportStatus::Pass
    }
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    fs::write(path, json).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let data =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let value = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(value)
}

fn sync_git_checkout(repo: &str, rev: &str, dir: &Path) -> Result<()> {
    if dir.join(".git").exists() {
        run_git(dir, &["fetch", "--tags", "--force", "origin"])?;
    } else {
        if let Some(parent) = dir.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let dir_arg = dir.display().to_string();
        run_command("git", &["clone", repo, &dir_arg], None)
            .with_context(|| format!("failed to clone {repo} into {}", dir.display()))?;
    }
    run_git(dir, &["checkout", "--detach", rev])?;
    run_git(dir, &["submodule", "update", "--init", "--recursive"])?;
    Ok(())
}

fn run_git(dir: &Path, args: &[&str]) -> Result<()> {
    run_command("git", args, Some(dir))
}

fn run_command(program: &str, args: &[&str], current_dir: Option<&Path>) -> Result<()> {
    let mut command = Command::new(program);
    command.args(args);
    if let Some(current_dir) = current_dir {
        command.current_dir(current_dir);
    }
    let output = command
        .output()
        .with_context(|| format!("failed to spawn {program} {}", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!(
            "`{} {}` failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
            program,
            args.join(" "),
            output.status.code(),
            stdout.trim(),
            stderr.trim()
        );
    }
    Ok(())
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn file_sha256(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_manifest_side_selection_defaults_to_both_sides() {
        let scope = SelectionArgs::default();
        assert_eq!(
            selected_api_manifest_sides(&scope),
            vec![ApiManifestSide::Official, ApiManifestSide::Rust]
        );

        let official_only = SelectionArgs {
            official: true,
            ..SelectionArgs::default()
        };
        assert_eq!(
            selected_api_manifest_sides(&official_only),
            vec![ApiManifestSide::Official]
        );

        let rust_only = SelectionArgs {
            rust: true,
            ..SelectionArgs::default()
        };
        assert_eq!(
            selected_api_manifest_sides(&rust_only),
            vec![ApiManifestSide::Rust]
        );
    }

    #[test]
    fn api_diff_detects_export_and_arity_mismatch() {
        let mut official = test_manifest(vec![("compile", 2)]);
        let mut rust = test_manifest(vec![("compile", 1)]);
        let diffs = compare_api_manifests(&official, &rust);
        assert!(
            diffs
                .iter()
                .any(|diff| diff.contains("export compile detail differs")),
            "{diffs:#?}"
        );

        rust.exports.push("extra".into());
        rust.export_details.insert(
            "extra".into(),
            ApiExportDetail {
                kind: "function".into(),
                tag: "[object Function]".into(),
                name: Some("extra".into()),
                function_arity: Some(0),
                is_async_function: Some(false),
                is_class_like: Some(false),
                own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
            },
        );
        let diffs = compare_api_manifests(&official, &rust);
        assert!(
            diffs.iter().any(|diff| diff.contains("exports differ")),
            "{diffs:#?}"
        );

        official.exports = rust.exports.clone();
        official.export_details = rust.export_details.clone();
        assert!(compare_api_manifests(&official, &rust).is_empty());
    }

    #[test]
    fn allowed_api_diff_requires_exact_target_diff_and_reason() {
        let target = TargetSpec {
            version_line: VersionLine::Vue26,
            package: "vue-template-compiler",
            entry: "index",
            kind: TargetKind::Vue26Template,
        };
        let diff = "exports differ: official=[] rust=[]";
        let allowed = AllowedApiDiffFile {
            entries: vec![AllowedApiDiffEntry {
                version_line: VersionLine::Vue26,
                package: "vue-template-compiler".into(),
                entry: "index".into(),
                diff: diff.into(),
                reason: "documented compatibility exception".into(),
            }],
        };
        assert!(is_allowed_api_diff(&allowed, target, diff));
        assert!(!is_allowed_api_diff(&allowed, target, "different diff"));
    }

    #[test]
    fn vue27_sfc_output_contract_exports_version_context() {
        let target = TargetSpec {
            version_line: VersionLine::Vue27,
            package: "vue",
            entry: "vue/compiler-sfc",
            kind: TargetKind::Vue27Sfc,
        };

        assert_eq!(output_contract_kind(target), "sfc");
        assert_eq!(api_require_request(target), "vue/compiler-sfc");
        assert!(OUTPUT_CONTRACT_PROBE_SCRIPT
            .contains("versionLine === 'vue2_7' && entry === 'vue/compiler-sfc'"));
        assert!(OUTPUT_CONTRACT_PROBE_SCRIPT.contains("api.parse({ source: fixture"));
    }

    #[test]
    fn report_value_status_uses_counts_and_nested_rows() {
        let passed = serde_json::json!({
            "counts": { "total": 1, "pass": 1, "pending": 0, "fail": 0 },
            "targets": [
                { "rows": [{ "status": "pass" }] },
                { "checks": [{ "status": "pass" }] }
            ]
        });
        assert_eq!(report_value_status(&passed), ReportStatus::Pass);

        let pending = serde_json::json!({
            "counts": { "total": 2, "pass": 1, "pending": 1, "fail": 0 },
            "targets": [{ "rows": [{ "status": "pending" }] }]
        });
        assert_eq!(report_value_status(&pending), ReportStatus::Pending);

        let failed = serde_json::json!({
            "counts": { "total": 1, "pass": 0, "pending": 0, "fail": 1 },
            "targets": [{ "checks": [{ "status": "fail" }] }]
        });
        assert_eq!(report_value_status(&failed), ReportStatus::Fail);
    }

    #[test]
    fn report_value_status_treats_discovery_only_as_pending_via_counts() {
        let discovered = serde_json::json!({
            "execution": "discovery-only",
            "counts": { "total": 3, "pass": 0, "pending": 3, "fail": 0 }
        });
        assert_eq!(report_value_status(&discovered), ReportStatus::Pending);
    }

    #[test]
    fn report_value_status_fails_on_failed_conformance_smoke() {
        let value = serde_json::json!({
            "counts": { "total": 1, "pass": 0, "pending": 1, "fail": 0 },
            "smoke": [{ "status": "fail", "request": "@vue/compiler-core" }]
        });
        assert_eq!(report_value_status(&value), ReportStatus::Fail);
    }

    #[test]
    fn vitest_counts_treat_failed_suite_without_tests_as_failure() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vitest-counts-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&temp).unwrap();
        let report = temp.join("vitest-report.json");
        fs::write(
            &report,
            r#"{
              "numTotalTestSuites": 0,
              "numFailedTestSuites": 1,
              "numTotalTests": 0,
              "numPassedTests": 0,
              "numFailedTests": 0,
              "numPendingTests": 0,
              "numTodoTests": 0
            }"#,
        )
        .unwrap();

        let counts = read_vitest_counts(&report).unwrap();
        assert_eq!(counts.total, 0);
        assert_eq!(counts.fail, 1);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn conformance_item_detail_uses_execution_counts() {
        let readiness = conformance_readiness(suite_spec(ConformanceSuite::Vue3Core));
        let execution = ConformanceExecutionResult {
            status: "failed".into(),
            runner: "vitest".into(),
            prepared_root: "prepared".into(),
            output_file: "report.json".into(),
            exit_code: Some(1),
            stdout: String::new(),
            stderr: String::new(),
            counts: ConformanceExecutionCounts {
                total: 618,
                pass: 9,
                fail: 609,
                skip: 0,
                pending: 0,
            },
        };

        assert_eq!(
            conformance_item_detail(20, &readiness, Some(&execution)),
            "9/618 official tests passed, 609 failed, 0 skipped, 0 pending"
        );
    }

    #[test]
    fn vue3_core_conformance_shims_use_relative_vitest_glob() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-core-shims-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        write_vue3_core_conformance_shims(&temp).unwrap();

        let parser = fs::read_to_string(
            temp.join("packages")
                .join("compiler-core")
                .join("src")
                .join("parser.ts"),
        )
        .unwrap();
        assert!(parser.contains("export * from \"@vue/compiler-core\""));

        let config = fs::read_to_string(temp.join("vitest.config.ts")).unwrap();
        assert!(!config.contains("vitest/config"));
        assert!(config.contains("include: ['packages/compiler-core/__tests__/**/*.spec.ts']"));
        let v_if = fs::read_to_string(
            temp.join("packages")
                .join("compiler-core")
                .join("src")
                .join("transforms")
                .join("vIf.ts"),
        )
        .unwrap();
        assert!(v_if.contains("__vuecRuntime"));
        assert!(v_if.contains("transformIf"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn node_dependency_available_handles_scoped_packages_and_subpaths() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-node-dep-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let node_modules = temp.join("node_modules");
        fs::create_dir_all(node_modules.join("@vue").join("compiler-core")).unwrap();
        fs::write(
            node_modules
                .join("@vue")
                .join("compiler-core")
                .join("package.json"),
            "{}",
        )
        .unwrap();
        fs::create_dir_all(node_modules.join("vue").join("compiler-sfc")).unwrap();
        fs::write(
            node_modules
                .join("vue")
                .join("compiler-sfc")
                .join("index.js"),
            "",
        )
        .unwrap();

        assert!(node_dependency_available(
            &node_modules,
            "@vue/compiler-core"
        ));
        assert!(alias_package_available(&temp, "vue/compiler-sfc"));
        assert!(!node_dependency_available(&node_modules, "vitest"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn conformance_targets_include_suite_package_requests() {
        let targets = conformance_targets(&[ConformanceSuite::Vue3Dom]);
        let requests = targets
            .into_iter()
            .map(api_require_request)
            .collect::<Vec<_>>();
        assert_eq!(requests, vec!["@vue/compiler-core", "@vue/compiler-dom"]);
    }

    #[test]
    fn runner_dependency_specs_use_locked_versions() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-runner-deps-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let root = temp.join("vue3");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("package.json"),
            r#"{
              "devDependencies": {
                "vitest": "^4.1.5",
                "esbuild": "^0.28.0",
                "typescript": "~5.6.2",
                "source-map-js": "catalog:"
              }
            }"#,
        )
        .unwrap();
        fs::write(
            root.join("pnpm-lock.yaml"),
            r#"
packages:
  .
snapshots:
  esbuild@0.28.0: {}
  source-map-js@1.2.1: {}
  typescript@5.6.3: {}
  vitest@4.1.5(@types/node@24.12.2): {}
"#,
        )
        .unwrap();

        let specs = runner_dependency_specs(suite_spec(ConformanceSuite::Vue3Core), &temp)
            .unwrap()
            .unwrap();
        assert_eq!(
            specs,
            vec!["esbuild@0.28.0", "source-map-js@1.2.1", "vitest@4.1.5"]
        );
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn runner_dependency_specs_fall_back_to_manifest_specs() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-runner-deps-manifest-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let root = temp.join("vue2_6");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("package.json"),
            r#"{
              "devDependencies": {
                "@babel/register": "^7.0.0",
                "jasmine": "^2.99.0"
              }
            }"#,
        )
        .unwrap();

        let specs = runner_dependency_specs(suite_spec(ConformanceSuite::Vue2Compiler), &temp)
            .unwrap()
            .unwrap();
        assert_eq!(specs, vec!["@babel/register@^7.0.0", "jasmine@^2.99.0"]);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn yarn_lock_dependency_lookup_matches_exact_package_name() {
        let lock = r#"
"@babel/register@^7.0.0":
  version "7.0.0"

eslint-plugin-jasmine@^2.8.4:
  version "2.10.1"

jasmine@^2.99.0:
  version "2.99.0"
"#;
        assert_eq!(
            locked_yarn_dependency_version(lock, "@babel/register"),
            Some("7.0.0".into())
        );
        assert_eq!(
            locked_yarn_dependency_version(lock, "jasmine"),
            Some("2.99.0".into())
        );
    }

    fn test_manifest(exports: Vec<(&str, u32)>) -> ManifestFile {
        let mut export_names = Vec::new();
        let mut export_details = BTreeMap::new();
        for (name, arity) in exports {
            export_names.push(name.to_string());
            export_details.insert(
                name.to_string(),
                ApiExportDetail {
                    kind: "function".into(),
                    tag: "[object Function]".into(),
                    name: Some(name.to_string()),
                    function_arity: Some(arity),
                    is_async_function: Some(false),
                    is_class_like: Some(false),
                    own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
                },
            );
        }
        ManifestFile {
            schema_version: 1,
            version_line: VersionLine::Vue26,
            package: "vue-template-compiler".into(),
            entry: "index".into(),
            package_version: Some("2.6.14".into()),
            exports: export_names,
            export_details,
            require: ApiRequireRecord {
                request: "vue-template-compiler".into(),
                success: true,
                resolved: Some("<probe-root>/node_modules/vue-template-compiler/index.js".into()),
                error_name: None,
                error_code: None,
                error_message: None,
            },
            types: ApiTypesRecord {
                package_types: Some("types/index.d.ts".into()),
                resolved: Some(
                    "<probe-root>/node_modules/vue-template-compiler/types/index.d.ts".into(),
                ),
                exists: true,
            },
            status: "pass".into(),
            source: "official".into(),
            lock_hash: Some("lock".into()),
            official_revision: Some("af43c9d14dd087b9852912bd15b1eacbda0e13b0".into()),
        }
    }
}
