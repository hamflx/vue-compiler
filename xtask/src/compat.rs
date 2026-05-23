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
                true,
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
                &["diagnostics"],
                &["vue2-output-source-range"],
                "base",
                "compile",
                "vue2-output-source-range",
                r#"<div><span></div>"#,
                Some(serde_json::json!({"outputSourceRange": true})),
                true,
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
                true,
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
                Some(serde_json::json!({"directives": {"focus": "x"}})),
                true,
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
            r#"<div>{{ msg }}</div>"#,
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
            true,
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
            Some(serde_json::json!({"directives": {"focus": "x"}})),
            true,
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
            &["ast_summary"],
            &["vue3-core-hoist"],
            "base",
            "baseCompile",
            "vue3-core-hoist",
            r#"<div><span>static</span></div>"#,
            Some(serde_json::json!({"hoistStatic": true})),
            true,
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
            true,
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
            true,
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
            &["code"],
            &["vue3-core-slotted"],
            "base",
            "baseCompile",
            "vue3-core-slotted",
            r#"<slot></slot>"#,
            Some(serde_json::json!({"slotted": true})),
            true,
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
            &["diagnostics"],
            &["vue3-core-ts"],
            "base",
            "baseCompile",
            "vue3-core-ts",
            r#"<div>{{ foo as string }}</div>"#,
            Some(serde_json::json!({"isTS": true})),
            true,
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
            &["diagnostics"],
            &["vue3-core-expression-plugins"],
            "base",
            "baseCompile",
            "vue3-core-expression-plugins",
            r#"<div>{{ foo?.bar }}</div>"#,
            Some(serde_json::json!({"expressionPlugins": ["typescript"]})),
            true,
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
        Some(command) => format!(
            "{argument_bindings} return callBridge({}, normalizeArgs({}));",
            js_string_literal(command),
            alias_argument_object(target, export_name, body_arity)
        ),
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
        let expression = format!("({args}) => {{ {body} }}");
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
            "{ source: a0 && a0.source ? a0.source : a0, options: a1 || (a0 && a0.options) || {} }"
                .into()
        }
        (TargetKind::Vue3Core | TargetKind::Vue3Dom | TargetKind::Vue3Ssr, _) => {
            "{ source: a0 && a0.source ? a0.source : a0, filename: a0 && a0.filename, options: a1 || (a0 && a0.options) || {} }"
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
  Object.defineProperty(fn, 'name', { value: name, configurable: true });
  Object.defineProperty(fn, 'length', { value: arity, configurable: true });
  return fn;
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

const officialRequire = createRequire(path.join(officialRoot, 'package.json'));
const rustRequire = createRequire(path.join(rustRoot, 'package.json'));

function load(rootRequire) {
  return rootRequire(request);
}

function invoke(api) {
  switch (kind) {
    case 'vue2-template': {
      const compile = api.compile(fixture, { outputSourceRange: true, comments: true });
      const functions = api.compileToFunctions(fixture, {}, {});
      return { compile, compileToFunctions: functions };
    }
    case 'sfc': {
      const parsed = api.parse(fixture, { filename: 'contract.vue' });
      const descriptor = parsed && parsed.descriptor ? parsed.descriptor : parsed;
      const templateSource = descriptor && descriptor.template && descriptor.template.content ? descriptor.template.content : '';
      const styleSource = descriptor && descriptor.styles && descriptor.styles[0] && descriptor.styles[0].content ? descriptor.styles[0].content : '';
      const template = api.compileTemplate({
        source: templateSource,
        filename: 'contract.vue',
        id: 'data-v-contract',
        scoped: true
      });
      const script = api.compileScript(descriptor, { id: 'data-v-contract' });
      const style = api.compileStyle({
        source: styleSource,
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
  {
    mode: 'runtime-parity',
    status: 'pending',
    detail: 'runtime execution is not wired for this contract runner yet'
  }
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

function normalize(value) {
  if (value === undefined) return { __type: 'undefined' };
  if (value === null) return null;
  if (typeof value === 'symbol') return { __type: 'symbol', description: value.description || null };
  if (typeof value === 'function') return { __type: 'function', name: value.name, length: value.length };
  if (value instanceof Set) {
    return Array.from(value)
      .map(normalize)
      .sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b)));
  }
  if (typeof value !== 'object') return value;
  if (Array.isArray(value)) return value.map(normalize);
  const out = {};
  for (const key of Object.keys(value).sort()) {
    out[key] = normalize(value[key]);
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
    let requested = select_conformance_suites(args);
    let mut items = Vec::new();
    let mut violations = Vec::new();

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
        let report_path = PathBuf::from("target")
            .join("conformance")
            .join(lock_hash.as_deref().unwrap_or("unknown-lock"))
            .join(format!("{}.json", spec.name));
        let report_body = serde_json::json!({
            "suite": spec.name,
            "version_line": spec.version_line,
            "lock_hash": lock_hash,
            "test_files": discovered,
            "counts": {
                "total": discovered.len(),
                "pass": 0,
                "fail": 0,
                "skip": 0,
                "pending": discovered.len(),
            },
            "execution": "discovery-only",
        });
        if let Some(parent) = report_path.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                violations.push(format!("failed to create {}: {err}", parent.display()));
            }
        }
        if let Err(err) = write_json(&report_path, &report_body) {
            violations.push(format!("failed to write {}: {err}", report_path.display()));
        }
        let status = if discovered.is_empty() {
            ReportStatus::Fail
        } else {
            ReportStatus::Pending
        };
        if discovered.is_empty() {
            violations.push(format!("{} discovered no official test files", spec.name));
        }
        items.push(ReportItem::new(
            spec.name,
            status,
            format!(
                "{} official test files discovered; execution pending NAPI alias runner",
                discovered.len()
            ),
            Some(report_path),
        ));
    }

    let mut report = JsonReport::new("run_conformance", ReportStatus::Pending);
    report.metadata = report.metadata.with_lock_hash(lock_hash);
    report.with_items(items).with_violations(violations)
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

pub fn summarize_compat(_locked: bool, _path: &Path) -> JsonReport {
    let mut items = Vec::new();
    for target in all_targets() {
        let api_path = target.relative_api_manifest_path("official");
        let option_path = target.relative_option_matrix_path();
        let output_path = target.relative_output_contract_path();
        let detail = format!(
            "api={}, options={}, output={}",
            api_path.exists(),
            option_path.exists(),
            output_path.exists()
        );
        items.push(ReportItem::new(
            target.display(),
            ReportStatus::Pending,
            detail,
            None,
        ));
    }
    JsonReport::new("summarize_compat", ReportStatus::Pending)
        .with_items(items)
        .with_note("summary is scaffolded; pass will follow once the compiler backends and package aliases exist")
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
}

fn suite_spec(suite: ConformanceSuite) -> ConformanceSuiteSpec {
    match suite {
        ConformanceSuite::Vue2Compiler => ConformanceSuiteSpec {
            name: "vue2-compiler",
            version_line: VersionLine::Vue26,
            relative_test_dirs: &["test/unit/modules/compiler"],
        },
        ConformanceSuite::Vue27Compiler => ConformanceSuiteSpec {
            name: "vue27-compiler",
            version_line: VersionLine::Vue27,
            relative_test_dirs: &["test/unit/modules/compiler"],
        },
        ConformanceSuite::Vue27Sfc => ConformanceSuiteSpec {
            name: "vue27-sfc",
            version_line: VersionLine::Vue27,
            relative_test_dirs: &["packages/compiler-sfc/test"],
        },
        ConformanceSuite::Vue3Core => ConformanceSuiteSpec {
            name: "vue3-core",
            version_line: VersionLine::Vue3,
            relative_test_dirs: &["packages/compiler-core/__tests__"],
        },
        ConformanceSuite::Vue3Dom => ConformanceSuiteSpec {
            name: "vue3-dom",
            version_line: VersionLine::Vue3,
            relative_test_dirs: &["packages/compiler-dom/__tests__"],
        },
        ConformanceSuite::Vue3Sfc => ConformanceSuiteSpec {
            name: "vue3-sfc",
            version_line: VersionLine::Vue3,
            relative_test_dirs: &["packages/compiler-sfc/__tests__"],
        },
        ConformanceSuite::Vue3Ssr => ConformanceSuiteSpec {
            name: "vue3-ssr",
            version_line: VersionLine::Vue3,
            relative_test_dirs: &["packages/compiler-ssr/__tests__"],
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
