#![forbid(unsafe_code)]

use anyhow::{ensure, Context, Result};
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
    pub official_commits: BTreeMap<String, String>,
    pub rust_compiler_commit: Option<String>,
    pub created_unix: u64,
}

impl ReportMetadata {
    fn capture() -> Self {
        let mut metadata = Self {
            lock_hash: None,
            os: std::env::consts::OS.to_string(),
            rustc: command_output("rustc", &["--version"]),
            node: command_output("node", &["--version"]),
            official_commits: BTreeMap::new(),
            rust_compiler_commit: command_output("git", &["rev-parse", "HEAD"]),
            created_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or_default(),
        };
        if let Some((lock_hash, lock)) = default_official_lock_context() {
            metadata = metadata.with_lock_context(Some(lock_hash), Some(&lock));
        }
        metadata
    }

    fn with_lock_context(
        mut self,
        lock_hash: Option<String>,
        lock: Option<&OfficialRevisionsLock>,
    ) -> Self {
        self.lock_hash = lock_hash;
        self.official_commits = lock.map(official_commit_map).unwrap_or_default();
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
        option_case(
            "compileStyle.sourceMap",
            "compileStyle.sourceMap",
            &["boolean"],
            "false",
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
            Some(
                serde_json::json!({"id": "data-v-contract", "scoped": true, "vars": ["color"], "sourceMap": true}),
            ),
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
        option_case(
            "compileStyle.sourceMap",
            "compileStyle.sourceMap",
            &["boolean"],
            "false",
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
            Some(
                serde_json::json!({"id": "data-v-contract", "scoped": true, "vars": ["color"], "sourceMap": true}),
            ),
            false,
        ),
    ]
}

pub fn verify_official_lock(path: &Path, vendor_dir: &Path, require_vendor: bool) -> JsonReport {
    let lock_hash = file_sha256(path).ok();
    match load_official_lock(path) {
        Ok(lock) => {
            let mut items = Vec::new();
            let mut violations = validate_official_lock(&lock);
            let vendor_validation = if require_vendor || vendor_dir.exists() {
                validate_official_lock_vendor(&lock, vendor_dir)
            } else {
                Vec::new()
            };
            if require_vendor {
                for item in &vendor_validation {
                    if item.status == ReportStatus::Fail {
                        violations.push(item.detail.clone());
                    }
                }
            }
            items.extend(official_lock_static_items(&lock));
            items.extend(vendor_validation);
            let status = if violations.is_empty() {
                ReportStatus::Pass
            } else {
                ReportStatus::Fail
            };
            let mut report = JsonReport::new("verify_official_lock", status);
            report.metadata = report.metadata.with_lock_context(lock_hash, Some(&lock));
            report
                .with_items(items)
                .with_violations(violations)
                .with_note(format!(
                    "lock: {}, vendor: {}, require_vendor: {}",
                    path.display(),
                    vendor_dir.display(),
                    require_vendor
                ))
        }
        Err(err) => {
            let mut report = JsonReport::new("verify_official_lock", ReportStatus::Fail);
            report.metadata = report.metadata.with_lock_context(lock_hash, None);
            report
                .with_violations(vec![format!("failed to read/parse lock file: {err}")])
                .with_note(format!(
                    "lock: {}, vendor: {}, require_vendor: {}",
                    path.display(),
                    vendor_dir.display(),
                    require_vendor
                ))
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
                    let mut report = JsonReport::new("sync_official_tests", ReportStatus::Fail);
                    report.metadata = report
                        .metadata
                        .with_lock_context(lock_hash.clone(), Some(&lock));
                    return report
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
                    let mut report = JsonReport::new("sync_official_tests", ReportStatus::Fail);
                    report.metadata = report
                        .metadata
                        .with_lock_context(lock_hash.clone(), Some(&lock));
                    return report
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
            report.metadata = report.metadata.with_lock_context(lock_hash, Some(&lock));
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
            report.metadata = report.metadata.with_lock_context(lock_hash, None);
            report
                .with_violations(vec![format!("failed to read/parse lock file: {err}")])
                .with_note(format!("lock: {}", path.display()))
        }
    }
}

pub fn prepare_runtime_smoke(lock_path: &Path, vendor_dir: &Path) -> JsonReport {
    let lock_hash = file_sha256(lock_path).ok();
    match load_official_lock(lock_path) {
        Ok(lock) => {
            let mut items = Vec::new();
            let mut violations = Vec::new();
            let mut created = Vec::new();
            for (version_line, baseline) in [
                (VersionLine::Vue26, &lock.vue2_6),
                (VersionLine::Vue27, &lock.vue2_7),
                (VersionLine::Vue3, &lock.vue3),
            ] {
                match prepare_runtime_smoke_root(version_line, baseline, vendor_dir) {
                    Ok(root) => {
                        created.push(root.display().to_string());
                        items.push(ReportItem::new(
                            version_line.as_str(),
                            ReportStatus::Pass,
                            format!(
                                "prepared official Vue runtime packages and jsdom for {}",
                                version_line.as_str()
                            ),
                            Some(root),
                        ));
                    }
                    Err(err) => {
                        violations.push(format!(
                            "{} runtime smoke dependency preparation failed: {err:#}",
                            version_line.as_str()
                        ));
                        items.push(ReportItem::new(
                            version_line.as_str(),
                            ReportStatus::Fail,
                            format!("{err:#}"),
                            None,
                        ));
                    }
                }
            }
            let mut report = JsonReport::new("prepare_runtime_smoke", ReportStatus::Pending);
            report.metadata = report
                .metadata
                .with_lock_context(lock_hash.clone(), Some(&lock));
            report
                .with_items(items)
                .with_created(created)
                .with_violations(violations)
                .with_note(format!(
                    "lock: {}, vendor: {}",
                    lock_path.display(),
                    vendor_dir.display()
                ))
        }
        Err(err) => {
            let mut report = JsonReport::new("prepare_runtime_smoke", ReportStatus::Fail);
            report.metadata = report.metadata.with_lock_context(lock_hash, None);
            report
                .with_violations(vec![format!("failed to read/parse lock file: {err}")])
                .with_note(format!(
                    "lock: {}, vendor: {}",
                    lock_path.display(),
                    vendor_dir.display()
                ))
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
            report.metadata = report.metadata.with_lock_context(lock_hash, lock.as_ref());
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
    report.metadata = report.metadata.with_lock_context(lock_hash, lock.as_ref());
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
    let lock_path = PathBuf::from("compat/official-revisions.lock");
    let lock_hash = file_sha256(&lock_path).ok();
    let lock = load_official_lock(&lock_path).ok();
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
    report.metadata = report.metadata.with_lock_context(lock_hash, lock.as_ref());
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AliasBackend {
    Generated,
    Napi,
}

impl AliasBackend {
    fn name(self) -> &'static str {
        match self {
            AliasBackend::Generated => "generated",
            AliasBackend::Napi => "napi",
        }
    }

    fn label(self) -> &'static str {
        match self {
            AliasBackend::Generated => "generated Rust",
            AliasBackend::Napi => "NAPI-backed",
        }
    }

    fn root(self, version_line: VersionLine) -> PathBuf {
        match self {
            AliasBackend::Generated => rust_alias_root(version_line),
            AliasBackend::Napi => napi_alias_root(version_line),
        }
    }

    fn option_command(self) -> &'static str {
        match self {
            AliasBackend::Generated => "run_option_matrix",
            AliasBackend::Napi => "run_napi_option_matrix",
        }
    }

    fn output_command(self) -> &'static str {
        match self {
            AliasBackend::Generated => "run_output_contract",
            AliasBackend::Napi => "run_napi_output_contract",
        }
    }

    fn conformance_command(self) -> &'static str {
        match self {
            AliasBackend::Generated => "run_conformance",
            AliasBackend::Napi => "run_napi_conformance",
        }
    }

    fn option_report_name(self) -> &'static str {
        match self {
            AliasBackend::Generated => "option-matrix.json",
            AliasBackend::Napi => "napi-option-matrix.json",
        }
    }

    fn output_report_name(self) -> &'static str {
        match self {
            AliasBackend::Generated => "output-contract.json",
            AliasBackend::Napi => "napi-output-contract.json",
        }
    }

    fn conformance_report_name(self, spec: ConformanceSuiteSpec) -> String {
        match self {
            AliasBackend::Generated => format!("{}.json", spec.name),
            AliasBackend::Napi => format!("napi-{}.json", spec.name),
        }
    }

    fn option_side(self) -> &'static str {
        match self {
            AliasBackend::Generated => "rust",
            AliasBackend::Napi => "napi",
        }
    }

    fn option_note(self) -> &'static str {
        match self {
            AliasBackend::Generated => {
                "option matrix now executes official vs Rust probe cases and records per-row results"
            }
            AliasBackend::Napi => {
                "option matrix executes official packages against NAPI-backed official package-name aliases"
            }
        }
    }

    fn output_note(self) -> &'static str {
        match self {
            AliasBackend::Generated => {
                "output contract executes official npm packages and generated Rust alias packages against representative fixtures"
            }
            AliasBackend::Napi => {
                "output contract executes official npm packages and NAPI-backed official package-name aliases against representative fixtures"
            }
        }
    }

    fn conformance_note(self) -> &'static str {
        match self {
            AliasBackend::Generated => {
                "official conformance executes against generated Rust alias packages"
            }
            AliasBackend::Napi => {
                "official conformance executes against NAPI-backed official package-name aliases; coverage still distinguishes rust-backed, shim-backed, and mixed paths"
            }
        }
    }
}

fn rust_alias_root(version_line: VersionLine) -> PathBuf {
    PathBuf::from("target")
        .join("compat")
        .join("rust-alias")
        .join(version_line.as_str())
}

fn napi_alias_root(version_line: VersionLine) -> PathBuf {
    PathBuf::from("target")
        .join("compat")
        .join("napi-alias")
        .join(version_line.as_str())
}

fn prepare_alias_backend(backend: AliasBackend, targets: &[TargetSpec]) -> Result<Vec<PathBuf>> {
    match backend {
        AliasBackend::Generated => generate_rust_alias_packages(targets),
        AliasBackend::Napi => prepare_napi_alias_packages(targets),
    }
}

fn prepare_napi_alias_packages(targets: &[TargetSpec]) -> Result<Vec<PathBuf>> {
    run_command("cargo", &["build", "-p", "vuec_napi"], None)
        .context("failed to build vuec_napi")?;
    let mut version_lines = Vec::new();
    for target in targets {
        if !version_lines.contains(&target.version_line) {
            version_lines.push(target.version_line);
        }
    }
    let mut created = Vec::new();
    for version_line in version_lines {
        let root = napi_alias_root(version_line);
        reset_napi_alias_root(&root)?;
        prepare_napi_alias_root(version_line, &root)?;
        created.push(root);
    }
    Ok(created)
}

fn reset_napi_alias_root(root: &Path) -> Result<()> {
    ensure_target_compat_child(root, "napi-alias")?;
    if root.exists() {
        fs::remove_dir_all(root).with_context(|| format!("failed to remove {}", root.display()))?;
    }
    fs::create_dir_all(root).with_context(|| format!("failed to create {}", root.display()))
}

fn ensure_target_compat_child(path: &Path, child: &str) -> Result<()> {
    let cwd = std::env::current_dir().context("failed to resolve current directory")?;
    let expected = cwd.join("target").join("compat").join(child);
    let absolute = absolute_path(path);
    ensure!(
        absolute.starts_with(&expected),
        "refusing to recursively replace {}; expected a path under {}",
        absolute.display(),
        expected.display()
    );
    Ok(())
}

fn prepare_napi_alias_root(version_line: VersionLine, root: &Path) -> Result<()> {
    let node_modules = root.join("node_modules");
    fs::create_dir_all(&node_modules)
        .with_context(|| format!("failed to create {}", node_modules.display()))?;

    let native_target = node_modules.join("@vuec-rs").join("native");
    copy_dir_recursive(Path::new("packages/native"), &native_target)?;
    copy_napi_binding(&native_target.join("vuec_napi.node"))?;

    copy_napi_alias_package(
        Path::new("packages/native-aliases/vue-template-compiler"),
        &node_modules.join("vue-template-compiler"),
    )?;
    select_napi_vue_template_compiler(version_line, &node_modules.join("vue-template-compiler"))?;
    copy_napi_alias_package(
        Path::new("packages/native-aliases/vue"),
        &node_modules.join("vue"),
    )?;
    copy_napi_alias_package(
        Path::new("packages/native-aliases/@vue/compiler-core"),
        &node_modules.join("@vue").join("compiler-core"),
    )?;
    copy_napi_alias_package(
        Path::new("packages/native-aliases/@vue/compiler-dom"),
        &node_modules.join("@vue").join("compiler-dom"),
    )?;
    copy_napi_alias_package(
        Path::new("packages/native-aliases/@vue/compiler-ssr"),
        &node_modules.join("@vue").join("compiler-ssr"),
    )?;
    copy_napi_alias_package(
        Path::new("packages/native-aliases/@vue/compiler-sfc"),
        &node_modules.join("@vue").join("compiler-sfc"),
    )?;
    write_napi_alias_versions(version_line, &node_modules)?;
    Ok(())
}

fn copy_napi_alias_package(source: &Path, target: &Path) -> Result<()> {
    copy_dir_recursive(source, target)
}

fn select_napi_vue_template_compiler(version_line: VersionLine, package_dir: &Path) -> Result<()> {
    let variant = match version_line {
        VersionLine::Vue26 => "index-vue2_6.js",
        VersionLine::Vue27 | VersionLine::Vue3 => "index-vue2_7.js",
    };
    fs::copy(package_dir.join(variant), package_dir.join("index.js"))
        .with_context(|| format!("failed to select {} for {}", variant, package_dir.display()))?;
    Ok(())
}

fn write_napi_alias_versions(version_line: VersionLine, node_modules: &Path) -> Result<()> {
    for target in all_targets()
        .iter()
        .copied()
        .filter(|target| target.version_line == version_line)
    {
        let manifest = read_json::<ManifestFile>(&target.relative_api_manifest_path("official"))?;
        let package_json = napi_alias_package_json_path(target, node_modules);
        write_package_json_version(
            &package_json,
            manifest.package_version.as_deref().unwrap_or("0.0.0"),
        )?;
    }
    Ok(())
}

fn napi_alias_package_json_path(target: TargetSpec, node_modules: &Path) -> PathBuf {
    match target.kind {
        TargetKind::Vue26Template | TargetKind::Vue27Template => node_modules
            .join("vue-template-compiler")
            .join("package.json"),
        TargetKind::Vue27Sfc => node_modules.join("vue").join("package.json"),
        TargetKind::Vue3Core => node_modules
            .join("@vue")
            .join("compiler-core")
            .join("package.json"),
        TargetKind::Vue3Dom => node_modules
            .join("@vue")
            .join("compiler-dom")
            .join("package.json"),
        TargetKind::Vue3Ssr => node_modules
            .join("@vue")
            .join("compiler-ssr")
            .join("package.json"),
        TargetKind::Vue3Sfc => node_modules
            .join("@vue")
            .join("compiler-sfc")
            .join("package.json"),
    }
}

fn write_package_json_version(path: &Path, version: &str) -> Result<()> {
    let mut value = read_json::<serde_json::Value>(path)?;
    value["version"] = serde_json::Value::String(version.to_string());
    write_json(path, &value)
}

fn copy_napi_binding(target_path: &Path) -> Result<()> {
    let source_path = napi_library_path();
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::copy(&source_path, target_path).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source_path.display(),
            target_path.display()
        )
    })?;
    Ok(())
}

fn napi_library_path() -> PathBuf {
    let (prefix, suffix) = match std::env::consts::OS {
        "windows" => ("", ".dll"),
        "macos" => ("lib", ".dylib"),
        _ => ("lib", ".so"),
    };
    PathBuf::from("target")
        .join("debug")
        .join(format!("{prefix}vuec_napi{suffix}"))
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
        source.push_str("Object.defineProperty(vue3CoreRuntime, 'callBridge', { value: callBridge, enumerable: false });\n");
        source.push_str("Object.defineProperty(exports, '__vuecRuntime', { value: vue3CoreRuntime, enumerable: false });\n");
    } else if target.kind == TargetKind::Vue3Dom {
        source.push_str("Object.defineProperty(exports, '__vuecRuntime', { value: Object.assign({}, vue3CoreRuntime, { decodeHtmlBrowser: vue3CoreRuntime.decodeHtmlBrowser, ignoreSideEffectTags: vue3CoreRuntime.ignoreSideEffectTags, transformOn: vue3CoreRuntime.transformDomOn, transformModel: vue3CoreRuntime.transformDomModel, transformTransition: vue3CoreRuntime.transformDomTransition, validateHtmlNesting: vue3CoreRuntime.validateHtmlNesting, isValidHTMLNesting: vue3CoreRuntime.isValidHTMLNesting }), enumerable: false });\n");
    } else if matches!(
        target.kind,
        TargetKind::Vue26Template | TargetKind::Vue27Template
    ) {
        source.push_str("Object.defineProperty(exports, '__vuecRuntime', { value: vuecBridgeRuntime, enumerable: false });\n");
    } else if target.kind == TargetKind::Vue3Sfc {
        source.push_str("Object.defineProperty(exports, '__vuecRuntime', { value: vuecBridgeRuntime, enumerable: false });\n");
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
    if target.kind == TargetKind::Vue3Dom && export_name == "parserOptions" {
        return "vue3DomParserOptions".into();
    }
    if target.kind == TargetKind::Vue3Dom && export_name == "transformOn" {
        return alias_runtime_function_expression_as(
            "vue3CoreRuntime",
            "transformDomOn",
            export_name,
            detail,
        );
    }
    if target.kind == TargetKind::Vue3Dom && export_name == "transformModel" {
        return alias_runtime_function_expression_as(
            "vue3CoreRuntime",
            "transformDomModel",
            export_name,
            detail,
        );
    }
    if target.kind == TargetKind::Vue3Dom
        && !matches!(
            export_name,
            "baseCompile" | "baseParse" | "compile" | "generate" | "parse"
        )
        && vue3_core_runtime_export(export_name, detail).is_some()
    {
        if detail.kind == "function" {
            return alias_runtime_function_expression("vue3CoreRuntime", export_name, detail);
        }
        return format!("vue3CoreRuntime[{}]", js_string_literal(export_name));
    }
    if target.kind == TargetKind::Vue3Sfc && matches!(export_name, "babelParse" | "walkIdentifiers")
    {
        return alias_runtime_function_expression("vue3CoreRuntime", export_name, detail);
    }
    if target.kind == TargetKind::Vue27Sfc
        && matches!(export_name, "compileStyle" | "compileStyleAsync")
        && detail.kind == "function"
    {
        return vue27_sfc_style_function_expression(export_name, detail);
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

fn vue27_sfc_style_function_expression(export_name: &str, detail: &ApiExportDetail) -> String {
    let name = detail.name.as_deref().unwrap_or(export_name);
    let arity = detail.function_arity.unwrap_or(0);
    let command = bridge_command(
        TargetSpec {
            version_line: VersionLine::Vue27,
            package: "vue",
            entry: "vue/compiler-sfc",
            kind: TargetKind::Vue27Sfc,
        },
        export_name,
    )
    .unwrap_or("sfc.vue27.compileStyle");
    let postcss_call = if export_name == "compileStyleAsync" {
        "applyVue27StylePostcssAsync(__vuecBridgeResult, __vuecPayload.options)"
    } else {
        "applyVue27StylePostcssSync(__vuecBridgeResult, __vuecPayload.options)"
    };
    let body = format!(
        "const __vuecPayload = resolveStylePreprocessPayload(normalizeArgs({})); preflightAliasCall({}, __vuecPayload); const __vuecBridgePayload = vue27StyleBridgePayload(__vuecPayload); const __vuecBridgeResult = callBridge({}, bridgePayloadForCall(__vuecBridgePayload)); return {postcss_call};",
        alias_argument_object(
            TargetSpec {
                version_line: VersionLine::Vue27,
                package: "vue",
                entry: "vue/compiler-sfc",
                kind: TargetKind::Vue27Sfc,
            },
            export_name,
            arity,
        ),
        js_string_literal(alias_preflight_name(
            TargetSpec {
                version_line: VersionLine::Vue27,
                package: "vue",
                entry: "vue/compiler-sfc",
                kind: TargetKind::Vue27Sfc,
            },
            export_name,
        )),
        js_string_literal(command),
    );
    let expression = format!("function {name}(a0) {{ {body} }}");
    if detail
        .own_property_names
        .iter()
        .any(|prop| prop == "prototype")
    {
        expression
    } else {
        format!(
            "namedArity({}, {}, {})",
            js_string_literal(name),
            arity,
            expression
        )
    }
}

fn vue3_core_runtime_export(export_name: &str, detail: &ApiExportDetail) -> Option<()> {
    match export_name {
        "baseCompile" | "baseParse" | "generate" => None,
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
        Some("vue3.core.baseCompile") => format!(
            "{argument_bindings} const __vuecPayload = normalizeArgs({}); preflightAliasCall({}, __vuecPayload); if (usesAliasRuntimeCompile(__vuecPayload.options)) return vue3CoreRuntime.baseCompile(__vuecPayload.source, __vuecPayload.options || {{}}); return callBridge({}, bridgePayloadForCall(__vuecPayload));",
            alias_argument_object(target, export_name, body_arity),
            js_string_literal(alias_preflight_name(target, export_name)),
            js_string_literal("vue3.core.baseCompile"),
        ),
        Some("vue3.dom.compile") => format!(
            "{argument_bindings} const __vuecPayload = normalizeArgs({}); preflightAliasCall({}, __vuecPayload); const __vuecResult = callBridge({}, bridgePayloadForCall(__vuecPayload)); emitVue3CompileDiagnostics(__vuecResult, __vuecPayload.options); return __vuecResult;",
            alias_argument_object(target, export_name, body_arity),
            js_string_literal(alias_preflight_name(target, export_name)),
            js_string_literal("vue3.dom.compile"),
        ),
        Some(command) => {
            let call = if matches!(
                (target.kind, export_name),
                (TargetKind::Vue3Core, "baseParse") | (TargetKind::Vue3Dom, "parse")
            ) {
                format!(
                    "hydrateVue3Ast(callBridge({}, bridgePayloadForCall(__vuecBridgePayload)), __vuecPayload.options)",
                    js_string_literal(command)
                )
            } else {
                format!(
                    "callBridge({}, bridgePayloadForCall(__vuecBridgePayload))",
                    js_string_literal(command)
                )
            };
            let is_vue3_generate = target.kind == TargetKind::Vue3Core && export_name == "generate";
            let is_vue2_template_compile = matches!(
                (target.kind, export_name),
                (
                    TargetKind::Vue26Template | TargetKind::Vue27Template,
                    "compile" | "compileToFunctions" | "ssrCompile" | "ssrCompileToFunctions"
                )
            );
            let is_vue27_sfc_compile_script =
                target.kind == TargetKind::Vue27Sfc && export_name == "compileScript";
            let is_vue3_sfc_compile_script =
                target.kind == TargetKind::Vue3Sfc && export_name == "compileScript";
            let is_vue3_sfc_compile_template =
                target.kind == TargetKind::Vue3Sfc && export_name == "compileTemplate";
            let is_vue3_sfc_parse = target.kind == TargetKind::Vue3Sfc && export_name == "parse";
            let is_vue3_ssr_compile = target.kind == TargetKind::Vue3Ssr && export_name == "compile";
            let is_sfc_compile_style = matches!(
                (target.kind, export_name),
                (
                    TargetKind::Vue3Sfc | TargetKind::Vue27Sfc,
                    "compileStyle" | "compileStyleAsync"
                )
            );
            let is_vue3_sfc_compile_style = target.kind == TargetKind::Vue3Sfc
                && matches!(export_name, "compileStyle" | "compileStyleAsync");
            let payload = if is_vue3_generate {
                "Object.assign({}, __vuecPayload, { ast: vue3CoreRuntime.dehydrateForBridge(a0), source: '' })"
            } else if is_vue3_sfc_parse {
                "vue3SfcParseBridgePayload(__vuecPayload)"
            } else if is_vue27_sfc_compile_script {
                "vue27CompileScriptBridgePayload(__vuecPayload)"
            } else if is_vue3_sfc_compile_script {
                "vue3CompileScriptBridgePayload(__vuecPayload)"
            } else if is_vue3_sfc_compile_template {
                "vue3SfcCompileTemplateBridgePayload(__vuecPayload)"
            } else if is_vue3_sfc_compile_style {
                "vue3StyleBridgePayload(__vuecPayload)"
            } else {
                "__vuecPayload"
            };
            let payload_init = if is_sfc_compile_style {
                format!(
                    "resolveStylePreprocessPayload(normalizeArgs({}))",
                    alias_argument_object(target, export_name, body_arity)
                )
            } else {
                format!(
                    "normalizeArgs({})",
                    alias_argument_object(target, export_name, body_arity)
                )
            };
            let return_expr = if is_vue3_sfc_compile_style {
                format!(
                    "(() => {{ const __vuecStyleResult = {call}; return normalizeStyleAliasResult(emitVue3StyleWarnings(__vuecStyleResult)); }})()"
                )
            } else if is_vue3_generate {
                format!(
                    "(() => {{ const __vuecGenerateResult = {call}; __vuecGenerateResult.ast = a0; return __vuecGenerateResult; }})()"
                )
            } else if is_vue3_sfc_parse {
                format!("hydrateVue3SfcParseResult(applyVue3SfcCustomCompilerParse({call}, __vuecPayload.source, __vuecPayload.options, __vuecPayload.filename))")
            } else if is_vue3_sfc_compile_script {
                format!("hydrateVue3CompileScriptResult({call})")
            } else if is_vue3_sfc_compile_template {
                format!("(() => {{ const __vuecCustomTemplateResult = vue3SfcCustomCompileTemplateResult(__vuecPayload); if (__vuecCustomTemplateResult !== undefined) return __vuecCustomTemplateResult; return hydrateVue3SfcCompileTemplateResult({call}); }})()")
            } else if is_vue27_sfc_compile_script {
                format!("hydrateVue27CompileScriptResult({call})")
            } else if is_vue3_ssr_compile {
                format!("hydrateVue3SsrCompileResult({call})")
            } else if is_vue2_template_compile {
                format!(
                    "(() => {{ const __vuecVue2Result = {call}; emitVue2CompileWarnings(__vuecVue2Result, __vuecPayload.options); return __vuecVue2Result; }})()"
                )
            } else {
                call
            };
            format!(
                "{argument_bindings} const __vuecPayload = {payload_init}; preflightAliasCall({}, __vuecPayload); const __vuecBridgePayload = {payload}; return {return_expr};",
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
    alias_runtime_function_expression_as(runtime_object, export_name, export_name, detail)
}

fn alias_runtime_function_expression_as(
    runtime_object: &str,
    runtime_key: &str,
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
        js_string_literal(runtime_key)
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
        | (TargetKind::Vue3Core, "generate")
        | (TargetKind::Vue3Dom, "parse")
        | (TargetKind::Vue3Core | TargetKind::Vue3Dom | TargetKind::Vue3Ssr, "compile")
        | (TargetKind::Vue3Sfc, "parse")
        | (TargetKind::Vue27Sfc, "parseComponent")
        | (TargetKind::Vue27Sfc, "rewriteDefault")
        | (TargetKind::Vue27Sfc | TargetKind::Vue3Sfc, "compileScript") => arity.max(2),
        (TargetKind::Vue27Sfc, "prefixIdentifiers") => arity.max(5),
        (TargetKind::Vue26Template | TargetKind::Vue27Template, "generateCodeFrame") => {
            arity.max(3)
        }
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
        (TargetKind::Vue27Sfc, "parseComponent") => Some("sfc.vue27.parseComponent"),
        (TargetKind::Vue27Sfc, "rewriteDefault") => Some("sfc.vue27.rewriteDefault"),
        (TargetKind::Vue27Sfc, "prefixIdentifiers") => Some("sfc.vue27.prefixIdentifiers"),
        (TargetKind::Vue3Sfc, "parse") => Some("sfc.parse"),
        (TargetKind::Vue3Sfc, "rewriteDefault") => Some("sfc.rewriteDefault"),
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
        (TargetKind::Vue3Core, "generate") => Some("vue3.core.generate"),
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
        (TargetKind::Vue27Sfc, "parseComponent") => {
            "{ source: a0 == null ? '' : String(a0), options: a1 || {} }".into()
        }
        (TargetKind::Vue27Sfc, "rewriteDefault") => {
            "{ source: a0 == null ? '' : String(a0), variable: a1 || 'script', plugins: a2 }"
                .into()
        }
        (TargetKind::Vue27Sfc, "prefixIdentifiers") => {
            "{ source: a0 == null ? '' : String(a0), isFunctional: !!a1, isTS: !!a2, babelOptions: a3 || {}, bindings: a4 || {} }"
                .into()
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
        (TargetKind::Vue3Sfc, "rewriteDefault") => {
            "{ source: a0 == null ? '' : String(a0), variable: a1 || 'script', plugins: a2 }"
                .into()
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
        (TargetKind::Vue3Core, "baseCompile")
        | (TargetKind::Vue3Dom, "compile")
        | (TargetKind::Vue3Ssr, "compile") => {
            "vue3CompileBridgePayload(a0, a0 && a0.filename, a1 || (a0 && a0.options) || {})"
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
    reset_official_npm_node_modules(&install_root)?;
    fs::create_dir_all(&install_root)
        .with_context(|| format!("failed to create {}", install_root.display()))?;
    let package_json = serde_json::json!({
        "private": true,
        "name": format!("vuec-compat-{}", version_line.as_str()),
        "version": "0.0.0",
    });
    write_json(&install_root.join("package.json"), &package_json)?;
    run_npm_install_specs(&install_root, &specs, "official npm package install")?;

    let marker_body = serde_json::json!({
        "version_line": version_line,
        "packages": specs,
        "rev": baseline.rev,
        "platform": npm_install_platform_marker(),
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
        && value
            .get("platform")
            .and_then(|value| value.as_str())
            .is_some_and(|platform| platform == npm_install_platform_marker())
}

fn npm_install_platform_marker() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn reset_official_npm_node_modules(install_root: &Path) -> Result<()> {
    ensure_target_compat_child(install_root, "npm")?;
    let node_modules = install_root.join("node_modules");
    if node_modules.exists() {
        fs::remove_dir_all(&node_modules)
            .with_context(|| format!("failed to remove {}", node_modules.display()))?;
    }
    Ok(())
}

fn run_npm_install_specs(install_root: &Path, specs: &[String], label: &str) -> Result<()> {
    let npm = resolve_program("npm");
    let mut command = Command::new(npm);
    command
        .arg("install")
        .arg("--ignore-scripts")
        .arg("--include=optional")
        .arg("--no-audit")
        .arg("--no-fund")
        .arg("--package-lock=false")
        .arg("--omit=dev")
        .args(specs)
        .current_dir(install_root);
    let output = command.output().with_context(|| {
        format!(
            "failed to spawn npm install for {label} in {}",
            install_root.display()
        )
    })?;
    if !output.status.success() {
        anyhow::bail!(
            "`npm install {}` for {label} failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
            specs.join(" "),
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
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
    if node_modules.exists()
        && official_install_marker_matches(&marker, &runner_specs)
        && verify_conformance_runner_startup_dependencies(spec, &node_modules).is_ok()
    {
        return Ok(install_root);
    }

    run_npm_install_specs(
        &install_root,
        &runner_specs,
        "official runner dependency install",
    )?;
    if let Err(first_err) = verify_conformance_runner_startup_dependencies(spec, &node_modules) {
        reset_official_npm_node_modules(&install_root)?;
        ensure_official_npm_install(spec.version_line, baseline)?;
        run_npm_install_specs(
            &install_root,
            &runner_specs,
            "official runner dependency reinstall",
        )?;
        verify_conformance_runner_startup_dependencies(spec, &node_modules).with_context(|| {
            format!(
                "{} runner dependencies still cannot start after reinstall; first failure: {first_err:#}",
                spec.name
            )
        })?;
    }

    let marker_body = serde_json::json!({
        "version_line": spec.version_line,
        "suite": spec.name,
        "packages": runner_specs,
        "rev": baseline.rev,
        "platform": npm_install_platform_marker(),
    });
    write_json(&marker, &marker_body)?;
    Ok(install_root)
}

fn prepare_runtime_smoke_root(
    version_line: VersionLine,
    baseline: &BaselineLock,
    vendor_dir: &Path,
) -> Result<PathBuf> {
    let install_root = ensure_official_npm_install(version_line, baseline)?;
    ensure_runtime_smoke_dependencies(version_line, vendor_dir, &install_root)?;
    let node_modules = install_root.join("node_modules");
    verify_runtime_smoke_root(version_line, &node_modules)?;
    Ok(node_modules)
}

fn ensure_runtime_smoke_dependencies(
    version_line: VersionLine,
    vendor_dir: &Path,
    install_root: &Path,
) -> Result<()> {
    let specs = runtime_smoke_dependency_specs(version_line, vendor_dir)?;
    let node_modules = install_root.join("node_modules");
    let marker = install_root.join("runtime-smoke-install.json");
    if node_modules.exists()
        && official_install_marker_matches(&marker, &specs)
        && node_dependency_available(&node_modules, "jsdom")
    {
        return Ok(());
    }

    run_npm_install_specs(install_root, &specs, "runtime smoke dependency install")?;
    let marker_body = serde_json::json!({
        "version_line": version_line,
        "packages": specs,
        "platform": npm_install_platform_marker(),
    });
    write_json(&marker, &marker_body)?;
    Ok(())
}

fn runtime_smoke_dependency_specs(
    version_line: VersionLine,
    vendor_dir: &Path,
) -> Result<Vec<String>> {
    let root = vendor_dir.join(version_line.as_str());
    let package_json = root.join("package.json");
    if !package_json.is_file() {
        anyhow::bail!(
            "{} is missing; run `cargo xtask sync-official-tests --locked` before preparing runtime smoke dependencies",
            package_json.display()
        );
    }
    let manifest = read_json::<serde_json::Value>(&package_json)?;
    let mut specs = Vec::new();
    for dependency in runtime_smoke_runner_dependencies() {
        let version = locked_runner_dependency_version(&root, dependency)
            .or_else(|| manifest_dependency_version(&manifest, dependency))
            .or_else(|| fallback_runner_dependency_version(vendor_dir, version_line, dependency));
        let Some(version) = version else {
            anyhow::bail!(
                "failed to resolve deterministic npm version for runtime smoke dependency {dependency} from {}",
                root.display()
            );
        };
        if is_unpublished_dependency_spec(&version) {
            anyhow::bail!(
                "runtime smoke dependency {dependency} resolved to unpublished spec {version:?}"
            );
        }
        specs.push(format!("{dependency}@{version}"));
    }
    specs.sort();
    specs.dedup();
    Ok(specs)
}

fn runtime_smoke_runner_dependencies() -> &'static [&'static str] {
    &["jsdom"]
}

fn verify_runtime_smoke_root(version_line: VersionLine, node_modules: &Path) -> Result<()> {
    for dependency in runtime_smoke_required_node_dependencies(version_line) {
        if !node_dependency_available(node_modules, dependency) {
            anyhow::bail!(
                "missing runtime smoke dependency {dependency} in {}",
                node_modules.display()
            );
        }
    }
    Ok(())
}

fn runtime_smoke_required_node_dependencies(version_line: VersionLine) -> &'static [&'static str] {
    match version_line {
        VersionLine::Vue26 | VersionLine::Vue27 => &["vue", "jsdom"],
        VersionLine::Vue3 => &["vue", "@vue/compiler-ssr", "@vue/server-renderer", "jsdom"],
    }
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
            .or_else(|| manifest_dependency_version(&manifest, dependency))
            .or_else(|| {
                fallback_runner_dependency_version(vendor_dir, spec.version_line, dependency)
            });
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

fn fallback_runner_dependency_version(
    vendor_dir: &Path,
    current: VersionLine,
    dependency: &str,
) -> Option<String> {
    [VersionLine::Vue26, VersionLine::Vue27, VersionLine::Vue3]
        .into_iter()
        .filter(|version_line| *version_line != current)
        .find_map(|version_line| {
            let root = vendor_dir.join(version_line.as_str());
            let manifest = read_json::<serde_json::Value>(&root.join("package.json")).ok();
            locked_runner_dependency_version(&root, dependency).or_else(|| {
                manifest
                    .as_ref()
                    .and_then(|manifest| manifest_dependency_version(manifest, dependency))
            })
        })
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
        let trimmed = line.trim_start().trim_start_matches(['\'', '"']);
        let candidate = trimmed
            .strip_prefix(&format!("{dependency}@"))
            .or_else(|| trimmed.strip_prefix(&format!("/{dependency}@")));
        let Some(candidate) = candidate else {
            continue;
        };
        let version_end = candidate
            .find(['(', ':', '\'', '"'])
            .unwrap_or(candidate.len());
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

fn normalize_command_output(stdout: &[u8], stderr: &[u8]) -> String {
    let mut text = String::new();
    let stdout = String::from_utf8_lossy(stdout);
    let stdout = stdout.trim();
    if !stdout.is_empty() {
        text.push_str(stdout);
    }
    let stderr = String::from_utf8_lossy(stderr);
    let stderr = stderr.trim();
    if !stderr.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(stderr);
    }
    text.lines().take(40).collect::<Vec<_>>().join("\n")
}

const RUNNER_STARTUP_PROBE_SCRIPT: &str = r#"
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const nodeModules = process.env.VUEC_RUNNER_NODE_MODULES;
const request = process.env.VUEC_RUNNER_PROBE_REQUEST;
const rootRequire = createRequire(path.join(nodeModules, '__vuec_runner_probe__.cjs'));

const resolved = rootRequire.resolve(request);
await import(pathToFileURL(resolved).href);
"#;

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
  errorMessages[24] = 'Element is missing end tag.';
  errorMessages[25] = 'Interpolation end sign was not found.';
  errorMessages[5] = 'Unexpected EOF in tag.';
  errorMessages[7] = 'Unexpected EOF in comment.';
  errorMessages[9] = 'Unexpected EOF in tag.';
  errorMessages[14] = 'End tag name was expected.';
  errorMessages[19] = "Attribute name cannot start with '='.";
  errorMessages[21] = "'<?' is allowed only in XML context.";
  errorMessages[22] = "Illegal '/' in tags.";
  errorMessages[27] = 'End bracket for dynamic directive argument was not found. Note that dynamic directive argument cannot contain spaces.';
  errorMessages[41] = 'v-model is missing expression.';
  errorMessages[42] = 'v-model value must be a valid JavaScript member expression.';
  errorMessages[43] = 'v-model cannot be used on v-for or v-slot scope variables because they are not writable.';
  errorMessages[44] = 'v-model cannot be used on a prop, because local prop bindings are not writable.\nUse a v-bind binding combined with a v-on listener that emits update:x event instead.';
  errorMessages[45] = 'v-model cannot be used on a const binding because it is not writable.';
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
    ['TRANSITION', 'Transition'],
    ['TRANSITION_GROUP', 'TransitionGroup'],
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
    ['V_MODEL_RADIO', 'vModelRadio'],
    ['V_MODEL_CHECKBOX', 'vModelCheckbox'],
    ['V_MODEL_TEXT', 'vModelText'],
    ['V_MODEL_SELECT', 'vModelSelect'],
    ['V_MODEL_DYNAMIC', 'vModelDynamic'],
    ['V_ON_WITH_MODIFIERS', 'withModifiers'],
    ['V_ON_WITH_KEYS', 'withKeys'],
    ['V_SHOW', 'vShow'],
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
    return callBridge('vue3.core.advancePositionWithClone', {
      pos: runtime.dehydrateForBridge(pos),
      source: String(source || ''),
      numberOfCharacters: numberOfCharacters === undefined ? undefined : numberOfCharacters,
    });
  };
  runtime.advancePositionWithMutation = function advancePositionWithMutation(pos, source, numberOfCharacters) {
    const projection = callBridge('vue3.core.advancePositionWithMutation', {
      pos: runtime.dehydrateForBridge(pos),
      source: String(source || ''),
      numberOfCharacters: numberOfCharacters === undefined ? undefined : numberOfCharacters,
    });
    pos.offset = projection.offset;
    pos.line = projection.line;
    pos.column = projection.column;
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
      if (directives) {
        context.helper(runtime.WITH_DIRECTIVES);
      }
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
      const name = Object.getOwnPropertyDescriptor(symbol, 'description') && symbol.description;
      if (name && !runtime[name]) runtime[name] = symbol;
    });
  };
  runtime.stringifyExpression = function stringifyExpression(exp) {
    return typeof exp === 'string'
      ? exp
      : exp && exp.type === NodeTypes.SIMPLE_EXPRESSION
        ? exp.content
        : exp && Array.isArray(exp.children)
          ? exp.children.map(runtime.stringifyExpression).join('')
          : exp && exp.loc
            ? exp.loc.source
            : '';
  };
  runtime.isStaticExp = function isStaticExp(p) {
    return !!(p && p.type === NodeTypes.SIMPLE_EXPRESSION && p.isStatic);
  };
  runtime.dehydrateForBridge = function dehydrateForBridge(value, seen = new WeakSet()) {
    if (value == null || typeof value !== 'object') return typeof value === 'symbol' ? projectionNameFromHelperSymbol(value) : value;
    if (typeof value === 'symbol') return projectionNameFromHelperSymbol(value);
    if (seen.has(value)) return undefined;
    seen.add(value);
    if (value instanceof Set) {
      const out = Array.from(value, item => runtime.dehydrateForBridge(item, seen));
      seen.delete(value);
      return out;
    }
    if (Array.isArray(value)) {
      const out = value.map(item => runtime.dehydrateForBridge(item, seen));
      seen.delete(value);
      return out;
    }
    const out = {};
    for (const key of Object.keys(value)) {
      if (key === 'loc' || key === 'start' || key === 'end' || key === 'offset' || key === 'line' || key === 'column' || key === 'type' || key === 'tag' || key === 'tagType' || key === 'content' || key === 'isStatic' || key === 'constType' || key === 'props' || key === 'children' || key === 'codegenNode' || key === 'patchFlag' || key === 'dynamicProps' || key === 'directives' || key === 'isBlock' || key === 'isComponent' || key === 'disableTracking' || key === 'branches' || key === 'source' || key === 'transformed' || key === 'parseResult' || key === 'valueAlias' || key === 'keyAlias' || key === 'objectIndexAlias' || key === 'returns' || key === 'body' || key === 'params' || key === 'newline' || key === 'isSlot' || key === 'isNonScopedSlot' || key === 'needPauseTracking' || key === 'inVOnce' || key === 'needArraySpread' || key === 'index' || key === 'elements' || key === 'test' || key === 'consequent' || key === 'alternate' || key === 'left' || key === 'right' || key === 'expressions' || key === 'expression' || key === 'helpers' || key === 'ssrHelpers' || key === 'components' || key === 'directives' || key === 'imports' || key === 'path' || key === 'hoists' || key === 'cached' || key === 'temps' || key === 'properties' || key === 'key' || key === 'value' || key === 'arguments' || key === 'argument' || key === 'callee' || key === 'object' || key === 'property' || key === 'name' || key === 'arg' || key === 'exp' || key === 'modifiers' || key === 'program' || key === 'declarations' || key === 'declaration' || key === 'id' || key === 'init' || key === 'update' || key === 'computed' || key === 'shorthand' || key === 'kind' || key === 'declare' || key === 'operator' || key === 'prefix' || key === 'async' || key === 'cases' || key === 'discriminant' || key === 'handler' || key === 'finalizer' || key === 'block' || key === 'param' || key === 'parameter' || key === 'specifiers' || key === 'local' || key === 'imported' || key === 'superClass' || key === 'quasi') {
        out[key] = runtime.dehydrateForBridge(value[key], seen);
      }
    }
    seen.delete(value);
    return out;
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
    const projection = callBridge('vue3.core.toValidAssetId', { name: String(name), type: String(type) });
    return projection && projection.id || '';
  };
  runtime.injectProp = function injectProp(node, prop) {
    let props = node.type === NodeTypes.VNODE_CALL ? node.props : node.arguments && node.arguments[2];
    let callPath = [];
    let parentCall;
    if (props && typeof props !== 'string' && props.type === NodeTypes.JS_CALL_EXPRESSION) {
      const ret = runtime.getUnnormalizedProps(props);
      props = ret[0];
      callPath = ret[1];
      parentCall = callPath[callPath.length - 1];
    }
    let propsWithInjection;
    if (!props || typeof props === 'string') {
      propsWithInjection = runtime.createObjectExpression([prop]);
    } else if (props.type === NodeTypes.JS_CALL_EXPRESSION) {
      const first = props.arguments && props.arguments[0];
      if (first && typeof first !== 'string' && first.type === NodeTypes.JS_OBJECT_EXPRESSION) {
        runtime.prependPropOnce(first, prop);
      } else if (props.callee === runtime.TO_HANDLERS) {
        propsWithInjection = runtime.createCallExpression(runtime.MERGE_PROPS, [runtime.createObjectExpression([prop]), props]);
      } else {
        props.arguments.unshift(runtime.createObjectExpression([prop]));
      }
      if (!propsWithInjection) propsWithInjection = props;
    } else if (props.type === NodeTypes.JS_OBJECT_EXPRESSION) {
      runtime.prependPropOnce(props, prop);
      propsWithInjection = props;
    } else {
      propsWithInjection = runtime.createCallExpression(runtime.MERGE_PROPS, [runtime.createObjectExpression([prop]), props]);
      if (parentCall && parentCall.callee === runtime.GUARD_REACTIVE_PROPS) {
        parentCall = callPath[callPath.length - 2];
      }
    }
    if (node.type === NodeTypes.JS_CALL_EXPRESSION && node.callee === runtime.RENDER_SLOT && node.arguments) {
      node.arguments[2] = propsWithInjection;
    } else if (node.type === NodeTypes.VNODE_CALL) {
      if (parentCall) parentCall.arguments[0] = propsWithInjection;
      else node.props = propsWithInjection;
    } else if (node.arguments) {
      if (parentCall) parentCall.arguments[0] = propsWithInjection;
      else node.arguments[2] = propsWithInjection;
    }
  };
  runtime.getUnnormalizedProps = function getUnnormalizedProps(props, callPath = []) {
    if (props && typeof props !== 'string' && props.type === NodeTypes.JS_CALL_EXPRESSION) {
      if (props.callee === runtime.NORMALIZE_PROPS || props.callee === runtime.GUARD_REACTIVE_PROPS) {
        return runtime.getUnnormalizedProps(props.arguments[0], callPath.concat(props));
      }
    }
    return [props, callPath];
  };
  runtime.prependPropOnce = function prependPropOnce(props, prop) {
    const keyName = runtime.staticPropertyKeyName(prop);
    if (!keyName || !(props.properties || []).some(existing => runtime.staticPropertyKeyName(existing) === keyName)) {
      props.properties.unshift(prop);
    }
  };
  runtime.prependPropsExpressionProp = function prependPropsExpressionProp(props, prop, loc = locStub) {
    if (!props || typeof props === 'string') return runtime.createObjectExpression([prop], loc);
    if (props.type === NodeTypes.JS_OBJECT_EXPRESSION) {
      runtime.prependPropOnce(props, prop);
      return props;
    }
    const objectArg = runtime.createObjectExpression([prop], loc);
    if (props.type === NodeTypes.JS_CALL_EXPRESSION && props.callee === runtime.MERGE_PROPS) {
      const first = props.arguments && props.arguments[0];
      if (first && typeof first !== 'string' && first.type === NodeTypes.JS_OBJECT_EXPRESSION) {
        runtime.prependPropOnce(first, prop);
      } else {
        props.arguments.unshift(objectArg);
      }
      return props;
    }
    return runtime.createCallExpression(runtime.MERGE_PROPS, [objectArg, props], loc);
  };
  runtime.applyInlineTemplateRefProjection = function applyInlineTemplateRefProjection(props, refs, loc = locStub) {
    for (const ref of refs || []) {
      const content = ref && ref.content;
      if (!content) continue;
      props = runtime.prependPropsExpressionProp(
        props,
        runtime.createObjectProperty('ref_key', runtime.createSimpleExpression(content, true, loc)),
        loc,
      );
      for (const object of runtime.propsExpressionObjects(props)) {
        for (const prop of object.properties || []) {
          if (runtime.staticPropertyKeyName(prop) === 'ref' && prop.value && prop.value.type === NodeTypes.SIMPLE_EXPRESSION && prop.value.content === content) {
            prop.value.isStatic = false;
            prop.value.constType = ConstantTypes.NOT_CONSTANT;
          }
        }
      }
    }
    return props;
  };
  runtime.propsExpressionObjects = function propsExpressionObjects(props) {
    if (!props || typeof props === 'string') return [];
    if (props.type === NodeTypes.JS_OBJECT_EXPRESSION) return [props];
    if (props.type === NodeTypes.JS_CALL_EXPRESSION) {
      return (props.arguments || []).flatMap(arg => runtime.propsExpressionObjects(arg));
    }
    return [];
  };
  runtime.dedupeProperties = function dedupeProperties(properties) {
    const known = new Map();
    const deduped = [];
    for (const prop of properties || []) {
      const keyName = runtime.staticPropertyKeyName(prop);
      if (!keyName) {
        deduped.push(prop);
        continue;
      }
      const existing = known.get(keyName);
      if (existing) {
        if (keyName === 'class' || keyName === 'style' || /^on[A-Z]/.test(keyName)) {
          runtime.mergePropertyAsArray(existing, prop);
        }
      } else {
        known.set(keyName, prop);
        deduped.push(prop);
      }
    }
    return deduped;
  };
  runtime.mergePropertyAsArray = function mergePropertyAsArray(existing, incoming) {
    if (existing.value && existing.value.type === NodeTypes.JS_ARRAY_EXPRESSION) {
      existing.value.elements.push(incoming.value);
    } else {
      existing.value = runtime.createArrayExpression([existing.value, incoming.value], existing.loc || locStub);
    }
  };
  runtime.staticPropertyKeyName = function staticPropertyKeyName(prop) {
    const key = prop && prop.key;
    return key && key.type === NodeTypes.SIMPLE_EXPRESSION && key.isStatic ? key.content : undefined;
  };
  runtime.normalizeObjectProp = function normalizeObjectProp(props, name, helper) {
    let target = props;
    if (target && target.type === NodeTypes.JS_CALL_EXPRESSION && target.callee === runtime.NORMALIZE_PROPS) {
      target = target.arguments && target.arguments[0];
    }
    if (!target || target.type !== NodeTypes.JS_OBJECT_EXPRESSION) return;
    const prop = (target.properties || []).find(property => runtime.staticPropertyKeyName(property) === name);
    if (prop && prop.value && !runtime.isStaticExp(prop.value) && !(prop.value.type === NodeTypes.JS_CALL_EXPRESSION && prop.value.callee === helper)) {
      prop.value = runtime.createCallExpression(helper, [prop.value], prop.value.loc || prop.loc || locStub);
    }
  };
  runtime.hasScopeRef = function hasScopeRef(node, identifiers = {}) {
    const names = Object.keys(identifiers).filter(name => identifiers[name] > 0);
    if (!names.length) return false;
    const source = runtime.stringifyExpression(node);
    return names.some(name => source.includes(name));
  };
  runtime.expressionIdentifierNames = function expressionIdentifierNames(exp) {
    if (!exp) return [];
    if (typeof exp === 'string') return exp ? [exp] : [];
    if (Array.isArray(exp.identifiers)) return exp.identifiers.filter(Boolean);
    if (exp.type === NodeTypes.SIMPLE_EXPRESSION && exp.content) return [exp.content];
    return [];
  };
  runtime.getMemoedVNodeCall = function getMemoedVNodeCall(node) {
    return node && node.type === NodeTypes.JS_CALL_EXPRESSION && node.callee === runtime.WITH_MEMO ? node.arguments[1].returns : node;
  };
  runtime.isCoreComponent = function isCoreComponent(tag) {
    return tag === 'Teleport' || tag === 'teleport' ? runtime.TELEPORT
      : tag === 'Suspense' || tag === 'suspense' ? runtime.SUSPENSE
      : tag === 'KeepAlive' || tag === 'keep-alive' ? runtime.KEEP_ALIVE
      : tag === 'BaseTransition' || tag === 'base-transition' ? runtime.BASE_TRANSITION
      : undefined;
  };
  runtime.isBuiltInDirective = function isBuiltInDirective(name) {
    return new Set(['bind', 'cloak', 'else-if', 'else', 'for', 'html', 'if', 'model', 'on', 'once', 'pre', 'show', 'slot', 'text', 'memo']).has(String(name || ''));
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
    const projection = callBridge('vue3.core.isMemberExpression', {
      mode: 'browser',
      node: runtime.dehydrateForBridge(path),
      context: {},
    });
    return !!(projection && projection.isMemberExpression);
  };
  runtime.isMemberExpressionNode = function isMemberExpressionNode(path, context = {}) {
    const projection = callBridge('vue3.core.isMemberExpression', {
      mode: 'node',
      node: runtime.dehydrateForBridge(path),
      context: vue3ExpressionUtilityContextPayload(context),
    });
    return !!(projection && projection.isMemberExpression);
  };
  runtime.isMemberExpression = runtime.isMemberExpressionNode;
  runtime.isFnExpressionBrowser = function isFnExpressionBrowser(exp) {
    const content = typeof exp === 'string' ? exp : exp && exp.content;
    return /^\s*(?:async\s*)?(?:\([^)]*\)|[A-Za-z_$][\w$]*)[\s\S]*=>/.test(String(content || '')) || /^\s*(?:async\s+)?function\b/.test(String(content || ''));
  };
  runtime.isFnExpressionNode = function isFnExpressionNode(exp) { return runtime.isFnExpressionBrowser(exp); };
  runtime.isFnExpression = runtime.isFnExpressionNode;
  runtime.isFunctionType = function isFunctionType(node) {
    const projection = callBridge('vue3.core.isFunctionType', {
      node: runtime.dehydrateForBridge(node),
    });
    return !!(projection && projection.isFunctionType);
  };
  runtime.nodeAtBridgePath = function nodeAtBridgePath(root, path) {
    let node = root;
    for (const segment of path || []) {
      if (node == null) return undefined;
      node = node[segment];
    }
    return node;
  };
  runtime.bridgePathForChild = function bridgePathForChild(parent, child) {
    if (!parent || !child || typeof parent !== 'object') return undefined;
    for (const key of Object.keys(parent)) {
      const value = parent[key];
      if (value === child) return [key];
      if (Array.isArray(value)) {
        const index = value.indexOf(child);
        if (index !== -1) return [key, index];
      }
    }
    return undefined;
  };
  runtime.bridgeRelationForChild = function bridgeRelationForChild(parent, child) {
    const path = runtime.bridgePathForChild(parent, child);
    return path && typeof path[0] === 'string' ? path[0] : undefined;
  };
  runtime.isStaticProperty = function isStaticProperty(node) {
    const projection = callBridge('vue3.core.isStaticProperty', {
      node: runtime.dehydrateForBridge(node),
    });
    return !!(projection && projection.isStaticProperty);
  };
  runtime.isStaticPropertyKey = function isStaticPropertyKey(node, parent) { return !!parent && runtime.isStaticProperty(parent) && parent.key === node; };
  runtime.unwrapTSNode = function unwrapTSNode(node) {
    while (node && runtime.TS_NODE_TYPES.includes(node.type)) node = node.expression;
    return node;
  };
  runtime.isReferencedIdentifier = function isReferencedIdentifier(id, parent, parentStack = []) {
    const projection = callBridge('vue3.core.isReferencedIdentifier', {
      node: runtime.dehydrateForBridge(id),
      parent: runtime.dehydrateForBridge(parent),
      parentStack: runtime.dehydrateForBridge(parentStack),
      relation: runtime.bridgeRelationForChild(parent, id),
    });
    return !!(projection && projection.isReferencedIdentifier);
  };
  runtime.isInDestructureAssignment = function isInDestructureAssignment(parent, parentStack = []) {
    const projection = callBridge('vue3.core.isInDestructureAssignment', {
      parent: runtime.dehydrateForBridge(parent),
      parentStack: runtime.dehydrateForBridge(parentStack),
    });
    return !!(projection && projection.isInDestructureAssignment);
  };
  runtime.isInNewExpression = function isInNewExpression() { return false; };
  runtime.walkIdentifiers = function walkIdentifiers(root, onIdentifier, includeAll = false, parentStack = [], knownIds = Object.create(null)) {
    const projection = callBridge('vue3.core.walkIdentifiers', {
      root: runtime.dehydrateForBridge(root),
      includeAll: !!includeAll,
      knownIds: runtime.dehydrateForBridge(knownIds),
    });
    for (const event of (projection && projection.identifiers) || []) {
      const id = runtime.nodeAtBridgePath(root, event.path);
      const parent = runtime.nodeAtBridgePath(root, event.parentPath);
      const stack = (event.parentStackPaths || [])
        .map(path => runtime.nodeAtBridgePath(root, path))
        .filter(Boolean);
      if (id) onIdentifier(id, parent || null, stack.length ? stack : parentStack.slice(), !!event.isReferenced, !!event.isLocal);
    }
    if (projection && projection.knownIds) {
      for (const key of Object.keys(knownIds)) delete knownIds[key];
      Object.assign(knownIds, projection.knownIds);
    }
  };
  runtime.extractIdentifiers = function extractIdentifiers(param) {
    if (!param) return [];
    if (typeof param === 'string') return param.split(',').map(s => s.trim()).filter(Boolean).map(content => runtime.createSimpleExpression(content, false));
    if (param.type === NodeTypes.SIMPLE_EXPRESSION) return [param];
    const projection = callBridge('vue3.core.extractIdentifiers', {
      node: runtime.dehydrateForBridge(param),
    });
    return ((projection && projection.identifiers) || [])
      .map(item => runtime.nodeAtBridgePath(param, item.path))
      .filter(Boolean);
  };
  runtime.walkFunctionParams = function walkFunctionParams(node, onIdent) {
    for (const ident of runtime.extractIdentifiers(node && node.params)) onIdent(ident);
  };
  runtime.extractBabelIdentifiers = function extractBabelIdentifiers(node) {
    if (!node) return [];
    if (Array.isArray(node)) return node.flatMap(runtime.extractBabelIdentifiers);
    if (node.type === 'Identifier') return [node];
    if (runtime.TS_NODE_TYPES.includes(node.type)) return runtime.extractBabelIdentifiers(node.expression);
    if (node.type === 'MemberExpression') {
      let object = node;
      while (object && object.type === 'MemberExpression') object = object.object;
      return runtime.extractBabelIdentifiers(object);
    }
    if (node.type === 'ObjectPattern') return (node.properties || []).flatMap(runtime.extractBabelIdentifiers);
    if (node.type === 'ObjectProperty') {
      const out = [];
      if (node.computed && node.key) out.push(...runtime.extractBabelIdentifiers(node.key));
      if (node.value) out.push(...runtime.extractBabelIdentifiers(node.value));
      return out;
    }
    if (node.type === 'ArrayPattern') return (node.elements || []).flatMap(runtime.extractBabelIdentifiers);
    if (node.type === 'RestElement') return runtime.extractBabelIdentifiers(node.argument);
    if (node.type === 'AssignmentPattern') return runtime.extractBabelIdentifiers(node.left);
    return [];
  };
  runtime.walkBlockDeclarations = function walkBlockDeclarations() {};
  runtime.babelParse = function babelParse(source, options) {
    const parser = runtime.getBabelParser();
    if (!parser || typeof parser.parse !== 'function') throw new Error('@babel/parser is unavailable');
    return parser.parse(source, options);
  };
  runtime.createTransformContext = function createTransformContext(root, options = {}) {
    const canonicalHelpers = new Map();
    const canonicalHelper = name => {
      const helperName = runtime.helperNameMap[name];
      if (!helperName) return name;
      if (!canonicalHelpers.has(helperName)) canonicalHelpers.set(helperName, name);
      return canonicalHelpers.get(helperName);
    };
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
        name = canonicalHelper(name);
        context.helpers.set(name, (context.helpers.get(name) || 0) + 1);
        return name;
      },
      removeHelper(name) {
        name = canonicalHelper(name);
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
      addIdentifiers(exp) {
        for (const name of runtime.expressionIdentifierNames(exp)) {
          context.identifiers[name] = (context.identifiers[name] || 0) + 1;
        }
      },
      removeIdentifiers(exp) {
        for (const name of runtime.expressionIdentifierNames(exp)) {
          if (!context.identifiers[name]) continue;
          context.identifiers[name]--;
          if (context.identifiers[name] <= 0) delete context.identifiers[name];
        }
      },
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
    if (options.hoistStatic) runtime.cacheStatic(root, context);
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
  runtime.baseCompile = function baseCompile(source, options = {}) {
    const onError = options.onError || (error => { throw error; });
    const isModuleMode = options.mode === 'module';
    const prefixIdentifiers = !runtime.isBrowserBuild() && (options.prefixIdentifiers === true || isModuleMode);
    if (!prefixIdentifiers && options.cacheHandlers) {
      onError(runtime.createCompilerError(ErrorCodes.X_CACHE_HANDLER_NOT_SUPPORTED));
    }
    if (options.scopeId && !isModuleMode) {
      onError(runtime.createCompilerError(ErrorCodes.X_SCOPE_ID_NOT_SUPPORTED));
    }
    const resolvedOptions = Object.assign({}, options, { prefixIdentifiers });
    const ast = typeof source === 'string'
      ? hydrateVue3Ast(callBridge('vue3.core.baseParse', bridgePayloadForCall(vue3BridgePayload(source, resolvedOptions.filename, resolvedOptions))), resolvedOptions)
      : hydrateVue3Ast(source, resolvedOptions);
    const [nodeTransforms, directiveTransforms] = runtime.getBaseTransformPreset(prefixIdentifiers);
    runtime.transform(ast, Object.assign({}, resolvedOptions, {
      nodeTransforms: [
        ...nodeTransforms,
        ...(options.nodeTransforms || []),
      ],
      directiveTransforms: Object.assign(
        {},
        directiveTransforms,
        options.directiveTransforms || {},
      ),
    }));
    return runtime.generate(ast, resolvedOptions);
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
    const isSetupInlined = !!options.inline;
    let code = '';
    let indentLevel = 0;
    let preamble = '';
    let pure = false;
    let activeBuffer = isSetupInlined ? 'preamble' : 'code';
    const currentOutput = () => activeBuffer === 'preamble' ? preamble : code;
    const push = value => {
      if (activeBuffer === 'preamble') preamble += String(value);
      else code += String(value);
    };
    const currentIndent = () => '  '.repeat(indentLevel);
    const newline = () => { push(`\n${currentIndent()}`); };
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
      if (!currentOutput()) push(`\n`);
      else {
        if (!currentOutput().endsWith('\n')) newline();
        if (!currentOutput().endsWith('\n\n')) newline();
      }
      if (!isSetupInlined) push(`export `);
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
      genHoists(ast.hoists || []);
      if (!currentOutput()) push(`\n`);
      else newline();
      push(`return `);
    }
    if (isSetupInlined) {
      activeBuffer = 'code';
      indentLevel = 0;
    }

    const functionName = ssr ? 'ssrRender' : 'render';
    const args = ssr ? ['_ctx', '_push', '_parent', '_attrs'] : ['_ctx', '_cache'];
    if (options.bindingMetadata && !options.inline) args.push('$props', '$setup', '$data', '$options');
    if (isSetupInlined) {
      push(`(${args.join(', ')}) => {`);
    } else {
      push(`function ${functionName}(${args.join(', ')}) {`);
    }
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
          else genForExpression(node);
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
      if (multilines) {
        indent();
      }
      genNodeList(nodes, multilines, true);
      if (multilines) {
        deindent();
      }
      push(`]`);
    }

    function genVNodeCall(node) {
      const call = node.isBlock ? helper(runtime.getVNodeBlockHelper(ssr, node.isComponent)) : helper(runtime.getVNodeHelper(ssr, node.isComponent));
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
      if (pure) push(`/*@__PURE__*/`);
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
      if (ssr && node.test && node.test.type !== NodeTypes.SIMPLE_EXPRESSION) {
        push(`(`);
        genNode(node.test);
        push(`)`);
      } else if (node.test && node.test.type === NodeTypes.SIMPLE_EXPRESSION && !node.test.isStatic && !runtime.isSimpleIdentifier(node.test.content)) {
        push(`(`);
        genNode(node.test);
        push(`)`);
      } else {
        genNode(node.test);
      }
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
      indentLevel++;
      genNode(node.consequent);
      indentLevel--;
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

    function genForExpression(node) {
      const blockHelper = helper(runtime.getVNodeBlockHelper(ssr, false));
      push(`(${helper(runtime.OPEN_BLOCK)}(true), ${blockHelper}(${helper(runtime.FRAGMENT)}, null, ${helper(runtime.RENDER_LIST)}(`);
      genNode(node.source);
      push(`, (`);
      genNodeList(runtime.createForLoopParams(node.parseResult || node), false, true);
      push(`) => {`);
      indent();
      push(`return `);
      const children = node.children || [];
      const child = children.length === 1 ? children[0] : children;
      if (Array.isArray(child)) {
        push(`(${helper(runtime.OPEN_BLOCK)}(), ${blockHelper}(${helper(runtime.FRAGMENT)}, null, `);
        genNodeListAsArray(child);
        push(`, 64 /* STABLE_FRAGMENT */))`);
      } else if (child && child.type === NodeTypes.TEXT_CALL) {
        push(`(${helper(runtime.OPEN_BLOCK)}(), ${blockHelper}(${helper(runtime.FRAGMENT)}, null, [`);
        indent();
        genNode(child);
        deindent();
        push(`], 64 /* STABLE_FRAGMENT */))`);
      } else {
        genNode(child);
      }
      deindent();
      push(`}), 256 /* UNKEYED_FRAGMENT */))`);
    }

    function genTemplateLiteral(node) {
      push('`');
      const elements = node.elements || [];
      const multiline = ssr && elements.filter(element => typeof element !== 'string').length > 1;
      for (const element of elements) {
        if (typeof element === 'string') {
          push(element.replace(/(`|\$|\\)/g, '\\$1'));
        } else {
          push('${');
          if (multiline) {
            indentLevel++;
            newline();
          }
          genNode(element);
          if (multiline) {
            indentLevel--;
            newline();
          }
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
      const previousPure = pure;
      pure = true;
      newline();
      for (let i = 0; i < hoists.length; i++) {
        const exp = hoists[i];
        if (!exp) continue;
        push(`const _hoisted_${i + 1} = `);
        genNode(exp);
        newline();
      }
      pure = previousPure;
    }

    function patchFlagText(flag) {
      if (flag == null) return flag;
      if (typeof flag === 'string' && /\/\*/.test(flag)) return flag;
      const value = Number(flag);
      if (!Number.isFinite(value) || value === 0) return flag;
      const names = {
        1: 'TEXT', 2: 'CLASS', 4: 'STYLE', 8: 'PROPS', 16: 'FULL_PROPS',
        32: 'NEED_HYDRATION', 64: 'STABLE_FRAGMENT', 128: 'KEYED_FRAGMENT',
        256: 'UNKEYED_FRAGMENT', 512: 'NEED_PATCH', 1024: 'DYNAMIC_SLOTS',
        2048: 'DEV_ROOT_FRAGMENT', [-1]: 'CACHED', [-2]: 'BAIL',
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
      if (node.tagType === ElementTypes.TEMPLATE && (node.props || []).some(runtime.isVSlot)) return;
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
    const projection = callBridge('vue3.core.processExpression', {
      node: runtime.dehydrateForBridge(node),
      context: vue3ProcessExpressionContextPayload(context),
      asParams: !!asParams,
      asRawStatements: !!asRawStatements,
      localVars: localVars || null,
    });
    return materializeVue3ProcessExpressionProjection(projection, node, context);
  };
  runtime.transformExpression = function transformExpression(node, context) {
    const projection = callBridge('vue3.core.transformExpression', {
      node: runtime.dehydrateForBridge(node),
      context: vue3ProcessExpressionContextPayload(context),
    });
    materializeVue3TransformExpressionProjection(projection, node, context);
  };
  runtime.isBrowserBuild = function isBrowserBuild() {
    return typeof __BROWSER__ !== 'undefined' && !!__BROWSER__;
  };
  runtime.modifierName = function modifierName(modifier) {
    return typeof modifier === 'string' ? modifier : modifier && modifier.content;
  };
  runtime.hasModifier = function hasModifier(dir, name) {
    return (dir.modifiers || []).some(modifier => runtime.modifierName(modifier) === name);
  };
  runtime.injectBindPrefix = function injectBindPrefix(arg, prefix) {
    if (arg.type === NodeTypes.SIMPLE_EXPRESSION) {
      if (arg.isStatic) {
        arg.content = prefix + arg.content;
      } else {
        arg.content = `\`${prefix}\${${arg.content}}\``;
      }
    } else {
      arg.children.unshift(`'${prefix}' + (`);
      arg.children.push(`)`);
    }
  };
  runtime.transformBind = function transformBind(dir, _node, context) {
    context = context || {
      helper: name => name,
      helperString: name => `_${runtime.helperNameMap[name] || name}`,
      inSSR: false,
      onError: error => { throw error; },
    };
    const projection = callBridge('vue3.core.transformBind', {
      dir,
      context: vue3TransformBindContextPayload(context),
    });
    materializeVue3BindErrors(projection, dir, context);
    return {
      props: (projection && projection.props || []).map(prop => {
        const key = materializeVue3OnProjection(prop.key, dir, context);
        const value = materializeVue3OnProjection(prop.value, dir, context);
        return runtime.createObjectProperty(key, value);
      }),
    };
  };
  runtime.transformOn = function transformOn(dir, node, context, augmentor) {
    context = context || { helperString: name => `_${runtime.helperNameMap[name] || name}`, helper: name => name, cache: value => value, onError: error => { throw error; } };
    const projection = callBridge('vue3.core.transformOn', {
      dir,
      node,
      context: vue3TransformOnContextPayload(context),
    });
    materializeVue3OnErrors(projection, dir, context);
    const onMeta = (projection.props || []).map(prop => ({
      cache: !!prop.cache,
      valueConstant: !!prop.valueConstant,
      handlerKey: !!prop.handlerKey,
      dynamicKey: !!prop.dynamicKey,
      ignoreDynamicKeyForNormalize: !!prop.ignoreDynamicKeyForNormalize,
    }));
    let result = {
      props: (projection.props || []).map(prop => {
        const key = materializeVue3OnProjection(prop.key, dir, context);
        const value = materializeVue3OnProjection(prop.value, dir, context) || runtime.createSimpleExpression('() => {}', false, dir.loc);
        return runtime.createObjectProperty(key, value);
      }),
    };
    if (typeof augmentor === 'function') result = augmentor(result) || result;
    for (const [index, prop] of (result.props || []).entries()) {
      const meta = onMeta[index] || onMeta[0] || {};
      if (prop.key && meta.handlerKey) prop.key.isHandlerKey = true;
      if (meta.cache && context && context.cache) prop.value = context.cache(prop.value);
      prop.__vuecOn = meta;
    }
    return result;
  };
  runtime.transformModel = function transformModel(dir) {
    const projection = callBridge('vue3.core.transformModel', {
      dir,
      node: arguments[1],
      context: vue3TransformModelContextPayload(arguments[2]),
    });
    for (const code of projection.errors || []) {
      const loc = code === ErrorCodes.X_V_MODEL_NO_EXPRESSION ? dir.loc : dir.exp && dir.exp.loc || dir.loc;
      if (arguments[2] && arguments[2].onError) arguments[2].onError(runtime.createCompilerError(code, loc));
    }
    return {
      props: (projection.props || []).map(prop => {
        const key = materializeVue3ModelProjection(prop.key, dir, arguments[2]);
        const value = materializeVue3ModelProjection(prop.value, dir, arguments[2]);
        const objectProp = runtime.createObjectProperty(key, value);
        objectProp.__vuecModel = {
          dynamic: !!prop.dynamic,
          cache: !!prop.cache,
          hydrate: !!prop.hydrate,
          kind: prop.kind,
        };
        if (prop.cache && arguments[2]) objectProp.value = arguments[2].cache(objectProp.value);
        return objectProp;
      }),
    };
  };
  runtime.transformVBindShorthand = function transformVBindShorthand(node, context) {
    if (!node || node.type !== NodeTypes.ELEMENT) return;
    const projection = callBridge('vue3.core.transformVBindShorthand', {
      node: runtime.dehydrateForBridge(node),
      context: vue3TransformVBindShorthandContextPayload(context),
    });
    materializeVue3VBindShorthandProjection(projection, node, context);
  };
  runtime.transformElement = function transformElement(node, context) {
    return () => {
      node = context.currentNode;
      if (!node || node.type !== NodeTypes.ELEMENT) return;
      if (node.tagType !== ElementTypes.ELEMENT && node.tagType !== ElementTypes.COMPONENT) return;
      const isComponent = node.tagType === ElementTypes.COMPONENT;
      const tag = isComponent ? runtime.resolveComponentType(node, context) : `"${node.tag}"`;
      const isDynamicComponent = isComponent && tag && typeof tag === 'object' && tag.type === NodeTypes.JS_CALL_EXPRESSION && tag.callee === runtime.RESOLVE_DYNAMIC_COMPONENT;
      let patchFlag;
      let props;
      let hasDynamicKey = false;
      let hasHydrationEvent = false;
      const dynamicProps = [];
      const propSummaries = [];
      let vnodeDirectives;
      let shouldUseBlock = !!(
        isDynamicComponent
        || tag === runtime.TELEPORT
        || tag === runtime.SUSPENSE
        || (!isComponent && (node.tag === 'svg' || node.tag === 'foreignObject' || node.tag === 'math'))
      );
      if (node.props && node.props.length) {
        const objectProps = [];
        const mergeArgs = [];
        const runtimeDirectives = [];
        const pushMergeArg = arg => {
          if (objectProps.length) {
            mergeArgs.push(runtime.createObjectExpression(objectProps.splice(0), node.loc));
          }
          if (arg) mergeArgs.push(arg);
        };
        for (const prop of node.props) {
          if (prop.type === NodeTypes.ATTRIBUTE) {
            if (
              prop.name === 'is'
              && (
                node.tag === 'component'
                || node.tag === 'Component'
                || (prop.value && String(prop.value.content || '').startsWith('vue:'))
              )
            ) {
              continue;
            }
            objectProps.push(runtime.createObjectProperty(prop.name, runtime.createSimpleExpression(prop.value ? prop.value.content : '', true)));
            propSummaries.push({ kind: 'attribute', name: prop.name, value: prop.value && prop.value.content });
          } else if (prop.name === 'bind' && prop.arg) {
            if (
              runtime.isStaticArgOf(prop.arg, 'is')
              && (node.tag === 'component' || node.tag === 'Component')
            ) {
              continue;
            }
            const transform = context.directiveTransforms && context.directiveTransforms.bind;
            if (!transform) {
              if (runtime.isStaticArgOf(prop.arg, 'key')) propSummaries.push({ kind: 'directiveProp', forceBlock: true });
              continue;
            }
            const result = transform(prop, node, context);
            objectProps.push(...((result && result.props) || []));
            propSummaries.push(...vue3ElementDirectivePropSummaries(prop, result, {
              forceBlock: runtime.isStaticArgOf(prop.arg, 'key'),
              propModifier: runtime.hasModifier(prop, 'prop'),
            }));
            if (result && result.props && result.props.some(p => p.key && !runtime.isStaticExp(p.key))) hasDynamicKey = true;
            else if (prop.arg.isStatic) dynamicProps.push(prop.arg.content);
          } else if (prop.name === 'on' && prop.arg) {
            const transform = context.directiveTransforms && context.directiveTransforms.on;
            const result = transform ? transform(prop, node, context) : undefined;
            objectProps.push(...((result && result.props) || []));
            if (!result && node.children && node.children.length && runtime.isStaticArgOf(prop.arg, 'vue:before-update')) {
              propSummaries.push({ kind: 'directiveProp', forceBlock: true });
            }
            propSummaries.push(...vue3ElementDirectivePropSummaries(prop, result, {
              forceBlock: !!(node.children && node.children.length && runtime.isStaticArgOf(prop.arg, 'vue:before-update')),
            }));
            if (!result || !result.props || !result.props.some(p => p.value && p.value.type === NodeTypes.JS_CACHE_EXPRESSION)) {
              if (result && result.props && result.props.some(p => p.key && p.key.isHandlerKey)) dynamicProps.push(result.props[0].key.content || prop.arg.content);
            }
          } else if (prop.name === 'bind' && !prop.arg) {
            if (prop.exp) {
              pushMergeArg(prop.exp);
              hasDynamicKey = true;
              propSummaries.push({ kind: 'objectBind' });
            } else {
              context.onError(runtime.createCompilerError(ErrorCodes.X_V_BIND_NO_EXPRESSION, prop.loc));
            }
          } else if (prop.name === 'on' && !prop.arg) {
            if (prop.exp) {
              pushMergeArg(runtime.createCallExpression(context.helper(runtime.TO_HANDLERS), isComponent ? [prop.exp] : [prop.exp, 'true'], prop.loc));
              hasDynamicKey = true;
              propSummaries.push({ kind: 'objectOn' });
            } else {
              context.onError(runtime.createCompilerError(ErrorCodes.X_V_ON_NO_EXPRESSION, prop.loc));
            }
          } else if (prop.name === 'model' && context.directiveTransforms && context.directiveTransforms.model) {
            const result = context.directiveTransforms.model(prop, node, context);
            const modelProps = (result && result.props) || [];
            objectProps.push(...modelProps);
            propSummaries.push(...vue3ElementDirectivePropSummaries(prop, result));
            if (modelProps.some(p => p.key && !runtime.isStaticExp(p.key))) hasDynamicKey = true;
            for (const modelProp of modelProps) {
              if (modelProp.__vuecModel && modelProp.__vuecModel.dynamic && runtime.isStaticExp(modelProp.key)) {
                dynamicProps.push(modelProp.key.content);
              }
              if (modelProp.__vuecModel && modelProp.__vuecModel.hydrate) {
                hasHydrationEvent = true;
              }
            }
            if (result && result.needRuntime) {
              prop.__vuecNeedRuntime = result.needRuntime;
              runtimeDirectives.push(prop);
              propSummaries.push({ kind: 'runtimeDirective' });
            }
          } else if (prop.name === 'once' || prop.name === 'memo') {
            continue;
          } else if (prop.name === 'slot') {
            if (!isComponent) context.onError(runtime.createCompilerError(ErrorCodes.X_V_SLOT_MISPLACED, prop.loc));
            continue;
          } else if (context.directiveTransforms && context.directiveTransforms[prop.name]) {
            const result = context.directiveTransforms[prop.name](prop, node, context);
            objectProps.push(...((result && result.props) || []));
            propSummaries.push(...vue3ElementDirectivePropSummaries(prop, result));
            if (result && result.needRuntime) {
              prop.__vuecNeedRuntime = result.needRuntime;
              runtimeDirectives.push(prop);
              propSummaries.push({ kind: 'runtimeDirective' });
            }
          } else {
            runtimeDirectives.push(prop);
            if (!runtime.isBuiltInDirective(prop.name)) propSummaries.push({ kind: 'runtimeDirective' });
          }
        }
        if (mergeArgs.length) {
          pushMergeArg();
          props = mergeArgs.length > 1 ? runtime.createCallExpression(context.helper(runtime.MERGE_PROPS), mergeArgs, node.loc) : mergeArgs[0];
        } else if (objectProps.length) {
          props = runtime.createObjectExpression(runtime.dedupeProperties(objectProps), node.loc);
        }
        const propsProjection = callBridge('vue3.core.transformElementProps', {
          props: propSummaries,
          hasChildren: !!(node.children && node.children.length),
          isComponent,
          isDynamicComponent,
          context: vue3TransformElementContextPayload(context),
        });
        if (propsProjection && propsProjection.refForMarker) {
          props = runtime.prependPropsExpressionProp(
            props,
            runtime.createObjectProperty('ref_for', runtime.createSimpleExpression('true')),
            node.loc,
          );
        }
        if (props && propsProjection && propsProjection.inlineTemplateRefs) {
          props = runtime.applyInlineTemplateRefProjection(props, propsProjection.inlineTemplateRefs, node.loc);
        }
        if (props && propsProjection && propsProjection.normalizeClass) runtime.normalizeObjectProp(props, 'class', context.helper(runtime.NORMALIZE_CLASS));
        if (props && propsProjection && propsProjection.normalizeStyle) runtime.normalizeObjectProp(props, 'style', context.helper(runtime.NORMALIZE_STYLE));
        if (props && propsProjection && propsProjection.normalizeProps) {
          if (!(props.type === NodeTypes.JS_CALL_EXPRESSION && (props.callee === runtime.MERGE_PROPS || props.callee === runtime.TO_HANDLERS))) {
            const argument = propsProjection.guardReactiveProps
              ? runtime.createCallExpression(context.helper(runtime.GUARD_REACTIVE_PROPS), [props], node.loc)
              : props;
            props = runtime.createCallExpression(context.helper(runtime.NORMALIZE_PROPS), [argument], node.loc);
          } else if (propsProjection.guardReactiveProps && props.type !== NodeTypes.JS_CALL_EXPRESSION) {
            props = runtime.createCallExpression(context.helper(runtime.NORMALIZE_PROPS), [runtime.createCallExpression(context.helper(runtime.GUARD_REACTIVE_PROPS), [props], node.loc)], node.loc);
          }
        }
        patchFlag = propsProjection && propsProjection.patchFlag || undefined;
        dynamicProps.splice(0, dynamicProps.length, ...((propsProjection && propsProjection.dynamicPropNames) || dynamicProps));
        if (propsProjection && propsProjection.shouldUseBlock) shouldUseBlock = true;
        if (hasHydrationEvent) patchFlag = (patchFlag || 0) | 32;
        if (runtimeDirectives.length) {
          const directiveArgs = runtimeDirectives.map(d => {
            return runtime.buildDirectiveArgs(d, context);
          });
          vnodeDirectives = runtime.createArrayExpression(directiveArgs);
          if (!shouldUseBlock && (!patchFlag || patchFlag === 32)) patchFlag = (patchFlag || 0) | 512;
        }
      }
      const onlyChild = node.children && node.children.length === 1 ? node.children[0] : undefined;
      let children = onlyChild && [NodeTypes.TEXT, NodeTypes.INTERPOLATION, NodeTypes.COMPOUND_EXPRESSION].includes(onlyChild.type)
        ? onlyChild
        : node.children && node.children.length
          ? node.children
          : undefined;
      if (isComponent && node.children && node.children.length && tag !== runtime.TELEPORT && tag !== runtime.KEEP_ALIVE) {
        const builtSlots = runtime.buildSlots(node, context);
        children = builtSlots.slots;
        if (builtSlots.hasDynamicSlots) patchFlag = (patchFlag || 0) | 1024;
      } else {
        const childrenProjection = callBridge('vue3.core.transformElementChildren', {
          tag: projectionNameFromHelperSymbol(tag),
          children: node.children || [],
        });
        if (childrenProjection && childrenProjection.kind === 'slots') {
          children = materializeVue3ElementSlotsProjection(childrenProjection, node, context);
          if (childrenProjection.shouldUseBlock) shouldUseBlock = true;
        } else if (childrenProjection && childrenProjection.kind === 'children') {
          if (childrenProjection.shouldUseBlock) shouldUseBlock = true;
          if (childrenProjection.patchFlag) patchFlag = (patchFlag || 0) | childrenProjection.patchFlag;
        }
      }
      if (!patchFlag && children && (children.type === NodeTypes.INTERPOLATION || children.type === NodeTypes.COMPOUND_EXPRESSION) && runtime.getConstantType(children, context) === ConstantTypes.NOT_CONSTANT) patchFlag = 1;
      node.codegenNode = runtime.createVNodeCall(context, tag, props, children, patchFlag, dynamicProps.length ? stringifyDynamicPropNames(dynamicProps) : undefined, vnodeDirectives, shouldUseBlock, false, isComponent, node.loc);
    };
  };
  runtime.processSlotOutlet = function processSlotOutlet(node, context) {
    const projection = callBridge('vue3.core.transformSlotOutlet', {
      node: runtime.dehydrateForBridge(node),
      context: vue3TransformSlotOutletContextPayload(context),
    });
    const process = projection && projection.process || {};
    materializeVue3SlotOutletMutations(process, node, context);
    const nonNameProps = (process.nonNameProps || [])
      .map(index => node.props && node.props[index])
      .filter(Boolean);
    const slotName = materializeVue3SlotOutletName(process.slotName, node, context);
    let slotProps;
    if (nonNameProps.length) {
      const built = runtime.buildProps(node, context, nonNameProps, false, false);
      slotProps = built.props;
      if (built.directives && built.directives.length) {
        context.onError(runtime.createCompilerError(ErrorCodes.X_V_SLOT_UNEXPECTED_DIRECTIVE_ON_SLOT_OUTLET, built.directives[0].loc));
      }
    }
    return { slotName, slotProps };
  };
  runtime.transformSlotOutlet = function transformSlotOutlet(node, context) {
    if (node.type === NodeTypes.ELEMENT && node.tagType === ElementTypes.SLOT) {
      return () => {
        const projection = callBridge('vue3.core.transformSlotOutlet', {
          node: runtime.dehydrateForBridge(node),
          context: vue3TransformSlotOutletContextPayload(context),
        });
        if (!projection || !projection.transform) return;
        const process = projection.process || {};
        materializeVue3SlotOutletMutations(process, node, context);
        const nonNameProps = (process.nonNameProps || [])
          .map(index => node.props && node.props[index])
          .filter(Boolean);
        const slotName = materializeVue3SlotOutletName(process.slotName, node, context);
        let slotProps;
        if (nonNameProps.length) {
          const built = runtime.buildProps(node, context, nonNameProps, false, false);
          slotProps = built.props;
          if (built.directives && built.directives.length) {
            context.onError(runtime.createCompilerError(ErrorCodes.X_V_SLOT_UNEXPECTED_DIRECTIVE_ON_SLOT_OUTLET, built.directives[0].loc));
          }
        }
        const codegen = projection.codegen || {};
        const args = [codegen.slots || (context.prefixIdentifiers ? '_ctx.$slots' : '$slots'), slotName, '{}', 'undefined', 'true'];
        let expectedLen = codegen.expectedLen == null ? 2 : codegen.expectedLen;
        if (slotProps) {
          args[2] = slotProps;
          expectedLen = Math.max(expectedLen, 3);
        }
        if (node.children && node.children.length) {
          args[3] = runtime.createFunctionExpression([], node.children, false, false, node.loc);
          expectedLen = Math.max(expectedLen, 4);
        }
        args.splice(expectedLen);
        node.codegenNode = runtime.createCallExpression(context.helper(runtime.RENDER_SLOT), args);
      };
    }
  };
  runtime.transformText = function transformText(node, context) {
    if (![NodeTypes.ROOT, NodeTypes.ELEMENT, NodeTypes.FOR, NodeTypes.IF_BRANCH].includes(node.type)) return;
    return () => {
      const projection = callBridge('vue3.core.transformText', {
        node: runtime.dehydrateForBridge(node),
        context: vue3TransformTextContextPayload(context),
      });
      materializeVue3TransformTextProjection(projection, node, context);
    };
  };
  runtime.findUntransformedCustomDirective = function findUntransformedCustomDirective(node, context) {
    return (node.props || []).find(prop => prop.type === NodeTypes.DIRECTIVE && !(context.directiveTransforms || {})[prop.name]);
  };
  runtime.processIf = function processIf(node, dir, context, processCodegen) {
    const siblings = context.parent && context.parent.children || [];
    const nodeIndex = siblings.indexOf(node);
    const projection = callBridge('vue3.core.transformIf', {
      phase: 'process',
      node,
      dir,
      parent: context.parent,
      siblings: vue3IfSiblingPayload(siblings),
      nodeIndex,
      currentUserKey: runtime.findProp(node, 'key'),
      context: vue3TransformIfContextPayload(context),
    });
    materializeVue3IfErrors(projection, node, dir, context);
    if (projection && projection.branch && projection.branch.condition) {
      dir.exp = materializeVue3IfProjection(projection.branch.condition, node, dir, context);
    }
    const branch = {
      type: NodeTypes.IF_BRANCH,
      loc: node.loc,
      condition: dir.name === 'else' ? undefined : dir.exp,
      children: projection && projection.branch && projection.branch.children === 'template' ? (node.children || []) : [node],
      userKey: runtime.findProp(node, 'key'),
      isTemplateIf: node.tagType === ElementTypes.TEMPLATE,
    };
    const action = projection && projection.action || { kind: 'noop' };
    const finalizeBranch = (ifNode, targetBranch, isRoot) => {
      if (processCodegen) return processCodegen(ifNode, targetBranch, isRoot);
      if (context && context.ssr) return undefined;
      return () => {
        if (isRoot) {
          ifNode.codegenNode = runtime.createIfCodegenNodeForBranch(targetBranch, action.keyBase || 0, context);
        } else {
          const parentCondition = runtime.getParentCondition(ifNode.codegenNode);
          parentCondition.alternate = runtime.createIfCodegenNodeForBranch(targetBranch, (ifNode.__vuecKeyBase || 0) + ifNode.branches.length - 1, context);
        }
      };
    };
    if (dir.name !== 'if') {
      if (action.kind === 'append') {
        const comments = (action.commentIndices || []).map(index => siblings[index]).filter(Boolean);
        for (const index of [...(action.removeIndices || [])].sort((a, b) => b - a)) {
          const sibling = siblings[index];
          if (sibling) context.removeNode(sibling);
        }
        const target = siblings[action.targetIndex];
        context.removeNode();
        if (comments.length) branch.children = [...comments, ...branch.children];
        target.branches.push(branch);
        const onExit = finalizeBranch(target, branch, false);
        runtime.traverseNode(branch, context);
        if (onExit) onExit();
        context.currentNode = null;
      }
      return;
    }
    const ifNode = { type: NodeTypes.IF, loc: node.loc, branches: [branch], codegenNode: undefined };
    ifNode.__vuecKeyBase = action.keyBase || 0;
    context.replaceNode(ifNode);
    const onExit = finalizeBranch(ifNode, branch, true);
    return () => {
      if (onExit) onExit();
    };
  };
  runtime.refreshIfCodegen = function refreshIfCodegen(ifNode, context, keyBase = 0) {
    let alternate = runtime.createCallExpression(context.helper(runtime.CREATE_COMMENT), ['"v-if"', 'true']);
    for (let i = ifNode.branches.length - 1; i >= 0; i--) {
      const branch = ifNode.branches[i];
      const childCodegen = runtime.createIfBranchCodegen(branch, keyBase + i, context);
      if (branch.condition) {
        alternate = runtime.createConditionalExpression(branch.condition, childCodegen, alternate);
      } else {
        alternate = childCodegen;
      }
    }
    if (ifNode.codegenNode && ifNode.codegenNode.type === NodeTypes.JS_CACHE_EXPRESSION) {
      ifNode.codegenNode.value = alternate;
    } else {
      ifNode.codegenNode = alternate;
    }
  };
  runtime.createIfCodegenNodeForBranch = function createIfCodegenNodeForBranch(branch, keyIndex, context) {
    const childCodegen = runtime.createIfBranchCodegen(branch, keyIndex, context);
    if (branch.condition) {
      return runtime.createConditionalExpression(
        branch.condition,
        childCodegen,
        runtime.createCallExpression(context.helper(runtime.CREATE_COMMENT), ['"v-if"', 'true']),
      );
    }
    return childCodegen;
  };
  runtime.createIfBranchCodegen = function createIfBranchCodegen(branch, keyIndex, context) {
    const keyProperty = runtime.createObjectProperty('key', runtime.createSimpleExpression(String(keyIndex), false, locStub, ConstantTypes.CAN_CACHE));
    const children = branch.children || [];
    const firstChild = children[0];
    const projection = callBridge('vue3.core.transformIf', {
      phase: 'branchCodegen',
      branch: vue3IfBranchCodegenPayload(branch),
      keyIndex,
    });
    if (projection.kind === 'for') {
      const vnodeCall = firstChild.codegenNode;
      runtime.injectProp(vnodeCall, keyProperty);
      return vnodeCall;
    }
    if (projection.kind === 'fragment') {
      return runtime.createVNodeCall(context, context.helper(runtime.FRAGMENT), runtime.createObjectExpression([keyProperty]), children, projection.patchFlag, undefined, undefined, true, false, false, branch.loc);
    }
    const ret = firstChild.codegenNode;
    const vnodeCall = runtime.getMemoedVNodeCall(ret);
    if (vnodeCall) {
      if (vnodeCall.type === NodeTypes.VNODE_CALL) {
        runtime.convertToBlock(vnodeCall, context);
      }
      runtime.injectProp(vnodeCall, keyProperty);
    }
    return ret;
  };
  runtime.getParentCondition = function getParentCondition(node) {
    while (node) {
      if (node.type === NodeTypes.JS_CONDITIONAL_EXPRESSION) {
        if (node.alternate && node.alternate.type === NodeTypes.JS_CONDITIONAL_EXPRESSION) {
          node = node.alternate;
        } else {
          return node;
        }
      } else if (node.type === NodeTypes.JS_CACHE_EXPRESSION) {
        node = node.value;
      } else {
        return node;
      }
    }
    return node;
  };
  runtime.transformIf = runtime.createStructuralDirectiveTransform(/^(if|else|else-if)$/, runtime.processIf);
  runtime.processFor = function processFor(node, dir, context, processCodegen) {
    const projection = callBridge('vue3.core.transformFor', {
      node,
      dir,
      context: vue3TransformForContextPayload(context),
    });
    materializeVue3ForErrors(projection, node, dir, context);
    if (!projection || !projection.parseResult) return;
    const parsed = materializeVue3ForParseResult(projection.parseResult, dir, context);
    const aliases = projection.locals || [];
    if (context.prefixIdentifiers) aliases.forEach(alias => context.addIdentifiers(alias));
    const children = node.tagType === ElementTypes.TEMPLATE ? node.children || [] : [node];
    const forNode = {
      type: NodeTypes.FOR,
      loc: dir.loc,
      source: parsed.source,
      valueAlias: parsed.value,
      keyAlias: parsed.key,
      objectIndexAlias: parsed.index,
      parseResult: parsed,
      children,
      codegenNode: undefined,
      __vuecProjection: projection,
    };
    let renderExp;
    if (!processCodegen && !(context && context.ssr)) {
      renderExp = runtime.createCallExpression(context.helper(runtime.RENDER_LIST), [forNode.source]);
      forNode.codegenNode = runtime.createVNodeCall(context, context.helper(runtime.FRAGMENT), undefined, renderExp, 256, undefined, undefined, true, true, false, node.loc);
    }
    context.replaceNode(forNode);
    context.scopes.vFor++;
    const onExit = processCodegen ? processCodegen(forNode) : undefined;
    return () => {
      context.scopes.vFor--;
      if (context.prefixIdentifiers) aliases.forEach(alias => context.removeIdentifiers(alias));
      if (onExit) {
        onExit();
      } else if (renderExp) {
        materializeVue3ForTemplateKeyErrors(projection, node, dir, context);
        runtime.finalizeForCodegen(forNode, renderExp, context);
      }
    };
  };
  runtime.transformFor = runtime.createStructuralDirectiveTransform('for', runtime.processFor);
  runtime.transformFor = runtime.createStructuralDirectiveTransform('for', (node, dir, context) => {
    return runtime.processFor(node, dir, context, (forNode) => {
      const renderExp = runtime.createCallExpression(context.helper(runtime.RENDER_LIST), [forNode.source]);
      const codegenProjection = callBridge('vue3.core.transformFor', {
        phase: 'codegen',
        node,
        forNode: vue3ForNodePayload(forNode),
        context: vue3TransformForContextPayload(context),
      });
      const keyProperty = materializeVue3ForKeyProperty(codegenProjection && codegenProjection.keyProperty, dir, context);
      const isStableFragment = !!(codegenProjection && codegenProjection.isStableFragment);
      forNode.codegenNode = runtime.createVNodeCall(
        context,
        context.helper(runtime.FRAGMENT),
        undefined,
        renderExp,
        codegenProjection && codegenProjection.fragmentFlag || 256,
        undefined,
        undefined,
        true,
        codegenProjection ? !!codegenProjection.disableTracking : true,
        false,
        node.loc,
      );
      return () => {
        materializeVue3ForTemplateKeyErrors(forNode.__vuecProjection, node, dir, context);
        const exitProjection = callBridge('vue3.core.transformFor', {
          phase: 'exitCodegen',
          node,
          forNode: vue3ForNodePayload(forNode),
          isStableFragment,
        });
        const childBlock = materializeVue3ForChildBlock(exitProjection, node, forNode, keyProperty, context);
        renderExp.arguments.push(runtime.createFunctionExpression(runtime.createForLoopParams(forNode.parseResult), childBlock, true));
      };
    });
  });
  runtime.createForLoopParams = function createForLoopParams(parseResult) {
    const args = [parseResult.value, parseResult.key, parseResult.index];
    let i = args.length;
    while (i--) {
      if (args[i]) break;
    }
    return args.slice(0, i + 1).map((arg, index) => arg || runtime.createSimpleExpression(`_`.repeat(index + 1), false));
  };
  runtime.finalizeForCodegen = function finalizeForCodegen(forNode, renderExp, context) {
    if (!renderExp || renderExp.arguments.length > 1) return;
    const children = forNode.children || [];
    let childBlock;
    if (children.length === 1 && children[0].type === NodeTypes.ELEMENT) {
      childBlock = children[0].codegenNode;
      if (childBlock && childBlock.type === NodeTypes.VNODE_CALL && !childBlock.isBlock) {
        context.removeHelper(runtime.getVNodeHelper(context.inSSR, childBlock.isComponent));
        childBlock.isBlock = true;
        context.helper(runtime.OPEN_BLOCK);
        context.helper(runtime.getVNodeBlockHelper(context.inSSR, childBlock.isComponent));
      }
    } else {
      childBlock = runtime.createVNodeCall(context, context.helper(runtime.FRAGMENT), undefined, children, 64, undefined, undefined, true, undefined, false, forNode.loc);
    }
    renderExp.arguments.push(runtime.createFunctionExpression(runtime.createForLoopParams(forNode.parseResult), childBlock, true));
  };
  runtime.trackSlotScopes = function trackSlotScopes(node, context) {
    if (!node || node.type !== NodeTypes.ELEMENT || !(node.tagType === ElementTypes.COMPONENT || node.tagType === ElementTypes.TEMPLATE)) return;
    const projection = callBridge('vue3.core.trackSlotScopes', { node, context: vue3TransformSlotContextPayload(context) });
    if (!projection || !projection.track) return;
    const props = materializeVue3SlotProjectionNode(projection.slotProps, node, context);
    const locals = (projection.locals || []).filter(Boolean);
    if (context && context.prefixIdentifiers && props) context.addIdentifiers(props);
    if (context && context.prefixIdentifiers) locals.forEach(local => context.addIdentifiers(local));
    if (context && context.scopes) context.scopes.vSlot++;
    return () => {
      if (context && context.prefixIdentifiers && props) context.removeIdentifiers(props);
      if (context && context.prefixIdentifiers) locals.forEach(local => context.removeIdentifiers(local));
      if (context && context.scopes) context.scopes.vSlot--;
    };
  };
  runtime.trackVForSlotScopes = function trackVForSlotScopes(node, context) {
    if (!node || node.type !== NodeTypes.ELEMENT || node.tagType !== ElementTypes.TEMPLATE || !(node.props || []).some(runtime.isVSlot)) return;
    const projection = callBridge('vue3.core.trackVForSlotScopes', { node, context: vue3TransformSlotContextPayload(context) });
    if (!projection || !projection.track) return;
    const parseResult = materializeVue3ForParseResult(projection.parseResult, projection.dir || runtime.findDir(node, 'for', true), context);
    const locals = [parseResult.value, parseResult.key, parseResult.index].filter(Boolean);
    for (const local of locals) context.addIdentifiers(local);
    const dir = runtime.findDir(node, 'for', true);
    if (dir) dir.forParseResult = parseResult;
    return () => {
      for (const local of locals) context.removeIdentifiers(local);
    };
  };
  runtime.buildProps = function buildProps(node, context, props = node && node.props || []) {
    const objectProps = [];
    const mergeArgs = [];
    const directives = [];
    let hasDynamicKey = false;
    const pushMergeArg = arg => {
      if (objectProps.length) {
        mergeArgs.push(runtime.createObjectExpression(runtime.dedupeProperties(objectProps.splice(0)), node && node.loc || locStub));
      }
      if (arg) mergeArgs.push(arg);
    };
    for (const prop of props || []) {
      if (prop.type === NodeTypes.ATTRIBUTE) {
        if (
          prop.name === 'is'
          && (
            node && (node.tag === 'component' || node.tag === 'Component')
            || prop.value && String(prop.value.content || '').startsWith('vue:')
          )
        ) {
          continue;
        }
        objectProps.push(runtime.createObjectProperty(
          runtime.createSimpleExpression(prop.name, true, prop.nameLoc || prop.loc),
          runtime.createSimpleExpression(prop.value ? prop.value.content : '', true, prop.value ? prop.value.loc : prop.loc),
        ));
        continue;
      }
      if (prop.name === 'bind' && prop.arg) {
        if (runtime.isStaticArgOf(prop.arg, 'is') && node && (node.tag === 'component' || node.tag === 'Component')) {
          continue;
        }
        const transform = context && context.directiveTransforms && context.directiveTransforms.bind;
        const result = transform ? transform(prop, node, context) : runtime.transformBind(prop, node, context);
        objectProps.push(...((result && result.props) || []));
        if (result && result.props && result.props.some(p => p.key && !runtime.isStaticExp(p.key))) hasDynamicKey = true;
      } else if (prop.name === 'bind' && !prop.arg) {
        if (prop.exp) {
          pushMergeArg(prop.exp);
          hasDynamicKey = true;
        } else if (context && context.onError) {
          context.onError(runtime.createCompilerError(ErrorCodes.X_V_BIND_NO_EXPRESSION, prop.loc));
        }
      } else if (prop.name === 'on' && prop.arg) {
        const transform = context && context.directiveTransforms && context.directiveTransforms.on;
        const result = transform ? transform(prop, node, context) : runtime.transformOn(prop, node, context);
        objectProps.push(...((result && result.props) || []));
      } else if (prop.name === 'on' && !prop.arg && context && context.inSSR) {
        continue;
      } else if (prop.name === 'model' && context && context.directiveTransforms && context.directiveTransforms.model) {
        const result = context.directiveTransforms.model(prop, node, context);
        const modelProps = (result && result.props) || [];
        objectProps.push(...modelProps);
        if (modelProps.some(p => p.key && !runtime.isStaticExp(p.key))) hasDynamicKey = true;
        for (const modelProp of modelProps) {
          if (modelProp.__vuecModel && modelProp.__vuecModel.dynamic && runtime.isStaticExp(modelProp.key)) {
            dynamicPropNames.push(modelProp.key.content);
          }
        }
      } else if (context && context.directiveTransforms && context.directiveTransforms[prop.name]) {
        const result = context.directiveTransforms[prop.name](prop, node, context);
        objectProps.push(...((result && result.props) || []));
        if (result && result.props && result.props.some(p => p.key && !runtime.isStaticExp(p.key))) hasDynamicKey = true;
      } else if (!runtime.isBuiltInDirective(prop.name)) {
        directives.push(prop);
      }
    }
    let propsExpression;
    if (mergeArgs.length) {
      pushMergeArg();
      propsExpression = mergeArgs.length > 1
        ? runtime.createCallExpression(context && context.helper ? context.helper(runtime.MERGE_PROPS) : runtime.MERGE_PROPS, mergeArgs, node && node.loc || locStub)
        : mergeArgs[0];
    } else if (objectProps.length) {
      propsExpression = runtime.createObjectExpression(runtime.dedupeProperties(objectProps), node && node.loc || locStub);
    }
    if (propsExpression && hasDynamicKey && context && !context.inSSR) {
      propsExpression = runtime.createCallExpression(context.helper(runtime.NORMALIZE_PROPS), [propsExpression], node && node.loc || locStub);
    }
    return {
      props: propsExpression,
      directives,
      patchFlag: 0,
      dynamicPropNames: [],
      shouldUseBlock: false,
    };
  };
  runtime.buildDirectiveArgs = function buildDirectiveArgs(dir, context) {
    const projection = callBridge('vue3.core.buildDirectiveArgs', {
      dir,
      needRuntime: vue3DirectiveRuntimePayload(dir && dir.__vuecNeedRuntime),
    });
    const elements = materializeVue3DirectiveArgsProjection(projection, dir, context);
    return runtime.createArrayExpression(elements);
  };
  runtime.buildSlots = function buildSlots(node, context, buildSlotFn) {
    const projection = callBridge('vue3.core.buildSlots', {
      node,
      context: vue3TransformSlotContextPayload(context),
    });
    materializeVue3SlotErrors(projection, node, context);
    const slots = materializeVue3SlotsProjection(projection, node, context, buildSlotFn);
    return {
      slots,
      hasDynamicSlots: !!(projection && projection.hasDynamicSlots),
    };
  };
  runtime.resolveComponentType = function resolveComponentType(node, context, ssr = false) {
    const projection = callBridge('vue3.core.resolveComponentType', {
      node,
      context: vue3ResolveComponentContextPayload(context),
      ssr: !!ssr,
    });
    return materializeVue3ComponentTypeProjection(projection, node, context);
  };
  runtime.getBaseTransformPreset = function getBaseTransformPreset(prefixIdentifiers = false) {
    return [[
      runtime.transformOnce,
      runtime.transformIf,
      runtime.transformMemo,
      runtime.transformFor,
      ...(prefixIdentifiers ? [runtime.trackVForSlotScopes] : []),
      runtime.transformExpression,
      runtime.transformSlotOutlet,
      runtime.transformElement,
      runtime.trackSlotScopes,
      runtime.transformText,
    ], { on: runtime.transformOn, bind: runtime.transformBind, model: runtime.transformModel }];
  };
  runtime.getConstantType = function getConstantType(node) {
    if (!node) return ConstantTypes.NOT_CONSTANT;
    const projection = callBridge('vue3.core.getConstantType', {
      node: runtime.dehydrateForBridge(node),
      context: vue3CacheStaticContextPayload(arguments[1]),
    });
    return projection && projection.constantType || ConstantTypes.NOT_CONSTANT;
  };
  runtime.cacheStatic = function cacheStatic(root, context) {
    const projection = callBridge('vue3.core.cacheStatic', {
      root: runtime.dehydrateForBridge(root),
      context: vue3CacheStaticContextPayload(context),
    });
    for (const operation of projection && projection.operations || []) {
      materializeVue3CacheStaticOperation(operation, root, context);
    }
    if (context && typeof context.transformHoist === 'function') {
      vue3ApplyTransformHoist(root, context);
    }
  };
  runtime.stringifyStatic = function stringifyStatic(children, context, parent) {
    if (!Array.isArray(children)) return;
    const projection = callBridge('vue3.core.stringifyStatic', {
      children: runtime.dehydrateForBridge(children),
      parent: runtime.dehydrateForBridge(parent),
      context: vue3StringifyStaticContextPayload(context),
    });
    materializeVue3StringifyStaticProjection(projection, children, context, runtime);
  };
  runtime.transformOnce = function transformOnce(node, context) {
    const projection = callBridge('vue3.core.transformOnce', {
      node: runtime.dehydrateForBridge(node),
      context: vue3TransformOnceContextPayload(context),
      seen: !!(node && node.__vuecOnceSeen),
    });
    if (!projection || projection.kind !== 'enter') return;
    if (projection.markSeen) {
      Object.defineProperty(node, '__vuecOnceSeen', { value: true, configurable: true });
    }
    if (projection.enterInVOnce) context.inVOnce = true;
    if (projection.helper === 'SET_BLOCK_TRACKING') context.helper(runtime.SET_BLOCK_TRACKING);
    return () => {
      if (projection.exit && Object.prototype.hasOwnProperty.call(projection.exit, 'restoreInVOnce')) {
        context.inVOnce = !!projection.exit.restoreInVOnce;
      }
      const current = context.currentNode || node;
      if (projection.exit && projection.exit.cacheCodegen && current && current.codegenNode) {
        current.codegenNode = context.cache(
          current.codegenNode,
          projection.exit.isVNode !== false,
          projection.exit.inVOnce !== false,
        );
      }
    };
  };
  runtime.transformMemo = function transformMemo(node, context) {
    const projection = callBridge('vue3.core.transformMemo', {
      node: runtime.dehydrateForBridge(node),
      context: vue3TransformMemoContextPayload(context),
      seen: !!(node && node.__vuecMemoSeen),
    });
    if (!projection || projection.kind !== 'enter') return;
    if (projection.markSeen) {
      Object.defineProperty(node, '__vuecMemoSeen', { value: true, configurable: true });
    }
    return () => {
      const exit = projection.exit || {};
      if (!exit.wrapMemo) return;
      const current = context.currentNode || node;
      const codegenNode = node.codegenNode || current && current.codegenNode;
      if (!codegenNode || codegenNode.type !== NodeTypes.VNODE_CALL) return;
      if (exit.convertToBlock) runtime.convertToBlock(codegenNode, context);
      node.codegenNode = runtime.createCallExpression(context.helper(runtime.WITH_MEMO), [
        exit.exp,
        runtime.createFunctionExpression(undefined, codegenNode),
        '_cache',
        String(exit.cacheIndex || context.cached.length),
      ]);
      context.cached.push(null);
    };
  };
  runtime.checkCompatEnabled = () => false;
  runtime.warnDeprecation = () => {};
  const DOMErrorMessages = {
    54: 'v-html is missing expression.',
    55: 'v-html will override element children.',
    56: 'v-text is missing expression.',
    57: 'v-text will override element children.',
    58: 'v-model can only be used on <input>, <textarea> and <select> elements.',
    59: 'v-model argument is not supported on plain elements.',
    60: 'v-model cannot be used on file inputs since they are read-only. Use a v-on:change listener instead.',
    61: "Unnecessary value binding used alongside v-model. It will interfere with v-model's behavior.",
    62: 'v-show is missing expression.',
    63: '<Transition> expects exactly one child element or component.',
    64: 'Tags with side effect (<script> and <style>) are ignored in client component templates.',
  };
  runtime.createDOMCompilerError = function createDOMCompilerError(code, loc) {
    return runtime.createCompilerError(code, loc, DOMErrorMessages);
  };
  function vue3DomDirectiveLoc(loc, dir, node) {
    if (loc && typeof loc === 'object') return loc;
    if (loc === 'dir') return dir && dir.loc || locStub;
    if (loc === 'node') return node && node.loc || dir && dir.loc || locStub;
    return locStub;
  }
  function materializeVue3DomDirectiveValue(projection, dir, node, context) {
    if (!projection || projection.kind === 'undefined') return undefined;
    if (projection.type) return projection;
    switch (projection.kind) {
      case 'node':
        if (projection.path === 'dir.exp') return dir && dir.exp;
        if (projection.path === 'dir.arg') return dir && dir.arg;
        return undefined;
      case 'simple': {
        const loc = Object.prototype.hasOwnProperty.call(projection, 'loc')
          ? vue3DomDirectiveLoc(projection.loc, dir, node)
          : locStub;
        const simple = runtime.createSimpleExpression(projection.content || '', !!projection.isStatic, loc);
        if (projection.constType !== undefined) simple.constType = projection.constType;
        return simple;
      }
      case 'displayString': {
        const argument = materializeVue3DomDirectiveValue(projection.argument, dir, node, context);
        const helper = runtime.TO_DISPLAY_STRING;
        const callee = context && typeof context.helperString === 'function'
          ? context.helperString(helper)
          : `_${runtime.helperNameMap[helper] || 'toDisplayString'}`;
        return runtime.createCallExpression(
          callee,
          [argument],
          vue3DomDirectiveLoc(projection.loc, dir, node),
        );
      }
      default:
        throw new Error(`Unsupported Rust Vue 3 DOM directive projection: ${projection.kind}`);
    }
  }
  function materializeVue3DomDirectiveErrors(projection, dir, node, context) {
    if (!projection || !Array.isArray(projection.errors) || !context || typeof context.onError !== 'function') return;
    for (const error of projection.errors) {
      context.onError(runtime.createDOMCompilerError(error.code, vue3DomDirectiveLoc(error.loc, dir, node)));
    }
  }
  function materializeVue3DomModelErrors(projection, dir, node, context) {
    if (!projection || !Array.isArray(projection.errors) || !context || typeof context.onError !== 'function') return;
    for (const error of projection.errors) {
      const code = typeof error === 'number' ? error : error.code;
      const loc = vue3DomDirectiveLoc(error && error.loc, dir, node);
      context.onError(
        code >= 54
          ? runtime.createDOMCompilerError(code, loc)
          : runtime.createCompilerError(code, loc),
      );
    }
  }
  function materializeVue3DomContentDirective(command, dir, node, context) {
    context = context || {
      helperString: name => `_${runtime.helperNameMap[name] || name}`,
      onError: error => { throw error; },
    };
    const projection = callBridge(command, {
      dir: runtime.dehydrateForBridge(dir),
      node: runtime.dehydrateForBridge(node),
    });
    materializeVue3DomDirectiveErrors(projection, dir, node, context);
    if (projection && projection.clearChildren && node && Array.isArray(node.children)) {
      node.children.length = 0;
    }
    return {
      props: (projection && projection.props || []).map(prop => {
        const key = prop.keyLoc
          ? runtime.createSimpleExpression(prop.key || '', true, vue3DomDirectiveLoc(prop.keyLoc, dir, node))
          : (prop.key || '');
        return runtime.createObjectProperty(
          key,
          materializeVue3DomDirectiveValue(prop.value, dir, node, context),
        );
      }),
    };
  }
  runtime.transformVHtml = function transformVHtml(dir, node, context) {
    return materializeVue3DomContentDirective('vue3.dom.transformVHtml', dir, node, context);
  };
  runtime.transformVText = function transformVText(dir, node, context) {
    return materializeVue3DomContentDirective('vue3.dom.transformVText', dir, node, context);
  };
  runtime.transformShow = function transformShow(dir, node, context) {
    context = context || {
      onError: error => { throw error; },
    };
    const projection = callBridge('vue3.dom.transformShow', {
      dir: runtime.dehydrateForBridge(dir),
    });
    materializeVue3DomDirectiveErrors(projection, dir, node, context);
    return {
      props: [],
      needRuntime: projection && projection.needRuntime,
    };
  };
  runtime.transformDomModel = function transformDomModel(dir, node, context) {
    context = context || { helper: name => name, cache: value => value, onError: error => { throw error; } };
    const projection = callBridge('vue3.dom.transformModel', {
      dir,
      node,
      context: vue3TransformDomModelContextPayload(context, node),
    });
    materializeVue3DomModelErrors(projection, dir, node, context);
    const result = {
      props: (projection.props || []).map(prop => {
        const key = materializeVue3ModelProjection(prop.key, dir, context);
        const value = materializeVue3ModelProjection(prop.value, dir, context);
        const objectProp = runtime.createObjectProperty(key, value);
        objectProp.__vuecModel = {
          dynamic: !!prop.dynamic,
          cache: !!prop.cache,
          hydrate: !!prop.hydrate,
          kind: prop.kind,
        };
        if (prop.cache && context && context.cache) objectProp.value = context.cache(objectProp.value);
        return objectProp;
      }),
    };
    if (projection && projection.needRuntime) {
      const helper = helperSymbolFromProjection(projection.needRuntime, context);
      result.needRuntime = helper && context && context.helper
        ? context.helper(helper)
        : helper || projection.needRuntime;
    }
    return result;
  };
  function vue3DomNodeIsTransition(node, context) {
    if (!node || node.type !== NodeTypes.ELEMENT || node.tagType !== ElementTypes.COMPONENT) return false;
    if (!context || typeof context.isBuiltInComponent !== 'function') return false;
    let component;
    try {
      component = context.isBuiltInComponent(node.tag);
    } catch (_error) {
      component = undefined;
    }
    const transition = context.__vuecDomHelpers && context.__vuecDomHelpers.TRANSITION || vue3CoreRuntime.TRANSITION;
    return component === transition || vue3CoreRuntime.helperNameMap[component] === 'Transition';
  }
  function vue3DomTransitionContextPayload(context, node) {
    return {
      isTransition: vue3DomNodeIsTransition(node, context),
    };
  }
  function materializeVue3DomTransitionProjection(projection, node, context) {
    if (!projection || !projection.transform || !node) return;
    if (Array.isArray(projection.keepChildren) && Array.isArray(node.children)) {
      node.children = projection.keepChildren
        .map(index => node.children[index])
        .filter(Boolean);
    }
    materializeVue3DomDirectiveErrors(projection, null, node, context);
    if (projection.injectPersisted) {
      node.props = node.props || [];
      node.props.push({
        type: NodeTypes.ATTRIBUTE,
        name: 'persisted',
        nameLoc: node.loc,
        value: undefined,
        loc: node.loc,
      });
    }
  }
  runtime.ignoreSideEffectTags = function ignoreSideEffectTags(node, context) {
    const projection = callBridge('vue3.dom.ignoreSideEffectTags', { node });
    materializeVue3DomDirectiveErrors(projection, null, node, context);
    if (projection && projection.remove && context && typeof context.removeNode === 'function') {
      context.removeNode();
    }
  };
  runtime.transformDomTransition = function transformDomTransition(node, context) {
    if (!vue3DomNodeIsTransition(node, context)) return;
    return () => {
      const projection = callBridge('vue3.dom.transformTransition', {
        node,
        context: vue3DomTransitionContextPayload(context, node),
      });
      materializeVue3DomTransitionProjection(projection, node, context);
    };
  };
  runtime.isValidHTMLNesting = function isValidHTMLNesting(parent, child) {
    const projection = callBridge('vue3.dom.isValidHTMLNesting', {
      parent: String(parent || ''),
      child: String(child || ''),
    });
    return !!(projection && projection.valid);
  };
  function materializeVue3DomNestingWarnings(projection, context) {
    if (!projection || !Array.isArray(projection.warnings) || !context || typeof context.onWarn !== 'function') return;
    for (const warning of projection.warnings) {
      const error = new SyntaxError(String(warning.message || ''));
      error.loc = warning.loc || vue3CoreRuntime.locStub;
      context.onWarn(error);
    }
  }
  runtime.validateHtmlNesting = function validateHtmlNesting(node, context) {
    const projection = callBridge('vue3.dom.validateHtmlNesting', {
      node,
      parent: context && context.parent,
    });
    materializeVue3DomNestingWarnings(projection, context);
  };
  runtime.transformDomOn = function transformDomOn(dir, node, context) {
    context = context || { helperString: name => `_${runtime.helperNameMap[name] || name}`, helper: name => name, cache: value => value, onError: error => { throw error; } };
    const projection = callBridge('vue3.dom.transformOn', {
      dir,
      node,
      context: vue3TransformOnContextPayload(context),
    });
    materializeVue3OnErrors(projection, dir, context);
    const onMeta = (projection.props || []).map(prop => ({
      cache: !!prop.cache,
      valueConstant: !!prop.valueConstant,
      handlerKey: !!prop.handlerKey,
      dynamicKey: !!prop.dynamicKey,
      ignoreDynamicKeyForNormalize: !!prop.ignoreDynamicKeyForNormalize,
    }));
    const result = {
      props: (projection.props || []).map(prop => {
        const key = materializeVue3OnProjection(prop.key, dir, context);
        const value = materializeVue3OnProjection(prop.value, dir, context) || runtime.createSimpleExpression('() => {}', false, dir.loc);
        return runtime.createObjectProperty(key, value);
      }),
    };
    for (const [index, prop] of (result.props || []).entries()) {
      const meta = onMeta[index] || onMeta[0] || {};
      if (prop.key && meta.handlerKey) prop.key.isHandlerKey = true;
      if (meta.cache && context && context.cache) prop.value = context.cache(prop.value);
      prop.__vuecOn = meta;
    }
    return result;
  };
  runtime.transformStyle = function transformStyle(node) {
    if (!node || node.type !== NodeTypes.ELEMENT) return;
    const projection = callBridge('vue3.dom.transformStyle', { node });
    for (const replacement of projection && projection.replacements || []) {
      const original = node.props && node.props[replacement.index];
      if (!original || original.type !== NodeTypes.ATTRIBUTE) continue;
      node.props[replacement.index] = {
        type: NodeTypes.DIRECTIVE,
        name: 'bind',
        rawName: ':style',
        arg: runtime.createSimpleExpression('style', true, original.loc),
        exp: runtime.createSimpleExpression(replacement.expression || '{}', false, original.loc, ConstantTypes.CAN_STRINGIFY),
        modifiers: [],
        loc: original.loc,
      };
    }
  };
  runtime.decodeHtmlBrowser = function decodeHtmlBrowser(raw, asAttr = false) {
    const source = String(raw == null ? '' : raw);
    const projection = callBridge('vue3.dom.decodeHtmlBrowser', {
      raw: source,
      asAttr: !!asAttr,
    });
    return projection && typeof projection.decoded === 'string'
      ? projection.decoded
      : source;
  };
  return runtime;
})();

function capitalize(value) {
  value = String(value || '');
  return value ? value.charAt(0).toUpperCase() + value.slice(1) : value;
}

function camelize(value) {
  return String(value || '').replace(/-(\w)/g, (_, c) => c ? c.toUpperCase() : '');
}

function toHandlerKey(value) {
  value = String(value || '');
  return value ? `on${capitalize(value)}` : '';
}

function stringifyDynamicPropNames(props) {
  return `[${(props || []).map(prop => JSON.stringify(prop)).join(', ')}]`;
}

function selfNameFromFilename(filename) {
  const match = String(filename).replace(/\?.*$/, '').match(/([^/\\]+)\.\w+$/);
  if (!match) return null;
  return match[1].replace(/(^|[-_])(\w)/g, (_, _sep, ch) => ch.toUpperCase());
}

function vue3TransformModelContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    cacheHandlers: !!context.cacheHandlers,
    inVOnce: !!context.inVOnce,
    inline: !!context.inline,
    isTS: !!context.isTS,
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
  };
}

function vue3TransformDomModelContextPayload(context, node) {
  context = context || {};
  let isCustomElement = false;
  if (typeof context.isCustomElement === 'function' && node) {
    try {
      isCustomElement = !!context.isCustomElement(node.tag);
    } catch (_error) {
      isCustomElement = false;
    }
  }
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    cacheHandlers: !!context.cacheHandlers,
    inVOnce: !!context.inVOnce,
    inline: !!context.inline,
    isTS: !!context.isTS,
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
    isCustomElement,
  };
}

function vue3TransformOnContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    cacheHandlers: !!context.cacheHandlers,
    inVOnce: !!context.inVOnce,
    inline: !!context.inline,
    isTS: !!context.isTS,
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
  };
}

function vue3TransformBindContextPayload(context) {
  context = context || {};
  return {
    inSSR: !!context.inSSR,
    browser: vue3CoreRuntime.isBrowserBuild ? vue3CoreRuntime.isBrowserBuild() : false,
  };
}

function vue3TransformVBindShorthandContextPayload(_context) {
  return {
    browser: vue3CoreRuntime.isBrowserBuild ? vue3CoreRuntime.isBrowserBuild() : false,
  };
}

function vue3TransformOnceContextPayload(context) {
  context = context || {};
  return {
    inVOnce: !!context.inVOnce,
    inSSR: !!context.inSSR,
  };
}

function vue3TransformMemoContextPayload(context) {
  context = context || {};
  return {
    inSSR: !!context.inSSR,
    cachedLength: Array.isArray(context.cached) ? context.cached.length : 0,
  };
}

function materializeVue3OnErrors(projection, dir, context) {
  if (!projection || !Array.isArray(projection.errors) || !context || !context.onError) return;
  for (const error of projection.errors) {
    const loc = error.loc === 'arg'
      ? dir && dir.arg && dir.arg.loc || dir && dir.loc
      : dir && dir.loc || locStub;
    context.onError(vue3CoreRuntime.createCompilerError(error.code, loc));
  }
}

function materializeVue3BindErrors(projection, dir, context) {
  if (!projection || !Array.isArray(projection.errors) || !context || !context.onError) return;
  for (const error of projection.errors) {
    const code = typeof error === 'number' ? error : error.code;
    const loc = error && error.loc === 'arg'
      ? dir && dir.arg && dir.arg.loc || dir && dir.loc || locStub
      : dir && dir.loc || locStub;
    context.onError(vue3CoreRuntime.createCompilerError(code, loc));
  }
}

function materializeVue3VBindShorthandProjection(projection, node, context) {
  for (const operation of projection && projection.operations || []) {
    const prop = node && node.props && node.props[operation.index];
    if (!prop || operation.kind !== 'setExp') continue;
    for (const error of operation.errors || []) {
      if (context && context.onError) {
        const loc = error.loc === 'arg'
          ? prop.arg && prop.arg.loc || prop.loc || vue3CoreRuntime.locStub
          : prop.loc || vue3CoreRuntime.locStub;
        context.onError(vue3CoreRuntime.createCompilerError(error.code, loc));
      }
    }
    prop.exp = materializeVue3VBindShorthandExpression(operation.exp, prop);
  }
}

function materializeVue3VBindShorthandExpression(projection, prop) {
  if (!projection || projection.kind !== 'simple') return undefined;
  return vue3CoreRuntime.createSimpleExpression(
    projection.content || '',
    !!projection.isStatic,
    projection.loc || prop && prop.arg && prop.arg.loc || prop && prop.loc || vue3CoreRuntime.locStub,
    projection.constType || 0,
  );
}

function materializeVue3OnProjection(projection, dir, context) {
  if (!projection || projection.kind === 'undefined') return undefined;
  if (typeof projection === 'string') return projection;
  if (projection.type) return projection;
  switch (projection.kind) {
    case 'node':
      if (projection.path === 'dir.arg') return dir && dir.arg;
      if (projection.path === 'dir.exp') return dir && dir.exp;
      if (projection.path === 'dir.arg.children') return (dir && dir.arg && dir.arg.children) || [];
      return undefined;
    case 'children': {
      const children = [];
      for (const child of projection.children || []) {
        const materialized = materializeVue3OnProjection(child, dir, context);
        if (Array.isArray(materialized)) children.push(...materialized);
        else children.push(materialized);
      }
      return children;
    }
    case 'helperString': {
      const helper = helperSymbolFromProjection(projection.helper, context);
      return `${context && helper ? context.helperString(helper) : `_${vue3CoreRuntime.helperNameMap[helper]}`}(`;
    }
    case 'static':
      return vue3CoreRuntime.createSimpleExpression(
        projection.content || '',
        true,
        projection.loc || (dir && dir.loc) || locStub,
      );
    case 'simple':
      for (const name of projection.helpers || []) {
        const symbol = helperSymbolFromProjection(name);
        if (symbol && context) context.helper(symbol);
      }
      return vue3CoreRuntime.createSimpleExpression(
        projection.content || '',
        !!projection.isStatic,
        projection.loc || (dir && dir.exp && dir.exp.loc) || (dir && dir.loc) || locStub,
        projection.constType || 0,
      );
    case 'compound': {
      for (const name of projection.helpers || []) {
        const symbol = helperSymbolFromProjection(name);
        if (symbol && context) context.helper(symbol);
      }
      const children = [];
      for (const child of projection.children || []) {
        const materialized = materializeVue3OnProjection(child, dir, context);
        if (Array.isArray(materialized)) children.push(...materialized);
        else children.push(materialized);
      }
      return vue3CoreRuntime.createCompoundExpression(
        children,
        projection.loc || (dir && dir.arg && dir.arg.loc) || (dir && dir.exp && dir.exp.loc) || locStub,
      );
    }
    case 'call': {
      const helper = helperSymbolFromProjection(projection.callee || projection.helper, context);
      if (helper && context) context.helper(helper);
      const args = (projection.arguments || []).map(arg => materializeVue3OnProjection(arg, dir, context));
      return vue3CoreRuntime.createCallExpression(
        helper || projection.callee || projection.helper,
        args,
        projection.loc || (dir && dir.loc) || locStub,
      );
    }
    default:
      throw new Error(`Unsupported Rust v-on projection: ${projection.kind}`);
  }
}

function materializeVue3ModelProjection(projection, dir, context) {
  if (!projection || projection.kind === 'undefined') return undefined;
  if (typeof projection === 'string') return projection;
  if (projection.type) return projection;
  switch (projection.kind) {
    case 'node':
      return projection.path === 'dir.arg' ? dir.arg : dir.exp;
    case 'static':
      return vue3CoreRuntime.createSimpleExpression(projection.content || '', true);
    case 'simple':
      return vue3CoreRuntime.createSimpleExpression(
        projection.content || '',
        !!projection.isStatic,
        projection.loc || (projection.path === 'dir.arg' ? dir.arg && dir.arg.loc : dir.exp && dir.exp.loc),
        projection.constType || 0,
      );
    case 'compound': {
      if (context && Array.isArray(projection.helpers)) {
        for (const helper of projection.helpers) {
          if (helper === 'IS_REF') context.helper(vue3CoreRuntime.IS_REF);
        }
      }
      return vue3CoreRuntime.createCompoundExpression(
        (projection.children || []).map(child => materializeVue3ModelProjection(child, dir, context)),
      );
    }
    default:
      throw new Error(`Unsupported Rust v-model projection: ${projection.kind}`);
  }
}

function vue3TransformIfContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    inline: !!context.inline,
    isTS: !!context.isTS,
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
  };
}

function vue3TransformForContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    inline: !!context.inline,
    isTS: !!context.isTS,
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
  };
}

function vue3TransformElementContextPayload(context) {
  context = context || {};
  return {
    inSSR: !!context.inSSR,
    inline: !!context.inline,
    bindingMetadata: context.bindingMetadata || {},
    vForDepth: context.scopes && context.scopes.vFor || 0,
  };
}

function vue3TransformSlotContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    inline: !!context.inline,
    isTS: !!context.isTS,
    ssr: !!(context.ssr || context.inSSR),
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
    vForDepth: context.scopes && context.scopes.vFor || 0,
    vSlotDepth: context.scopes && context.scopes.vSlot || 0,
  };
}

function vue3TransformSlotOutletContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    inline: !!context.inline,
    isTS: !!context.isTS,
    scopeId: context.scopeId || undefined,
    slotted: !!context.slotted,
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
  };
}

function vue3TransformTextContextPayload(context) {
  context = context || {};
  return {
    compat: typeof __COMPAT__ !== 'undefined' && !!__COMPAT__,
    ssr: !!context.ssr,
    inSSR: !!context.inSSR,
    directiveTransforms: Object.keys(context.directiveTransforms || {}),
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
  };
}

function vue3CacheStaticContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    hmr: !!context.hmr,
    inSSR: !!context.inSSR,
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
  };
}

function vue3StringifyStaticContextPayload(context) {
  context = context || {};
  return {
    scopeId: context.scopeId || undefined,
    scopes: {
      vSlot: Number(context.scopes && context.scopes.vSlot || 0),
    },
  };
}

function vue3ProcessExpressionContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    inline: !!context.inline,
    isTS: !!context.isTS,
    expressionPlugins: context.expressionPlugins || [],
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
  };
}

function vue3ExpressionUtilityContextPayload(context) {
  context = context || {};
  return {
    expressionPlugins: context.expressionPlugins || [],
    isTS: !!context.isTS,
    allowLexerFallback: false,
  };
}

function materializeVue3ProcessExpressionProjection(projection, node, context) {
  if (!projection || projection.kind === 'unchanged') return node;
  if (projection.kind === 'error') {
    if (context && context.onError) {
      context.onError(vue3CoreRuntime.createCompilerError(
        projection.code || vue3CoreRuntime.ErrorCodes.X_INVALID_EXPRESSION,
        projection.loc || node.loc,
        undefined,
        projection.message || 'Error parsing JavaScript expression',
      ));
    }
    return node;
  }
  if (projection.kind === 'setConstType') {
    node.constType = Number(projection.constType || 0);
    return node;
  }
  if (Array.isArray(projection.helpers) && context && context.helper) {
    for (const helper of projection.helpers) {
      if (helper === 'UNREF') context.helper(vue3CoreRuntime.UNREF);
      else if (helper === 'IS_REF') context.helper(vue3CoreRuntime.IS_REF);
    }
  }
  if (projection.kind === 'simple') {
    node.content = projection.content || '';
    node.isStatic = !!projection.isStatic;
    node.constType = Number(projection.constType || 0);
    if (projection.loc) node.loc = projection.loc;
    return node;
  }
  if (projection.kind === 'compound') {
    const compound = vue3CoreRuntime.createCompoundExpression(
      (projection.children || []).map(child => materializeVue3ProcessExpressionChild(child, context)),
      projection.loc || node.loc,
    );
    compound.identifiers = projection.identifiers || [];
    return compound;
  }
  throw new Error(`Unsupported Rust processExpression projection: ${projection.kind}`);
}

function materializeVue3ProcessExpressionChild(child, context) {
  if (typeof child === 'string') return child;
  if (!child || typeof child !== 'object') return child;
  if (child.kind === 'simple') {
    return vue3CoreRuntime.createSimpleExpression(
      child.content || '',
      !!child.isStatic,
      child.loc || vue3CoreRuntime.locStub,
      Number(child.constType || 0),
    );
  }
  if (child.kind === 'compound') {
    return materializeVue3ProcessExpressionProjection(
      child,
      vue3CoreRuntime.createSimpleExpression('', false),
      context,
    );
  }
  return child;
}

function materializeVue3TransformExpressionProjection(projection, node, context) {
  if (!projection || !Array.isArray(projection.operations)) return;
  for (const operation of projection.operations) {
    if (!operation || operation.kind !== 'process') continue;
    const holder = vue3HolderAtPath(node, operation.path);
    if (!holder || !holder.owner) continue;
    const current = holder.owner[holder.key];
    holder.owner[holder.key] = materializeVue3ProcessExpressionProjection(
      operation.projection,
      current,
      context,
    );
  }
}

function materializeVue3TransformTextProjection(projection, node, context) {
  if (!projection || !Array.isArray(projection.operations) || !node || !Array.isArray(node.children)) return;
  const children = node.children;
  for (const operation of projection.operations) {
    if (!operation || !operation.kind) continue;
    if (operation.kind === 'mergeText') {
      const start = Number(operation.start || 0);
      const end = Number(operation.end || start);
      if (start < 0 || end < start || end >= children.length) continue;
      const mergedChildren = [];
      for (let i = start; i <= end; i++) {
        if (i > start) mergedChildren.push(' + ');
        mergedChildren.push(children[i]);
      }
      children.splice(start, end - start + 1, vue3CoreRuntime.createCompoundExpression(mergedChildren, children[start] && children[start].loc || vue3CoreRuntime.locStub));
    } else if (operation.kind === 'wrapTextCall') {
      const index = Number(operation.index || 0);
      const child = children[index];
      if (!child) continue;
      const callArgs = [];
      if (operation.includeContent !== false) callArgs.push(child);
      if (operation.patchFlag) callArgs.push(operation.patchFlag);
      children[index] = {
        type: vue3CoreRuntime.NodeTypes.TEXT_CALL,
        content: child,
        loc: child.loc,
        codegenNode: vue3CoreRuntime.createCallExpression(
          context.helper(vue3CoreRuntime.CREATE_TEXT),
          callArgs,
        ),
      };
    } else {
      throw new Error(`Unsupported Rust transformText projection: ${operation.kind}`);
    }
  }
}

function materializeVue3CacheStaticOperation(operation, root, context) {
  if (!operation || !operation.kind) return;
  switch (operation.kind) {
    case 'setPatchFlag': {
      const target = vue3NodeAtPath(root, operation.path);
      if (target) target.patchFlag = operation.patchFlag;
      return;
    }
    case 'appendTextCallPatchFlag': {
      const target = vue3NodeAtPath(root, operation.path);
      if (target && target.type === vue3CoreRuntime.NodeTypes.JS_CALL_EXPRESSION && target.arguments && target.arguments.length > 0 && target.arguments.length < 2) {
        target.arguments.push(operation.patchFlag || '-1 /* CACHED */');
      }
      return;
    }
    case 'setBlock': {
      const target = vue3NodeAtPath(root, operation.path);
      if (target && target.type === vue3CoreRuntime.NodeTypes.VNODE_CALL && target.isBlock !== !!operation.isBlock) {
        vue3SetVNodeBlock(target, !!operation.isBlock, context);
      }
      return;
    }
    case 'cacheCodegen': {
      const holder = vue3HolderAtPath(root, operation.path);
      if (holder && holder.owner) holder.owner[holder.key] = context.cache(holder.owner[holder.key]);
      return;
    }
    case 'cacheChildrenArray': {
      const holder = vue3HolderAtPath(root, operation.path);
      const children = vue3NodeAtPath(root, operation.childrenPath);
      if (holder && holder.owner && Array.isArray(children)) {
        const cacheExp = context.cache(vue3CoreRuntime.createArrayExpression(children));
        cacheExp.needArraySpread = operation.needArraySpread !== false;
        holder.owner[holder.key] = cacheExp;
      }
      return;
    }
    case 'cacheSlotReturns': {
      const owner = vue3NodeAtPath(root, operation.ownerPath);
      const slot = vue3FindSlotFunction(owner && owner.codegenNode, operation.slot);
      if (slot && Array.isArray(slot.returns)) {
        const cacheExp = context.cache(vue3CoreRuntime.createArrayExpression(slot.returns));
        cacheExp.needArraySpread = operation.needArraySpread !== false;
        slot.returns = cacheExp;
      }
      return;
    }
    case 'hoistProps':
    case 'hoistDynamicProps': {
      const holder = vue3HolderAtPath(root, operation.path);
      if (holder && holder.owner && holder.owner[holder.key]) holder.owner[holder.key] = context.hoist(holder.owner[holder.key]);
      return;
    }
    default:
      throw new Error(`Unsupported Rust cacheStatic projection: ${operation.kind}`);
  }
}

function materializeVue3StringifyStaticProjection(projection, children, context, runtime = vue3CoreRuntime) {
  if (!projection || !Array.isArray(projection.operations) || !Array.isArray(children)) return;
  context = context || {};
  for (const operation of projection.operations) {
    if (!operation || !operation.kind) continue;
    const call = runtime.createCallExpression(
      context && typeof context.helper === 'function'
        ? context.helper(runtime.CREATE_STATIC)
        : runtime.CREATE_STATIC,
      [operation.html || '""', String(operation.domNodes || operation.count || 0)],
    );
    const start = Number(operation.start) || 0;
    const count = Math.max(1, Number(operation.count) || 1);
    if (operation.kind === 'stringifyParentCachedRange') {
      children.splice(start, count, call);
      continue;
    }
    if (operation.kind === 'stringifyCachedChildRange') {
      const first = children[start];
      const last = children[start + count - 1];
      const lastCache = last && last.codegenNode;
      if (first && first.codegenNode) first.codegenNode.value = call;
      if (count > 1) {
        children.splice(start + 1, count - 1);
        const cacheIndex = context.cached && context.cached.indexOf(lastCache);
        if (cacheIndex > -1) {
          for (let index = cacheIndex; index < context.cached.length; index++) {
            const cache = context.cached[index];
            if (cache) cache.index -= count - 1;
          }
          context.cached.splice(cacheIndex - count + 2, count - 1);
        }
      }
      continue;
    }
    throw new Error(`Unsupported Rust stringifyStatic projection: ${operation.kind}`);
  }
}

function vue3ApplyTransformHoist(root, context) {
  vue3ApplyTransformHoistToNode(root, context);
}

function vue3ApplyTransformHoistToNode(node, context) {
  if (!node || !Array.isArray(node.children)) return;
  if (vue3NodeHasCachedChildrenArray(node) || vue3ChildrenHaveCachedNodes(node.children)) {
    context.transformHoist(node.children, context, node);
  }
  for (const child of node.children.slice()) {
    if (child && child.type === vue3CoreRuntime.NodeTypes.ELEMENT && child.tagType === vue3CoreRuntime.ElementTypes.COMPONENT) {
      context.scopes.vSlot++;
      vue3ApplyTransformHoistToNode(child, context);
      context.scopes.vSlot--;
    } else if (child && child.type === vue3CoreRuntime.NodeTypes.IF) {
      for (const branch of child.branches || []) vue3ApplyTransformHoistToNode(branch, context);
    } else {
      vue3ApplyTransformHoistToNode(child, context);
    }
  }
}

function vue3NodeHasCachedChildrenArray(node) {
  return !!(
    node
    && node.type === vue3CoreRuntime.NodeTypes.ELEMENT
    && node.codegenNode
    && node.codegenNode.type === vue3CoreRuntime.NodeTypes.VNODE_CALL
    && node.codegenNode.children
    && node.codegenNode.children.type === vue3CoreRuntime.NodeTypes.JS_CACHE_EXPRESSION
  );
}

function vue3ChildrenHaveCachedNodes(children) {
  return (children || []).some(child => {
    return child && (
      (
        child.type === vue3CoreRuntime.NodeTypes.ELEMENT
        && child.tagType === vue3CoreRuntime.ElementTypes.ELEMENT
        && child.codegenNode
        && child.codegenNode.type === vue3CoreRuntime.NodeTypes.JS_CACHE_EXPRESSION
      )
      || (
        child.type === vue3CoreRuntime.NodeTypes.TEXT_CALL
        && child.codegenNode
        && child.codegenNode.type === vue3CoreRuntime.NodeTypes.JS_CACHE_EXPRESSION
      )
    );
  });
}

function vue3SetVNodeBlock(node, isBlock, context) {
  if (!context || !node || node.isBlock === isBlock) return;
  if (isBlock) {
    context.removeHelper(vue3CoreRuntime.getVNodeHelper(context.inSSR, node.isComponent));
    node.isBlock = true;
    context.helper(vue3CoreRuntime.OPEN_BLOCK);
    context.helper(vue3CoreRuntime.getVNodeBlockHelper(context.inSSR, node.isComponent));
  } else {
    context.removeHelper(vue3CoreRuntime.OPEN_BLOCK);
    context.removeHelper(vue3CoreRuntime.getVNodeBlockHelper(context.inSSR, node.isComponent));
    node.isBlock = false;
    context.helper(vue3CoreRuntime.getVNodeHelper(context.inSSR, node.isComponent));
  }
}

function vue3NodeAtPath(root, path) {
  let current = root;
  for (const part of path || []) {
    if (current == null) return undefined;
    current = current[vue3PathKey(part)];
  }
  return current;
}

function vue3HolderAtPath(root, path) {
  let current = root;
  const parts = path || [];
  for (let i = 0; i < parts.length - 1; i++) {
    if (current == null) return undefined;
    current = current[vue3PathKey(parts[i])];
  }
  if (current == null || !parts.length) return undefined;
  return { owner: current, key: vue3PathKey(parts[parts.length - 1]) };
}

function vue3PathKey(part) {
  return typeof part === 'number' || /^\d+$/.test(String(part)) ? Number(part) : part;
}

function vue3FindSlotFunction(codegenNode, slotProjection) {
  if (!codegenNode || codegenNode.type !== vue3CoreRuntime.NodeTypes.VNODE_CALL) return undefined;
  const children = codegenNode.children;
  if (!children || children.type !== vue3CoreRuntime.NodeTypes.JS_OBJECT_EXPRESSION) return undefined;
  const props = children.properties || [];
  return (props.find(prop => vue3SlotKeyMatches(prop.key, slotProjection)) || {}).value;
}

function vue3SlotKeyMatches(key, slotProjection) {
  if (!key || !slotProjection) return false;
  if (slotProjection.kind === 'static') {
    return key === slotProjection.name || (key.type === vue3CoreRuntime.NodeTypes.SIMPLE_EXPRESSION && key.content === slotProjection.name);
  }
  if (slotProjection.kind === 'dynamic') {
    const node = slotProjection.node;
    return key === node || (
      key.type === vue3CoreRuntime.NodeTypes.COMPOUND_EXPRESSION
      && node
      && node.type === vue3CoreRuntime.NodeTypes.COMPOUND_EXPRESSION
      && vue3ProjectionSource(key) === vue3ProjectionSource(node)
    ) || (
      key.type === vue3CoreRuntime.NodeTypes.SIMPLE_EXPRESSION
      && node
      && node.type === vue3CoreRuntime.NodeTypes.SIMPLE_EXPRESSION
      && key.content === node.content
      && key.isStatic === node.isStatic
    );
  }
  return false;
}

function vue3ProjectionSource(node) {
  if (!node) return '';
  if (typeof node === 'string') return node;
  if (node.type === vue3CoreRuntime.NodeTypes.SIMPLE_EXPRESSION) return String(node.content || '');
  if (node.type === vue3CoreRuntime.NodeTypes.COMPOUND_EXPRESSION) return (node.children || []).map(vue3ProjectionSource).join('');
  return node.loc && node.loc.source || '';
}

function vue3ElementDirectivePropSummaries(dir, result, extra = {}) {
  return ((result && result.props) || []).map(prop => {
    const key = prop && prop.key;
    const value = prop && prop.value;
    return {
      kind: 'directiveProp',
      name: key && key.type === vue3CoreRuntime.NodeTypes.SIMPLE_EXPRESSION && key.isStatic ? key.content : undefined,
      dynamicKey: !(key && key.type === vue3CoreRuntime.NodeTypes.SIMPLE_EXPRESSION && key.isStatic),
      ignoreDynamicKeyForNormalize: !!(prop && prop.__vuecOn && prop.__vuecOn.ignoreDynamicKeyForNormalize),
      valueStartsWithArray: !!(value && value.type === vue3CoreRuntime.NodeTypes.SIMPLE_EXPRESSION && String(value.content || '').trim().startsWith('[')),
      valueStatic: !!(value && value.type === vue3CoreRuntime.NodeTypes.SIMPLE_EXPRESSION && value.isStatic),
      valueType: value && value.type,
      valueConstant: vue3ElementPropValueIsConstant(value) || !!(prop && prop.__vuecOn && prop.__vuecOn.valueConstant),
      valueCached: !!(value && value.type === vue3CoreRuntime.NodeTypes.JS_CACHE_EXPRESSION),
      propModifier: !!extra.propModifier,
      forceBlock: !!extra.forceBlock,
    };
  });
}

  function vue3ElementPropValueIsConstant(value) {
    if (!value) return false;
    if (value.__vuecOn && value.__vuecOn.cache) return true;
    if (value.type === vue3CoreRuntime.NodeTypes.JS_CACHE_EXPRESSION) return true;
  if (value.type === vue3CoreRuntime.NodeTypes.SIMPLE_EXPRESSION) {
    return !!value.isStatic || Number(value.constType || 0) > 0;
  }
  if (value.type === vue3CoreRuntime.NodeTypes.COMPOUND_EXPRESSION) {
    return Number(value.constType || 0) > 0;
  }
  return false;
}

function vue3DirectiveRuntimePayload(needRuntime) {
  if (typeof needRuntime === 'symbol') {
    return { kind: 'helper', helper: projectionNameFromHelperSymbol(needRuntime), helperName: vue3CoreRuntime.helperNameMap[needRuntime] };
  }
  if (typeof needRuntime === 'string') {
    const helper = helperSymbolFromProjection(needRuntime);
    if (helper) {
      return { kind: 'helper', helper: needRuntime, helperName: vue3CoreRuntime.helperNameMap[helper] };
    }
  }
  if (needRuntime) {
    return { kind: 'asset' };
  }
  return null;
}

function projectionNameFromHelperSymbol(symbol) {
  const helperName = vue3CoreRuntime.helperNameMap[symbol];
  if (helperName) return helperName;
  const entries = Object.entries(vue3CoreRuntime).filter(([, value]) => value === symbol);
  if (entries.length) return entries[0][0];
  return symbol && symbol.description;
}

function materializeVue3DirectiveArgsProjection(projection, dir, context) {
  const elements = [];
  const runtimeProjection = projection && projection.runtime || {};
  if (runtimeProjection.kind === 'helper') {
    const helper = helperSymbolFromProjection(runtimeProjection.helper);
    const helperName = runtimeProjection.helperName || (helper && vue3CoreRuntime.helperNameMap[helper]);
    elements.push(context && helper ? context.helperString(helper) : `_${helperName || runtimeProjection.helper || ''}`);
  } else {
    if (context) {
      context.helper(vue3CoreRuntime.RESOLVE_DIRECTIVE);
      context.directives.add(runtimeProjection.name || (dir && dir.name) || '');
    }
    elements.push(vue3CoreRuntime.toValidAssetId(runtimeProjection.name || (dir && dir.name) || '', 'directive'));
  }
  if (projection && projection.includeExp && dir && dir.exp) elements.push(dir.exp);
  if (projection && projection.includeArg && dir && dir.arg) elements.push(dir.arg);
  if (projection && projection.modifiers && projection.modifiers.length) {
    if (!(projection && projection.includeArg)) {
      if (!(projection && projection.includeExp)) elements.push('void 0');
      elements.push('void 0');
    }
    elements.push(vue3CoreRuntime.createObjectExpression((projection.modifiers || []).map(modifier => {
      const name = modifier && modifier.name || '';
      return vue3CoreRuntime.createObjectProperty(
        vue3CoreRuntime.createSimpleExpression(name, true),
        vue3CoreRuntime.createSimpleExpression('true', false, dir && dir.loc || vue3CoreRuntime.locStub, vue3CoreRuntime.ConstantTypes.CAN_SKIP_PATCH),
      );
    }), dir && dir.loc || vue3CoreRuntime.locStub));
  }
  return elements;
}

function materializeVue3ElementSlotsProjection(projection, node, context) {
  const properties = [];
  for (const slot of projection.slots || []) {
    const slotChildren = [];
    for (const index of slot.indices || []) {
      const child = node.children && node.children[index];
      if (!child) continue;
      if (slot.unwrapTemplate && child.type === vue3CoreRuntime.NodeTypes.ELEMENT && child.tag === 'template') {
        slotChildren.push(...(child.children || []));
      } else {
        slotChildren.push(child);
      }
    }
    properties.push(vue3CoreRuntime.createObjectProperty(
      slot.name || 'default',
      vue3CoreRuntime.createFunctionExpression([], slotChildren, false, true, node.loc),
    ));
  }
  properties.push(vue3CoreRuntime.createObjectProperty(
    '_',
    vue3CoreRuntime.createSimpleExpression(projection.slotFlag || '1 /* STABLE */', false),
  ));
  if (context) context.helper(vue3CoreRuntime.WITH_CTX);
  return vue3CoreRuntime.createObjectExpression(properties, node.loc);
}

function materializeVue3SlotErrors(projection, node, context) {
  if (!projection || !Array.isArray(projection.errors) || !context || !context.onError) return;
  for (const error of projection.errors) {
    context.onError(vue3CoreRuntime.createCompilerError(error.code, error.loc || (node && node.loc) || vue3CoreRuntime.locStub));
  }
}

function materializeVue3SlotsProjection(projection, node, context, buildSlotFn) {
  projection = projection || {};
  if (context) context.helper(vue3CoreRuntime.WITH_CTX);
  const properties = [];
  for (const property of projection.properties || []) {
    properties.push(vue3CoreRuntime.createObjectProperty(
      materializeVue3SlotProjectionNode(property.key, node, context),
      materializeVue3SlotFunctionProjection(property, node, context, buildSlotFn),
    ));
  }
  const slotFlag = projection.slotFlag || 1;
  const flagText = projection.slotFlagText || (slotFlag === 2 ? 'DYNAMIC' : slotFlag === 3 ? 'FORWARDED' : 'STABLE');
  properties.push(vue3CoreRuntime.createObjectProperty(
    '_',
    vue3CoreRuntime.createSimpleExpression(`${slotFlag} /* ${flagText} */`, false),
  ));
  let slots = vue3CoreRuntime.createObjectExpression(properties, node && node.loc || vue3CoreRuntime.locStub);
  if (projection.dynamicSlots && projection.dynamicSlots.length) {
    const dynamicSlotArray = vue3CoreRuntime.createArrayExpression(
      projection.dynamicSlots.map(slot => materializeVue3DynamicSlotProjection(slot, node, context, buildSlotFn)),
    );
    if (context) context.helper(vue3CoreRuntime.CREATE_SLOTS);
    slots = vue3CoreRuntime.createCallExpression(
      context ? context.helper(vue3CoreRuntime.CREATE_SLOTS) : vue3CoreRuntime.CREATE_SLOTS,
      [
        slots,
        dynamicSlotArray,
      ],
      node && node.loc || vue3CoreRuntime.locStub,
    );
  }
  return slots;
}

function materializeVue3SlotFunctionProjection(property, node, context, buildSlotFn) {
  const loc = property.loc || (node && node.loc) || vue3CoreRuntime.locStub;
  const params = materializeVue3SlotProjectionNode(property.params, node, context);
  const returns = materializeVue3SlotChildren(property, node);
  if (typeof buildSlotFn === 'function') {
    const vFor = vue3SlotFunctionVFor(property, node, context);
    const fn = buildSlotFn(params, vFor, returns, loc);
    if (property.nonScoped && context && context.compatConfig && fn) fn.isNonScopedSlot = true;
    return fn;
  }
  const fn = vue3CoreRuntime.createFunctionExpression(params, returns, false, true, returns.length ? returns[0].loc : loc);
  if (property.nonScoped && context && context.compatConfig) fn.isNonScopedSlot = true;
  return fn;
}

function vue3SlotFunctionVFor(property, node, context) {
  for (const index of property.indices || []) {
    const child = node && node.children && node.children[index];
    const source = property.unwrapTemplate && child && child.type === vue3CoreRuntime.NodeTypes.ELEMENT && child.tag === 'template'
      ? child
      : null;
    const dir = source && vue3CoreRuntime.findDir(source, 'for', true);
    if (!dir) continue;
    if (!dir.forParseResult) {
      const projection = callBridge('vue3.core.trackVForSlotScopes', {
        node: source,
        context: vue3TransformSlotContextPayload(context),
      });
      if (projection && projection.parseResult) {
        dir.forParseResult = materializeVue3ForParseResult(projection.parseResult, dir, context);
      }
    }
    return dir;
  }
  return undefined;
}

function materializeVue3SlotChildren(property, node) {
  const out = [];
  for (const index of property.indices || []) {
    const child = node && node.children && node.children[index];
    if (!child) continue;
    if (property.unwrapTemplate && child.type === vue3CoreRuntime.NodeTypes.ELEMENT && child.tag === 'template') {
      out.push(...(child.children || []));
    } else {
      out.push(child);
    }
  }
  return out;
}

function materializeVue3DynamicSlotProjection(projection, node, context, buildSlotFn) {
  if (!projection) return vue3CoreRuntime.createSimpleExpression('undefined', false);
  if (projection.kind === 'conditional') {
    return vue3CoreRuntime.createConditionalExpression(
      materializeVue3SlotProjectionNode(projection.test, node, context),
      materializeVue3DynamicSlotProjection(projection.consequent, node, context, buildSlotFn),
      materializeVue3DynamicSlotProjection(projection.alternate, node, context, buildSlotFn),
    );
  }
  if (projection.kind === 'for') {
    const params = projection.params || {};
    const slot = materializeVue3DynamicSlotProjection(projection.slot, node, context, buildSlotFn);
    const source = materializeVue3SlotProjectionNode(projection.source, node, context);
    const loopParams = vue3CoreRuntime.createForLoopParams({
      value: materializeVue3SlotProjectionNode(params.value, node, context),
      key: materializeVue3SlotProjectionNode(params.key, node, context),
      index: materializeVue3SlotProjectionNode(params.index, node, context),
    });
    const renderListHelper = context ? context.helper(vue3CoreRuntime.RENDER_LIST) : vue3CoreRuntime.RENDER_LIST;
    return vue3CoreRuntime.createCallExpression(
      renderListHelper,
      [
        source,
        vue3CoreRuntime.createFunctionExpression(
          loopParams,
          slot,
          true,
        ),
      ],
      node && node.loc || vue3CoreRuntime.locStub,
    );
  }
  if (projection.kind === 'dynamicSlot') {
    const properties = [
      vue3CoreRuntime.createObjectProperty('name', materializeVue3SlotProjectionNode(projection.name, node, context)),
      vue3CoreRuntime.createObjectProperty('fn', materializeVue3SlotFunctionProjection(projection.slot || {}, node, context, buildSlotFn)),
    ];
    if (projection.key != null) {
      properties.push(vue3CoreRuntime.createObjectProperty('key', vue3CoreRuntime.createSimpleExpression(String(projection.key), true)));
    }
    return vue3CoreRuntime.createObjectExpression(properties);
  }
  return materializeVue3SlotProjectionNode(projection, node, context);
}

function materializeVue3SlotProjectionNode(projection, node, context) {
  if (!projection || projection.kind === 'undefined') return undefined;
  if (projection.type) return projection;
  for (const name of projection.helpers || []) {
    const symbol = helperSymbolFromProjection(name);
    if (symbol && context) context.helper(symbol);
  }
  switch (projection.kind) {
    case 'simple':
      return vue3CoreRuntime.createSimpleExpression(
        projection.content || '',
        !!projection.isStatic,
        projection.loc || (node && node.loc) || vue3CoreRuntime.locStub,
        projection.constType || 0,
      );
    case 'compound':
      return vue3CoreRuntime.createCompoundExpression(
        (projection.children || []).map(child => materializeVue3SlotProjectionNode(child, node, context)),
        projection.loc || (node && node.loc) || vue3CoreRuntime.locStub,
      );
    default:
      if (typeof projection === 'string') return projection;
      throw new Error(`Unsupported Rust v-slot projection: ${projection.kind}`);
  }
}

function materializeVue3SlotOutletMutations(process, node, context) {
  for (const mutation of process && process.mutations || []) {
    const prop = node && node.props && node.props[mutation.index];
    if (!prop) continue;
    if (mutation.kind === 'setPropName') {
      prop.name = mutation.name || prop.name;
    } else if (mutation.kind === 'setDirectiveArgContent' && prop.arg) {
      prop.arg.content = mutation.content || '';
    } else if (mutation.kind === 'setDirectiveExp') {
      prop.exp = materializeVue3SlotOutletProjection(mutation.value, node, context);
    }
  }
}

function materializeVue3SlotOutletName(projection, node, context) {
  if (!projection) return '"default"';
  if (projection.kind === 'literal') return projection.value || '"default"';
  return materializeVue3SlotOutletProjection(projection, node, context);
}

function materializeVue3SlotOutletProjection(projection, node, context) {
  if (!projection || projection.kind === 'undefined') return undefined;
  if (typeof projection === 'string') return projection;
  if (projection.type) return projection;
  switch (projection.kind) {
    case 'node': {
      if (projection.path === 'props') {
        const prop = node && node.props && node.props[projection.index];
        return prop && prop[projection.field || 'exp'];
      }
      return undefined;
    }
    case 'simple':
    case 'compound':
      return materializeVue3SlotProjectionNode(projection, node, context);
    default:
      throw new Error(`Unsupported Rust slot outlet projection: ${projection.kind}`);
  }
}

function vue3IfSiblingPayload(siblings) {
  return (siblings || []).map(vue3IfNodePayload);
}

function vue3IfNodePayload(node) {
  if (!node || typeof node !== 'object') return node;
  const payload = {
    type: node.type,
    tag: node.tag,
    tagType: node.tagType,
    content: node.content,
    locSource: node.loc && node.loc.source,
  };
  if (node.type === vue3CoreRuntime.NodeTypes.TEXT_CALL) {
    payload.content = vue3IfNodePayload(node.content);
  }
  if (node.type === vue3CoreRuntime.NodeTypes.IF) {
    payload.branches = (node.branches || []).map(branch => ({
      hasCondition: branch.condition !== undefined,
      userKey: branch.userKey || null,
    }));
  }
  return payload;
}

function vue3IfBranchCodegenPayload(branch) {
  return {
    isTemplateIf: !!(branch && branch.isTemplateIf),
    children: (branch && branch.children || []).map(child => ({
      type: child && child.type,
      memoedCodegenType: vue3MemoedCodegenType(child && child.codegenNode),
    })),
  };
}

function vue3MemoedCodegenType(codegenNode) {
  const node = vue3CoreRuntime.getMemoedVNodeCall(codegenNode);
  return node && node.type;
}

function materializeVue3IfErrors(projection, node, dir, context) {
  if (!projection || !Array.isArray(projection.errors) || !context || !context.onError) return;
  for (const error of projection.errors) {
    const loc = error.loc === 'userKey'
      ? runtimeIfUserKeyLoc(node, dir)
      : error.loc === 'dir'
        ? dir.loc
        : node.loc;
    context.onError(vue3CoreRuntime.createCompilerError(error.code, loc));
  }
}

function runtimeIfUserKeyLoc(node, dir) {
  const key = vue3CoreRuntime.findProp(node, 'key');
  return key && key.loc || dir && dir.loc || node && node.loc;
}

function materializeVue3IfProjection(projection, node, dir) {
  if (!projection || projection.kind === 'undefined') return undefined;
  switch (projection.kind) {
    case 'simple':
      return vue3CoreRuntime.createSimpleExpression(
        projection.content || '',
        !!projection.isStatic,
        projection.loc || (dir && dir.exp && dir.exp.loc) || (node && node.loc),
        projection.constType || 0,
      );
    default:
      throw new Error(`Unsupported Rust v-if projection: ${projection.kind}`);
  }
}

function materializeVue3ForErrors(projection, node, dir, context) {
  if (!projection || !Array.isArray(projection.errors) || !context || !context.onError) return;
  for (const error of projection.errors) {
    context.onError(vue3CoreRuntime.createCompilerError(error.code, vue3ForErrorLoc(error, node, dir)));
  }
}

function materializeVue3ForTemplateKeyErrors(projection, node, dir, context) {
  if (!projection || !Array.isArray(projection.templateKeyErrors) || !context || !context.onError) return;
  for (const error of projection.templateKeyErrors) {
    context.onError(vue3CoreRuntime.createCompilerError(error.code, vue3ForErrorLoc(error, node, dir)));
  }
}

function vue3ForErrorLoc(error, node, dir) {
  if (!error) return dir && dir.loc || node && node.loc || locStub;
  if (error.loc && typeof error.loc === 'object') return error.loc;
  if (error.loc === 'node') return node && node.loc || dir && dir.loc || locStub;
  return dir && dir.loc || node && node.loc || locStub;
}

function materializeVue3ForParseResult(parseResult, dir, context) {
  return {
    source: materializeVue3ForProjectionNode(parseResult && parseResult.source, dir, context),
    value: materializeVue3ForProjectionNode(parseResult && parseResult.value, dir, context),
    key: materializeVue3ForProjectionNode(parseResult && parseResult.key, dir, context),
    index: materializeVue3ForProjectionNode(parseResult && parseResult.index, dir, context),
    finalized: parseResult && parseResult.finalized !== undefined ? !!parseResult.finalized : true,
  };
}

function vue3ForNodePayload(forNode) {
  return {
    source: forNode && forNode.source,
    children: (forNode && forNode.children || []).map(child => ({
      type: child && child.type,
      tagType: child && child.tagType,
      codegenNode: child && child.codegenNode ? {
        type: child.codegenNode.type,
        isBlock: !!child.codegenNode.isBlock,
        isComponent: !!child.codegenNode.isComponent,
      } : null,
    })),
  };
}

function materializeVue3ForKeyProperty(projection, dir, context) {
  if (!projection || !projection.value) return null;
  return vue3CoreRuntime.createObjectProperty(
    'key',
    materializeVue3ForProjectionNode(projection.value, dir, context),
  );
}

function materializeVue3ForChildBlock(projection, node, forNode, keyProperty, context) {
  projection = projection || {};
  const children = forNode.children || [];
  if (projection.kind === 'slotOutlet') {
    const slotOutlet = projection.path === 'templateChild'
      ? (node.children || [])[projection.index || 0]
      : node;
    const childBlock = slotOutlet && slotOutlet.codegenNode;
    if (projection.path === 'templateChild' && keyProperty && childBlock) {
      runtime.injectProp(childBlock, keyProperty, context);
    }
    return childBlock;
  }
  if (projection.kind === 'fragmentWrapper') {
    return vue3CoreRuntime.createVNodeCall(
      context,
      context.helper(vue3CoreRuntime.FRAGMENT),
      keyProperty ? vue3CoreRuntime.createObjectExpression([keyProperty]) : undefined,
      node.children,
      projection.patchFlag || 64,
      undefined,
      undefined,
      true,
      undefined,
      false,
    );
  }
  const childBlock = children[0] && children[0].codegenNode;
  if (!childBlock) return undefined;
  if (node.tagType === vue3CoreRuntime.ElementTypes.TEMPLATE && keyProperty) {
    vue3CoreRuntime.injectProp(childBlock, keyProperty, context);
  }
  const shouldBeBlock = !!projection.childBlockIsBlock;
  if (childBlock.isBlock !== shouldBeBlock) {
    if (childBlock.isBlock) {
      context.removeHelper(vue3CoreRuntime.OPEN_BLOCK);
      context.removeHelper(vue3CoreRuntime.getVNodeBlockHelper(context.inSSR, childBlock.isComponent));
    } else {
      context.removeHelper(vue3CoreRuntime.getVNodeHelper(context.inSSR, childBlock.isComponent));
    }
  }
  childBlock.isBlock = shouldBeBlock;
  if (childBlock.isBlock) {
    context.helper(vue3CoreRuntime.OPEN_BLOCK);
    context.helper(vue3CoreRuntime.getVNodeBlockHelper(context.inSSR, childBlock.isComponent));
  } else {
    context.helper(vue3CoreRuntime.getVNodeHelper(context.inSSR, childBlock.isComponent));
  }
  return childBlock;
}

function materializeVue3ForProjectionNode(projection, dir, context) {
  if (!projection || projection.kind === 'undefined') return undefined;
  if (projection.type) return projection;
  for (const name of projection.helpers || []) {
    const symbol = helperSymbolFromProjection(name);
    if (symbol && context) context.helper(symbol);
  }
  switch (projection.kind) {
    case 'simple':
      return vue3CoreRuntime.createSimpleExpression(
        projection.content || '',
        !!projection.isStatic,
        projection.loc || (dir && dir.exp && dir.exp.loc) || locStub,
        projection.constType || 0,
      );
    case 'compound':
      return vue3CoreRuntime.createCompoundExpression(
        (projection.children || []).map(child => materializeVue3ForProjectionNode(child, dir, context)),
        projection.loc || (dir && dir.exp && dir.exp.loc) || locStub,
      );
    default:
      if (typeof projection === 'string') return projection;
      throw new Error(`Unsupported Rust v-for projection: ${projection.kind}`);
  }
}

function vue3ResolveComponentContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    inline: !!context.inline,
    selfName: context.selfName || null,
    bindingMetadata: context.bindingMetadata || {},
    isScriptSetup: context.bindingMetadata && Object.prototype.hasOwnProperty.call(context.bindingMetadata, '__isScriptSetup')
      ? context.bindingMetadata.__isScriptSetup
      : undefined,
    compatIsOnElement: false,
    builtInComponents: vue3BuiltInComponentPayload(context),
  };
}

function vue3BuiltInComponentPayload(context) {
  const names = ['Transition', 'transition', 'TransitionGroup', 'transition-group'];
  const out = [];
  const seen = new Set();
  for (const name of names) {
    let helper;
    try {
      helper = context && context.isBuiltInComponent && context.isBuiltInComponent(name);
    } catch (_) {
      helper = undefined;
    }
    if (!helper) helper = vue3CoreRuntime.isCoreComponent(name);
    const helperName = typeof helper === 'symbol' ? vue3CoreRuntime.helperNameMap[helper] : undefined;
    if (helperName && !seen.has(name)) {
      seen.add(name);
      out.push({ tag: name, helperName });
    }
  }
  return out;
}

function materializeVue3ComponentTypeProjection(projection, node, context) {
  if (!projection) return vue3CoreRuntime.toValidAssetId(node && node.tag || '', 'component');
  const helper = helperSymbolFromProjection(projection.helper);
  switch (projection.kind) {
    case 'dynamic':
      if (helper) context.helper(helper);
      return vue3CoreRuntime.createCallExpression(
        helper || vue3CoreRuntime.RESOLVE_DYNAMIC_COMPONENT,
        [materializeVue3ComponentProjectionNode(projection.argument, node, context)],
      );
    case 'helper':
      if (projection.helperName && context && typeof context.isBuiltInComponent === 'function') {
        const contextHelper = vue3ContextBuiltInComponentSymbol(context, node, projection.helperName);
        if (contextHelper) {
          if (projection.registerHelper !== false) context.helper(contextHelper);
          return contextHelper;
        }
      }
      if (helper && projection.registerHelper !== false) context.helper(helper);
      if (helper) return helper;
      if (projection.helperName) {
        const runtimeHelper = helperSymbolFromHelperName(projection.helperName);
        if (runtimeHelper && projection.registerHelper !== false) context.helper(runtimeHelper);
        return runtimeHelper || `_${projection.helperName}`;
      }
      return projection.helper;
    case 'expression':
      for (const name of projection.helpers || []) {
        const symbol = helperSymbolFromProjection(name);
        if (symbol) context.helper(symbol);
      }
      return projection.content || '';
    case 'asset':
      if (helper) context.helper(helper);
      if (projection.component) context.components.add(projection.component);
      return projection.assetId || vue3CoreRuntime.toValidAssetId(node && node.tag || '', 'component');
    default:
      throw new Error(`Unsupported Rust component projection: ${projection.kind}`);
  }
}

function vue3ContextBuiltInComponentSymbol(context, node, helperName) {
  const tag = node && node.tag;
  const names = [tag];
  if (helperName === 'Transition') names.push('Transition', 'transition');
  else if (helperName === 'TransitionGroup') names.push('TransitionGroup', 'transition-group');
  else if (helperName === 'BaseTransition') names.push('BaseTransition', 'base-transition');
  for (const name of names) {
    if (!name) continue;
    try {
      const helper = context.isBuiltInComponent(name);
      if (typeof helper === 'symbol' && vue3CoreRuntime.helperNameMap[helper] === helperName) {
        return helper;
      }
    } catch (_) {}
  }
  return undefined;
}

function materializeVue3ComponentProjectionNode(projection, node, context) {
  if (!projection || projection.kind === 'undefined') return undefined;
  if (projection.type) return projection;
  switch (projection.kind) {
    case 'simple':
      return vue3CoreRuntime.createSimpleExpression(
        projection.content || '',
        !!projection.isStatic,
        projection.loc || (node && node.loc) || locStub,
        projection.constType || 0,
      );
    case 'expression':
      for (const name of projection.helpers || []) {
        const symbol = helperSymbolFromProjection(name);
        if (symbol && context) context.helper(symbol);
      }
      return projection.content || '';
    default:
      return projection;
  }
}

function helperSymbolFromProjection(name, context) {
  if (!name) return undefined;
  if (context && context.__vuecDomHelpers && typeof context.__vuecDomHelpers[name] === 'symbol') {
    return context.__vuecDomHelpers[name];
  }
  if (vue3CoreRuntime[name]) {
    const direct = vue3CoreRuntime[name];
    const helperName = typeof direct === 'symbol' ? vue3CoreRuntime.helperNameMap[direct] : undefined;
    return helperSymbolFromHelperName(helperName) || direct;
  }
  return helperSymbolFromHelperName(name);
}

function helperSymbolFromHelperName(name) {
  if (!name) return undefined;
  const keys = Reflect.ownKeys(vue3CoreRuntime.helperNameMap);
  for (let index = keys.length - 1; index >= 0; index--) {
    const key = keys[index];
    if (typeof key === 'symbol' && vue3CoreRuntime.helperNameMap[key] === name) return key;
  }
  return Object.values(vue3CoreRuntime).find(value => {
    return typeof value === 'symbol' && vue3CoreRuntime.helperNameMap[value] === name;
  });
}

function createRootCodegen(root, context) {
  const projection = callBridge('vue3.core.rootCodegen', { root });
  if (!projection || projection.kind === 'none') return;
  if (projection.kind === 'child') {
    root.codegenNode = (root.children || [])[projection.index || 0];
    return;
  }
  if (projection.kind === 'childCodegen') {
    const child = (root.children || [])[projection.index || 0];
    const codegenNode = child && child.codegenNode;
    if (codegenNode && projection.asBlock) {
      vue3CoreRuntime.convertToBlock(codegenNode, context);
    }
    root.codegenNode = codegenNode;
    return;
  }
  if (projection.kind === 'fragment') {
    root.codegenNode = vue3CoreRuntime.createVNodeCall(
      context,
      context.helper(vue3CoreRuntime.FRAGMENT),
      undefined,
      root.children || [],
      projection.patchFlag,
      undefined,
      undefined,
      true,
      undefined,
      false,
    );
    return;
  }
  throw new Error(`Unsupported Rust root codegen projection: ${projection.kind}`);
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

const vuecBridgeRuntime = { callBridge };

function normalizeArgs(payload) {
  return payload || {};
}

function resolveStylePreprocessPayload(payload) {
  if (!payload || !payload.options || typeof payload.options !== 'object') return payload;
  const options = payload.options;
  const preprocessOptions = options.preprocessOptions;
  if (!preprocessOptions || typeof preprocessOptions !== 'object') return payload;
  if (typeof preprocessOptions.additionalData !== 'function') return payload;
  const source = payload.source == null ? '' : String(payload.source);
  const resolvedOptions = Object.assign({}, options, {
    preprocessOptions: Object.assign({}, preprocessOptions, {
      additionalData: preprocessOptions.additionalData(source, options.filename)
    })
  });
  return Object.assign({}, payload, { options: resolvedOptions });
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

function vue27StyleBridgePayload(payload) {
  if (!payload || !payload.options || typeof payload.options !== 'object') return payload;
  const options = payload.options;
  const bridgeOptions = {};
  for (const key of Object.keys(options)) {
    if (
      key !== 'postcssPlugins' &&
      key !== 'postcssOptions' &&
      key !== 'sourceMap' &&
      key !== 'source_map'
    ) {
      bridgeOptions[key] = options[key];
    }
  }
  return Object.assign({}, payload, { options: bridgeOptions });
}

function vue3StyleBridgePayload(payload) {
  if (!payload || !payload.options || typeof payload.options !== 'object') return payload;
  const options = payload.options;
  const bridgeOptions = {};
  for (const key of Object.keys(options)) {
    if (key !== 'sourceMap' && key !== 'source_map') {
      bridgeOptions[key] = options[key];
    }
  }
  return Object.assign({}, payload, { options: bridgeOptions });
}

function normalizeStyleAliasResult(result) {
  if (!result || typeof result !== 'object' || result.map !== null) return result;
  const out = Object.assign({}, result);
  out.map = undefined;
  return out;
}

function hydrateVue3SfcParseResult(result) {
  if (!result || typeof result !== 'object' || !result.descriptor) return result;
  const descriptor = result.descriptor;
  descriptor.shouldForceReload = function shouldForceReload(prevImports) {
    return vue3SfcShouldForceReload(prevImports, descriptor);
  };
  return result;
}

function throwVue3CompileScriptErrors(result) {
  if (!result || !Array.isArray(result.errors) || result.errors.length === 0) {
    return result;
  }
  const first = result.errors[0];
  const message = typeof first === 'string' ? first : (first && first.message) || String(first);
  throw new Error(message.startsWith('[@vue/compiler-sfc]') ? message : `[@vue/compiler-sfc] ${message}`);
}

function hydrateVue3CompileScriptResult(result) {
  result = throwVue3CompileScriptErrors(result);
  if (!result || typeof result !== 'object') return result;
  const bindings = result.bindings;
  if (bindings && typeof bindings === 'object' && result.propsAliases && typeof result.propsAliases === 'object') {
    bindings.__propsAliases = result.propsAliases;
    delete result.propsAliases;
  }
  if (Array.isArray(result.warnings)) {
    for (const warning of result.warnings) {
      if (warning == null) continue;
      const message = typeof warning === 'string' ? warning : (warning && warning.message) || String(warning);
      console.warn(message.startsWith('[@vue/compiler-sfc]') ? message : `[@vue/compiler-sfc] ${message}`);
    }
    delete result.warnings;
  }
  if (bindings && typeof bindings === 'object' && Object.prototype.hasOwnProperty.call(bindings, '__isScriptSetup')) {
    const isScriptSetup = bindings.__isScriptSetup === true || bindings.__isScriptSetup === 'true';
    delete bindings.__isScriptSetup;
    Object.defineProperty(bindings, '__isScriptSetup', {
      enumerable: false,
      configurable: true,
      value: isScriptSetup
    });
  }
  return result;
}

function vue3CompileScriptBridgePayload(payload) {
  const out = Object.assign({}, payload || {});
  const options = Object.assign({}, out.options || {});
  const filename = options.filename !== undefined ? options.filename : out.filename;
  if (typeof options.customElement === 'function') {
    try {
      options.__vuecCustomElement = !!options.customElement(filename);
    } catch (_) {
      options.__vuecCustomElement = false;
    }
    delete options.customElement;
  }
  if (typeof __TEST__ !== 'undefined' && __TEST__ === true) {
    options.__vuecEmitScriptSetupMarker = false;
  }
  out.options = options;
  return out;
}

function vue3SfcCompileTemplateBridgePayload(payload) {
  const out = Object.assign({}, payload || {});
  const options = Object.assign({}, out.options || {});
  const source = String(out.source || '');
  const bridgeOptions = vue3SfcCompileTemplateOptionsForBridge(options, source);
  if (options.ast) {
    out.ast = vue3CoreRuntime.dehydrateForBridge(options.ast);
    if (options.ast.source && !bridgeOptions.__vuecSourceMapSource) {
      bridgeOptions.__vuecSourceMapSource = options.ast.source;
      bridgeOptions.__vuecSourceMapBaseOffset = 0;
    }
  }
  out.options = options;
  out.bridgeOptions = bridgeOptions;
  return out;
}

function vue3SfcCompileTemplateOptionsForBridge(options, source) {
  const compilerOptions = options && options.compilerOptions && typeof options.compilerOptions === 'object'
    ? options.compilerOptions
    : {};
  const bridgeOptions = Object.assign({}, normalizeVue3OptionsForBridge(options, source));
  delete bridgeOptions.ast;
  delete bridgeOptions.compiler;
  delete bridgeOptions.compilerOptions;
  Object.assign(bridgeOptions, {
    mode: 'module',
    prefixIdentifiers: true,
    hoistStatic: true,
    cacheHandlers: true,
    sourceMap: true,
  });
  if (options && options.filename !== undefined) bridgeOptions.filename = options.filename;
  if (options && options.id !== undefined) bridgeOptions.id = options.id;
  if (options && options.scoped) {
    const shortId = String(options.id || '').replace(/^data-v-/, '');
    bridgeOptions.scopeId = `data-v-${shortId}`;
    bridgeOptions.scoped = true;
  } else {
    delete bridgeOptions.scopeId;
    delete bridgeOptions.scope_id;
  }
  if (options && options.slotted !== undefined) bridgeOptions.slotted = options.slotted;
  if (options && options.ssr !== undefined) bridgeOptions.ssr = options.ssr;
  if (options && options.ssrCssVars !== undefined) {
    bridgeOptions.ssrCssVars = options.ssrCssVars;
  }
  if (options && options.isProd !== undefined) bridgeOptions.isProd = options.isProd;
  if (options && options.preprocessLang !== undefined) bridgeOptions.preprocessLang = options.preprocessLang;
  if (options && options.transformAssetUrls !== undefined) bridgeOptions.transformAssetUrls = options.transformAssetUrls;
  Object.assign(bridgeOptions, normalizeVue3OptionsForBridge(compilerOptions, source));
  bridgeOptions.hmr = !(options && options.isProd);
  if (compilerOptions && compilerOptions.nodeTransforms && !Array.isArray(compilerOptions.nodeTransforms)) {
    delete bridgeOptions.nodeTransforms;
  }
  return bridgeOptions;
}

function vue3SfcCustomCompileTemplateResult(payload) {
  const options = payload && payload.options;
  const compiler = options && options.compiler;
  if (!compiler || typeof compiler.compile !== 'function') return undefined;
  const source = String(payload && payload.source || '');
  const compilerOptions = vue3SfcCompileTemplateOptionsForBridge(options, source);
  const result = compiler.compile(source, compilerOptions) || {};
  return hydrateVue3SfcCompileTemplateResult({
    code: result.code || '',
    ast: result.ast,
    preamble: result.preamble,
    map: result.map,
    source,
    errors: result.errors || [],
    tips: result.tips || [],
  });
}

function hydrateVue3SfcCompileTemplateResult(result) {
  if (!result || typeof result !== 'object') return result;
  const out = Object.assign({}, result);
  if (typeof out.ast === 'string') {
    try {
      out.ast = JSON.parse(out.ast);
    } catch (_) {}
  }
  if (out.ast && typeof out.ast === 'object' && out.ast.type === vue3CoreRuntime.NodeTypes.ROOT) {
    out.ast = hydrateVue3Ast(out.ast, {});
  }
  if (Array.isArray(out.errors)) {
    out.errors = out.errors.map(vue3SfcTemplateErrorForPublicApi);
  } else {
    out.errors = [];
  }
  if (!Array.isArray(out.tips)) out.tips = [];
  return out;
}

function vue3SfcTemplateErrorForPublicApi(error) {
  if (typeof error === 'string') return error;
  if (!error || typeof error !== 'object') return error;
  if (error instanceof Error) return error;
  const message = error.message || error.msg || String(error);
  const syntaxError = new SyntaxError(message);
  if (error.code !== undefined) syntaxError.code = error.code;
  if (error.loc !== undefined) syntaxError.loc = error.loc;
  return syntaxError;
}

function vue3SfcShouldForceReload(prevImports, descriptor) {
  const scriptSetup = descriptor && descriptor.scriptSetup;
  if (!scriptSetup || (scriptSetup.lang !== 'ts' && scriptSetup.lang !== 'tsx')) {
    return false;
  }
  for (const key in prevImports) {
    if (!prevImports[key].isUsedInTemplate && vue3SfcIsImportUsed(key, descriptor)) {
      return true;
    }
  }
  return false;
}

function vue3SfcIsImportUsed(local, descriptor) {
  return vue3SfcTemplateUsedIdentifiers(descriptor).has(local);
}

function vue3SfcTemplateUsedIdentifiers(descriptor) {
  const template = descriptor.template;
  const ids = new Set();
  const children = template.ast && Array.isArray(template.ast.children) ? template.ast.children : [];
  children.forEach(node => collectVue3SfcTemplateIds(node, ids));
  return ids;
}

function collectVue3SfcTemplateIds(node, ids) {
  if (!node || typeof node !== 'object') return;
  if (node.type === vue3CoreRuntime.NodeTypes.ELEMENT) {
    let tag = String(node.tag || '');
    if (tag.includes('.')) tag = tag.split('.')[0].trim();
    if (tag && !vue3SfcIsNativeTag(tag) && !vue3SfcIsDomBuiltInComponent(tag)) {
      ids.add(camelize(tag));
      ids.add(capitalize(camelize(tag)));
    }
    for (const prop of node.props || []) {
      if (prop && prop.type === vue3CoreRuntime.NodeTypes.DIRECTIVE) {
        if (!vue3CoreRuntime.isBuiltInDirective(prop.name)) {
          ids.add(`v${capitalize(camelize(prop.name))}`);
        }
        if (prop.arg && !prop.arg.isStatic) {
          collectVue3SfcExpressionIds(prop.arg, ids);
        }
        if (prop.name === 'for' && prop.forParseResult && prop.forParseResult.source) {
          collectVue3SfcExpressionIds(prop.forParseResult.source, ids);
        } else if (prop.exp) {
          collectVue3SfcExpressionIds(prop.exp, ids);
        } else if (prop.name === 'bind' && prop.arg && prop.arg.content) {
          ids.add(camelize(prop.arg.content));
        }
      } else if (prop && prop.type === vue3CoreRuntime.NodeTypes.ATTRIBUTE && prop.name === 'ref' && prop.value && prop.value.content) {
        ids.add(prop.value.content);
      }
    }
    for (const child of node.children || []) {
      collectVue3SfcTemplateIds(child, ids);
    }
  } else if (node.type === vue3CoreRuntime.NodeTypes.INTERPOLATION) {
    collectVue3SfcExpressionIds(node.content, ids);
  }
}

function collectVue3SfcExpressionIds(exp, ids) {
  if (!exp) return;
  if (exp.ast) {
    collectVue3SfcAstIds(exp.ast, ids, null);
  } else if (exp.ast === null) {
    collectVue3SfcStringExpressionIds(exp.content, ids);
  } else if (exp.content) {
    collectVue3SfcStringExpressionIds(exp.content, ids);
  }
}

function collectVue3SfcAstIds(node, ids, parent) {
  if (!node || typeof node !== 'object') return;
  if (Array.isArray(node)) {
    node.forEach(child => collectVue3SfcAstIds(child, ids, parent));
    return;
  }
  if (node.type === 'Identifier') {
    if (parent && parent.type === 'MemberExpression' && parent.property === node && !parent.computed) return;
    if (parent && (parent.type === 'ObjectProperty' || parent.type === 'Property') && parent.key === node && !parent.computed) return;
    ids.add(node.name);
  }
  for (const key of Object.keys(node)) {
    if (key === 'parent' || key === 'loc') continue;
    const value = node[key];
    if (value && typeof value === 'object') {
      collectVue3SfcAstIds(value, ids, node);
    }
  }
}

function collectVue3SfcStringExpressionIds(source, ids) {
  const text = String(source || '');
  const pattern = /[A-Za-z_$][\w$]*/g;
  let match;
  while ((match = pattern.exec(text))) {
    const before = text.slice(0, match.index).trimEnd();
    if (!before.endsWith('.')) ids.add(match[0]);
  }
}

function vue3SfcIsNativeTag(tag) {
  return /^(?:html|body|base|head|link|meta|style|title|address|article|aside|footer|header|hgroup|h1|h2|h3|h4|h5|h6|nav|section|div|dd|dl|dt|figcaption|figure|picture|hr|img|li|main|ol|p|pre|ul|a|b|abbr|bdi|bdo|br|cite|code|data|dfn|em|i|kbd|mark|q|rp|rt|ruby|s|samp|small|span|strong|sub|sup|time|u|var|wbr|area|audio|map|track|video|embed|object|param|source|canvas|script|noscript|del|ins|caption|col|colgroup|table|thead|tbody|td|th|tr|button|datalist|fieldset|form|input|label|legend|meter|optgroup|option|output|progress|select|textarea|details|dialog|menu|summary|template|blockquote|iframe|tfoot|svg|math)$/i.test(String(tag || ''));
}

function vue3SfcIsDomBuiltInComponent(tag) {
  return tag === 'Transition' || tag === 'transition' || tag === 'TransitionGroup' || tag === 'transition-group';
}

function hydrateVue3SsrCompileResult(result) {
  if (!result || typeof result !== 'object') return result;
  if (Array.isArray(result.ast_helpers)) {
    const helpers = new Set(result.ast_helpers.map(name => Symbol(name)));
    delete result.ast_helpers;
    result.ast = Object.assign({}, result.ast || {}, { helpers });
  }
  return result;
}

function vue27StylePostcssRequired(options) {
  return !!(
    options &&
    (Array.isArray(options.postcssPlugins) || options.postcssOptions)
  );
}

function vue27StylePostcssOptions(options) {
  const postcssOptions = Object.assign({}, options && options.postcssOptions ? options.postcssOptions : {});
  const filename = options && options.filename ? options.filename : undefined;
  if (filename !== undefined) {
    if (postcssOptions.to === undefined) postcssOptions.to = filename;
    if (postcssOptions.from === undefined) postcssOptions.from = filename;
  }
  return postcssOptions;
}

function applyVue27StylePostcssSync(result, options) {
  if (!vue27StylePostcssRequired(options)) return normalizeStyleAliasResult(result);
  const out = Object.assign({}, result);
  const errors = Array.isArray(out.errors) ? out.errors.slice() : [];
  let rawResult;
  try {
    const postcss = require('postcss');
    rawResult = postcss((options && options.postcssPlugins) || []).process(
      out.code || '',
      vue27StylePostcssOptions(options)
    );
    out.code = rawResult.css || '';
    out.map = rawResult.map && rawResult.map.toJSON ? rawResult.map.toJSON() : out.map;
  } catch (error) {
    errors.push(error);
  }
  out.errors = errors;
  out.rawResult = rawResult;
  return normalizeStyleAliasResult(out);
}

function applyVue27StylePostcssAsync(result, options) {
  const out = Object.assign({}, result);
  const errors = Array.isArray(out.errors) ? out.errors.slice() : [];
  if (!vue27StylePostcssRequired(options)) {
    return Promise.resolve(normalizeStyleAliasResult(out));
  }
  try {
    const postcss = require('postcss');
    const rawResult = postcss((options && options.postcssPlugins) || []).process(
      out.code || '',
      vue27StylePostcssOptions(options)
    );
    return Promise.resolve(rawResult)
      .then(postcssResult => {
        out.code = postcssResult.css || '';
        out.map = postcssResult.map && postcssResult.map.toJSON ? postcssResult.map.toJSON() : out.map;
        out.errors = errors;
        out.rawResult = postcssResult;
        return normalizeStyleAliasResult(out);
      })
      .catch(error => ({
        code: '',
        map: undefined,
        errors: errors.concat(error && error.message ? error.message : error),
        rawResult: undefined,
      }));
  } catch (error) {
    return Promise.resolve({
      code: '',
      map: undefined,
      errors: errors.concat(error && error.message ? error.message : error),
      rawResult: undefined,
    });
  }
}

function hydrateVue27CompileScriptResult(result) {
  if (!result || typeof result !== 'object') return result;
  const bindings = result.bindings;
  if (bindings && typeof bindings === 'object' && Object.prototype.hasOwnProperty.call(bindings, '__isScriptSetup')) {
    const isScriptSetup = bindings.__isScriptSetup === true || bindings.__isScriptSetup === 'true';
    delete bindings.__isScriptSetup;
    Object.defineProperty(bindings, '__isScriptSetup', {
      enumerable: false,
      configurable: true,
      value: isScriptSetup
    });
  }
  return result;
}

function vue27CompileScriptBridgePayload(payload) {
  const out = Object.assign({}, payload || {});
  const options = Object.assign({}, out.options || {});
  if (typeof __TEST__ !== 'undefined' && __TEST__ === true) {
    options.__vuecEmitScriptSetupMarker = false;
  }
  out.options = options;
  return out;
}

function vue3SfcParseBridgePayload(payload) {
  const out = Object.assign({}, payload || {});
  const source = String(out.source || '');
  out.bridgeOptions = normalizeVue3SfcParseOptionsForBridge(out.options, source);
  return out;
}

function normalizeVue3SfcParseOptionsForBridge(options, source) {
  if (!options || typeof options !== 'object') return {};
  const normalized = {};
  for (const key of Object.keys(options)) {
    if (key !== 'compiler' && typeof options[key] !== 'function') normalized[key] = options[key];
  }
  if (options.templateParseOptions && typeof options.templateParseOptions === 'object') {
    normalized.templateParseOptions = normalizeVue3OptionsForBridge(
      options.templateParseOptions,
      source,
    );
  }
  return normalized;
}

function applyVue3SfcCustomCompilerParse(result, source, options, filename) {
  if (!result || typeof result !== 'object' || !result.descriptor) return result;
  const compiler = options && options.compiler;
  if (!compiler || typeof compiler.parse !== 'function') return result;
  const customErrors = [];
  const ast = compiler.parse(String(source || ''), Object.assign({}, options.templateParseOptions || {}, {
    parseMode: 'sfc',
    prefixIdentifiers: true,
    onError: error => customErrors.push(error),
  }));
  const out = Object.assign({}, result);
  out.errors = customErrors.concat(Array.isArray(result.errors) ? result.errors : []);
  if (ast && Array.isArray(ast.children) && ast.children.length === 0) {
    out.errors.push(new SyntaxError(
      `At least one <template> or <script> is required in a single file component. ${filename || 'anonymous.vue'}`
    ));
  }
  return out;
}

function vue3BridgePayload(source, filename, options) {
  warnIgnoredDecodeEntities(options);
  return {
    source,
    filename,
    options,
    bridgeOptions: normalizeVue3OptionsForBridge(options, source),
  };
}

function vue3CompileBridgePayload(input, filename, options) {
  if (input && typeof input === 'object' && input.type === vue3CoreRuntime.NodeTypes.ROOT && Array.isArray(input.children)) {
    const source = typeof input.source === 'string' ? input.source : '';
    const normalizedSource = vue3AstTemplateSource(input, source);
    warnIgnoredDecodeEntities(options);
    return {
      source: normalizedSource,
      filename,
      options,
      ast: vue3CoreRuntime.dehydrateForBridge(input),
      bridgeOptions: Object.assign(
        normalizeVue3OptionsForBridge(options, normalizedSource),
        { __vuecSourceMapSource: source, __vuecSourceMapBaseOffset: 0 },
      ),
    };
  }
  return vue3BridgePayload(input && input.source ? input.source : input, filename, options);
}

function vue3AstTemplateSource(ast, source) {
  const children = Array.isArray(ast && ast.children) ? ast.children : [];
  if (!children.length) return '';
  let start = Infinity;
  let end = -Infinity;
  for (const child of children) {
    const locStart = child && child.loc && child.loc.start && child.loc.start.offset;
    const locEnd = child && child.loc && child.loc.end && child.loc.end.offset;
    if (Number.isFinite(locStart) && Number.isFinite(locEnd) && locEnd >= locStart) {
      start = Math.min(start, locStart);
      end = Math.max(end, locEnd);
    }
  }
  return Number.isFinite(start) && end >= start ? String(source || '').slice(start, end) : source;
}

function warnIgnoredDecodeEntities(options) {
  if (!options || typeof options !== 'object' || typeof options.decodeEntities !== 'function') return;
  const message = '[Vue warn]: decodeEntities option is passed but will be ignored in non-browser builds.';
  if (!globalThis.__VUEC_DECODE_ENTITIES_WARNED__) {
    globalThis.__VUEC_DECODE_ENTITIES_WARNED__ = true;
  }
  console.warn(message);
}

function emitVue2CompileWarnings(result, options) {
  const suppressed = options && options.__vuecSuppressWarnings;
  if (suppressed === true) return;
  if (!result || typeof result !== 'object') return;
  const warnings = [];
  if (Array.isArray(result.errors)) warnings.push(...result.errors);
  if (Array.isArray(result.tips)) warnings.push(...result.tips);
  const suppressedMessages = Array.isArray(suppressed) ? suppressed.map(String) : [];
  for (const warning of warnings) {
    if (warning == null) continue;
    const message = typeof warning === 'string'
      ? warning
      : typeof warning.msg === 'string'
        ? warning.msg
        : null;
    if (message == null) continue;
    if (suppressedMessages.some(suppressed => message.includes(suppressed))) continue;
    if (typeof warning === 'string') {
      console.error(message);
    } else {
      console.error(message);
    }
  }
}

function normalizeVue3OptionsForBridge(options, source) {
  if (!options || typeof options !== 'object') return {};
  const normalized = {};
  for (const key of Object.keys(options)) {
    if (typeof options[key] !== 'function') normalized[key] = options[key];
  }
  const tags = extractVueTemplateTags(String(source || ''));
  if (hasVuePredicateOption(options, 'isVoidTag')) {
    normalized.__vuecVoidTags = collectVuePredicateHits(options.isVoidTag, tags);
  }
  if (hasVuePredicateOption(options, 'isPreTag')) {
    normalized.__vuecPreTags = collectVuePredicateHits(options.isPreTag, tags);
  }
  if (hasVuePredicateOption(options, 'isIgnoreNewlineTag')) {
    normalized.__vuecIgnoreNewlineTags = collectVuePredicateHits(options.isIgnoreNewlineTag, tags);
  }
  if (typeof options.getNamespace === 'function') {
    normalized.__vuecNamespaces = collectVueNamespaceHits(options.getNamespace, tags);
    normalized.__vuecDomNamespaces = true;
  }
  if (Object.prototype.hasOwnProperty.call(options, 'ns')) {
    normalized.__vuecRootNamespace = options.ns;
  }
  if (hasVuePredicateOption(options, 'isNativeTag')) {
    normalized.__vuecNativeTags = collectVuePredicateHits(options.isNativeTag, tags);
  }
  normalized.__vuecCustomElements = collectVuePredicateHits(options.isCustomElement, tags);
  normalized.__vuecBuiltInComponents = collectVuePredicateHits(options.isBuiltInComponent, tags);
  normalized.__vuecStringifyStatic = typeof options.transformHoist === 'function';
  return normalized;
}

function hasVuePredicateOption(options, name) {
  return Object.prototype.hasOwnProperty.call(options, name) &&
    (typeof options[name] === 'function' || Array.isArray(options[name]));
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

function collectVueNamespaceHits(getNamespace, values) {
  if (!getNamespace || typeof getNamespace !== 'function') return {};
  const namespaces = {};
  for (const value of values) {
    try {
      const namespace = getNamespace(value);
      if (namespace !== undefined && namespace !== null) namespaces[value] = namespace;
    } catch (_) {}
  }
  return namespaces;
}

function usesAliasRuntimeCompile(options) {
  if (!options || typeof options !== 'object') return false;
  if (Array.isArray(options.nodeTransforms) && options.nodeTransforms.some(transform => typeof transform === 'function')) {
    return true;
  }
  if (options.directiveTransforms && typeof options.directiveTransforms === 'object') {
    return Object.values(options.directiveTransforms).some(transform => typeof transform === 'function');
  }
  return typeof options.transformHoist === 'function';
}

function emitVue3CompileDiagnostics(result, options) {
  if (!result || !Array.isArray(result.diagnostics) || !result.diagnostics.length) return;
  const onError = options && typeof options.onError === 'function'
    ? options.onError
    : error => { throw error; };
  for (const diagnostic of result.diagnostics) {
    const message = typeof diagnostic === 'string' ? diagnostic : diagnostic && diagnostic.message;
    const error = new SyntaxError(message || 'Vue compiler error');
    error.code = diagnostic && diagnostic.code !== undefined ? diagnostic.code : 64;
    error.loc = diagnostic && diagnostic.loc !== undefined ? diagnostic.loc : undefined;
    onError(error);
  }
}

function emitVue3StyleWarnings(result) {
  if (!result || !Array.isArray(result.diagnostics) || !result.diagnostics.length) return result;
  const diagnostics = [];
  for (const diagnostic of result.diagnostics) {
    const severity = diagnostic && diagnostic.severity;
    const code = diagnostic && diagnostic.code;
    const message = typeof diagnostic === 'string' ? diagnostic : diagnostic && diagnostic.message;
    if (severity === 'warning' && code === 'VUEC_STYLE_DEPRECATED_SCOPED_SELECTOR' && message) {
      console.warn(`[@vue/compiler-sfc] ${message}`);
    } else {
      diagnostics.push(diagnostic);
    }
  }
  if (diagnostics.length === result.diagnostics.length) return result;
  const out = Object.assign({}, result);
  if (diagnostics.length) {
    out.diagnostics = diagnostics;
  } else {
    delete out.diagnostics;
  }
  return out;
}

const vue3DomParserOptions = {
  parseMode: 'html',
  isVoidTag: tag => /^(?:area|base|br|col|embed|hr|img|input|link|meta|param|source|track|wbr)$/i.test(String(tag || '')),
  isNativeTag: tag => /^(?:html|body|base|head|link|meta|style|title|address|article|aside|footer|header|hgroup|h1|h2|h3|h4|h5|h6|nav|section|div|dd|dl|dt|figcaption|figure|picture|hr|img|li|main|ol|p|pre|ul|a|b|abbr|bdi|bdo|br|cite|code|data|dfn|em|i|kbd|mark|q|rp|rt|ruby|s|samp|small|span|strong|sub|sup|time|u|var|wbr|area|audio|map|track|video|embed|object|param|source|canvas|script|noscript|del|ins|caption|col|colgroup|table|thead|tbody|td|th|tr|button|datalist|fieldset|form|input|label|legend|meter|optgroup|option|output|progress|select|textarea|details|dialog|menu|summary|template|blockquote|iframe|tfoot|svg|math)$/i.test(String(tag || '')),
  isPreTag: tag => String(tag || '').toLowerCase() === 'pre',
  isIgnoreNewlineTag: tag => /^(?:pre|textarea)$/i.test(String(tag || '')),
  decodeEntities: vue3CoreRuntime.decodeHtmlBrowser,
  isBuiltInComponent: tag => {
    if (tag === 'Transition' || tag === 'transition') return vue3CoreRuntime.TRANSITION;
    if (tag === 'TransitionGroup' || tag === 'transition-group') return vue3CoreRuntime.TRANSITION_GROUP;
    return undefined;
  },
  getNamespace: (_tag, parent, rootNamespace) => parent && parent.ns !== undefined ? parent.ns : rootNamespace,
};

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
      return capture(() => normalizeSfcStyleResult(api.compileStyle(optionObjectWithSource(extractStyleSource(fixture)))));
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
    run_option_matrix_with_backend(scope, AliasBackend::Generated)
}

pub fn run_napi_option_matrix(scope: &SelectionArgs) -> JsonReport {
    run_option_matrix_with_backend(scope, AliasBackend::Napi)
}

fn run_option_matrix_with_backend(scope: &SelectionArgs, backend: AliasBackend) -> JsonReport {
    let targets = select_targets(scope);
    let lock_path = PathBuf::from("compat/official-revisions.lock");
    let lock_hash = file_sha256(&lock_path).ok();
    let lock = match load_official_lock(&lock_path) {
        Ok(lock) => lock,
        Err(err) => {
            let mut report = JsonReport::new(backend.option_command(), ReportStatus::Fail);
            report.metadata = report.metadata.with_lock_context(lock_hash, None);
            return report
                .with_scope(scope)
                .with_violations(vec![format!("failed to load official lock: {err}")]);
        }
    };
    let mut items = Vec::new();
    let mut violations = Vec::new();
    let mut target_reports = Vec::new();
    let mut created = Vec::new();
    match prepare_alias_backend(backend, &targets) {
        Ok(paths) => created.extend(paths.into_iter().map(|path| path.display().to_string())),
        Err(err) => violations.push(format!(
            "failed to prepare {} alias packages: {err:#}",
            backend.label()
        )),
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
        let rust_root = backend.root(target.version_line);
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
                backend.option_side(),
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
            "alias_backend": backend.name(),
            "rows": row_reports,
        }));
    }
    let metadata = ReportMetadata::capture().with_lock_context(lock_hash.clone(), Some(&lock));
    let report_path = PathBuf::from("target")
        .join("conformance")
        .join(lock_hash.as_deref().unwrap_or("unknown-lock"))
        .join(backend.option_report_name());
    if let Some(parent) = report_path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            violations.push(format!("failed to create {}: {err}", parent.display()));
        }
    }
    let report_body = serde_json::json!({
        "command": backend.option_command(),
        "metadata": metadata,
        "lock_hash": lock_hash,
        "alias_backend": backend.name(),
        "targets": target_reports,
        "counts": output_contract_counts_from_items(&items),
    });
    if let Err(err) = write_json(&report_path, &report_body) {
        violations.push(format!("failed to write {}: {err}", report_path.display()));
    }
    let mut report = JsonReport::new(backend.option_command(), aggregate_status(&items));
    report.metadata = metadata;
    report
        .with_scope(scope)
        .with_items(items)
        .with_violations(violations)
        .with_created(
            created
                .into_iter()
                .chain([report_path.display().to_string()])
                .collect(),
        )
        .with_note(backend.option_note())
}

pub fn run_conformance(args: &ConformanceArgs) -> JsonReport {
    run_conformance_with_backend(args, AliasBackend::Generated)
}

pub fn run_napi_conformance(args: &ConformanceArgs) -> JsonReport {
    run_conformance_with_backend(args, AliasBackend::Napi)
}

fn run_conformance_with_backend(args: &ConformanceArgs, backend: AliasBackend) -> JsonReport {
    let lock_hash = file_sha256(&args.lock).ok();
    let lock = load_official_lock(&args.lock).ok();
    let report_metadata =
        ReportMetadata::capture().with_lock_context(lock_hash.clone(), lock.as_ref());
    let requested = select_conformance_suites(args);
    let mut items = Vec::new();
    let mut violations = Vec::new();
    let mut created = Vec::new();
    let conformance_targets = conformance_targets(&requested);
    if let Err(err) = prepare_alias_backend(backend, &conformance_targets) {
        violations.push(format!(
            "failed to prepare {} alias packages for conformance: {err:#}",
            backend.label()
        ));
    }

    for suite in requested {
        let spec = suite_spec(suite);
        let root = args.vendor_dir.join(spec.version_line.as_str());
        let revision_metadata = root.join("official-revision.json");
        if !revision_metadata.exists() {
            violations.push(format!(
                "{} is missing; run `cargo xtask sync-official-tests --locked` first",
                revision_metadata.display()
            ));
            items.push(ReportItem::new(
                spec.name,
                ReportStatus::Fail,
                "official checkout metadata is missing",
                Some(revision_metadata),
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
        let readiness = conformance_readiness(spec, backend);
        let smoke_results = run_conformance_smokes(spec, backend);
        let smoke_failures = smoke_results
            .iter()
            .filter(|result| result.status == "fail")
            .count();
        let ready_to_execute =
            !discovered.is_empty() && readiness.alias_ready && readiness.runner_ready;
        let execution_result = if ready_to_execute {
            match run_conformance_execution(spec, &root, &discovered, lock_hash.as_deref(), backend)
            {
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
        let coverage = conformance_coverage_report(spec, backend, execution_result.as_ref());
        let report_path = PathBuf::from("target")
            .join("conformance")
            .join(lock_hash.as_deref().unwrap_or("unknown-lock"))
            .join(backend.conformance_report_name(spec));
        let report_body = serde_json::json!({
            "command": backend.conformance_command(),
            "metadata": report_metadata,
            "suite": spec.name,
            "version_line": spec.version_line,
            "alias_backend": backend.name(),
            "lock_hash": lock_hash,
            "test_files": discovered,
            "counts": counts,
            "coverage": coverage,
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

    let mut report = JsonReport::new(backend.conformance_command(), ReportStatus::Pending);
    report.metadata = report_metadata;
    report
        .with_items(items)
        .with_violations(violations)
        .with_created(created)
        .with_note(backend.conformance_note())
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
    run_output_contract_with_backend(scope, AliasBackend::Generated)
}

pub fn run_napi_output_contract(scope: &SelectionArgs) -> JsonReport {
    run_output_contract_with_backend(scope, AliasBackend::Napi)
}

fn run_output_contract_with_backend(scope: &SelectionArgs, backend: AliasBackend) -> JsonReport {
    let targets = select_targets(scope);
    let mut items = Vec::new();
    let mut violations = Vec::new();
    let mut created = Vec::new();
    let lock_path = PathBuf::from("compat/official-revisions.lock");
    let lock_hash = file_sha256(&lock_path).ok();
    let lock = match load_official_lock(&lock_path) {
        Ok(lock) => lock,
        Err(err) => {
            let mut report = JsonReport::new(backend.output_command(), ReportStatus::Fail);
            report.metadata = report.metadata.with_lock_context(lock_hash, None);
            return report
                .with_scope(scope)
                .with_violations(vec![format!("failed to load official lock: {err}")]);
        }
    };
    let metadata = ReportMetadata::capture().with_lock_context(lock_hash.clone(), Some(&lock));
    let report_path = PathBuf::from("target")
        .join("conformance")
        .join(lock_hash.as_deref().unwrap_or("unknown-lock"))
        .join(backend.output_report_name());
    let mut target_reports = Vec::new();

    match prepare_alias_backend(backend, &targets) {
        Ok(paths) => created.extend(paths.into_iter().map(|path| path.display().to_string())),
        Err(err) => violations.push(format!(
            "failed to prepare {} alias packages: {err:#}",
            backend.label()
        )),
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
        let rust_root = backend.root(target.version_line);
        match run_output_contract_probe(target, &official_root, &rust_root) {
            Ok(mut target_report) => {
                if let Some(object) = target_report.as_object_mut() {
                    object.insert(
                        "alias_backend".into(),
                        serde_json::Value::String(backend.name().into()),
                    );
                }
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
        "command": backend.output_command(),
        "metadata": metadata,
        "lock_hash": lock_hash,
        "alias_backend": backend.name(),
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
    let mut report = JsonReport::new(backend.output_command(), ReportStatus::Pending);
    report.metadata = metadata;
    report
        .with_scope(scope)
        .with_items(items)
        .with_violations(violations)
        .with_created(
            created
                .into_iter()
                .chain([report_path.display().to_string()])
                .collect(),
        )
        .with_note(backend.output_note())
}

pub fn verify_npm_alias(scope: &SelectionArgs) -> JsonReport {
    let targets = select_targets(scope);
    let lock_path = PathBuf::from("compat/official-revisions.lock");
    let lock_hash = file_sha256(&lock_path).ok();
    let lock = load_official_lock(&lock_path).ok();
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
    report.metadata = report.metadata.with_lock_context(lock_hash, lock.as_ref());
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
    let metadata = ReportMetadata::capture().with_lock_context(lock_hash.clone(), lock.as_ref());
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
    report.metadata = metadata;
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

fn official_lock_static_items(lock: &OfficialRevisionsLock) -> Vec<ReportItem> {
    [
        (VersionLine::Vue26, "vue2_6", &lock.vue2_6),
        (VersionLine::Vue27, "vue2_7", &lock.vue2_7),
        (VersionLine::Vue3, "vue3", &lock.vue3),
    ]
    .into_iter()
    .flat_map(|(_, label, baseline)| {
        let mut items = Vec::new();
        let rev_status = if is_commit_sha(&baseline.rev) {
            ReportStatus::Pass
        } else {
            ReportStatus::Fail
        };
        items.push(ReportItem::new(
            format!("{label}.rev"),
            rev_status,
            format!("rev={}", baseline.rev),
            None,
        ));
        for (package, version) in &baseline.npm {
            let status = if is_exact_npm_version(version) {
                ReportStatus::Pass
            } else {
                ReportStatus::Fail
            };
            items.push(ReportItem::new(
                format!("{label}.npm.{package}"),
                status,
                format!("version={version}"),
                None,
            ));
        }
        items
    })
    .collect()
}

fn validate_official_lock_vendor(
    lock: &OfficialRevisionsLock,
    vendor_dir: &Path,
) -> Vec<ReportItem> {
    [
        (VersionLine::Vue26, &lock.vue2_6),
        (VersionLine::Vue27, &lock.vue2_7),
        (VersionLine::Vue3, &lock.vue3),
    ]
    .into_iter()
    .flat_map(|(version_line, baseline)| {
        let checkout = vendor_dir.join(version_line.as_str());
        let mut items = Vec::new();
        items.push(validate_official_checkout_revision(
            version_line,
            baseline,
            &checkout,
        ));
        for (package, expected) in &baseline.npm {
            items.push(validate_official_package_manifest(
                version_line,
                package,
                expected,
                &checkout,
            ));
        }
        items
    })
    .collect()
}

fn validate_official_checkout_revision(
    version_line: VersionLine,
    baseline: &BaselineLock,
    checkout: &Path,
) -> ReportItem {
    if !checkout.join(".git").exists() {
        return ReportItem::new(
            format!("{}.checkout", version_line.as_str()),
            ReportStatus::Fail,
            format!("{} is not a git checkout", checkout.display()),
            Some(checkout.to_path_buf()),
        );
    }
    let object_type = git_output(checkout, &["cat-file", "-t", &baseline.rev]);
    if object_type.as_deref() != Some("commit") {
        return ReportItem::new(
            format!("{}.rev-object", version_line.as_str()),
            ReportStatus::Fail,
            format!(
                "lock rev {} resolves to {:?}, expected commit",
                baseline.rev, object_type
            ),
            Some(checkout.to_path_buf()),
        );
    }
    let head = git_output(checkout, &["rev-parse", "HEAD"]);
    let status = if head.as_deref() == Some(baseline.rev.as_str()) {
        ReportStatus::Pass
    } else {
        ReportStatus::Fail
    };
    ReportItem::new(
        format!("{}.checkout", version_line.as_str()),
        status,
        format!(
            "expected rev {}, checkout HEAD {}",
            baseline.rev,
            head.unwrap_or_else(|| "<unreadable>".into())
        ),
        Some(checkout.to_path_buf()),
    )
}

fn validate_official_package_manifest(
    version_line: VersionLine,
    package: &str,
    expected: &str,
    checkout: &Path,
) -> ReportItem {
    let Some(package_json) = official_package_manifest_path(version_line, package, checkout) else {
        return ReportItem::new(
            format!("{}.npm.{package}", version_line.as_str()),
            ReportStatus::Fail,
            format!("no package manifest mapping for {package}"),
            Some(checkout.to_path_buf()),
        );
    };
    let actual = read_package_manifest_version(&package_json);
    let status = if actual.as_deref() == Some(expected) {
        ReportStatus::Pass
    } else {
        ReportStatus::Fail
    };
    ReportItem::new(
        format!("{}.npm.{package}", version_line.as_str()),
        status,
        format!(
            "lock version {}, manifest version {}",
            expected,
            actual.unwrap_or_else(|| "<missing>".into())
        ),
        Some(package_json),
    )
}

fn official_package_manifest_path(
    version_line: VersionLine,
    package: &str,
    checkout: &Path,
) -> Option<PathBuf> {
    match (version_line, package) {
        (VersionLine::Vue26, "vue") | (VersionLine::Vue27, "vue") => {
            Some(checkout.join("package.json"))
        }
        (VersionLine::Vue26, "vue-template-compiler") => Some(
            checkout
                .join("packages")
                .join("vue-template-compiler")
                .join("package.json"),
        ),
        (VersionLine::Vue27, "vue-template-compiler") => Some(
            checkout
                .join("packages")
                .join("template-compiler")
                .join("package.json"),
        ),
        (VersionLine::Vue3, "vue") => {
            Some(checkout.join("packages").join("vue").join("package.json"))
        }
        (VersionLine::Vue3, "@vue/compiler-core") => Some(
            checkout
                .join("packages")
                .join("compiler-core")
                .join("package.json"),
        ),
        (VersionLine::Vue3, "@vue/compiler-dom") => Some(
            checkout
                .join("packages")
                .join("compiler-dom")
                .join("package.json"),
        ),
        (VersionLine::Vue3, "@vue/compiler-ssr") => Some(
            checkout
                .join("packages")
                .join("compiler-ssr")
                .join("package.json"),
        ),
        (VersionLine::Vue3, "@vue/compiler-sfc") => Some(
            checkout
                .join("packages")
                .join("compiler-sfc")
                .join("package.json"),
        ),
        _ => None,
    }
}

fn read_package_manifest_version(path: &Path) -> Option<String> {
    read_json::<serde_json::Value>(path)
        .ok()?
        .get("version")?
        .as_str()
        .map(ToOwned::to_owned)
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

fn default_official_lock_context() -> Option<(String, OfficialRevisionsLock)> {
    official_lock_context(Path::new("compat/official-revisions.lock"))
}

fn official_lock_context(path: &Path) -> Option<(String, OfficialRevisionsLock)> {
    let lock_hash = file_sha256(path).ok()?;
    let lock = load_official_lock(path).ok()?;
    Some((lock_hash, lock))
}

fn official_commit_map(lock: &OfficialRevisionsLock) -> BTreeMap<String, String> {
    [
        (VersionLine::Vue26.as_str(), &lock.vue2_6.rev),
        (VersionLine::Vue27.as_str(), &lock.vue2_7.rev),
        (VersionLine::Vue3.as_str(), &lock.vue3.rev),
    ]
    .into_iter()
    .map(|(version_line, rev)| (version_line.to_string(), rev.clone()))
    .collect()
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
            Some(value) if is_exact_npm_version(value) => {}
            Some(value) if !value.trim().is_empty() => violations.push(format!(
                "{label}.npm.{key} must be an exact npm package version, got {value:?}"
            )),
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

fn is_exact_npm_version(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty()
        || value.starts_with(['^', '~', '>', '<', '=', '*'])
        || value.contains(" - ")
        || value.contains("||")
        || matches!(
            value,
            "latest" | "next" | "v2-latest" | "main" | "master" | "dev" | "nightly"
        )
    {
        return false;
    }
    let suffix_start = value.find(['-', '+']).unwrap_or(value.len());
    let core = &value[..suffix_start];
    let parts = core.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
        && value[suffix_start..]
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '+' | '.'))
}

fn git_output(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
struct ConformanceExecutionCounts {
    total: usize,
    pass: usize,
    fail: usize,
    skip: usize,
    pending: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ConformanceCoverageKind {
    RustBacked,
    ShimBacked,
    Mixed,
}

impl ConformanceCoverageKind {
    const fn as_str(self) -> &'static str {
        match self {
            ConformanceCoverageKind::RustBacked => "rust-backed",
            ConformanceCoverageKind::ShimBacked => "shim-backed",
            ConformanceCoverageKind::Mixed => "mixed",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct ConformanceCoverageReport {
    source: ConformanceCoverageKind,
    reason: String,
    counts_by_source: BTreeMap<String, ConformanceExecutionCounts>,
    rust_backed_pass: usize,
    rust_backed_total: usize,
    files: Vec<ConformanceCoverageFile>,
}

#[derive(Clone, Debug, Serialize)]
struct ConformanceCoverageFile {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
    source: ConformanceCoverageKind,
    reason: String,
    counts: ConformanceExecutionCounts,
}

fn run_conformance_smokes(
    spec: ConformanceSuiteSpec,
    backend: AliasBackend,
) -> Vec<ConformanceSmokeResult> {
    conformance_smoke_targets(spec)
        .into_iter()
        .map(|target| {
            let request = api_require_request(target);
            match run_alias_smoke(target, &backend.root(target.version_line)) {
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
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    match spec.name {
        "vue2-compiler" => {
            run_vue2_compiler_conformance(spec, official_root, discovered, lock_hash, backend)
        }
        "vue27-compiler" => {
            run_vue27_compiler_conformance(spec, official_root, discovered, lock_hash, backend)
        }
        "vue27-sfc" => {
            run_vue27_sfc_conformance(spec, official_root, discovered, lock_hash, backend)
        }
        "vue3-core" => {
            run_vue3_core_conformance(spec, official_root, discovered, lock_hash, backend)
        }
        "vue3-dom" => run_vue3_dom_conformance(spec, official_root, discovered, lock_hash, backend),
        "vue3-sfc" => run_vue3_sfc_conformance(spec, official_root, discovered, lock_hash, backend),
        "vue3-ssr" => run_vue3_ssr_conformance(spec, official_root, discovered, lock_hash, backend),
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

fn run_vue2_compiler_conformance(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    discovered: &[String],
    lock_hash: Option<&str>,
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    let prepared_root = prepare_vue2_compiler_conformance_suite(spec, official_root, lock_hash)?;
    run_jasmine_conformance(spec, prepared_root, discovered, backend)
}

fn run_vue27_compiler_conformance(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    discovered: &[String],
    lock_hash: Option<&str>,
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    let prepared_root = prepare_vue27_compiler_conformance_suite(spec, official_root, lock_hash)?;
    run_vitest_conformance(spec, prepared_root, discovered, backend)
}

fn run_vue27_sfc_conformance(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    discovered: &[String],
    lock_hash: Option<&str>,
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    let prepared_root = prepare_vue27_sfc_conformance_suite(spec, official_root, lock_hash)?;
    run_vitest_conformance(spec, prepared_root, discovered, backend)
}

fn run_vue3_core_conformance(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    discovered: &[String],
    lock_hash: Option<&str>,
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    let prepared_root = prepare_vue3_core_conformance_suite(spec, official_root, lock_hash)?;
    run_vitest_conformance(spec, prepared_root, discovered, backend)
}

fn run_vue3_dom_conformance(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    discovered: &[String],
    lock_hash: Option<&str>,
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    let prepared_root = prepare_vue3_dom_conformance_suite(spec, official_root, lock_hash)?;
    run_vitest_conformance(spec, prepared_root, discovered, backend)
}

fn run_vue3_sfc_conformance(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    discovered: &[String],
    lock_hash: Option<&str>,
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    let prepared_root = prepare_vue3_sfc_conformance_suite(spec, official_root, lock_hash)?;
    run_vitest_conformance(spec, prepared_root, discovered, backend)
}

fn run_vue3_ssr_conformance(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    discovered: &[String],
    lock_hash: Option<&str>,
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    let prepared_root = prepare_vue3_ssr_conformance_suite(spec, official_root, lock_hash)?;
    run_vitest_conformance(spec, prepared_root, discovered, backend)
}

fn run_vitest_conformance(
    spec: ConformanceSuiteSpec,
    prepared_root: PathBuf,
    discovered: &[String],
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    let output_file = prepared_root.join("vitest-report.json");
    let npm_root = PathBuf::from("target")
        .join("compat")
        .join("npm")
        .join(spec.version_line.as_str());
    let alias_root = backend.root(spec.version_line);
    let absolute_npm_root = absolute_path(&npm_root);
    let absolute_alias_root = absolute_path(&alias_root);
    let absolute_prepared_root = absolute_path(&prepared_root);
    let absolute_output_file = absolute_path(&output_file);
    let absolute_bridge_bin = absolute_path(&ensure_node_bridge_binary()?);
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
        .arg("--testTimeout=30000")
        .arg("--reporter=json")
        .arg(format!("--outputFile={}", absolute_output_file.display()))
        .env("VUEC_NODE_BRIDGE", &absolute_bridge_bin)
        .env("VUEC_ALIAS_ROOT", &absolute_alias_root)
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

fn run_jasmine_conformance(
    spec: ConformanceSuiteSpec,
    prepared_root: PathBuf,
    discovered: &[String],
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    let output_file = prepared_root.join("jasmine-report.json");
    let npm_root = PathBuf::from("target")
        .join("compat")
        .join("npm")
        .join(spec.version_line.as_str());
    let alias_root = backend.root(spec.version_line);
    let absolute_npm_root = absolute_path(&npm_root);
    let absolute_alias_root = absolute_path(&alias_root);
    let absolute_prepared_root = absolute_path(&prepared_root);
    let absolute_output_file = absolute_path(&output_file);
    let absolute_bridge_bin = absolute_path(&ensure_node_bridge_binary()?);
    let node = resolve_program("node");
    let output = Command::new(node)
        .arg("vuec-jasmine-runner.js")
        .env("VUEC_NODE_BRIDGE", &absolute_bridge_bin)
        .env("VUEC_ALIAS_ROOT", &absolute_alias_root)
        .env("VUEC_RUST_ALIAS_ROOT", &absolute_alias_root)
        .env("VUEC_OFFICIAL_NPM_ROOT", &absolute_npm_root)
        .env("VUEC_JASMINE_REPORT", &absolute_output_file)
        .env(
            "NODE_PATH",
            conformance_node_path(&absolute_alias_root, &absolute_npm_root),
        )
        .current_dir(&absolute_prepared_root)
        .output()
        .with_context(|| format!("failed to spawn Jasmine for {}", spec.name))?;
    let stdout = normalize_conformance_output(&String::from_utf8_lossy(&output.stdout));
    let stderr = normalize_conformance_output(&String::from_utf8_lossy(&output.stderr));
    let counts = read_jasmine_counts(&output_file)
        .or_else(|_| read_jasmine_counts(&absolute_output_file))
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
        runner: "jasmine".into(),
        prepared_root: prepared_root.display().to_string(),
        output_file: output_file.display().to_string(),
        exit_code: output.status.code(),
        stdout,
        stderr,
        counts,
    })
}

fn prepare_vue2_compiler_conformance_suite(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    lock_hash: Option<&str>,
) -> Result<PathBuf> {
    let prepared_root = prepared_conformance_root(spec, lock_hash);
    reset_prepared_root(&prepared_root)?;
    let official_tests = official_root
        .join("test")
        .join("unit")
        .join("modules")
        .join("compiler");
    let prepared_tests = prepared_root
        .join("test")
        .join("unit")
        .join("modules")
        .join("compiler");
    copy_dir_recursive(&official_tests, &prepared_tests)?;
    write_vue2_compiler_source_shims(&prepared_root, false)?;
    write_vue2_jasmine_runner(&prepared_root)?;
    Ok(prepared_root)
}

fn prepare_vue27_compiler_conformance_suite(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    lock_hash: Option<&str>,
) -> Result<PathBuf> {
    let prepared_root = prepared_conformance_root(spec, lock_hash);
    reset_prepared_root(&prepared_root)?;
    let official_tests = official_root
        .join("test")
        .join("unit")
        .join("modules")
        .join("compiler");
    let prepared_tests = prepared_root
        .join("test")
        .join("unit")
        .join("modules")
        .join("compiler");
    copy_dir_recursive(&official_tests, &prepared_tests)?;
    write_vue2_compiler_source_shims(&prepared_root, true)?;
    write_vue27_compiler_conformance_shims(&prepared_root)?;
    Ok(prepared_root)
}

fn prepare_vue27_sfc_conformance_suite(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    lock_hash: Option<&str>,
) -> Result<PathBuf> {
    let prepared_root = prepared_conformance_root(spec, lock_hash);
    reset_prepared_root(&prepared_root)?;
    let official_tests = official_root
        .join("packages")
        .join("compiler-sfc")
        .join("test");
    let prepared_tests = prepared_root
        .join("packages")
        .join("compiler-sfc")
        .join("test");
    copy_dir_recursive(&official_tests, &prepared_tests)?;
    fs::copy(
        official_root.join("tsconfig.json"),
        prepared_root.join("tsconfig.json"),
    )
    .with_context(|| "failed to copy Vue 2.7 root tsconfig for SFC conformance")?;
    write_vue2_compiler_source_shims(&prepared_root, true)?;
    write_vue27_sfc_source_shims(&prepared_root)?;
    write_vue27_sfc_conformance_shims(&prepared_root)?;
    Ok(prepared_root)
}

fn prepare_vue3_core_conformance_suite(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    lock_hash: Option<&str>,
) -> Result<PathBuf> {
    let prepared_root = prepared_conformance_root(spec, lock_hash);
    reset_prepared_root(&prepared_root)?;
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

fn prepared_conformance_root(spec: ConformanceSuiteSpec, lock_hash: Option<&str>) -> PathBuf {
    PathBuf::from("target")
        .join("conformance")
        .join(lock_hash.unwrap_or("unknown-lock"))
        .join("prepared")
        .join(spec.name)
}

fn reset_prepared_root(prepared_root: &Path) -> Result<()> {
    if prepared_root.exists() {
        fs::remove_dir_all(prepared_root)
            .with_context(|| format!("failed to remove {}", prepared_root.display()))?;
    }
    Ok(())
}

fn write_vue2_compiler_source_shims(prepared_root: &Path, include_types: bool) -> Result<()> {
    let compiler_root = prepared_root.join("src").join("compiler");
    let parser_root = compiler_root.join("parser");
    fs::create_dir_all(&parser_root)
        .with_context(|| format!("failed to create {}", parser_root.display()))?;
    write_text(
        &parser_root.join("index.ts"),
        r#"
import { compile } from 'vue-template-compiler'

export function parse(template, options = {}) {
  const compiled = compile(template, vue2ParseBridgeOptions(options, template))
  const ast = compiled.element_public_ast || compiled.ast_public || compiled.ast || null
  if (ast && typeof ast === 'object') {
    Object.defineProperty(ast, '__vuecTemplate', { value: template, enumerable: false, configurable: true })
    Object.defineProperty(ast, '__vuecOptions', { value: options, enumerable: false, configurable: true })
    Object.defineProperty(ast, '__vuecInternal', { value: compiled.element_ast || null, enumerable: false, configurable: true })
    hydrateVue2PublicAst(ast, null, compiled.element_ast || null)
    runVue2ModuleTransforms(ast, options, 'preTransformNode')
    runVue2ModuleTransforms(ast, options, 'postTransformNode')
  }
  return ast
}

function vue2ParseBridgeOptions(options, template) {
  const hasMustUseProp = options && Object.prototype.hasOwnProperty.call(options, 'mustUseProp')
  const tags = extractVue2TemplateTags(template)
  return {
    ...normalizeVue2OptionsForBridge(options, tags, true),
    optimize: true,
    __vuecDisableDefaultMustUseProp: !hasMustUseProp,
    __vuecSuppressWarnings: ['Inline-template components must have exactly one child element.'],
  }
}

function normalizeVue2OptionsForBridge(options, tags, disableMissingPlatformOptions) {
  const normalized = {}
  if (options && typeof options === 'object') {
    for (const key of Object.keys(options)) {
      if (typeof options[key] !== 'function') normalized[key] = options[key]
    }
  }
  if (hasVue2PredicateOption(options, 'getTagNamespace')) {
    normalized.__vuecTagNamespaces = collectVue2NamespaceHits(options.getTagNamespace, tags)
    normalized.__vuecUseDefaultTagNamespaces = false
  } else if (disableMissingPlatformOptions) {
    normalized.__vuecTagNamespaces = {}
    normalized.__vuecUseDefaultTagNamespaces = false
  }
  if (hasVue2PredicateOption(options, 'isReservedTag')) {
    normalized.__vuecReservedTags = collectVue2PredicateHits(options.isReservedTag, tags)
    normalized.__vuecUseDefaultReservedTags = false
  } else if (disableMissingPlatformOptions) {
    normalized.__vuecReservedTags = []
    normalized.__vuecUseDefaultReservedTags = false
  }
  return normalized
}

function hasVue2PredicateOption(options, name) {
  return !!(options && Object.prototype.hasOwnProperty.call(options, name) &&
    (typeof options[name] === 'function' || Array.isArray(options[name])))
}

function extractVue2TemplateTags(source) {
  const tags = []
  const seen = new Set()
  const pattern = /<\/?\s*([A-Za-z][A-Za-z0-9._:-]*)/g
  let match
  while ((match = pattern.exec(String(source || '')))) {
    const tag = match[1]
    if (!seen.has(tag)) {
      seen.add(tag)
      tags.push(tag)
    }
  }
  return tags
}

function collectVue2PredicateHits(predicate, values) {
  if (Array.isArray(predicate)) return predicate.map(String)
  if (typeof predicate !== 'function') return []
  const hits = []
  for (const value of values) {
    try {
      if (predicate(value)) hits.push(value)
    } catch (_) {}
  }
  return hits
}

function collectVue2NamespaceHits(getNamespace, values) {
  if (typeof getNamespace !== 'function') return {}
  const namespaces = {}
  for (const value of values) {
    try {
      const namespace = getNamespace(value)
      if (namespace !== undefined && namespace !== null) namespaces[value] = String(namespace)
    } catch (_) {}
  }
  return namespaces
}

function runVue2ModuleTransforms(ast, options, hook) {
  if (!ast || !options || !Array.isArray(options.modules)) return
  walkVue2PublicElements(ast, element => {
    for (const module of options.modules) {
      const transform = module && module[hook]
      if (typeof transform === 'function') transform(element, options)
    }
  })
}

function walkVue2PublicElements(node, visit) {
  if (!node || typeof node !== 'object' || typeof node.tag !== 'string') return
  visit(node)
  if (Array.isArray(node.children)) {
    for (const child of node.children) walkVue2PublicElements(child, visit)
  }
  if (node.scopedSlots && typeof node.scopedSlots === 'object') {
    for (const slot of Object.values(node.scopedSlots)) walkVue2PublicElements(slot, visit)
  }
}

function hydrateVue2PublicAst(node, parent, internal) {
  if (!node || typeof node !== 'object') return node
  if (parent) {
    Object.defineProperty(node, 'parent', { value: parent, enumerable: false, configurable: true, writable: true })
  }
  if (internal) {
    Object.defineProperty(node, '__vuecInternal', { value: internal, enumerable: false, configurable: true })
  }
  const internalChildren = Array.isArray(internal && internal.children) ? internal.children : []
  if (Array.isArray(node.children)) {
    node.children.forEach((child, index) => {
      const internalChild = internalChildren[index]
      hydrateVue2PublicAst(child, node, internalChild && (internalChild.Element || internalChild.Text))
    })
  }
  const internalConditions = Array.isArray(internal && internal.if_conditions) ? internal.if_conditions : []
  if (Array.isArray(node.ifConditions)) {
    node.ifConditions.forEach((condition, index) => {
      hydrateVue2PublicAst(condition && condition.block, parent, internalConditions[index] && internalConditions[index].block)
    })
  }
  const internalSlots = internal && internal.scoped_slots && typeof internal.scoped_slots === 'object'
    ? internal.scoped_slots
    : {}
  if (node.scopedSlots && typeof node.scopedSlots === 'object') {
    for (const [name, slot] of Object.entries(node.scopedSlots)) {
      hydrateVue2PublicAst(slot, node, internalSlots[name] || internalSlots[`"${name}"`] || null)
    }
  }
  return node
}
"#,
    )?;
    write_text(
        &compiler_root.join("optimizer.ts"),
        r#"
import * as vueTemplateCompiler from 'vue-template-compiler'
import { normalizeVue2AstForBridge } from './codegen'

export function optimize(ast, options = {}) {
  if (!ast) return ast
  const optimized = vueTemplateCompiler.__vuecRuntime.callBridge('vue2.optimize', {
    ast: normalizeVue2AstForBridge(ast),
    options: vue2OptimizeBridgeOptions(ast, options),
  })
  mergeVue2OptimizedAst(ast, optimized && (optimized.element_public_ast || optimized.ast_public || optimized.ast), optimized && optimized.element_ast)
  return ast
}

function vue2OptimizeBridgeOptions(ast, options) {
  const tags = collectVue2AstTags(ast)
  return normalizeVue2OptionsForBridge(options, tags, true)
}

function normalizeVue2OptionsForBridge(options, tags, disableMissingPlatformOptions) {
  const normalized = {}
  if (options && typeof options === 'object') {
    for (const key of Object.keys(options)) {
      if (typeof options[key] !== 'function') normalized[key] = options[key]
    }
  }
  if (hasVue2PredicateOption(options, 'getTagNamespace')) {
    normalized.__vuecTagNamespaces = collectVue2NamespaceHits(options.getTagNamespace, tags)
    normalized.__vuecUseDefaultTagNamespaces = false
  } else if (disableMissingPlatformOptions) {
    normalized.__vuecTagNamespaces = {}
    normalized.__vuecUseDefaultTagNamespaces = false
  }
  if (hasVue2PredicateOption(options, 'isReservedTag')) {
    normalized.__vuecReservedTags = collectVue2PredicateHits(options.isReservedTag, tags)
    normalized.__vuecUseDefaultReservedTags = false
  } else if (disableMissingPlatformOptions) {
    normalized.__vuecReservedTags = []
    normalized.__vuecUseDefaultReservedTags = false
  }
  return normalized
}

function hasVue2PredicateOption(options, name) {
  return !!(options && Object.prototype.hasOwnProperty.call(options, name) &&
    (typeof options[name] === 'function' || Array.isArray(options[name])))
}

function collectVue2PredicateHits(predicate, values) {
  if (Array.isArray(predicate)) return predicate.map(String)
  if (typeof predicate !== 'function') return []
  const hits = []
  for (const value of values) {
    try {
      if (predicate(value)) hits.push(value)
    } catch (_) {}
  }
  return hits
}

function collectVue2NamespaceHits(getNamespace, values) {
  if (typeof getNamespace !== 'function') return {}
  const namespaces = {}
  for (const value of values) {
    try {
      const namespace = getNamespace(value)
      if (namespace !== undefined && namespace !== null) namespaces[value] = String(namespace)
    } catch (_) {}
  }
  return namespaces
}

function collectVue2AstTags(ast) {
  const tags = []
  const seen = new Set()
  walkVue2AstElements(ast, element => {
    const tag = String(element.tag || '')
    if (tag && !seen.has(tag)) {
      seen.add(tag)
      tags.push(tag)
    }
  })
  return tags
}

function walkVue2AstElements(node, visit) {
  if (!node || typeof node !== 'object') return
  if ('Element' in node) return walkVue2AstElements(node.Element, visit)
  if (typeof node.tag === 'string') {
    visit(node)
    if (Array.isArray(node.children)) {
      for (const child of node.children) walkVue2AstElements(child && (child.Element || child), visit)
    }
    const conditions = node.ifConditions || node.if_conditions
    if (Array.isArray(conditions)) {
      for (const condition of conditions) walkVue2AstElements(condition && condition.block, visit)
    }
    const scopedSlots = node.scopedSlots || node.scoped_slots
    if (scopedSlots && typeof scopedSlots === 'object') {
      for (const slot of Object.values(scopedSlots)) walkVue2AstElements(slot, visit)
    }
  }
}

function mergeVue2OptimizedAst(target, publicNode, internalNode) {
  if (!target || typeof target !== 'object') return
  if (internalNode) {
    Object.defineProperty(target, '__vuecInternal', { value: internalNode, enumerable: false, configurable: true })
  }
  if (publicNode && typeof publicNode === 'object') {
    target.static = Boolean(publicNode.static)
    target.staticRoot = Boolean(publicNode.staticRoot)
    target.staticInFor = Boolean(publicNode.staticInFor)
  } else if (internalNode && typeof internalNode === 'object') {
    target.static_node = Boolean(internalNode.static_node)
    target.static_root = Boolean(internalNode.static_root)
    target.static_in_for = Boolean(internalNode.static_in_for)
  }
  const targetChildren = Array.isArray(target.children) ? target.children : []
  const publicChildren = Array.isArray(publicNode && publicNode.children) ? publicNode.children : []
  const internalChildren = Array.isArray(internalNode && internalNode.children) ? internalNode.children : []
  targetChildren.forEach((child, index) => {
    const internalChild = internalChildren[index]
    mergeVue2OptimizedAst(child && (child.Element || child), publicChildren[index], internalChild && (internalChild.Element || internalChild.Text))
  })
  const targetConditions = target.ifConditions || target.if_conditions
  const publicConditions = publicNode && (publicNode.ifConditions || publicNode.if_conditions)
  const internalConditions = internalNode && internalNode.if_conditions
  if (Array.isArray(targetConditions)) {
    targetConditions.forEach((condition, index) => {
      mergeVue2OptimizedAst(
        condition && condition.block,
        publicConditions && publicConditions[index] && publicConditions[index].block,
        internalConditions && internalConditions[index] && internalConditions[index].block,
      )
    })
  }
  const targetSlots = target.scopedSlots || target.scoped_slots
  const publicSlots = publicNode && (publicNode.scopedSlots || publicNode.scoped_slots)
  const internalSlots = internalNode && internalNode.scoped_slots
  if (targetSlots && typeof targetSlots === 'object') {
    for (const [key, slot] of Object.entries(targetSlots)) {
      mergeVue2OptimizedAst(slot, publicSlots && (publicSlots[key] || publicSlots[`"${key}"`]), internalSlots && (internalSlots[key] || internalSlots[`"${key}"`]))
    }
  }
}
"#,
    )?;
    write_text(
        &compiler_root.join("codegen.ts"),
        r#"
import * as vueTemplateCompiler from 'vue-template-compiler'

export function generate(ast, options = {}) {
  const generated = vueTemplateCompiler.__vuecRuntime.callBridge('vue2.generate', {
    ast: normalizeVue2AstForBridge(ast),
    options,
  })
  emitVue2InlineTemplateWarnings(ast)
  return {
    render: generated.render,
    staticRenderFns: generated.staticRenderFns || generated.static_render_fns || [],
  }
}

export function normalizeVue2AstForBridge(node) {
  if (!node || typeof node !== 'object') return null
  if ('Element' in node) return normalizeVue2AstForBridge(node.Element)
  if (isInternalVue2ElementAst(node)) return normalizeVue2InternalAstForBridge(node)
  return normalizeVue2PublicElementForBridge(node)
}

function isInternalVue2ElementAst(node) {
  return !!(node && typeof node === 'object' && (
    Object.prototype.hasOwnProperty.call(node, 'attrs_list') ||
    Object.prototype.hasOwnProperty.call(node, 'static_node') ||
    Object.prototype.hasOwnProperty.call(node, 'if_conditions')
  ))
}

function normalizeVue2InternalAstForBridge(node) {
  if (!node || typeof node !== 'object') return null
  const copy = {}
  for (const key of Object.keys(node)) {
    if (key === 'parent' || key.startsWith('__vuec')) continue
    copy[key] = node[key]
  }
  normalizeVue2InternalEventsForBridge(copy.events)
  normalizeVue2InternalEventsForBridge(copy.native_events)
  if (Array.isArray(copy.children)) {
    copy.children = copy.children.map(normalizeVue2InternalNodeForBridge)
  }
  if (copy.scoped_slots && typeof copy.scoped_slots === 'object') {
    copy.scoped_slots = Object.fromEntries(
      Object.entries(copy.scoped_slots).map(([key, value]) => [key, normalizeVue2AstForBridge(value)])
    )
  }
  if (Array.isArray(copy.if_conditions)) {
    copy.if_conditions = copy.if_conditions.map(condition => ({
      ...condition,
      block: normalizeVue2AstForBridge(condition && condition.block),
    }))
  }
  return copy
}

function normalizeVue2InternalNodeForBridge(node) {
  if (!node || typeof node !== 'object') return node
  if ('Element' in node) return { Element: normalizeVue2AstForBridge(node.Element) }
  if ('Text' in node) return { Text: normalizeVue2TextForBridge(node.Text) }
  return normalizeVue2PublicNodeForBridge(node)
}

function normalizeVue2PublicElementForBridge(node) {
  const scopedSlots = normalizeVue2ScopedSlotsForBridge(node.scopedSlots || node.scoped_slots)
  return {
    tag: String(node.tag || ''),
    attrs_list: normalizeVue2RawAttrsForBridge(node.attrsList || node.attrs_list),
    raw_attrs_list: normalizeVue2RawAttrsForBridge(node.attrsList || node.raw_attrs_list || node.attrs_list),
    attrs_map: normalizeVue2AttrsMapForBridge(node.attrsMap || node.attrs_map, node.attrsList || node.attrs_list),
    raw_attrs_map: normalizeVue2RawAttrsMapForBridge(node.rawAttrsMap || node.raw_attrs_map, node.attrsList || node.attrs_list),
    attrs: normalizeVue2AttrsForBridge(node.attrs),
    props: normalizeVue2AttrsForBridge(node.props),
    dynamic_attrs: normalizeVue2AttrsForBridge(node.dynamicAttrs || node.dynamic_attrs),
    directives: normalizeVue2DirectivesForBridge(node.directives),
    events: normalizeVue2EventsForBridge(node.events),
    native_events: normalizeVue2EventsForBridge(node.nativeEvents || node.native_events),
    children: Array.isArray(node.children) ? node.children.map(normalizeVue2PublicNodeForBridge) : [],
    ns: node.ns,
    plain: Boolean(node.plain),
    forbidden: Boolean(node.forbidden),
    pre: Boolean(node.pre),
    once: Boolean(node.once),
    has_bindings: Boolean(node.hasBindings || node.has_bindings),
    if_exp: node.if ?? node.if_exp,
    elseif: node.elseif,
    else_branch: Boolean(node.else || node.else_branch),
    if_conditions: Array.isArray(node.ifConditions || node.if_conditions)
      ? (node.ifConditions || node.if_conditions).map(condition => ({
          exp: condition && condition.exp,
          block: normalizeVue2AstForBridge(condition && condition.block),
        }))
      : [],
    for_exp: node.for ?? node.for_exp,
    alias: node.alias,
    iterator1: node.iterator1,
    iterator2: node.iterator2,
    key: node.key,
    ref_name: node.ref ?? node.ref_name,
    ref_in_for: Boolean(node.refInFor || node.ref_in_for),
    slot_name: node.slotName ?? node.slot_name,
    slot_target: node.slotTarget ?? node.slot_target,
    slot_target_dynamic: Boolean(node.slotTargetDynamic || node.slot_target_dynamic),
    slot_scope: node.slotScope ?? node.slot_scope,
    slot_new_syntax: Boolean(node.slotNewSyntax || node.slot_new_syntax),
    scoped_slots: scopedSlots,
    component: node.component,
    inline_template: Boolean(node.inlineTemplate || node.inline_template),
    static_class: node.staticClass ?? node.static_class,
    class_binding: node.classBinding ?? node.class_binding,
    static_style: node.staticStyle ?? node.static_style,
    style_binding: node.styleBinding ?? node.style_binding,
    model: node.model,
    wrap_data: node.wrapData ?? node.wrap_data,
    wrap_listeners: node.wrapListeners ?? node.wrap_listeners,
    validate: node.validate,
    validators: Array.isArray(node.validators) ? node.validators : [],
    static_node: Boolean(node.static ?? node.static_node),
    static_root: Boolean(node.staticRoot ?? node.static_root),
    static_in_for: Boolean(node.staticInFor ?? node.static_in_for),
  }
}

function normalizeVue2NodeForBridge(node) {
  if (!node || typeof node !== 'object') return node
  if ('Element' in node) return { Element: normalizeVue2AstForBridge(node.Element) }
  if ('Text' in node) return { Text: normalizeVue2TextForBridge(node.Text) }
  return normalizeVue2PublicNodeForBridge(node)
}

function normalizeVue2EventsForBridge(events) {
  if (!events || typeof events !== 'object') return {}
  const normalized = {}
  for (const key of Object.keys(events)) {
    const value = events[key]
    if (value === undefined) {
      normalized[key] = []
    } else if (Array.isArray(value)) {
      normalized[key] = value.map(normalizeVue2EventHandlerForBridge)
    } else {
      normalized[key] = [normalizeVue2EventHandlerForBridge(value)]
    }
  }
  return normalized
}

function normalizeVue2InternalEventsForBridge(events) {
  if (!events || typeof events !== 'object') return
  for (const key of Object.keys(events)) {
    if (events[key] === undefined) events[key] = []
  }
}

function normalizeVue2PublicNodeForBridge(node) {
  if (!node || typeof node !== 'object') return node
  if ('Element' in node) return { Element: normalizeVue2AstForBridge(node.Element) }
  if ('Text' in node) return { Text: normalizeVue2TextForBridge(node.Text) }
  if (node.type === 1 || typeof node.tag === 'string') {
    return { Element: normalizeVue2AstForBridge(node) }
  }
  return { Text: normalizeVue2TextForBridge(node) }
}

function normalizeVue2TextForBridge(node) {
  const expression = node && Object.prototype.hasOwnProperty.call(node, 'expression')
    ? node.expression
    : null
  return {
    text: String((node && node.text) || ''),
    expression,
    is_comment: Boolean(node && (node.isComment || node.is_comment)),
    static_node: Boolean(node && (node.static ?? node.static_node)),
  }
}

function normalizeVue2RawAttrsForBridge(attrs) {
  if (!Array.isArray(attrs)) return []
  return attrs.map(attr => ({
    name: String((attr && attr.name) || ''),
    value: String((attr && attr.value) || ''),
    dynamic: Boolean(attr && attr.dynamic),
  }))
}

function normalizeVue2AttrsForBridge(attrs) {
  if (!Array.isArray(attrs)) return []
  return attrs.map(attr => ({
    name: String((attr && attr.name) || ''),
    value: String((attr && attr.value) || ''),
    dynamic: Boolean(attr && attr.dynamic),
  }))
}

function normalizeVue2AttrsMapForBridge(attrsMap, attrsList) {
  if (attrsMap && typeof attrsMap === 'object') return { ...attrsMap }
  return Object.fromEntries(normalizeVue2RawAttrsForBridge(attrsList).map(attr => [attr.name, attr.value]))
}

function normalizeVue2RawAttrsMapForBridge(rawAttrsMap, attrsList) {
  if (rawAttrsMap && typeof rawAttrsMap === 'object') {
    return Object.fromEntries(
      Object.entries(rawAttrsMap).map(([key, attr]) => [key, {
        name: String((attr && attr.name) || key),
        value: String((attr && attr.value) || ''),
        dynamic: Boolean(attr && attr.dynamic),
      }])
    )
  }
  return Object.fromEntries(normalizeVue2RawAttrsForBridge(attrsList).map(attr => [attr.name, attr]))
}

function normalizeVue2DirectivesForBridge(directives) {
  if (!Array.isArray(directives)) return []
  return directives.map(directive => ({
    name: String((directive && directive.name) || ''),
    raw_name: String((directive && (directive.rawName || directive.raw_name)) || ''),
    value: directive && directive.value,
    arg: directive && directive.arg,
    is_dynamic_arg: Boolean(directive && (directive.isDynamicArg || directive.is_dynamic_arg)),
    modifiers: directive && directive.modifiers && typeof directive.modifiers === 'object' ? { ...directive.modifiers } : {},
  }))
}

function normalizeVue2EventHandlerForBridge(handler) {
  if (!handler || typeof handler !== 'object') {
    return {
      value: handler == null ? '' : String(handler),
      modifiers: {},
      modifier_order: [],
      has_modifier_object: false,
      dynamic: false,
    }
  }
  const modifiers = handler.modifiers && typeof handler.modifiers === 'object' ? { ...handler.modifiers } : {}
  return {
    value: String(handler.value || ''),
    modifiers,
    modifier_order: Array.isArray(handler.modifierOrder || handler.modifier_order)
      ? (handler.modifierOrder || handler.modifier_order).map(String)
      : Object.keys(modifiers),
    has_modifier_object: Boolean(handler.hasModifierObject || handler.has_modifier_object || Object.keys(modifiers).length > 0),
    dynamic: Boolean(handler.dynamic),
  }
}

function normalizeVue2ScopedSlotsForBridge(scopedSlots) {
  if (!scopedSlots || typeof scopedSlots !== 'object') return {}
  return Object.fromEntries(
    Object.entries(scopedSlots).map(([key, slot]) => {
      const normalized = normalizeVue2AstForBridge(slot)
      return [normalized.slot_target || quoteVue2SlotKeyForBridge(key, normalized.slot_target_dynamic), normalized]
    })
  )
}

function quoteVue2SlotKeyForBridge(key, dynamic) {
  if (dynamic || key.startsWith('"') || key.startsWith("'")) return key
  return JSON.stringify(key)
}

function emitVue2InlineTemplateWarnings(node) {
  if (!node || typeof node !== 'object') return
  if ((node.inline_template || node.inlineTemplate) && (!Array.isArray(node.children) || node.children.length !== 1)) {
    console.error('Inline-template components must have exactly one child element.')
  }
  if (Array.isArray(node.children)) {
    for (const child of node.children) {
      emitVue2InlineTemplateWarnings(child && child.Element ? child.Element : child)
    }
  }
  const scopedSlots = node.scoped_slots || node.scopedSlots
  if (scopedSlots && typeof scopedSlots === 'object') {
    for (const slot of Object.values(scopedSlots)) emitVue2InlineTemplateWarnings(slot)
  }
  const ifConditions = node.if_conditions || node.ifConditions
  if (Array.isArray(ifConditions)) {
    for (const condition of ifConditions.slice(1)) {
      emitVue2InlineTemplateWarnings(condition && condition.block)
    }
  }
}
"#,
    )?;
    write_text(
        &compiler_root.join("codeframe.ts"),
        r#"
import { generateCodeFrame } from 'vue-template-compiler'
export { generateCodeFrame }
"#,
    )?;
    write_text(
        &compiler_root.join("helpers.ts"),
        r#"
export function getAndRemoveAttr(el, name) {
  if (!el || !el.attrsMap || !(name in el.attrsMap)) return undefined
  const value = el.attrsMap[name]
  delete el.attrsMap[name]
  if (Array.isArray(el.attrsList)) {
    const index = el.attrsList.findIndex(attr => attr && attr.name === name)
    if (index >= 0) el.attrsList.splice(index, 1)
  }
  return value
}
"#,
    )?;

    let web_compiler = prepared_root
        .join("src")
        .join("platforms")
        .join("web")
        .join("compiler");
    fs::create_dir_all(&web_compiler)
        .with_context(|| format!("failed to create {}", web_compiler.display()))?;
    write_text(
        &web_compiler.join("index.ts"),
        r#"
import { compile } from 'vue-template-compiler'
export { compile }
"#,
    )?;
    write_text(
        &web_compiler.join("options.ts"),
        r#"
export const baseOptions = {
  expectHTML: true,
  modules: [],
  directives: {},
  isPreTag: tag => tag === 'pre',
  isUnaryTag: tag => /^(area|base|br|col|embed|hr|img|input|link|meta|param|source|track|wbr)$/i.test(tag),
  mustUseProp: () => false,
  canBeLeftOpenTag: () => false,
  isReservedTag: tag => /^(html|body|base|head|link|meta|style|title|address|article|aside|footer|header|h1|h2|h3|h4|h5|h6|nav|section|div|dd|dl|dt|figcaption|figure|picture|hr|img|li|main|ol|p|pre|ul|a|b|abbr|bdi|bdo|br|cite|code|data|dfn|em|i|kbd|mark|q|rp|rt|rtc|ruby|s|samp|small|span|strong|sub|sup|time|u|var|wbr|area|audio|map|track|video|embed|object|param|source|canvas|script|noscript|del|ins|caption|col|colgroup|table|thead|tbody|td|th|tr|button|datalist|fieldset|form|input|label|legend|meter|optgroup|option|output|progress|select|textarea|details|dialog|menu|summary|template|blockquote|iframe|tfoot)$/i.test(tag),
  getTagNamespace: tag => tag === 'svg' ? 'svg' : undefined,
  staticKeys: '',
}
"#,
    )?;

    let web_util = prepared_root
        .join("src")
        .join("platforms")
        .join("web")
        .join("util");
    fs::create_dir_all(&web_util)
        .with_context(|| format!("failed to create {}", web_util.display()))?;
    write_text(
        &web_util.join("index.ts"),
        r#"
export const isReservedTag = tag => /^(html|body|base|head|link|meta|style|title|address|article|aside|footer|header|h1|h2|h3|h4|h5|h6|nav|section|div|dd|dl|dt|figcaption|figure|picture|hr|img|li|main|ol|p|pre|ul|a|b|abbr|bdi|bdo|br|cite|code|data|dfn|em|i|kbd|mark|q|rp|rt|rtc|ruby|s|samp|small|span|strong|sub|sup|time|u|var|wbr|area|audio|map|track|video|embed|object|param|source|canvas|script|noscript|del|ins|caption|col|colgroup|table|thead|tbody|td|th|tr|button|datalist|fieldset|form|input|label|legend|meter|optgroup|option|output|progress|select|textarea|details|dialog|menu|summary|template|blockquote|iframe|tfoot)$/i.test(tag)
"#,
    )?;

    let shared = prepared_root.join("src").join("shared");
    fs::create_dir_all(&shared)
        .with_context(|| format!("failed to create {}", shared.display()))?;
    write_text(
        &shared.join("util.ts"),
        r#"
export const isObject = value => value !== null && typeof value === 'object'
export const isFunction = value => typeof value === 'function'
export function extend(to, from) {
  return Object.assign(to, from)
}
export const noop = () => {}
"#,
    )?;

    let core_util = prepared_root.join("src").join("core").join("util");
    fs::create_dir_all(&core_util)
        .with_context(|| format!("failed to create {}", core_util.display()))?;
    write_text(
        &core_util.join("env.ts"),
        r#"
export const isIE = false
export const isEdge = false
"#,
    )?;

    let web_entry = prepared_root.join("src").join("platforms").join("web");
    write_text(
        &web_entry.join("entry-compiler.ts"),
        r#"
import Vue from 'vue'
export default Vue
export * from './compiler'
"#,
    )?;

    if include_types {
        let types_root = prepared_root.join("src").join("types");
        fs::create_dir_all(&types_root)
            .with_context(|| format!("failed to create {}", types_root.display()))?;
        write_text(
            &types_root.join("compiler.ts"),
            "export const WarningMessage = String\n",
        )?;
        let sfc_src = prepared_root
            .join("packages")
            .join("compiler-sfc")
            .join("src");
        fs::create_dir_all(&sfc_src)
            .with_context(|| format!("failed to create {}", sfc_src.display()))?;
        write_text(
            &sfc_src.join("types.ts"),
            r#"
export const BindingTypes = {
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
}
"#,
        )?;
    }

    Ok(())
}

fn write_vue27_sfc_source_shims(prepared_root: &Path) -> Result<()> {
    let sfc_src = prepared_root
        .join("packages")
        .join("compiler-sfc")
        .join("src");
    fs::create_dir_all(&sfc_src)
        .with_context(|| format!("failed to create {}", sfc_src.display()))?;
    write_text(
        &sfc_src.join("index.ts"),
        "export * from 'vue/compiler-sfc'\n",
    )?;
    for module in [
        "parse",
        "parseComponent",
        "compileTemplate",
        "compileScript",
        "compileStyle",
        "cssVars",
        "rewriteDefault",
    ] {
        write_text(
            &sfc_src.join(format!("{module}.ts")),
            "export * from 'vue/compiler-sfc'\n",
        )?;
    }
    write_text(
        &sfc_src.join("prefixIdentifiers.ts"),
        &vue27_sfc_prefix_identifiers_source_shim(),
    )?;
    Ok(())
}

fn vue27_sfc_prefix_identifiers_source_shim() -> String {
    let bridge_path = PathBuf::from("target")
        .join("debug")
        .join(if cfg!(windows) {
            "vuec_node_bridge.exe"
        } else {
            "vuec_node_bridge"
        });
    format!(
        r#"
import cp from 'node:child_process'
import path from 'node:path'

const bridgeBin = process.env.VUEC_NODE_BRIDGE || path.resolve(process.cwd(), {})

function callBridge(command, payload) {{
  const result = cp.spawnSync(bridgeBin, [command], {{
    input: JSON.stringify(payload || {{}}),
    encoding: 'utf8'
  }})
  if (result.error) throw result.error
  if (result.status !== 0) {{
    const error = new Error(result.stderr || result.stdout || `vuec bridge command failed: ${{command}}`)
    ;(error as any).code = 'VUEC_BRIDGE_FAILED'
    throw error
  }}
  return result.stdout.trim() ? JSON.parse(result.stdout) : undefined
}}

export function prefixIdentifiers(source, isFunctional = false, isTS = false, babelOptions = {{}}, bindings) {{
  return callBridge('sfc.vue27.prefixIdentifiers', {{
    source: source == null ? '' : String(source),
    isFunctional: !!isFunctional,
    isTS: !!isTS,
    babelOptions: babelOptions || {{}},
    bindings: bindings || {{}}
  }})
}}
"#,
        js_string_literal(&bridge_path.to_string_lossy())
    )
}

fn write_vue27_compiler_conformance_shims(prepared_root: &Path) -> Result<()> {
    write_vue2_vitest_setup(prepared_root)?;
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
  },
  resolve: {
    alias: {
      compiler: path.resolve(root, 'src/compiler'),
      core: path.resolve(root, 'src/core'),
      shared: path.resolve(root, 'src/shared'),
      web: path.resolve(root, 'src/platforms/web'),
      types: path.resolve(root, 'src/types'),
      vue: path.resolve(npmRoot, 'node_modules/vue/dist/vue.common.js'),
      vitest: path.resolve(npmRoot, 'node_modules/vitest/dist/index.js'),
      'vue-template-compiler': path.resolve(aliasRoot, 'node_modules/vue-template-compiler/index.js'),
    },
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./vuec-vitest-setup.ts'],
    include: ['test/unit/modules/compiler/**/*.spec.ts'],
  },
}
"#;
    write_text(&prepared_root.join("vitest.config.ts"), config)
}

fn write_vue27_sfc_conformance_shims(prepared_root: &Path) -> Result<()> {
    write_vue2_vitest_setup(prepared_root)?;
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
  },
  resolve: {
    alias: {
      compiler: path.resolve(root, 'src/compiler'),
      core: path.resolve(root, 'src/core'),
      shared: path.resolve(root, 'src/shared'),
      web: path.resolve(root, 'src/platforms/web'),
      types: path.resolve(root, 'src/types'),
      vitest: path.resolve(npmRoot, 'node_modules/vitest/dist/index.js'),
      'vue/compiler-sfc': path.resolve(aliasRoot, 'node_modules/vue/compiler-sfc/index.js'),
      vue: path.resolve(npmRoot, 'node_modules/vue/dist/vue.common.js'),
      'vue-template-compiler': path.resolve(aliasRoot, 'node_modules/vue-template-compiler/index.js'),
      '@babel/parser': path.resolve(npmRoot, 'node_modules/@babel/parser/lib/index.js'),
      postcss: path.resolve(npmRoot, 'node_modules/postcss/lib/postcss.mjs'),
      prettier: path.resolve(npmRoot, 'node_modules/prettier/index.js'),
      typescript: path.resolve(npmRoot, 'node_modules/typescript/lib/typescript.js'),
    },
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./vuec-vitest-setup.ts'],
    include: ['packages/compiler-sfc/test/**/*.spec.ts'],
  },
}
"#;
    write_text(&prepared_root.join("vitest.config.ts"), config)
}

fn write_vue2_vitest_setup(prepared_root: &Path) -> Result<()> {
    write_text(
        &prepared_root.join("vuec-vitest-setup.ts"),
        r#"
import { beforeEach, expect } from 'vitest'

const warnings: string[] = []
const warnMock = (...args: unknown[]) => {
  warnings.push(args.map(arg => String(arg)).join(' '))
}
;(warnMock as any).mock = { calls: [] as unknown[][] }

beforeEach(() => {
  warnings.length = 0
  ;(warnMock as any).mock.calls.length = 0
})

console.warn = (...args: unknown[]) => {
  ;(warnMock as any).mock.calls.push(args)
  warnMock(...args)
}
console.error = console.warn
;(console.error as any).mock = (warnMock as any).mock

expect.extend({
  toHaveBeenWarned(received) {
    const expected = String(received)
    const pass = warnings.some(warning => warning.includes(expected))
    return {
      pass,
      message: () => `expected ${JSON.stringify(expected)} ${pass ? 'not ' : ''}to have been warned`,
    }
  },
})
"#,
    )
}

fn write_vue2_jasmine_runner(prepared_root: &Path) -> Result<()> {
    write_text(
        &prepared_root.join("vuec-jasmine-runner.js"),
        r#"
const fs = require('fs')
const path = require('path')
const Module = require('module')
const Jasmine = require('jasmine')
const { JSDOM } = require('jsdom')

const dom = new JSDOM('<!doctype html><html><body></body></html>')
global.window = dom.window
global.document = dom.window.document
global.navigator = dom.window.navigator

function vuecInteropDefault(value) {
  return value && Object.prototype.hasOwnProperty.call(value, 'default') ? value.default : value
}
globalThis.__vuecInteropDefault = vuecInteropDefault

require('@babel/register')({
  cache: false,
  extensions: ['.js', '.ts'],
  ignore: [/node_modules/],
  plugins: [
    function vuecModuleToCommonJs() {
      return {
        visitor: {
          ImportDeclaration(path) {
            const t = require('@babel/core').types
            const source = path.node.source
            const statements = []
            for (const spec of path.node.specifiers) {
              if (t.isImportDefaultSpecifier(spec)) {
                statements.push(t.variableDeclaration('const', [
                  t.variableDeclarator(spec.local, t.callExpression(t.memberExpression(t.identifier('globalThis'), t.identifier('__vuecInteropDefault')), [t.callExpression(t.identifier('require'), [source])])),
                ]))
              } else if (t.isImportNamespaceSpecifier(spec)) {
                statements.push(t.variableDeclaration('const', [
                  t.variableDeclarator(spec.local, t.callExpression(t.identifier('require'), [source])),
                ]))
              } else if (t.isImportSpecifier(spec)) {
                statements.push(t.variableDeclaration('const', [
                  t.variableDeclarator(
                    t.objectPattern([t.objectProperty(spec.imported, spec.local, false, spec.imported.name === spec.local.name)]),
                    t.callExpression(t.identifier('require'), [source])
                  ),
                ]))
              }
            }
            path.replaceWithMultiple(statements.length ? statements : [t.expressionStatement(t.callExpression(t.identifier('require'), [source]))])
          },
          ExportNamedDeclaration(path) {
            const t = require('@babel/core').types
            const node = path.node
            const statements = []
            if (node.declaration) {
              const decl = node.declaration
              statements.push(decl)
              if (t.isFunctionDeclaration(decl) || t.isClassDeclaration(decl)) {
                statements.push(t.expressionStatement(t.assignmentExpression('=', t.memberExpression(t.identifier('exports'), decl.id), decl.id)))
              } else if (t.isVariableDeclaration(decl)) {
                for (const d of decl.declarations) {
                  if (t.isIdentifier(d.id)) statements.push(t.expressionStatement(t.assignmentExpression('=', t.memberExpression(t.identifier('exports'), d.id), d.id)))
                }
              }
            }
            for (const spec of node.specifiers || []) {
              statements.push(t.expressionStatement(t.assignmentExpression('=', t.memberExpression(t.identifier('exports'), spec.exported), spec.local)))
            }
            path.replaceWithMultiple(statements)
          },
        },
      }
    },
  ],
})

const root = __dirname
const aliasRoot = process.env.VUEC_RUST_ALIAS_ROOT
const npmRoot = process.env.VUEC_OFFICIAL_NPM_ROOT
const reportPath = process.env.VUEC_JASMINE_REPORT || path.join(root, 'jasmine-report.json')
const originalResolve = Module._resolveFilename
Module._resolveFilename = function(request, parent, isMain, options) {
  const aliases = {
    compiler: path.join(root, 'src/compiler'),
    core: path.join(root, 'src/core'),
    shared: path.join(root, 'src/shared'),
    web: path.join(root, 'src/platforms/web'),
    types: path.join(root, 'src/types'),
    vue: path.join(npmRoot, 'node_modules/vue/dist/vue.common.js'),
    'vue-template-compiler': path.join(aliasRoot, 'node_modules/vue-template-compiler/index.js'),
  }
  for (const [key, target] of Object.entries(aliases)) {
    if (request === key) return originalResolve.call(this, target, parent, isMain, options)
    if (request.startsWith(key + '/')) {
      return originalResolve.call(this, path.join(target, request.slice(key.length + 1)), parent, isMain, options)
    }
  }
  return originalResolve.call(this, request, parent, isMain, options)
}

const warnings = []
global.__VUEC_WARNINGS__ = warnings
console.error = (...args) => {
  warnings.push(args.map(String).join(' '))
}
console.warn = console.error
console.error.calls = {
  count() {
    return warnings.length
  },
  argsFor(index) {
    const warning = warnings[index]
    return warning == null ? [] : [warning]
  },
}

fs.writeFileSync(path.join(root, 'vuec-jasmine-helper.js'), `
const warnings = global.__VUEC_WARNINGS__ || []
beforeEach(() => {
  warnings.length = 0
  jasmine.addMatchers({
    toHaveBeenWarned() {
      return {
        compare(actual) {
          const expected = String(actual)
          const pass = warnings.some(warning => warning.includes(expected))
          return {
            pass,
            message: pass
              ? 'expected ' + JSON.stringify(expected) + ' not to have been warned'
              : 'expected ' + JSON.stringify(expected) + ' to have been warned',
          }
        }
      }
    }
  })
})
`)

const jasmine = new Jasmine()
const specFiles = [
  'codeframe.spec.js',
  'codegen.spec.js',
  'compiler-options.spec.js',
  'optimizer.spec.js',
  'parser.spec.js',
].map(file => path.join(root, 'test/unit/modules/compiler', file))
jasmine.loadConfig({
  spec_dir: root,
  spec_files: [],
  helpers: [path.join(root, 'vuec-jasmine-helper.js')],
  random: false,
})
for (const file of specFiles) {
  jasmine.addSpecFile(file)
}

function normalizedPath(file) {
  return path.resolve(file).replace(/\\/g, '/')
}

const normalizedSpecFiles = specFiles.map(normalizedPath)
const specFileById = new Map()
const originalIt = global.it
global.it = function() {
  const stack = String(new Error().stack || '').replace(/\\/g, '/')
  const sourceFile = normalizedSpecFiles.find(file => stack.includes(file)) || '<unknown>'
  const spec = originalIt.apply(this, arguments)
  if (spec && spec.id) specFileById.set(spec.id, sourceFile)
  return spec
}

const testResultsByFile = new Map()
function fileResult(file) {
  if (!testResultsByFile.has(file)) {
    testResultsByFile.set(file, { name: file, assertionResults: [] })
  }
  return testResultsByFile.get(file)
}

function reportStatus(status) {
  if (status === 'passed') return 'passed'
  if (status === 'failed') return 'failed'
  if (status === 'pending' || status === 'disabled' || status === 'excluded') return 'skipped'
  return 'pending'
}

const counts = { total: 0, pass: 0, fail: 0, skip: 0, pending: 0 }
jasmine.addReporter({
  specDone(result) {
    counts.total += 1
    if (result.status === 'passed') counts.pass += 1
    else if (result.status === 'failed') counts.fail += 1
    else if (result.status === 'pending' || result.status === 'disabled' || result.status === 'excluded') counts.skip += 1
    else counts.pending += 1
    const sourceFile = specFileById.get(result.id) || '<unknown>'
    fileResult(sourceFile).assertionResults.push({
      title: result.fullName || result.description || '',
      status: reportStatus(result.status),
      failureMessages: (result.failedExpectations || []).map(expectation => expectation.message || '').filter(Boolean),
    })
  },
  jasmineDone() {
    counts.pending = Math.max(0, counts.total - counts.pass - counts.fail - counts.skip)
    fs.writeFileSync(reportPath, JSON.stringify({ counts, testResults: Array.from(testResultsByFile.values()) }, null, 2))
  },
})

jasmine.execute()
"#,
    )
}

fn write_vue3_core_conformance_shims(prepared_root: &Path) -> Result<()> {
    write_vue3_core_source_shims(prepared_root)?;
    write_vue3_core_test_setup(prepared_root)?;
    rewrite_vue3_core_v_bind_public_api_spec(prepared_root)?;
    rewrite_vue3_core_v_model_public_api_spec(prepared_root)?;
    rewrite_vue3_core_v_on_public_api_spec(prepared_root)?;
    rewrite_vue3_core_v_for_public_api_spec(prepared_root)?;
    rewrite_vue3_core_transform_element_public_api_spec(prepared_root)?;
    rewrite_vue3_core_transform_public_api_spec(prepared_root)?;
    rewrite_vue3_core_v_if_public_api_spec(prepared_root)?;
    rewrite_vue3_core_transform_slot_outlet_public_api_spec(prepared_root)?;
    rewrite_vue3_core_v_slot_public_api_spec(prepared_root)?;
    rewrite_vue3_core_cache_static_public_api_spec(prepared_root)?;
    rewrite_vue3_core_transform_expressions_public_api_spec(prepared_root)?;
    rewrite_vue3_core_transform_text_public_api_spec(prepared_root)?;
    rewrite_vue3_core_v_once_public_api_spec(prepared_root)?;

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
    setupFiles: ['./vuec-vitest-setup.ts'],
    include: ['packages/compiler-core/__tests__/**/*.spec.ts'],
  },
}
"#;
    write_text(&prepared_root.join("vitest.config.ts"), config)?;
    Ok(())
}

fn rewrite_vue3_core_v_bind_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("vBind.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  type CallExpression,
  type CompilerOptions,
  type ElementNode,
  ErrorCodes,
  NodeTypes,
  type ObjectExpression,
  type VNodeCall,
  baseParse as parse,
  transform,
} from '../../src'
import { transformBind } from '../../src/transforms/vBind'
import { transformElement } from '../../src/transforms/transformElement'
import {
  CAMELIZE,
  NORMALIZE_PROPS,
  helperNameMap,
} from '../../src/runtimeHelpers'
import { transformExpression } from '../../src/transforms/transformExpression'
import { transformVBindShorthand } from '../../src/transforms/transformVBindShorthand'"#,
        r#"import {
  type CallExpression,
  type CompilerOptions,
  type ElementNode,
  ErrorCodes,
  NodeTypes,
  type ObjectExpression,
  type VNodeCall,
} from '../../src'
import {
  CAMELIZE,
  NORMALIZE_PROPS,
  helperNameMap,
} from '../../src/runtimeHelpers'
import { parseWithVBind } from './vBind.rust-api'"#,
        "Vue 3 core vBind Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function parseWithVBind(
  template: string,
  options: CompilerOptions = {},
): ElementNode {
  const ast = parse(template)
  transform(ast, {
    nodeTransforms: [
      transformVBindShorthand,
      ...(options.prefixIdentifiers ? [transformExpression] : []),
      transformElement,
    ],
    directiveTransforms: {
      bind: transformBind,
    },
    ...options,
  })
  return ast.children[0] as ElementNode
}
"#,
        "",
        "Vue 3 core vBind local transform helper",
    )?;
    write_text(
        &transforms.join("vBind.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function parseWithVBind(
  template: string,
  options: CompilerOptions = {},
) {
  const node = runtime.callBridge('vue3.core.transformBindSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  hydrateVBindAst(node)
  emitErrors(node, options)
  return node
}

function normalizeOptions(options: CompilerOptions) {
  const normalized: Record<string, unknown> = {}
  for (const key of Object.keys(options || {}) as Array<keyof CompilerOptions>) {
    const value = options[key]
    if (typeof value !== 'function') normalized[key as string] = value
  }
  normalized.__vuecBrowser = currentBrowserFlag()
  return normalized
}

function currentBrowserFlag() {
  return (
    (typeof __BROWSER__ !== 'undefined' && !!__BROWSER__) ||
    (typeof globalThis !== 'undefined' && !!(globalThis as any).__BROWSER__)
  )
}

function emitErrors(node: any, options: CompilerOptions) {
  const onError = (options as any).onError
  if (typeof onError !== 'function') return
  for (const error of node.__vuecErrors || []) {
    onError({ code: error.code, loc: error.loc })
  }
}

function hydrateVBindAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateVBindAst)
    return node
  }
  if (
    node.type === NodeTypes.JS_CALL_EXPRESSION &&
    typeof node.callee === 'string'
  ) {
    node.callee = helperSymbol(node.callee) || node.callee
  }
  if (node.type === NodeTypes.VNODE_CALL && typeof node.tag === 'string') {
    node.tag = helperSymbol(node.tag) || node.tag
  }
  for (const key of [
    'children',
    'props',
    'content',
    'codegenNode',
    'arguments',
    'returns',
    'params',
    'directives',
    'source',
    'valueAlias',
    'keyAlias',
    'objectIndexAlias',
    'parseResult',
    'branches',
    'condition',
    'test',
    'consequent',
    'alternate',
    'value',
    'elements',
    'properties',
    'key',
  ]) {
    hydrateVBindAst(node[key])
  }
  return node
}

function helperSymbol(name: string) {
  return typeof name === 'string' ? runtime[name] : undefined
}
"#,
    )?;
    Ok(())
}

fn rewrite_vue3_core_v_on_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("vOn.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  type CompilerOptions,
  type ElementNode,
  ErrorCodes,
  NodeTypes,
  type ObjectExpression,
  TO_HANDLER_KEY,
  type VNodeCall,
  helperNameMap,
  baseParse as parse,
  transform,
} from '../../src'
import { transformFor } from '../../src/transforms/vFor'
import { transformOn } from '../../src/transforms/vOn'
import { transformElement } from '../../src/transforms/transformElement'
import { transformExpression } from '../../src/transforms/transformExpression'"#,
        r#"import {
  type CompilerOptions,
  type ElementNode,
  ErrorCodes,
  NodeTypes,
  type ObjectExpression,
  TO_HANDLER_KEY,
  type VNodeCall,
  helperNameMap,
} from '../../src'
import { parseWithVOn } from './vOn.rust-api'"#,
        "Vue 3 core vOn Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function parseWithVOn(template: string, options: CompilerOptions = {}) {
  const ast = parse(template, options)
  transform(ast, {
    nodeTransforms: [transformExpression, transformElement, transformFor],
    directiveTransforms: {
      on: transformOn,
    },
    ...options,
  })
  return {
    root: ast,
    node: ast.children[0] as ElementNode,
  }
}
"#,
        "",
        "Vue 3 core vOn local transform helper",
    )?;
    write_text(
        &transforms.join("vOn.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  type ElementNode,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function parseWithVOn(
  template: string,
  options: CompilerOptions = {},
) {
  const result = runtime.callBridge('vue3.core.transformOnSuite', {
    source: template,
    options: normalizeOptions(options, template),
  })
  const root = result.root || result
  hydrateVOnAst(root)
  emitErrors(root, options)
  return {
    root,
    node: root.children[0] as ElementNode,
  }
}

function normalizeOptions(options: CompilerOptions, template: string) {
  const normalized: Record<string, unknown> = {}
  for (const key of Object.keys(options || {}) as Array<keyof CompilerOptions>) {
    const value = options[key]
    if (typeof value !== 'function') normalized[key as string] = value
  }
  const tags = extractVueTemplateTags(template)
  if (hasPredicateOption(options, 'isNativeTag')) {
    normalized.__vuecNativeTags = collectPredicateHits(
      (options as any).isNativeTag,
      tags,
    )
  }
  return normalized
}

function emitErrors(root: any, options: CompilerOptions) {
  const onError = (options as any).onError
  if (typeof onError !== 'function') return
  for (const error of root.__vuecErrors || []) {
    onError({ code: error.code, loc: error.loc })
  }
}

function hydrateVOnAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateVOnAst)
    return node
  }
  if (node.type === NodeTypes.ROOT && Array.isArray(node.helpers)) {
    node.helpers = new Set(node.helpers.map((name: string) => helperSymbol(name) || name))
  }
  if (
    node.type === NodeTypes.JS_CALL_EXPRESSION &&
    typeof node.callee === 'string'
  ) {
    node.callee = helperSymbol(node.callee) || node.callee
  }
  if (node.type === NodeTypes.VNODE_CALL) {
    if (typeof node.tag === 'string') node.tag = helperSymbol(node.tag) || node.tag
    if (node.patchFlag == null) delete node.patchFlag
  }
  for (const key of [
    'children',
    'props',
    'content',
    'codegenNode',
    'arguments',
    'returns',
    'params',
    'directives',
    'source',
    'valueAlias',
    'keyAlias',
    'objectIndexAlias',
    'parseResult',
    'branches',
    'condition',
    'test',
    'consequent',
    'alternate',
    'value',
    'elements',
    'properties',
    'key',
  ]) {
    hydrateVOnAst(node[key])
  }
  return node
}

function helperSymbol(name: string) {
  return typeof name === 'string' ? runtime[name] : undefined
}

function hasPredicateOption(options: CompilerOptions, name: string) {
  return (
    Object.prototype.hasOwnProperty.call(options || {}, name) &&
    (typeof (options as any)[name] === 'function' ||
      Array.isArray((options as any)[name]))
  )
}

function extractVueTemplateTags(source: string) {
  const tags: string[] = []
  const seen = new Set<string>()
  const pattern = /<\/?\s*([A-Za-z][A-Za-z0-9._:-]*)/g
  let match: RegExpExecArray | null
  while ((match = pattern.exec(source))) {
    const tag = match[1]
    if (!seen.has(tag)) {
      seen.add(tag)
      tags.push(tag)
    }
  }
  return tags
}

function collectPredicateHits(predicate: unknown, values: string[]) {
  if (Array.isArray(predicate)) return predicate.map(String)
  if (typeof predicate !== 'function') return []
  const hits: string[] = []
  for (const value of values) {
    try {
      if (predicate(value)) hits.push(value)
    } catch (_) {}
  }
  return hits
}
"#,
    )?;
    Ok(())
}

fn rewrite_vue3_core_v_for_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("vFor.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import { baseParse as parse } from '../../src/parser'
import { transform } from '../../src/transform'
import { transformIf } from '../../src/transforms/vIf'
import { transformFor } from '../../src/transforms/vFor'
import { transformBind } from '../../src/transforms/vBind'
import { transformElement } from '../../src/transforms/transformElement'
import { transformSlotOutlet } from '../../src/transforms/transformSlotOutlet'
import { transformExpression } from '../../src/transforms/transformExpression'
import {
  ConstantTypes,
  type ElementNode,
  type ForCodegenNode,
  type ForNode,
  type InterpolationNode,
  NodeTypes,
  type RootNode,
  type SimpleExpressionNode,
} from '../../src/ast'
import { ErrorCodes } from '../../src/errors'
import { type CompilerOptions, generate } from '../../src'
import { FRAGMENT, RENDER_LIST, RENDER_SLOT } from '../../src/runtimeHelpers'
import { PatchFlags } from '@vue/shared'
import { createObjectMatcher } from '../testUtils'
import { transformVBindShorthand } from '../../src/transforms/transformVBindShorthand'"#,
        r#"import {
  ConstantTypes,
  type ElementNode,
  type ForCodegenNode,
  type ForNode,
  type InterpolationNode,
  NodeTypes,
  type RootNode,
  type SimpleExpressionNode,
} from '../../src/ast'
import { ErrorCodes } from '../../src/errors'
import { type CompilerOptions, generate } from '../../src'
import { FRAGMENT, RENDER_LIST, RENDER_SLOT } from '../../src/runtimeHelpers'
import { PatchFlags } from '@vue/shared'
import { createObjectMatcher } from '../testUtils'
import { parseWithForTransform } from './vFor.rust-api'
export { parseWithForTransform } from './vFor.rust-api'"#,
        "Vue 3 core vFor Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"export function parseWithForTransform(
  template: string,
  options: CompilerOptions = {},
): {
  root: RootNode
  node: ForNode & { codegenNode: ForCodegenNode }
} {
  const ast = parse(template, options)
  transform(ast, {
    nodeTransforms: [
      transformVBindShorthand,
      transformIf,
      transformFor,
      ...(options.prefixIdentifiers ? [transformExpression] : []),
      transformSlotOutlet,
      transformElement,
    ],
    directiveTransforms: {
      bind: transformBind,
    },
    ...options,
  })
  return {
    root: ast,
    node: ast.children[0] as ForNode & { codegenNode: ForCodegenNode },
  }
}
"#,
        "",
        "Vue 3 core vFor local transform helper",
    )?;
    write_text(
        &transforms.join("vFor.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  type ForCodegenNode,
  type ForNode,
  type RootNode,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function parseWithForTransform(
  template: string,
  options: CompilerOptions = {},
): {
  root: RootNode
  node: ForNode & { codegenNode: ForCodegenNode }
} {
  const result = runtime.callBridge('vue3.core.transformForSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  const root = result.root || result
  hydrateVForAst(root)
  emitErrors(root, options)
  return {
    root,
    node: root.children[0] as ForNode & { codegenNode: ForCodegenNode },
  }
}

function normalizeOptions(options: CompilerOptions) {
  const normalized: Record<string, unknown> = {}
  for (const key of Object.keys(options || {}) as Array<keyof CompilerOptions>) {
    const value = options[key]
    if (typeof value !== 'function') normalized[key as string] = value
  }
  return normalized
}

function emitErrors(root: any, options: CompilerOptions) {
  const onError = (options as any).onError
  if (typeof onError !== 'function') return
  for (const error of root.__vuecErrors || []) {
    onError({ code: error.code, loc: error.loc })
  }
}

function hydrateVForAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateVForAst)
    return node
  }
  if (node.type === NodeTypes.ROOT && Array.isArray(node.helpers)) {
    node.helpers = new Set(node.helpers.map((name: string) => helperSymbol(name) || name))
  }
  if (
    node.type === NodeTypes.JS_CALL_EXPRESSION &&
    typeof node.callee === 'string'
  ) {
    node.callee = helperSymbol(node.callee) || node.callee
  }
  if (node.type === NodeTypes.VNODE_CALL) {
    if (typeof node.tag === 'string') node.tag = helperSymbol(node.tag) || node.tag
    for (const key of ['props', 'children', 'patchFlag', 'dynamicProps', 'directives']) {
      if (node[key] == null) node[key] = undefined
    }
  }
  if (node.type === NodeTypes.FOR) {
    for (const key of ['valueAlias', 'keyAlias', 'objectIndexAlias']) {
      if (node[key] == null) delete node[key]
    }
  }
  if (node.parseResult) {
    for (const key of ['value', 'key', 'index']) {
      if (node.parseResult[key] == null) delete node.parseResult[key]
    }
  }
  for (const key of [
    'children',
    'props',
    'content',
    'codegenNode',
    'arguments',
    'returns',
    'params',
    'directives',
    'source',
    'valueAlias',
    'keyAlias',
    'objectIndexAlias',
    'parseResult',
    'branches',
    'condition',
    'test',
    'consequent',
    'alternate',
    'value',
    'elements',
    'properties',
    'key',
  ]) {
    hydrateVForAst(node[key])
  }
  return node
}

function helperSymbol(name: string) {
  return typeof name === 'string' ? runtime[name] : undefined
}
"#,
    )?;
    Ok(())
}

fn rewrite_vue3_core_transform_element_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("transformElement.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  BindingTypes,
  type CompilerOptions,
  ErrorCodes,
  type NodeTransform,
  baseCompile,
  baseParse as parse,
  transform,
  transformExpression,
} from '../../src'
import {
  BASE_TRANSITION,
  CREATE_VNODE,
  GUARD_REACTIVE_PROPS,
  KEEP_ALIVE,
  MERGE_PROPS,
  NORMALIZE_CLASS,
  NORMALIZE_PROPS,
  NORMALIZE_STYLE,
  RESOLVE_COMPONENT,
  RESOLVE_DIRECTIVE,
  RESOLVE_DYNAMIC_COMPONENT,
  SUSPENSE,
  TELEPORT,
  TO_HANDLERS,
  helperNameMap,
} from '../../src/runtimeHelpers'
import {
  type DirectiveNode,
  NodeTypes,
  type RootNode,
  type VNodeCall,
  createObjectProperty,
} from '../../src/ast'
import { transformElement } from '../../src/transforms/transformElement'
import { transformStyle } from '../../../compiler-dom/src/transforms/transformStyle'
import { transformOn } from '../../src/transforms/vOn'
import { transformBind } from '../../src/transforms/vBind'
import { PatchFlags } from '@vue/shared'
import { createObjectMatcher } from '../testUtils'
import { transformText } from '../../src/transforms/transformText'
import { parseWithForTransform } from './vFor.spec'"#,
        r#"import {
  BindingTypes,
  type CompilerOptions,
  ErrorCodes,
  type NodeTransform,
  baseCompile,
  baseParse as parse,
  transform,
  transformExpression,
} from '../../src'
import {
  BASE_TRANSITION,
  CREATE_VNODE,
  GUARD_REACTIVE_PROPS,
  KEEP_ALIVE,
  MERGE_PROPS,
  NORMALIZE_CLASS,
  NORMALIZE_PROPS,
  NORMALIZE_STYLE,
  RESOLVE_COMPONENT,
  RESOLVE_DIRECTIVE,
  RESOLVE_DYNAMIC_COMPONENT,
  SUSPENSE,
  TELEPORT,
  TO_HANDLERS,
  helperNameMap,
} from '../../src/runtimeHelpers'
import {
  type DirectiveNode,
  NodeTypes,
  type RootNode,
  type VNodeCall,
  createObjectProperty,
} from '../../src/ast'
import { transformElement } from '../../src/transforms/transformElement'
import { transformStyle } from '../../../compiler-dom/src/transforms/transformStyle'
import { transformOn } from '../../src/transforms/vOn'
import { transformBind } from '../../src/transforms/vBind'
import { PatchFlags } from '@vue/shared'
import { createObjectMatcher } from '../testUtils'
import { transformText } from '../../src/transforms/transformText'
import {
  parseWithBind,
  parseWithElementTransform,
} from './transformElement.rust-api'
import { parseWithForTransform } from './vFor.rust-api'"#,
        "Vue 3 core transformElement Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function parseWithElementTransform(
  template: string,
  options: CompilerOptions = {},
): {
  root: RootNode
  node: VNodeCall
} {
  // wrap raw template in an extra div so that it doesn't get turned into a
  // block as root node
  const ast = parse(`<div>${template}</div>`, options)
  transform(ast, {
    nodeTransforms: [transformElement, transformText],
    ...options,
  })
  const codegenNode = (ast as any).children[0].children[0]
    .codegenNode as VNodeCall
  expect(codegenNode.type).toBe(NodeTypes.VNODE_CALL)
  return {
    root: ast,
    node: codegenNode,
  }
}

function parseWithBind(template: string, options?: CompilerOptions) {
  return parseWithElementTransform(template, {
    ...options,
    directiveTransforms: {
      ...options?.directiveTransforms,
      bind: transformBind,
    },
  })
}
"#,
        r#"function parseWithElementTransformOriginal(
  template: string,
  options: CompilerOptions = {},
): {
  root: RootNode
  node: VNodeCall
} {
  // wrap raw template in an extra div so that it doesn't get turned into a
  // block as root node
  const ast = parse(`<div>${template}</div>`, options)
  transform(ast, {
    nodeTransforms: [transformElement, transformText],
    ...options,
  })
  const codegenNode = (ast as any).children[0].children[0]
    .codegenNode as VNodeCall
  expect(codegenNode.type).toBe(NodeTypes.VNODE_CALL)
  return {
    root: ast,
    node: codegenNode,
  }
}
"#,
        "Vue 3 core transformElement local transform helper",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"  test('should handle <KeepAlive>', () => {
    function assert(tag: string) {
      const root = parse(`<div><${tag}><span /></${tag}></div>`)
      transform(root, {
        nodeTransforms: [transformElement, transformText],
      })
      expect(root.components.length).toBe(0)
      expect(root.helpers).toContain(KEEP_ALIVE)
      const node = (root.children[0] as any).children[0].codegenNode
      expect(node).toMatchObject({
        type: NodeTypes.VNODE_CALL,
        tag: KEEP_ALIVE,
        isBlock: true, // should be forced into a block
        props: undefined,
        // keep-alive should not compile content to slots
        children: [{ type: NodeTypes.ELEMENT, tag: 'span' }],
        // should get a dynamic slots flag to force updates
        patchFlag: PatchFlags.DYNAMIC_SLOTS,
      })
    }

    assert(`keep-alive`)
    assert(`KeepAlive`)
  })"#,
        r#"  test('should handle <KeepAlive>', () => {
    function assert(tag: string) {
      const { root, node } = parseWithElementTransform(
        `<${tag}><span /></${tag}>`,
      )
      expect(root.components.length).toBe(0)
      expect(root.helpers).toContain(KEEP_ALIVE)
      expect(node).toMatchObject({
        type: NodeTypes.VNODE_CALL,
        tag: KEEP_ALIVE,
        isBlock: true, // should be forced into a block
        props: undefined,
        // keep-alive should not compile content to slots
        children: [{ type: NodeTypes.ELEMENT, tag: 'span' }],
        // should get a dynamic slots flag to force updates
        patchFlag: PatchFlags.DYNAMIC_SLOTS,
      })
    }

    assert(`keep-alive`)
    assert(`KeepAlive`)
  })"#,
        "Vue 3 core transformElement KeepAlive Rust helper",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"  test('<svg> should be forced into blocks', () => {
    const ast = parse(`<div><svg/></div>`)
    transform(ast, {
      nodeTransforms: [transformElement],
    })
    expect((ast as any).children[0].children[0].codegenNode).toMatchObject({
      type: NodeTypes.VNODE_CALL,
      tag: `"svg"`,
      isBlock: true,
    })
  })"#,
        r#"  test('<svg> should be forced into blocks', () => {
    const { node } = parseWithElementTransform(`<svg/>`)
    expect(node).toMatchObject({
      type: NodeTypes.VNODE_CALL,
      tag: `"svg"`,
      isBlock: true,
    })
  })"#,
        "Vue 3 core transformElement svg Rust helper",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"  test('<math> should be forced into blocks', () => {
    const ast = parse(`<div><math/></div>`)
    transform(ast, {
      nodeTransforms: [transformElement],
    })
    expect((ast as any).children[0].children[0].codegenNode).toMatchObject({
      type: NodeTypes.VNODE_CALL,
      tag: `"math"`,
      isBlock: true,
    })
  })"#,
        r#"  test('<math> should be forced into blocks', () => {
    const { node } = parseWithElementTransform(`<math/>`)
    expect(node).toMatchObject({
      type: NodeTypes.VNODE_CALL,
      tag: `"math"`,
      isBlock: true,
    })
  })"#,
        "Vue 3 core transformElement math Rust helper",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"  // #938
  test('element with dynamic keys should be forced into blocks', () => {
    const ast = parse(`<div><div :key="foo" /></div>`)
    transform(ast, {
      nodeTransforms: [transformElement],
    })
    expect((ast as any).children[0].children[0].codegenNode).toMatchObject({
      type: NodeTypes.VNODE_CALL,
      tag: `"div"`,
      isBlock: true,
    })
  })"#,
        r#"  // #938
  test('element with dynamic keys should be forced into blocks', () => {
    const { node } = parseWithElementTransform(`<div :key="foo" />`)
    expect(node).toMatchObject({
      type: NodeTypes.VNODE_CALL,
      tag: `"div"`,
      isBlock: true,
    })
  })"#,
        "Vue 3 core transformElement dynamic key Rust helper",
    )?;
    rewrite_text_file_import(
        &spec,
        "const { node } = parseWithElementTransform(`<div v-foo:bar=\"hello\" />`, {\n      directiveTransforms: {\n        foo(dir) {",
        "const { node } = parseWithElementTransformOriginal(`<div v-foo:bar=\"hello\" />`, {\n      directiveTransforms: {\n        foo(dir) {",
    )?;
    rewrite_text_file_import(
        &spec,
        "const { root, node } = parseWithElementTransform(\n      `<div v-foo:bar=\"hello\" />`,\n      {\n        directiveTransforms: {\n          foo() {\n            return {\n              props: [],\n              needRuntime: true,",
        "const { root, node } = parseWithElementTransformOriginal(\n      `<div v-foo:bar=\"hello\" />`,\n      {\n        directiveTransforms: {\n          foo() {\n            return {\n              props: [],\n              needRuntime: true,",
    )?;
    rewrite_text_file_import(
        &spec,
        "const { root, node } = parseWithElementTransform(\n      `<div v-foo:bar=\"hello\" />`,\n      {\n        directiveTransforms: {\n          foo() {\n            return {\n              props: [],\n              needRuntime: CREATE_VNODE,",
        "const { root, node } = parseWithElementTransformOriginal(\n      `<div v-foo:bar=\"hello\" />`,\n      {\n        directiveTransforms: {\n          foo() {\n            return {\n              props: [],\n              needRuntime: CREATE_VNODE,",
    )?;
    write_text(
        &transforms.join("transformElement.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  NodeTypes,
  type RootNode,
  type VNodeCall,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function parseWithElementTransform(
  template: string,
  options: CompilerOptions = {},
): {
  root: RootNode
  node: VNodeCall
} {
  return parseWithElementTransformBridge(template, options, {})
}

export function parseWithBind(
  template: string,
  options: CompilerOptions = {},
) {
  return parseWithElementTransformBridge(template, options, {
    transformBind: true,
  })
}

function parseWithElementTransformBridge(
  template: string,
  options: CompilerOptions,
  extra: Record<string, unknown>,
) {
  const result = runtime.callBridge('vue3.core.transformElementSuite', {
    source: `<div>${template}</div>`,
    options: normalizeOptions(options, template, extra),
  })
  const root = result.root || result
  hydrateTransformElementAst(root)
  emitErrors(root, options)
  const node =
    root.children?.[0]?.children?.[0]?.codegenNode || result.node || null
  hydrateTransformElementAst(node)
  expect(node && node.type).toBe(NodeTypes.VNODE_CALL)
  return {
    root,
    node: node as VNodeCall,
  }
}

function normalizeOptions(
  options: CompilerOptions,
  template: string,
  extra: Record<string, unknown>,
) {
  const normalized: Record<string, unknown> = { ...extra }
  for (const key of Object.keys(options || {}) as Array<keyof CompilerOptions>) {
    if (key === 'directiveTransforms' || key === 'nodeTransforms') continue
    if (key === 'bindingMetadata') continue
    const value = options[key]
    if (typeof value !== 'function') normalized[key as string] = value
  }
  normalized.transformBind =
    Boolean((extra as any).transformBind) ||
    Boolean((options as any).directiveTransforms?.bind)
  normalized.transformOn = Boolean((options as any).directiveTransforms?.on)
  normalized.transformStyle = hasNamedNodeTransform(options, 'transformStyle')
  if ((options as any).bindingMetadata) {
    normalized.bindingMetadata = normalizeBindingMetadata(
      (options as any).bindingMetadata,
    )
  }
  const tags = ['div', ...extractVueTemplateTags(template)]
  if (hasPredicateOption(options, 'isNativeTag')) {
    normalized.__vuecNativeTags = collectPredicateHits(
      (options as any).isNativeTag,
      tags,
      ['div'],
    )
  }
  return normalized
}

function normalizeBindingMetadata(metadata: Record<string, unknown>) {
  const normalized: Record<string, unknown> = { ...metadata }
  if (Object.prototype.hasOwnProperty.call(metadata, '__isScriptSetup')) {
    normalized.__isScriptSetup = (metadata as any).__isScriptSetup
  }
  return normalized
}

function emitErrors(root: any, options: CompilerOptions) {
  const onError = (options as any).onError
  if (typeof onError !== 'function') return
  for (const error of root.__vuecErrors || []) {
    onError({ code: error.code, loc: error.loc })
  }
}

function hydrateTransformElementAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateTransformElementAst)
    return node
  }
  if (node.type === NodeTypes.ROOT && Array.isArray(node.helpers)) {
    node.helpers = new Set(node.helpers.map((name: string) => helperSymbol(name) || name))
  }
  if (
    node.type === NodeTypes.JS_CALL_EXPRESSION &&
    typeof node.callee === 'string'
  ) {
    node.callee = helperSymbol(node.callee) || node.callee
  }
  if (node.type === NodeTypes.VNODE_CALL) {
    if (typeof node.tag === 'string') node.tag = helperSymbol(node.tag) || node.tag
    for (const key of ['props', 'children', 'patchFlag', 'dynamicProps', 'directives']) {
      if (node[key] == null) node[key] = undefined
    }
  }
  if (node.type === NodeTypes.JS_FUNCTION_EXPRESSION) {
    if (node.params == null) node.params = undefined
  }
  if (node.type === NodeTypes.FOR) {
    for (const key of ['valueAlias', 'keyAlias', 'objectIndexAlias']) {
      if (node[key] == null) delete node[key]
    }
  }
  if (node.parseResult) {
    for (const key of ['value', 'key', 'index']) {
      if (node.parseResult[key] == null) delete node.parseResult[key]
    }
  }
  for (const key of [
    'children',
    'props',
    'content',
    'codegenNode',
    'arguments',
    'returns',
    'params',
    'directives',
    'source',
    'valueAlias',
    'keyAlias',
    'objectIndexAlias',
    'parseResult',
    'branches',
    'condition',
    'test',
    'consequent',
    'alternate',
    'value',
    'elements',
    'properties',
    'key',
    'tag',
    'hoists',
    'cached',
  ]) {
    hydrateTransformElementAst(node[key])
  }
  return node
}

function helperSymbol(name: string) {
  return typeof name === 'string' ? runtime[name] : undefined
}

function hasPredicateOption(options: CompilerOptions, name: string) {
  return (
    Object.prototype.hasOwnProperty.call(options || {}, name) &&
    (typeof (options as any)[name] === 'function' ||
      Array.isArray((options as any)[name]))
  )
}

function hasNamedNodeTransform(options: CompilerOptions, name: string) {
  const transforms = (options as any).nodeTransforms
  return (
    Array.isArray(transforms) &&
    transforms.some(
      transform =>
        typeof transform === 'function' && transform.name === name,
    )
  )
}

function extractVueTemplateTags(source: string) {
  const tags: string[] = []
  const seen = new Set<string>()
  const pattern = /<\/?\s*([A-Za-z][A-Za-z0-9._:-]*)/g
  let match: RegExpExecArray | null
  while ((match = pattern.exec(source))) {
    const tag = match[1]
    if (!seen.has(tag)) {
      seen.add(tag)
      tags.push(tag)
    }
  }
  return tags
}

function collectPredicateHits(
  predicate: unknown,
  values: string[],
  forced: string[],
) {
  const hits = new Set<string>(forced)
  if (Array.isArray(predicate)) {
    for (const value of predicate) hits.add(String(value))
    return Array.from(hits)
  }
  if (typeof predicate !== 'function') return Array.from(hits)
  for (const value of values) {
    try {
      if (predicate(value)) hits.add(value)
    } catch (_) {}
  }
  return Array.from(hits)
}
"#,
    )?;
    Ok(())
}

fn rewrite_vue3_core_transform_public_api_spec(prepared_root: &Path) -> Result<()> {
    let tests = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__");
    let spec = tests.join("transform.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import { PatchFlags } from '@vue/shared'"#,
        r#"import { PatchFlags } from '@vue/shared'
import { transformWithCodegen } from './transform.rust-api'"#,
        "Vue 3 core transform Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"  test('should inject toString helper for interpolations', () => {
    const ast = baseParse(`{{ foo }}`)
    transform(ast, {})
    expect(ast.helpers).toContain(TO_DISPLAY_STRING)
  })"#,
        r#"  test('should inject toString helper for interpolations', () => {
    const ast = transformWithCodegen(`{{ foo }}`)
    expect(ast.helpers).toContain(TO_DISPLAY_STRING)
  })"#,
        "Vue 3 core transform interpolation helper Rust path",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"  test('should inject createVNode and Comment for comments', () => {
    const ast = baseParse(`<!--foo-->`)
    transform(ast, {})
    expect(ast.helpers).toContain(CREATE_COMMENT)
  })"#,
        r#"  test('should inject createVNode and Comment for comments', () => {
    const ast = transformWithCodegen(`<!--foo-->`)
    expect(ast.helpers).toContain(CREATE_COMMENT)
  })"#,
        "Vue 3 core transform comment helper Rust path",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"    function transformWithCodegen(template: string) {
      const ast = baseParse(template)
      transform(ast, {
        nodeTransforms: [
          transformIf,
          transformFor,
          transformText,
          transformSlotOutlet,
          transformElement,
        ],
      })
      return ast
    }
"#,
        "",
        "Vue 3 core transform local codegen helper",
    )?;
    write_text(
        &tests.join("transform.rust-api.ts"),
        r#"import { NodeTypes, __vuecRuntime } from '../src'

const runtime = __vuecRuntime as any

export function transformWithCodegen(template: string) {
  const root = runtime.callBridge('vue3.core.transformSuite', {
    source: template,
    options: {},
  })
  hydrateTransformAst(root)
  return root
}

function hydrateTransformAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateTransformAst)
    return node
  }
  if (node.type === NodeTypes.ROOT) {
    if (Array.isArray(node.helpers)) {
      node.helpers = new Set(node.helpers.map((name: string) => helperSymbol(name) || name))
    }
    if (node.codegenNode == null) node.codegenNode = undefined
  }
  if (
    node.type === NodeTypes.JS_CALL_EXPRESSION &&
    typeof node.callee === 'string'
  ) {
    node.callee = helperSymbol(node.callee) || node.callee
  }
  if (node.type === NodeTypes.VNODE_CALL) {
    if (typeof node.tag === 'string') node.tag = helperSymbol(node.tag) || node.tag
    for (const key of ['props', 'children', 'patchFlag', 'dynamicProps', 'directives']) {
      if (node[key] == null) node[key] = undefined
    }
  }
  if (node.type === NodeTypes.JS_FUNCTION_EXPRESSION) {
    if (node.params == null) node.params = undefined
  }
  if (node.type === NodeTypes.FOR) {
    for (const key of ['valueAlias', 'keyAlias', 'objectIndexAlias']) {
      if (node[key] == null) delete node[key]
    }
  }
  if (node.parseResult) {
    for (const key of ['value', 'key', 'index']) {
      if (node.parseResult[key] == null) delete node.parseResult[key]
    }
  }
  for (const key of [
    'children',
    'props',
    'content',
    'codegenNode',
    'arguments',
    'returns',
    'params',
    'directives',
    'source',
    'valueAlias',
    'keyAlias',
    'objectIndexAlias',
    'parseResult',
    'branches',
    'condition',
    'test',
    'consequent',
    'alternate',
    'value',
    'elements',
    'properties',
    'key',
    'tag',
    'hoists',
    'cached',
  ]) {
    hydrateTransformAst(node[key])
  }
  return node
}

function helperSymbol(name: string) {
  return typeof name === 'string' ? runtime[name] : undefined
}
"#,
    )?;
    Ok(())
}

fn rewrite_vue3_core_v_if_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("vIf.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import { baseParse as parse } from '../../src/parser'
import { transform } from '../../src/transform'
import { transformIf } from '../../src/transforms/vIf'
import { transformElement } from '../../src/transforms/transformElement'
import { transformSlotOutlet } from '../../src/transforms/transformSlotOutlet'
import {
  type CommentNode,
  type ConditionalExpression,
  type ElementNode,
  ElementTypes,
  type IfBranchNode,
  type IfConditionalExpression,
  type IfNode,
  NodeTypes,
  type SimpleExpressionNode,
  type TextNode,
  type VNodeCall,
} from '../../src/ast'
import { ErrorCodes } from '../../src/errors'
import {
  type CompilerOptions,
  TO_HANDLERS,
  generate,
  transformVBindShorthand,
} from '../../src'
import {
  CREATE_COMMENT,
  FRAGMENT,
  MERGE_PROPS,
  NORMALIZE_PROPS,
  RENDER_SLOT,
} from '../../src/runtimeHelpers'
import { createObjectMatcher } from '../testUtils'"#,
        r#"import {
  type CommentNode,
  type ConditionalExpression,
  type ElementNode,
  ElementTypes,
  type IfBranchNode,
  type IfConditionalExpression,
  type IfNode,
  NodeTypes,
  type SimpleExpressionNode,
  type TextNode,
  type VNodeCall,
} from '../../src/ast'
import { ErrorCodes } from '../../src/errors'
import { type CompilerOptions, TO_HANDLERS, generate } from '../../src'
import {
  CREATE_COMMENT,
  FRAGMENT,
  MERGE_PROPS,
  NORMALIZE_PROPS,
  RENDER_SLOT,
} from '../../src/runtimeHelpers'
import { createObjectMatcher } from '../testUtils'
import { parseWithIfTransform } from './vIf.rust-api'"#,
        "Vue 3 core vIf Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function parseWithIfTransform(
  template: string,
  options: CompilerOptions = {},
  returnIndex: number = 0,
  childrenLen: number = 1,
) {
  const ast = parse(template, options)
  transform(ast, {
    nodeTransforms: [
      transformVBindShorthand,
      transformIf,
      transformSlotOutlet,
      transformElement,
    ],
    ...options,
  })
  if (!options.onError) {
    expect(ast.children.length).toBe(childrenLen)
    for (let i = 0; i < childrenLen; i++) {
      expect(ast.children[i].type).toBe(NodeTypes.IF)
    }
  }
  return {
    root: ast,
    node: ast.children[returnIndex] as IfNode & {
      codegenNode: IfConditionalExpression
    },
  }
}
"#,
        "",
        "Vue 3 core vIf local transform helper",
    )?;
    write_text(
        &transforms.join("vIf.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  type IfConditionalExpression,
  type IfNode,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function parseWithIfTransform(
  template: string,
  options: CompilerOptions = {},
  returnIndex: number = 0,
  childrenLen: number = 1,
) {
  const result = runtime.callBridge('vue3.core.transformIfSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  const root = result.root || result
  hydrateVIfAst(root)
  emitErrors(root, options)
  if (!(options as any).onError) {
    expect(root.children.length).toBe(childrenLen)
    for (let i = 0; i < childrenLen; i++) {
      expect(root.children[i].type).toBe(NodeTypes.IF)
    }
  }
  return {
    root,
    node: root.children[returnIndex] as IfNode & {
      codegenNode: IfConditionalExpression
    },
  }
}

function normalizeOptions(options: CompilerOptions) {
  const normalized: Record<string, unknown> = {}
  for (const key of Object.keys(options || {}) as Array<keyof CompilerOptions>) {
    const value = options[key]
    if (typeof value !== 'function') normalized[key as string] = value
  }
  return normalized
}

function emitErrors(root: any, options: CompilerOptions) {
  const onError = (options as any).onError
  if (typeof onError !== 'function') return
  for (const error of root.__vuecErrors || []) {
    onError({ code: error.code, loc: error.loc })
  }
}

function hydrateVIfAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateVIfAst)
    return node
  }
  if (node.type === NodeTypes.ROOT && Array.isArray(node.helpers)) {
    node.helpers = new Set(node.helpers.map((name: string) => helperSymbol(name) || name))
  }
  if (
    node.type === NodeTypes.JS_CALL_EXPRESSION &&
    typeof node.callee === 'string'
  ) {
    node.callee = helperSymbol(node.callee) || node.callee
  }
  if (node.type === NodeTypes.VNODE_CALL) {
    if (typeof node.tag === 'string') node.tag = helperSymbol(node.tag) || node.tag
    for (const key of ['props', 'children', 'patchFlag', 'dynamicProps', 'directives']) {
      if (node[key] == null) node[key] = undefined
    }
  }
  if (node.type === NodeTypes.IF_BRANCH) {
    if (node.condition == null) delete node.condition
    if (node.userKey == null) delete node.userKey
  }
  for (const key of [
    'children',
    'props',
    'content',
    'codegenNode',
    'arguments',
    'returns',
    'params',
    'directives',
    'source',
    'valueAlias',
    'keyAlias',
    'objectIndexAlias',
    'parseResult',
    'branches',
    'condition',
    'test',
    'consequent',
    'alternate',
    'value',
    'elements',
    'properties',
    'key',
  ]) {
    hydrateVIfAst(node[key])
  }
  return node
}

function helperSymbol(name: string) {
  return typeof name === 'string' ? runtime[name] : undefined
}
"#,
    )?;
    Ok(())
}

fn rewrite_vue3_core_v_model_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("vModel.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  BindingTypes,
  type CompilerOptions,
  type ComponentNode,
  type ElementNode,
  type ForNode,
  NORMALIZE_PROPS,
  NodeTypes,
  type ObjectExpression,
  type PlainElementNode,
  type VNodeCall,
  generate,
  baseParse as parse,
  transform,
} from '../../src'
import { ErrorCodes } from '../../src/errors'
import { transformModel } from '../../src/transforms/vModel'
import { transformElement } from '../../src/transforms/transformElement'
import { transformExpression } from '../../src/transforms/transformExpression'
import { transformFor } from '../../src/transforms/vFor'
import { trackSlotScopes } from '../../src/transforms/vSlot'
import type { CallExpression } from '@babel/types'"#,
        r#"import {
  BindingTypes,
  type CompilerOptions,
  type ComponentNode,
  type ElementNode,
  type ForNode,
  NORMALIZE_PROPS,
  NodeTypes,
  type ObjectExpression,
  type PlainElementNode,
  type VNodeCall,
  generate,
} from '../../src'
import { ErrorCodes } from '../../src/errors'
import type { CallExpression } from '@babel/types'
import { parseWithVModel } from './vModel.rust-api'"#,
        "Vue 3 core vModel Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function parseWithVModel(template: string, options: CompilerOptions = {}) {
  const ast = parse(template)

  transform(ast, {
    nodeTransforms: [
      transformFor,
      transformExpression,
      transformElement,
      trackSlotScopes,
    ],
    directiveTransforms: {
      ...options.directiveTransforms,
      model: transformModel,
    },
    ...options,
  })

  return ast
}
"#,
        "",
        "Vue 3 core vModel local transform helper",
    )?;
    write_text(
        &transforms.join("vModel.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function parseWithVModel(
  template: string,
  options: CompilerOptions = {},
) {
  const root = runtime.callBridge('vue3.core.transformModelSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  hydrateVModelAst(root)
  emitErrors(root, options)
  return root
}

function normalizeOptions(options: CompilerOptions) {
  const normalized: Record<string, unknown> = {}
  for (const key of Object.keys(options || {}) as Array<keyof CompilerOptions>) {
    const value = options[key]
    if (typeof value !== 'function') normalized[key as string] = value
  }
  return normalized
}

function emitErrors(root: any, options: CompilerOptions) {
  const onError = (options as any).onError
  if (typeof onError !== 'function') return
  for (const error of root.__vuecErrors || []) {
    onError({ code: error.code, loc: error.loc })
  }
}

function hydrateVModelAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateVModelAst)
    return node
  }
  if (node.type === NodeTypes.ROOT && Array.isArray(node.helpers)) {
    node.helpers = new Set(node.helpers.map((name: string) => helperSymbol(name) || name))
  }
  if (
    node.type === NodeTypes.JS_CALL_EXPRESSION &&
    typeof node.callee === 'string'
  ) {
    node.callee = helperSymbol(node.callee) || node.callee
  }
  if (node.type === NodeTypes.VNODE_CALL && typeof node.tag === 'string') {
    node.tag = helperSymbol(node.tag) || node.tag
  }
  for (const key of [
    'children',
    'props',
    'content',
    'codegenNode',
    'arguments',
    'returns',
    'params',
    'directives',
    'source',
    'valueAlias',
    'keyAlias',
    'objectIndexAlias',
    'parseResult',
    'branches',
    'condition',
    'test',
    'consequent',
    'alternate',
    'value',
    'elements',
    'properties',
    'key',
  ]) {
    hydrateVModelAst(node[key])
  }
  return node
}

function helperSymbol(name: string) {
  return typeof name === 'string' ? runtime[name] : undefined
}
"#,
    )?;
    Ok(())
}

fn rewrite_vue3_core_transform_slot_outlet_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("transformSlotOutlet.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  type CompilerOptions,
  type ElementNode,
  ErrorCodes,
  NodeTypes,
  baseParse as parse,
  transform,
} from '../../src'
import { transformElement } from '../../src/transforms/transformElement'
import { transformOn } from '../../src/transforms/vOn'
import { transformBind } from '../../src/transforms/vBind'
import { transformExpression } from '../../src/transforms/transformExpression'
import { RENDER_SLOT } from '../../src/runtimeHelpers'
import { transformSlotOutlet } from '../../src/transforms/transformSlotOutlet'"#,
        r#"import {
  type CompilerOptions,
  type ElementNode,
  ErrorCodes,
  NodeTypes,
} from '../../src'
import { RENDER_SLOT } from '../../src/runtimeHelpers'
import { parseWithSlots } from './transformSlotOutlet.rust-api'"#,
        "Vue 3 core transformSlotOutlet Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function parseWithSlots(template: string, options: CompilerOptions = {}) {
  const ast = parse(template)
  transform(ast, {
    nodeTransforms: [
      ...(options.prefixIdentifiers ? [transformExpression] : []),
      transformSlotOutlet,
      transformElement,
    ],
    directiveTransforms: {
      on: transformOn,
      bind: transformBind,
    },
    ...options,
  })
  return ast
}
"#,
        "",
        "Vue 3 core transformSlotOutlet local transform helper",
    )?;
    write_text(
        &transforms.join("transformSlotOutlet.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function parseWithSlots(
  template: string,
  options: CompilerOptions = {},
) {
  const root = runtime.callBridge('vue3.core.transformSlotOutletSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  hydrateTransformSlotOutletAst(root)
  emitErrors(root, options)
  return root
}

function normalizeOptions(options: CompilerOptions) {
  const normalized: Record<string, unknown> = {}
  for (const key of Object.keys(options || {}) as Array<keyof CompilerOptions>) {
    const value = options[key]
    if (typeof value !== 'function') normalized[key as string] = value
  }
  return normalized
}

function emitErrors(root: any, options: CompilerOptions) {
  const onError = (options as any).onError
  if (typeof onError !== 'function') return
  for (const error of root.__vuecErrors || []) {
    onError({ code: error.code, loc: error.loc })
  }
}

function hydrateTransformSlotOutletAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateTransformSlotOutletAst)
    return node
  }
  if (node.type === NodeTypes.ROOT && Array.isArray(node.helpers)) {
    node.helpers = new Set(node.helpers.map((name: string) => helperSymbol(name) || name))
  }
  if (
    node.type === NodeTypes.JS_CALL_EXPRESSION &&
    typeof node.callee === 'string'
  ) {
    node.callee = helperSymbol(node.callee) || node.callee
  }
  if (node.type === NodeTypes.VNODE_CALL && typeof node.tag === 'string') {
    node.tag = helperSymbol(node.tag) || node.tag
  }
  for (const key of [
    'children',
    'props',
    'content',
    'codegenNode',
    'arguments',
    'returns',
    'params',
    'directives',
    'source',
    'valueAlias',
    'keyAlias',
    'objectIndexAlias',
    'parseResult',
    'branches',
    'condition',
    'test',
    'consequent',
    'alternate',
    'value',
    'elements',
    'properties',
    'key',
  ]) {
    hydrateTransformSlotOutletAst(node[key])
  }
  return node
}

function helperSymbol(name: string) {
  return typeof name === 'string' ? runtime[name] : undefined
}
"#,
    )?;
    Ok(())
}

fn rewrite_vue3_core_v_slot_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("vSlot.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  type CompilerOptions,
  type ComponentNode,
  type ElementNode,
  ErrorCodes,
  type ForNode,
  NodeTypes,
  type ObjectExpression,
  type RenderSlotCall,
  type SimpleExpressionNode,
  type SlotsExpression,
  type VNodeCall,
  generate,
  baseParse as parse,
  transform,
} from '../../src'
import { transformElement } from '../../src/transforms/transformElement'
import { transformOn } from '../../src/transforms/vOn'
import { transformBind } from '../../src/transforms/vBind'
import { transformExpression } from '../../src/transforms/transformExpression'
import { transformSlotOutlet } from '../../src/transforms/transformSlotOutlet'
import {
  trackSlotScopes,
  trackVForSlotScopes,
} from '../../src/transforms/vSlot'
import { CREATE_SLOTS, RENDER_LIST } from '../../src/runtimeHelpers'
import { createObjectMatcher } from '../testUtils'
import { PatchFlags } from '@vue/shared'
import { transformFor } from '../../src/transforms/vFor'
import { transformIf } from '../../src/transforms/vIf'
import { transformText } from '../../src/transforms/transformText'"#,
        r#"import {
  type CompilerOptions,
  type ComponentNode,
  type ElementNode,
  ErrorCodes,
  type ForNode,
  NodeTypes,
  type ObjectExpression,
  type RenderSlotCall,
  type SimpleExpressionNode,
  type VNodeCall,
  generate,
} from '../../src'
import { CREATE_SLOTS, RENDER_LIST } from '../../src/runtimeHelpers'
import { createObjectMatcher } from '../testUtils'
import { PatchFlags } from '@vue/shared'
import { parseWithSlots } from './vSlot.rust-api'"#,
        "Vue 3 core vSlot Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function parseWithSlots(
  template: string,
  options: CompilerOptions & { transformText?: boolean } = {},
) {
  const ast = parse(template, {
    whitespace: options.whitespace,
  })
  transform(ast, {
    nodeTransforms: [
      transformIf,
      transformFor,
      ...(options.prefixIdentifiers
        ? [trackVForSlotScopes, transformExpression]
        : []),
      transformSlotOutlet,
      transformElement,
      trackSlotScopes,
      ...(options.transformText ? [transformText] : []),
    ],
    directiveTransforms: {
      on: transformOn,
      bind: transformBind,
    },
    ...options,
  })
  return {
    root: ast,
    slots:
      ast.children[0].type === NodeTypes.ELEMENT
        ? ((ast.children[0].codegenNode as VNodeCall)
            .children as SlotsExpression)
        : null,
  }
}
"#,
        "",
        "Vue 3 core vSlot local transform helper",
    )?;
    write_text(
        &transforms.join("vSlot.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function parseWithSlots(
  template: string,
  options: CompilerOptions & { transformText?: boolean } = {},
) {
  const result = runtime.callBridge('vue3.core.transformSlotSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  const root = result.root || result
  hydrateVSlotAst(root)
  hydrateVSlotAst(result.slots)
  emitErrors(root, options)
  return {
    root,
    slots: result.slots == null ? null : result.slots,
  }
}

function normalizeOptions(options: CompilerOptions & { transformText?: boolean }) {
  const normalized: Record<string, unknown> = {}
  for (const key of Object.keys(options || {}) as Array<keyof typeof options>) {
    const value = options[key]
    if (typeof value !== 'function') normalized[key as string] = value
  }
  return normalized
}

function emitErrors(root: any, options: CompilerOptions) {
  const onError = (options as any).onError
  if (typeof onError !== 'function') return
  for (const error of root.__vuecErrors || []) {
    onError({ code: error.code, loc: error.loc })
  }
}

function hydrateVSlotAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateVSlotAst)
    return node
  }
  if (node.type === NodeTypes.ROOT && Array.isArray(node.helpers)) {
    node.helpers = new Set(node.helpers.map((name: string) => helperSymbol(name) || name))
  }
  if (
    node.type === NodeTypes.JS_CALL_EXPRESSION &&
    typeof node.callee === 'string'
  ) {
    node.callee = helperSymbol(node.callee) || node.callee
  }
  if (node.type === NodeTypes.VNODE_CALL) {
    if (typeof node.tag === 'string') node.tag = helperSymbol(node.tag) || node.tag
    for (const key of ['props', 'children', 'patchFlag', 'dynamicProps', 'directives']) {
      if (node[key] == null) node[key] = undefined
    }
  }
  if (node.type === NodeTypes.JS_FUNCTION_EXPRESSION) {
    if (node.params == null) node.params = undefined
  }
  if (node.type === NodeTypes.FOR) {
    for (const key of ['valueAlias', 'keyAlias', 'objectIndexAlias']) {
      if (node[key] == null) delete node[key]
    }
  }
  if (node.type === NodeTypes.IF_BRANCH) {
    if (node.condition == null) delete node.condition
    if (node.userKey == null) delete node.userKey
  }
  if (node.parseResult) {
    for (const key of ['value', 'key', 'index']) {
      if (node.parseResult[key] == null) delete node.parseResult[key]
    }
  }
  for (const key of [
    'children',
    'props',
    'content',
    'codegenNode',
    'arguments',
    'returns',
    'params',
    'directives',
    'source',
    'valueAlias',
    'keyAlias',
    'objectIndexAlias',
    'parseResult',
    'branches',
    'condition',
    'test',
    'consequent',
    'alternate',
    'value',
    'elements',
    'properties',
    'key',
  ]) {
    hydrateVSlotAst(node[key])
  }
  return node
}

function helperSymbol(name: string) {
  return typeof name === 'string' ? runtime[name] : undefined
}
"#,
    )?;
    Ok(())
}

fn rewrite_vue3_core_cache_static_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("cacheStatic.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  type CompilerOptions,
  ConstantTypes,
  type ElementNode,
  type ForNode,
  type IfNode,
  NodeTypes,
  type VNodeCall,
  generate,
  baseParse as parse,
  transform,
} from '../../src'
import {
  FRAGMENT,
  NORMALIZE_CLASS,
  RENDER_LIST,
} from '../../src/runtimeHelpers'
import { transformElement } from '../../src/transforms/transformElement'
import { transformExpression } from '../../src/transforms/transformExpression'
import { transformIf } from '../../src/transforms/vIf'
import { transformFor } from '../../src/transforms/vFor'
import { transformBind } from '../../src/transforms/vBind'
import { transformOn } from '../../src/transforms/vOn'
import { createObjectMatcher } from '../testUtils'
import { transformText } from '../../src/transforms/transformText'
import { PatchFlags } from '@vue/shared'"#,
        r#"import {
  type CompilerOptions,
  ConstantTypes,
  type ElementNode,
  type ForNode,
  type IfNode,
  NodeTypes,
  type VNodeCall,
  generate,
} from '../../src'
import {
  FRAGMENT,
  NORMALIZE_CLASS,
  RENDER_LIST,
} from '../../src/runtimeHelpers'
import { createObjectMatcher } from '../testUtils'
import { PatchFlags } from '@vue/shared'
import { transformWithCache } from './cacheStatic.rust-api'"#,
        "Vue 3 core cacheStatic Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function transformWithCache(template: string, options: CompilerOptions = {}) {
  const ast = parse(template)
  transform(ast, {
    hoistStatic: true,
    nodeTransforms: [
      transformIf,
      transformFor,
      ...(options.prefixIdentifiers ? [transformExpression] : []),
      transformElement,
      transformText,
    ],
    directiveTransforms: {
      on: transformOn,
      bind: transformBind,
    },
    ...options,
  })
  expect(ast.codegenNode).toMatchObject({
    type: NodeTypes.VNODE_CALL,
    isBlock: true,
  })
  return ast
}
"#,
        "",
        "Vue 3 core cacheStatic local transform helper",
    )?;
    write_text(
        &transforms.join("cacheStatic.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function transformWithCache(
  template: string,
  options: CompilerOptions = {},
) {
  const root = runtime.callBridge('vue3.core.cacheStaticSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  hydrateCacheStaticAst(root)
  emitErrors(root, options)
  expect(root.codegenNode).toMatchObject({
    type: NodeTypes.VNODE_CALL,
    isBlock: true,
  })
  return root
}

function normalizeOptions(options: CompilerOptions) {
  const normalized: Record<string, unknown> = { hoistStatic: true }
  for (const key of Object.keys(options || {}) as Array<keyof CompilerOptions>) {
    const value = options[key]
    if (typeof value !== 'function') normalized[key as string] = value
  }
  return normalized
}

function emitErrors(root: any, options: CompilerOptions) {
  const onError = (options as any).onError
  if (typeof onError !== 'function') return
  for (const error of root.__vuecErrors || []) {
    onError({ code: error.code, loc: error.loc })
  }
}

function hydrateCacheStaticAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateCacheStaticAst)
    return node
  }
  if (node.type === NodeTypes.ROOT && Array.isArray(node.helpers)) {
    node.helpers = new Set(node.helpers.map((name: string) => helperSymbol(name) || name))
  }
  if (
    node.type === NodeTypes.JS_CALL_EXPRESSION &&
    typeof node.callee === 'string'
  ) {
    node.callee = helperSymbol(node.callee) || node.callee
  }
  if (node.type === NodeTypes.VNODE_CALL) {
    if (typeof node.tag === 'string') node.tag = helperSymbol(node.tag) || node.tag
    for (const key of ['props', 'children', 'patchFlag', 'dynamicProps', 'directives']) {
      if (node[key] == null) node[key] = undefined
    }
  }
  if (node.type === NodeTypes.JS_FUNCTION_EXPRESSION) {
    if (node.params == null) node.params = undefined
  }
  if (node.type === NodeTypes.FOR) {
    for (const key of ['valueAlias', 'keyAlias', 'objectIndexAlias']) {
      if (node[key] == null) delete node[key]
    }
  }
  if (node.parseResult) {
    for (const key of ['value', 'key', 'index']) {
      if (node.parseResult[key] == null) delete node.parseResult[key]
    }
  }
  for (const key of [
    'children',
    'props',
    'content',
    'codegenNode',
    'arguments',
    'returns',
    'params',
    'directives',
    'source',
    'valueAlias',
    'keyAlias',
    'objectIndexAlias',
    'parseResult',
    'branches',
    'condition',
    'test',
    'consequent',
    'alternate',
    'value',
    'elements',
    'properties',
    'key',
    'hoists',
    'cached',
  ]) {
    hydrateCacheStaticAst(node[key])
  }
  return node
}

function helperSymbol(name: string) {
  return typeof name === 'string' ? runtime[name] : undefined
}
"#,
    )?;
    Ok(())
}

fn rewrite_vue3_core_transform_expressions_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("transformExpressions.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  BindingTypes,
  type CompilerOptions,
  ConstantTypes,
  type DirectiveNode,
  type ElementNode,
  type InterpolationNode,
  NodeTypes,
  baseCompile,
  baseParse as parse,
  transform,
} from '../../src'
import { transformIf } from '../../src/transforms/vIf'
import { transformExpression } from '../../src/transforms/transformExpression'
import { PatchFlagNames, PatchFlags } from '../../../shared/src'"#,
        r#"import {
  BindingTypes,
  type CompilerOptions,
  ConstantTypes,
  type DirectiveNode,
  type ElementNode,
  type InterpolationNode,
  NodeTypes,
  baseCompile,
} from '../../src'
import { PatchFlagNames, PatchFlags } from '../../../shared/src'
import { parseWithExpressionTransform } from './transformExpressions.rust-api'"#,
        "Vue 3 core transformExpressions Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function parseWithExpressionTransform(
  template: string,
  options: CompilerOptions = {},
) {
  const ast = parse(template, options)
  transform(ast, {
    prefixIdentifiers: true,
    nodeTransforms: [transformIf, transformExpression],
    ...options,
  })
  return ast.children[0]
}
"#,
        "",
        "Vue 3 core transformExpressions local transform helper",
    )?;
    write_text(
        &transforms.join("transformExpressions.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function parseWithExpressionTransform(
  template: string,
  options: CompilerOptions = {},
) {
  const node = runtime.callBridge('vue3.core.transformExpressionSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  hydrateTransformExpressionsAst(node)
  emitErrors(node, options)
  return node
}

function normalizeOptions(options: CompilerOptions) {
  const normalized: Record<string, unknown> = {}
  for (const key of Object.keys(options || {}) as Array<keyof CompilerOptions>) {
    const value = options[key]
    if (typeof value !== 'function') normalized[key as string] = value
  }
  return normalized
}

function emitErrors(node: any, options: CompilerOptions) {
  const onError = (options as any).onError
  if (typeof onError !== 'function') return
  for (const error of node.__vuecErrors || []) {
    const emitted = new SyntaxError(error.message || 'Vue compiler error')
    ;(emitted as any).code = error.code
    ;(emitted as any).loc = error.loc
    onError(emitted)
  }
}

function hydrateTransformExpressionsAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateTransformExpressionsAst)
    return node
  }
  if (node.type === NodeTypes.ROOT && Array.isArray(node.helpers)) {
    node.helpers = new Set(node.helpers.map((name: string) => helperSymbol(name) || name))
  }
  if (
    node.type === NodeTypes.JS_CALL_EXPRESSION &&
    typeof node.callee === 'string'
  ) {
    node.callee = helperSymbol(node.callee) || node.callee
  }
  if (node.type === NodeTypes.VNODE_CALL && typeof node.tag === 'string') {
    node.tag = helperSymbol(node.tag) || node.tag
  }
  for (const key of [
    'children',
    'props',
    'content',
    'codegenNode',
    'arguments',
    'returns',
    'params',
    'directives',
    'source',
    'valueAlias',
    'keyAlias',
    'objectIndexAlias',
    'parseResult',
    'branches',
    'condition',
    'test',
    'consequent',
    'alternate',
    'value',
    'elements',
    'properties',
    'key',
  ]) {
    hydrateTransformExpressionsAst(node[key])
  }
  return node
}

function helperSymbol(name: string) {
  return typeof name === 'string' ? runtime[name] : undefined
}
"#,
    )?;
    Ok(())
}

fn rewrite_vue3_core_transform_text_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("transformText.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  type CompilerOptions,
  type ElementNode,
  type ForNode,
  NodeTypes,
  generate,
  isWhitespaceText,
  baseParse as parse,
  transform,
} from '../../src'
import { transformFor } from '../../src/transforms/vFor'
import { transformText } from '../../src/transforms/transformText'
import { transformExpression } from '../../src/transforms/transformExpression'
import { transformElement } from '../../src/transforms/transformElement'
import { CREATE_TEXT } from '../../src/runtimeHelpers'"#,
        r#"import {
  type CompilerOptions,
  type ElementNode,
  type ForNode,
  NodeTypes,
  generate,
  isWhitespaceText,
} from '../../src'
import { CREATE_TEXT } from '../../src/runtimeHelpers'
import { transformWithTextOpt } from './transformText.rust-api'"#,
        "Vue 3 core transformText Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function transformWithTextOpt(template: string, options: CompilerOptions = {}) {
  const ast = parse(template)
  transform(ast, {
    nodeTransforms: [
      transformFor,
      ...(options.prefixIdentifiers ? [transformExpression] : []),
      transformElement,
      transformText,
    ],
    ...options,
  })
  return ast
}
"#,
        "",
        "Vue 3 core transformText local transform helper",
    )?;
    write_text(
        &transforms.join("transformText.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function transformWithTextOpt(
  template: string,
  options: CompilerOptions = {},
) {
  const root = runtime.callBridge('vue3.core.transformTextSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  return hydrateTransformTextAst(root)
}

function normalizeOptions(options: CompilerOptions) {
  const normalized: Record<string, unknown> = {}
  for (const key of Object.keys(options || {}) as Array<keyof CompilerOptions>) {
    const value = options[key]
    if (typeof value !== 'function') normalized[key as string] = value
  }
  return normalized
}

function hydrateTransformTextAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateTransformTextAst)
    return node
  }
  if (node.type === NodeTypes.ROOT && Array.isArray(node.helpers)) {
    node.helpers = new Set(node.helpers.map((name: string) => helperSymbol(name) || name))
  }
  if (
    node.type === NodeTypes.JS_CALL_EXPRESSION &&
    typeof node.callee === 'string'
  ) {
    node.callee = helperSymbol(node.callee) || node.callee
  }
  if (node.type === NodeTypes.VNODE_CALL && typeof node.tag === 'string') {
    node.tag = helperSymbol(node.tag) || node.tag
  }
  for (const key of [
    'children',
    'props',
    'content',
    'codegenNode',
    'arguments',
    'returns',
    'params',
    'directives',
    'source',
    'valueAlias',
    'keyAlias',
    'objectIndexAlias',
    'parseResult',
  ]) {
    hydrateTransformTextAst(node[key])
  }
  return node
}

function helperSymbol(name: string) {
  return typeof name === 'string' ? runtime[name] : undefined
}
"#,
    )?;
    Ok(())
}

fn rewrite_vue3_core_v_once_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("vOnce.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  type CompilerOptions,
  NodeTypes,
  generate,
  getBaseTransformPreset,
  baseParse as parse,
  transform,
} from '../../src'
import { RENDER_SLOT, SET_BLOCK_TRACKING } from '../../src/runtimeHelpers'"#,
        r#"import {
  type CompilerOptions,
  NodeTypes,
  generate,
} from '../../src'
import { RENDER_SLOT, SET_BLOCK_TRACKING } from '../../src/runtimeHelpers'
import { transformWithOnce } from './vOnce.rust-api'"#,
        "Vue 3 core vOnce Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function transformWithOnce(template: string, options: CompilerOptions = {}) {
  const ast = parse(template)
  const [nodeTransforms, directiveTransforms] = getBaseTransformPreset()
  transform(ast, {
    nodeTransforms,
    directiveTransforms,
    ...options,
  })
  return ast
}
"#,
        "",
        "Vue 3 core vOnce local transform helper",
    )?;
    write_text(
        &transforms.join("vOnce.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function transformWithOnce(
  template: string,
  options: CompilerOptions = {},
) {
  const root = runtime.callBridge('vue3.core.transformOnceSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  return hydrateTransformOnceAst(root)
}

function normalizeOptions(options: CompilerOptions) {
  const normalized: Record<string, unknown> = {}
  for (const key of Object.keys(options || {}) as Array<keyof CompilerOptions>) {
    const value = options[key]
    if (typeof value !== 'function') normalized[key as string] = value
  }
  return normalized
}

function hydrateTransformOnceAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateTransformOnceAst)
    return node
  }
  if (node.type === NodeTypes.ROOT && Array.isArray(node.helpers)) {
    node.helpers = new Set(node.helpers.map((name: string) => helperSymbol(name) || name))
  }
  if (
    node.type === NodeTypes.JS_CALL_EXPRESSION &&
    typeof node.callee === 'string'
  ) {
    node.callee = helperSymbol(node.callee) || node.callee
  }
  if (node.type === NodeTypes.VNODE_CALL && typeof node.tag === 'string') {
    node.tag = helperSymbol(node.tag) || node.tag
  }
  for (const key of [
    'children',
    'props',
    'content',
    'codegenNode',
    'arguments',
    'returns',
    'params',
    'directives',
    'source',
    'valueAlias',
    'keyAlias',
    'objectIndexAlias',
    'parseResult',
    'branches',
    'condition',
    'test',
    'consequent',
    'alternate',
    'value',
    'elements',
    'properties',
    'key',
  ]) {
    hydrateTransformOnceAst(node[key])
  }
  return node
}

function helperSymbol(name: string) {
  return typeof name === 'string' ? runtime[name] : undefined
}
"#,
    )?;
    Ok(())
}

fn write_vue3_core_source_shims(prepared_root: &Path) -> Result<()> {
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
    Ok(())
}

fn prepare_vue3_dom_conformance_suite(
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
        .join("compiler-dom")
        .join("__tests__");
    let prepared_tests = prepared_root
        .join("packages")
        .join("compiler-dom")
        .join("__tests__");
    copy_dir_recursive(&official_tests, &prepared_tests)?;
    let official_src = official_root
        .join("packages")
        .join("compiler-dom")
        .join("src");
    let prepared_src = prepared_root
        .join("packages")
        .join("compiler-dom")
        .join("src");
    copy_dir_recursive(&official_src, &prepared_src)?;

    let core_test_utils = official_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("testUtils.ts");
    let prepared_core_tests = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__");
    fs::create_dir_all(&prepared_core_tests)
        .with_context(|| format!("failed to create {}", prepared_core_tests.display()))?;
    fs::copy(&core_test_utils, prepared_core_tests.join("testUtils.ts")).with_context(|| {
        format!(
            "failed to copy {} into {}",
            core_test_utils.display(),
            prepared_core_tests.display()
        )
    })?;

    write_vue3_core_source_shims(&prepared_root)?;
    write_vue3_dom_conformance_shims(&prepared_root)?;
    Ok(prepared_root)
}

fn write_vue3_dom_conformance_shims(prepared_root: &Path) -> Result<()> {
    rewrite_vue3_dom_public_index_spec_import(prepared_root)?;

    let dom_src = prepared_root
        .join("packages")
        .join("compiler-dom")
        .join("src");
    let transforms = dom_src.join("transforms");
    fs::create_dir_all(&transforms)
        .with_context(|| format!("failed to create {}", transforms.display()))?;

    write_text(
        &transforms.join("transformStyle.ts"),
        "export { transformStyle } from '@vue/compiler-dom'\n",
    )?;
    write_vue3_dom_stringify_static_shim(&transforms.join("stringifyStatic.ts"))?;
    write_vue3_dom_transform_shim(&transforms.join("vHtml.ts"), "transformVHtml")?;
    write_vue3_dom_transform_shim(&transforms.join("vText.ts"), "transformVText")?;
    write_vue3_dom_transform_shim(&transforms.join("vShow.ts"), "transformShow")?;
    write_vue3_dom_v_on_transform_shim(&transforms.join("vOn.ts"))?;
    write_vue3_dom_v_model_transform_shim(&transforms.join("vModel.ts"))?;
    write_vue3_dom_transition_transform_shim(&transforms.join("Transition.ts"))?;
    write_vue3_dom_ignore_side_effect_tags_shim(&transforms.join("ignoreSideEffectTags.ts"))?;
    write_vue3_dom_validate_html_nesting_shim(&transforms.join("validateHtmlNesting.ts"))?;
    write_vue3_dom_decode_html_browser_shim(&dom_src.join("decodeHtmlBrowser.ts"))?;
    write_vue3_dom_html_nesting_shim(&dom_src.join("htmlNesting.ts"))?;
    write_vue3_core_test_setup(prepared_root)?;

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
    setupFiles: ['./vuec-vitest-setup.ts'],
    include: ['packages/compiler-dom/__tests__/**/*.spec.ts'],
  },
}
"#;
    write_text(&prepared_root.join("vitest.config.ts"), config)?;
    Ok(())
}

fn rewrite_vue3_dom_public_index_spec_import(prepared_root: &Path) -> Result<()> {
    let index_spec = prepared_root
        .join("packages")
        .join("compiler-dom")
        .join("__tests__")
        .join("index.spec.ts");
    if !index_spec.exists() {
        return Ok(());
    }
    let original = fs::read_to_string(&index_spec)
        .with_context(|| format!("failed to read {}", index_spec.display()))?;
    let rewritten = original.replace(
        "import { compile } from '../src'",
        "import { compile } from '@vue/compiler-dom'",
    );
    if rewritten != original {
        write_text(&index_spec, &rewritten)?;
    }
    Ok(())
}

fn prepare_vue3_sfc_conformance_suite(
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

    let official_sfc_tests = official_root
        .join("packages")
        .join("compiler-sfc")
        .join("__tests__");
    let prepared_sfc_tests = prepared_root
        .join("packages")
        .join("compiler-sfc")
        .join("__tests__");
    copy_dir_recursive(&official_sfc_tests, &prepared_sfc_tests)?;
    rewrite_vue3_sfc_public_api_spec_imports(&prepared_root)?;

    let official_sfc_src = official_root
        .join("packages")
        .join("compiler-sfc")
        .join("src");
    let prepared_sfc_src = prepared_root
        .join("packages")
        .join("compiler-sfc")
        .join("src");
    copy_dir_recursive(&official_sfc_src, &prepared_sfc_src)?;
    patch_vue3_sfc_compile_template_asset_bridge(&prepared_sfc_src.join("compileTemplate.ts"))?;

    let official_dom_stringify = official_root
        .join("packages")
        .join("compiler-dom")
        .join("src")
        .join("transforms")
        .join("stringifyStatic.ts");
    let prepared_dom_transforms = prepared_root
        .join("packages")
        .join("compiler-dom")
        .join("src")
        .join("transforms");
    fs::create_dir_all(&prepared_dom_transforms)
        .with_context(|| format!("failed to create {}", prepared_dom_transforms.display()))?;
    fs::copy(
        &official_dom_stringify,
        prepared_dom_transforms.join("stringifyStatic.ts"),
    )
    .with_context(|| {
        format!(
            "failed to copy {} into {}",
            official_dom_stringify.display(),
            prepared_dom_transforms.display()
        )
    })?;

    write_vue3_core_source_shims(&prepared_root)?;
    write_vue3_sfc_conformance_shims(&prepared_root)?;
    Ok(prepared_root)
}

fn patch_vue3_sfc_compile_template_asset_bridge(path: &Path) -> Result<()> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    if source.contains("transformAssetUrls:")
        && source.contains("normalizeOptions(transformAssetUrls)")
        && source.contains("(compilerOptions as any).transformAssetUrls")
    {
        return Ok(());
    }
    let needle = "    ...compilerOptions,\n    hmr: !isProd,";
    let replacement = "    ...compilerOptions,\n    transformAssetUrls:\n      isObject(transformAssetUrls)\n        ? normalizeOptions(transformAssetUrls)\n        : transformAssetUrls === false\n          ? false\n          : (compilerOptions as any).transformAssetUrls,\n    hmr: !isProd,";
    ensure!(
        source.replace("\r\n", "\n").contains(needle),
        "Vue 3 SFC compileTemplate asset bridge patch anchor not found in {}",
        path.display()
    );
    write_text(
        path,
        &source.replace("\r\n", "\n").replace(needle, replacement),
    )
}

const VUE3_SFC_ASSET_TRANSFORM_INTERNAL_HELPER: &str = r#"import {
  type TransformOptions,
  baseParse,
  generate,
  transform,
} from '@vue/compiler-core'
import {
  type AssetURLOptions,
  createAssetUrlTransformWithOptions,
  normalizeOptions,
  transformAssetUrl,
} from '../src/template/transformAssetUrl'
import { transformElement } from '../../compiler-core/src/transforms/transformElement'
import { transformBind } from '../../compiler-core/src/transforms/vBind'
import { stringifyStatic } from '../../compiler-dom/src/transforms/stringifyStatic'

function compileWithAssetUrls(
  template: string,
  options?: AssetURLOptions,
  transformOptions?: TransformOptions,
) {
  const ast = baseParse(template)
  const t = options
    ? createAssetUrlTransformWithOptions(normalizeOptions(options))
    : transformAssetUrl
  transform(ast, {
    nodeTransforms: [t, transformElement],
    directiveTransforms: {
      bind: transformBind,
    },
    ...transformOptions,
  })
  return generate(ast, { mode: 'module' })
}
"#;

const VUE3_SFC_SRCSET_TRANSFORM_INTERNAL_HELPER: &str = r#"import {
  type TransformOptions,
  baseParse,
  generate,
  transform,
} from '@vue/compiler-core'
import {
  createSrcsetTransformWithOptions,
  transformSrcset,
} from '../src/template/transformSrcset'
import { transformElement } from '../../compiler-core/src/transforms/transformElement'
import { transformBind } from '../../compiler-core/src/transforms/vBind'
import {
  type AssetURLOptions,
  normalizeOptions,
} from '../src/template/transformAssetUrl'
import { stringifyStatic } from '../../compiler-dom/src/transforms/stringifyStatic'

function compileWithSrcset(
  template: string,
  options?: AssetURLOptions,
  transformOptions?: TransformOptions,
) {
  const ast = baseParse(template)
  const srcsetTransform = options
    ? createSrcsetTransformWithOptions(normalizeOptions(options))
    : transformSrcset
  transform(ast, {
    hoistStatic: true,
    nodeTransforms: [srcsetTransform, transformElement],
    directiveTransforms: {
      bind: transformBind,
    },
    ...transformOptions,
  })
  return generate(ast, { mode: 'module' })
}
"#;

const VUE3_SFC_ASSET_TRANSFORM_PUBLIC_HELPER_IMPORT: &str = r#"import {
  type AssetURLOptions,
  type TransformOptions,
  compileWithAssetUrls,
  stringifyStatic,
} from './templateTransforms.public-api'
"#;

const VUE3_SFC_SRCSET_TRANSFORM_PUBLIC_HELPER_IMPORT: &str = r#"import {
  type AssetURLOptions,
  type TransformOptions,
  compileWithSrcset,
  stringifyStatic,
} from './templateTransforms.public-api'
"#;

const VUE3_SFC_TEMPLATE_TRANSFORMS_PUBLIC_API_HELPER: &str = r#"import { compileTemplate } from '@vue/compiler-sfc'

export interface AssetURLOptions {
  base?: string | null
  includeAbsolute?: boolean
  tags?: Record<string, string[]>
}

export interface TransformOptions {
  hoistStatic?: boolean
  transformHoist?: unknown
}

export function stringifyStatic(): void {}

function compilePublic(
  template: string,
  transformAssetUrls: AssetURLOptions | undefined,
  transformOptions: TransformOptions | undefined,
  defaultHoistStatic: boolean,
) {
  const compilerOptions: Record<string, unknown> = {
    hoistStatic:
      transformOptions && transformOptions.hoistStatic !== undefined
        ? transformOptions.hoistStatic
        : defaultHoistStatic,
  }
  if (transformOptions && transformOptions.transformHoist !== undefined) {
    compilerOptions.transformHoist = transformOptions.transformHoist
  }
  return compileTemplate({
    source: template,
    filename: 'template.vue',
    id: 'data-v-template-transform',
    transformAssetUrls,
    compilerOptions,
  } as any)
}

export function compileWithAssetUrls(
  template: string,
  options?: AssetURLOptions,
  transformOptions?: TransformOptions,
) {
  return compilePublic(template, options, transformOptions, false)
}

export function compileWithSrcset(
  template: string,
  options?: AssetURLOptions,
  transformOptions?: TransformOptions,
) {
  return compilePublic(
    template,
    {
      ...(options || {}),
      tags: {
        img: [],
        source: [],
      },
    },
    transformOptions,
    true,
  )
}
"#;

const VUE3_SFC_RESOLVE_TYPE_INTERNAL_IMPORTS: &str = r#"import { normalize } from 'node:path'
import type { Identifier } from '@babel/types'
import { type SFCScriptCompileOptions, parse } from '../../src'
import { ScriptCompileContext } from '../../src/script/context'
import {
  inferRuntimeType,
  invalidateTypeCache,
  recordImports,
  registerTS,
  resolveTypeElements,
} from '../../src/script/resolveType'
import { UNKNOWN_TYPE } from '../../src/script/utils'
import ts from 'typescript'

registerTS(() => ts)
"#;

const VUE3_SFC_RESOLVE_TYPE_PUBLIC_IMPORTS: &str = r#"import { normalize } from 'node:path'
import { resolve, UNKNOWN_TYPE } from './resolveType.rust-api'
"#;

const VUE3_SFC_RESOLVE_TYPE_INTERNAL_HELPER: &str = r#"function resolve(
  code: string,
  files: Record<string, string> = {},
  options?: Partial<SFCScriptCompileOptions>,
  sourceFileName: string = '/Test.vue',
  invalidateCache = true,
) {
  const { descriptor } = parse(`<script setup lang="ts">\n${code}\n</script>`, {
    filename: sourceFileName,
  })
  const ctx = new ScriptCompileContext(descriptor, {
    id: 'test',
    fs: {
      fileExists(file) {
        return !!(files[file] ?? files[normalize(file)])
      },
      readFile(file) {
        return files[file] ?? files[normalize(file)]
      },
    },
    ...options,
  })

  if (invalidateCache) {
    for (const file in files) {
      invalidateTypeCache(file)
    }
  }

  // ctx.userImports is collected when calling compileScript(), but we are
  // skipping that here, so need to manually register imports
  ctx.userImports = recordImports(ctx.scriptSetupAst!.body) as any

  let target: any
  for (const s of ctx.scriptSetupAst!.body) {
    if (
      s.type === 'ExpressionStatement' &&
      s.expression.type === 'CallExpression' &&
      (s.expression.callee as Identifier).name === 'defineProps'
    ) {
      target = s.expression.typeParameters!.params[0]
    }
  }
  const raw = resolveTypeElements(ctx, target)
  const props: Record<string, string[]> = {}
  for (const key in raw.props) {
    props[key] = inferRuntimeType(ctx, raw.props[key])
  }
  return {
    props,
    calls: raw.calls,
    deps: ctx.deps,
    raw,
  }
}
"#;

const VUE3_SFC_RESOLVE_TYPE_RUST_API_HELPER: &str = r#"import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { __vuecRuntime } from '@vue/compiler-sfc'

export const UNKNOWN_TYPE = 'Unknown'

type ResolveOptions = {
  globalTypeFiles?: string[]
  [key: string]: unknown
}

type MaterializedFiles = {
  root: string
  virtualToReal: Map<string, string>
  realToVirtual: Map<string, string>
}

function normalizeReal(file: string): string {
  return path.resolve(file).replace(/\\/g, '/')
}

function normalizeVirtual(file: string): string {
  return file.replace(/\\/g, '/')
}

function virtualRelativePath(file: string): string {
  const normalized = normalizeVirtual(file)
  const drive = /^([A-Za-z]):\/(.*)$/.exec(normalized)
  if (drive) {
    return path.join(`__drive_${drive[1].toUpperCase()}`, ...drive[2].split('/').filter(Boolean))
  }
  return path.join(...normalized.replace(/^\/+/, '').split('/').filter(Boolean))
}

function materializedPath(root: string, file: string): string {
  const relative = virtualRelativePath(file)
  return relative ? path.join(root, relative) : root
}

function materializeFiles(files: Record<string, string>): MaterializedFiles {
  const root = mkdtempSync(path.join(tmpdir(), 'vuec-resolve-type-'))
  const virtualToReal = new Map<string, string>()
  const realToVirtual = new Map<string, string>()
  for (const [virtual, source] of Object.entries(files)) {
    const real = materializedPath(root, virtual)
    mkdirSync(path.dirname(real), { recursive: true })
    writeFileSync(real, source)
    virtualToReal.set(virtual, real)
    virtualToReal.set(normalizeVirtual(virtual), real)
    realToVirtual.set(normalizeReal(real), virtual)
  }
  return { root, virtualToReal, realToVirtual }
}

function mapVirtualToReal(materialized: MaterializedFiles, file: string): string {
  return materialized.virtualToReal.get(file)
    || materialized.virtualToReal.get(normalizeVirtual(file))
    || materializedPath(materialized.root, file)
}

function mapOptions(options: ResolveOptions | undefined, materialized: MaterializedFiles): ResolveOptions | undefined {
  if (!options) {
    return options
  }
  const mapped: ResolveOptions = { ...options }
  if (Array.isArray(options.globalTypeFiles)) {
    mapped.globalTypeFiles = options.globalTypeFiles.map(file => mapVirtualToReal(materialized, file))
  }
  return mapped
}

function mapDep(dep: string, materialized: MaterializedFiles): string {
  const direct = materialized.realToVirtual.get(normalizeReal(dep))
  if (direct) {
    return direct
  }
  const relative = path.relative(materialized.root, dep).replace(/\\/g, '/')
  if (!relative.startsWith('..') && !path.isAbsolute(relative)) {
    const drive = /^__drive_([A-Z])\/(.*)$/.exec(relative)
    return drive ? `${drive[1]}:/${drive[2]}` : `/${relative}`
  }
  return dep
}

function mapDeps(deps: string[] | undefined, materialized: MaterializedFiles): string[] {
  const mapped: string[] = []
  const seen = new Set<string>()
  for (const dep of deps || []) {
    const virtual = mapDep(dep, materialized)
    if (!seen.has(virtual)) {
      seen.add(virtual)
      mapped.push(virtual)
    }
  }
  return mapped
}

export function resolve(
  code: string,
  files: Record<string, string> = {},
  options?: ResolveOptions,
  sourceFileName: string = '/Test.vue',
  _invalidateCache = true,
) {
  const materialized = materializeFiles(files)
  try {
    const filename = mapVirtualToReal(materialized, sourceFileName)
    const parent = path.dirname(filename)
    if (!existsSync(parent)) {
      mkdirSync(parent, { recursive: true })
    }
    const result = __vuecRuntime.callBridge('sfc.resolveType', {
      code,
      filename,
      options: mapOptions(options, materialized),
    })
    if (Array.isArray(result.errors) && result.errors.length > 0) {
      throw new Error(String(result.errors[0]))
    }
    const calls = Array.isArray(result.calls) ? result.calls : []
    const raw = result.raw || {}
    return {
      props: result.props || {},
      calls,
      deps: mapDeps(result.deps, materialized),
      raw: {
        ...raw,
        props: raw.props || {},
        calls: Array.isArray(raw.calls) ? raw.calls : calls,
      },
    }
  } finally {
    rmSync(materialized.root, { recursive: true, force: true })
  }
}
"#;

fn rewrite_vue3_sfc_public_api_spec_imports(prepared_root: &Path) -> Result<()> {
    let tests = prepared_root
        .join("packages")
        .join("compiler-sfc")
        .join("__tests__");
    let parse_spec = prepared_root
        .join("packages")
        .join("compiler-sfc")
        .join("__tests__")
        .join("parse.spec.ts");
    rewrite_text_file_import(
        &parse_spec,
        "import { parse } from '../src'",
        "import { parse } from '@vue/compiler-sfc'",
    )?;

    let rewrite_default_spec = prepared_root
        .join("packages")
        .join("compiler-sfc")
        .join("__tests__")
        .join("rewriteDefault.spec.ts");
    rewrite_text_file_import(
        &rewrite_default_spec,
        "import { rewriteDefault } from '../src'",
        "import { rewriteDefault } from '@vue/compiler-sfc'",
    )?;

    let compile_style_spec = prepared_root
        .join("packages")
        .join("compiler-sfc")
        .join("__tests__")
        .join("compileStyle.spec.ts");
    rewrite_text_file_import(
        &compile_style_spec,
        "from '../src/compileStyle'",
        "from '@vue/compiler-sfc'",
    )?;

    let compile_template_spec = tests.join("compileTemplate.spec.ts");
    rewrite_text_file_import(
        &compile_template_spec,
        "from '../src/compileTemplate'",
        "from '@vue/compiler-sfc'",
    )?;
    rewrite_text_file_import(
        &compile_template_spec,
        "from '../src/parse'",
        "from '@vue/compiler-sfc'",
    )?;
    rewrite_text_file_import(
        &compile_template_spec,
        "import { compileScript } from '../src'",
        "import { compileScript } from '@vue/compiler-sfc'",
    )?;
    rewrite_text_file_import(
        &compile_template_spec,
        "from './utils'",
        "from './utils.public-api'",
    )?;

    let css_vars_spec = tests.join("cssVars.spec.ts");
    rewrite_text_file_import(
        &css_vars_spec,
        "import { compileStyle, parse } from '../src'",
        "import { compileStyle, parse } from '@vue/compiler-sfc'",
    )?;
    rewrite_text_file_import(
        &css_vars_spec,
        "from './utils'",
        "from './utils.public-api'",
    )?;
    let compile_script_spec = tests.join("compileScript.spec.ts");
    rewrite_text_file_import(
        &compile_script_spec,
        "from './utils'",
        "from './utils.public-api'",
    )?;
    for compile_script_spec in [
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
        rewrite_text_file_import(
            &tests.join("compileScript").join(compile_script_spec),
            "from '../utils'",
            "from '../utils.public-api'",
        )?;
    }
    rewrite_vue3_sfc_resolve_type_public_api_spec(&tests)?;
    let template_utils_spec = tests.join("templateUtils.spec.ts");
    rewrite_text_file_import(
        &template_utils_spec,
        "from '../src/template/templateUtils'",
        "from './templateUtils.rust-api'",
    )?;
    if template_utils_spec.exists() {
        write_text(
            &tests.join("templateUtils.rust-api.ts"),
            r#"import { __vuecRuntime } from '@vue/compiler-sfc'

function callTemplateUtils(command: string, url: string): boolean {
  return __vuecRuntime.callBridge(command, { url }) === true
}

export function isRelativeUrl(url: string): boolean {
  return callTemplateUtils('sfc.templateUtils.isRelativeUrl', url)
}

export function isExternalUrl(url: string): boolean {
  return callTemplateUtils('sfc.templateUtils.isExternalUrl', url)
}

export function isDataUrl(url: string): boolean {
  return callTemplateUtils('sfc.templateUtils.isDataUrl', url)
}
"#,
        )?;
    }
    rewrite_vue3_sfc_template_transform_public_api_specs(&tests)?;
    if css_vars_spec.exists() || compile_template_spec.exists() || compile_script_spec.exists() {
        write_text(
            &tests.join("utils.public-api.ts"),
            r#"import {
  type SFCParseOptions,
  type SFCScriptBlock,
  type SFCScriptCompileOptions,
  compileScript,
  parse,
} from '@vue/compiler-sfc'
import { parse as babelParse } from '@babel/parser'
import { warnOnce } from '../src/warn'

export const mockId = 'xxxxxxxx'

export function compileSFCScript(
  src: string,
  options?: Partial<SFCScriptCompileOptions>,
  parseOptions?: SFCParseOptions,
): SFCScriptBlock {
  const { descriptor, errors } = parse(src, parseOptions)
  if (errors.length) {
    console.warn(errors[0])
  }
  const warnings: string[] = []
  const originalWarn = console.warn
  console.warn = (...args: unknown[]) => {
    const message = args.map(arg => String(arg)).join(' ')
    warnings.push(message)
    originalWarn(...args)
  }
  try {
    return compileScript(descriptor, {
      __vuecEmitScriptSetupMarker: false,
      ...options,
      id: mockId,
    } as any)
  } finally {
    console.warn = originalWarn
    for (const warning of warnings) {
      warnOnce(warning)
    }
  }
}

export function assertCode(code: string): void {
  try {
    babelParse(code, {
      sourceType: 'module',
      plugins: [
        'typescript',
        ['importAttributes', { deprecatedAssertSyntax: true }],
      ],
    })
  } catch (e: any) {
    console.log(code)
    throw e
  }
  expect(code).toMatchSnapshot()
}

interface Pos {
  line: number
  column: number
  name?: string
}

export function getPositionInCode(
  code: string,
  token: string,
  expectName: string | boolean = false,
): Pos {
  const generatedOffset = code.indexOf(token)
  let line = 1
  let lastNewLinePos = -1
  for (let i = 0; i < generatedOffset; i++) {
    if (code.charCodeAt(i) === 10) {
      line++
      lastNewLinePos = i
    }
  }
  const res: Pos = {
    line,
    column:
      lastNewLinePos === -1
        ? generatedOffset
        : generatedOffset - lastNewLinePos - 1,
  }
  if (expectName) {
    res.name = typeof expectName === 'string' ? expectName : token
  }
  return res
}
"#,
        )?;
    }
    Ok(())
}

fn rewrite_vue3_sfc_resolve_type_public_api_spec(tests: &Path) -> Result<()> {
    let resolve_type_spec = tests.join("compileScript").join("resolveType.spec.ts");
    rewrite_text_file_block(
        &resolve_type_spec,
        VUE3_SFC_RESOLVE_TYPE_INTERNAL_IMPORTS,
        VUE3_SFC_RESOLVE_TYPE_PUBLIC_IMPORTS,
        "Vue 3 SFC resolveType public Rust helper imports",
    )?;
    rewrite_text_file_block(
        &resolve_type_spec,
        VUE3_SFC_RESOLVE_TYPE_INTERNAL_HELPER,
        "",
        "Vue 3 SFC resolveType public Rust helper body",
    )?;
    if resolve_type_spec.exists() {
        write_text(
            &tests.join("compileScript").join("resolveType.rust-api.ts"),
            VUE3_SFC_RESOLVE_TYPE_RUST_API_HELPER,
        )?;
    }
    Ok(())
}

fn rewrite_vue3_sfc_template_transform_public_api_specs(tests: &Path) -> Result<()> {
    let asset_transform_spec = tests.join("templateTransformAssetUrl.spec.ts");
    rewrite_text_file_block(
        &asset_transform_spec,
        VUE3_SFC_ASSET_TRANSFORM_INTERNAL_HELPER,
        VUE3_SFC_ASSET_TRANSFORM_PUBLIC_HELPER_IMPORT,
        "Vue 3 SFC template asset transform public helper",
    )?;

    let srcset_transform_spec = tests.join("templateTransformSrcset.spec.ts");
    rewrite_text_file_block(
        &srcset_transform_spec,
        VUE3_SFC_SRCSET_TRANSFORM_INTERNAL_HELPER,
        VUE3_SFC_SRCSET_TRANSFORM_PUBLIC_HELPER_IMPORT,
        "Vue 3 SFC template srcset transform public helper",
    )?;

    if asset_transform_spec.exists() || srcset_transform_spec.exists() {
        write_text(
            &tests.join("templateTransforms.public-api.ts"),
            VUE3_SFC_TEMPLATE_TRANSFORMS_PUBLIC_API_HELPER,
        )?;
    }
    Ok(())
}

fn rewrite_text_file_import(path: &Path, from: &str, to: &str) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let original =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let rewritten = original.replace(from, to);
    if rewritten != original {
        write_text(path, &rewritten)?;
    }
    Ok(())
}

fn rewrite_text_file_block(path: &Path, from: &str, to: &str, label: &str) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let original =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let normalized = original.replace("\r\n", "\n");
    if normalized.contains(from) {
        return write_text(path, &normalized.replace(from, to));
    }
    ensure!(
        normalized.contains(to),
        "{} anchor not found in {}",
        label,
        path.display()
    );
    Ok(())
}

fn write_vue3_sfc_conformance_shims(prepared_root: &Path) -> Result<()> {
    write_json(
        &prepared_root.join("package.json"),
        &serde_json::json!({
            "private": true,
            "type": "module",
        }),
    )?;
    write_vue3_core_test_setup(prepared_root)?;

    let config = r#"
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.dirname(fileURLToPath(import.meta.url))
const aliasRoot = process.env.VUEC_RUST_ALIAS_ROOT
const npmRoot = process.env.VUEC_OFFICIAL_NPM_ROOT

export default {
  oxc: {
    target: 'es2020',
  },
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
      '@vue/compiler-ssr': path.resolve(aliasRoot, 'node_modules/@vue/compiler-ssr/dist/compiler-ssr.cjs.js'),
      '@vue/compiler-sfc': path.resolve(aliasRoot, 'node_modules/@vue/compiler-sfc/dist/compiler-sfc.cjs.js'),
      '@vue/shared': path.resolve(npmRoot, 'node_modules/@vue/shared/index.js'),
      '@babel/parser': path.resolve(npmRoot, 'node_modules/@babel/parser/lib/index.js'),
      '@babel/types': path.resolve(npmRoot, 'node_modules/@babel/types/lib/index.js'),
      '@vue/consolidate': path.resolve(npmRoot, 'node_modules/@vue/consolidate/index.js'),
      'estree-walker': path.resolve(npmRoot, 'node_modules/estree-walker/dist/esm/estree-walker.js'),
      'hash-sum': path.resolve(npmRoot, 'node_modules/hash-sum/hash-sum.js'),
      'lru-cache': path.resolve(npmRoot, 'node_modules/lru-cache/dist/esm/index.js'),
      'magic-string': path.resolve(npmRoot, 'node_modules/magic-string/dist/magic-string.es.mjs'),
      'merge-source-map': path.resolve(npmRoot, 'node_modules/merge-source-map/index.js'),
      'minimatch': path.resolve(npmRoot, 'node_modules/minimatch/dist/esm/index.js'),
      'postcss': path.resolve(npmRoot, 'node_modules/postcss/lib/postcss.mjs'),
      'postcss-modules': path.resolve(npmRoot, 'node_modules/postcss-modules/build/index.js'),
      'postcss-selector-parser': path.resolve(npmRoot, 'node_modules/postcss-selector-parser/dist/index.js'),
      'pug': path.resolve(npmRoot, 'node_modules/pug/lib/index.js'),
      'sass': path.resolve(npmRoot, 'node_modules/sass/sass.node.mjs'),
      'source-map-js': path.resolve(npmRoot, 'node_modules/source-map-js/source-map.js'),
      'typescript': path.resolve(npmRoot, 'node_modules/typescript/lib/typescript.js'),
    },
  },
  test: {
    globals: true,
    pool: 'forks',
    fileParallelism: false,
    maxWorkers: 1,
    setupFiles: ['./vuec-vitest-setup.ts'],
    include: ['packages/compiler-sfc/__tests__/**/*.spec.ts'],
  },
}
"#;
    write_text(&prepared_root.join("vitest.config.ts"), config)?;
    Ok(())
}

fn prepare_vue3_ssr_conformance_suite(
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

    let official_ssr_tests = official_root
        .join("packages")
        .join("compiler-ssr")
        .join("__tests__");
    let prepared_ssr_tests = prepared_root
        .join("packages")
        .join("compiler-ssr")
        .join("__tests__");
    copy_dir_recursive(&official_ssr_tests, &prepared_ssr_tests)?;

    let official_ssr_src = official_root
        .join("packages")
        .join("compiler-ssr")
        .join("src");
    let prepared_ssr_src = prepared_root
        .join("packages")
        .join("compiler-ssr")
        .join("src");
    copy_dir_recursive(&official_ssr_src, &prepared_ssr_src)?;

    write_vue3_core_source_shims(&prepared_root)?;
    let official_dom_src = official_root
        .join("packages")
        .join("compiler-dom")
        .join("src");
    let prepared_dom_src = prepared_root
        .join("packages")
        .join("compiler-dom")
        .join("src");
    copy_dir_recursive(&official_dom_src, &prepared_dom_src)?;
    rewrite_vue3_ssr_rust_backed_public_compile_imports(&prepared_root)?;
    write_vue3_ssr_conformance_shims(&prepared_root)?;
    Ok(prepared_root)
}

fn rewrite_vue3_ssr_rust_backed_public_compile_imports(prepared_root: &Path) -> Result<()> {
    let tests = prepared_root
        .join("packages")
        .join("compiler-ssr")
        .join("__tests__");
    let ssr_text = tests.join("ssrText.spec.ts");
    if ssr_text.exists() {
        let original = fs::read_to_string(&ssr_text)
            .with_context(|| format!("failed to read {}", ssr_text.display()))?;
        let rewritten = original
            .replace(
                "import { compile } from '../src'",
                "import { compile } from '@vue/compiler-ssr'",
            )
            .replace(
                "import { getCompiledString } from './utils'",
                "import { getCompiledString } from './utils.rust-ssr-text'",
            );
        if rewritten != original {
            write_text(&ssr_text, &rewritten)?;
        }
    }

    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrVIf.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrVFor.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrScopeId.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrFallthroughAttrs.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrInjectCssVars.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrVShow.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrVModel.spec.ts"))?;
    rewrite_vue3_ssr_element_public_compile_imports(&tests.join("ssrElement.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrSlotOutlet.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrPortal.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrSuspense.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrTransition.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrTransitionGroup.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrComponent.spec.ts"))?;

    let utils = tests.join("utils.ts");
    if utils.exists() {
        let original = fs::read_to_string(&utils)
            .with_context(|| format!("failed to read {}", utils.display()))?;
        let rewritten = original.replace(
            "import { compile } from '../src'",
            "import { compile } from '@vue/compiler-ssr'",
        );
        write_text(&tests.join("utils.rust-ssr-text.ts"), &rewritten)?;
    }
    Ok(())
}

fn rewrite_vue3_ssr_element_public_compile_imports(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let original =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let rewritten = original
        .replace(
            "import { compile } from '../src'",
            "import { compile } from '@vue/compiler-ssr'",
        )
        .replace(
            "import { getCompiledString } from './utils'",
            "import { getCompiledString } from './utils.rust-ssr-text'",
        );
    if rewritten != original {
        write_text(path, &rewritten)?;
    }
    Ok(())
}

fn rewrite_vue3_ssr_spec_compile_import(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let original =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let rewritten = original.replace(
        "import { compile } from '../src'",
        "import { compile } from '@vue/compiler-ssr'",
    );
    if rewritten != original {
        write_text(path, &rewritten)?;
    }
    Ok(())
}

fn write_vue3_ssr_conformance_shims(prepared_root: &Path) -> Result<()> {
    write_vue3_core_test_setup(prepared_root)?;

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
      '@vue/compiler-dom': path.resolve(root, 'packages/compiler-dom/src/index.ts'),
      '@vue/compiler-ssr': path.resolve(aliasRoot, 'node_modules/@vue/compiler-ssr/dist/compiler-ssr.cjs.js'),
      '@vue/compiler-sfc': path.resolve(aliasRoot, 'node_modules/@vue/compiler-sfc/dist/compiler-sfc.cjs.js'),
      '@vue/shared': path.resolve(npmRoot, 'node_modules/@vue/shared/index.js'),
      'packages/compiler-core/src/transform': path.resolve(root, 'packages/compiler-core/src/transform.ts'),
      'source-map-js': path.resolve(npmRoot, 'node_modules/source-map-js/source-map.js'),
    },
  },
  test: {
    globals: true,
    pool: 'forks',
    setupFiles: ['./vuec-vitest-setup.ts'],
    include: ['packages/compiler-ssr/__tests__/**/*.spec.ts'],
  },
}
"#;
    write_text(&prepared_root.join("vitest.config.ts"), config)?;
    Ok(())
}

fn write_vue3_core_test_setup(prepared_root: &Path) -> Result<()> {
    fs::create_dir_all(prepared_root)
        .with_context(|| format!("failed to create {}", prepared_root.display()))?;
    write_text(
        &prepared_root.join("vuec-vitest-setup.ts"),
        r#"
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
    let failed_suites = json_usize(&value, &["numFailedTestSuites"]);
    let failed_tests = json_usize(&value, &["numFailedTests"]);
    let fail = failed_tests + failed_suites.saturating_sub(failed_tests);
    let skip = json_usize(&value, &["numPendingTests"]) + json_usize(&value, &["numTodoTests"]);
    let pass = json_usize(&value, &["numPassedTests"]);
    let total = json_usize(&value, &["numTotalTests"]).max(pass + fail + skip);
    let pending = total.saturating_sub(pass + fail + skip);
    Ok(ConformanceExecutionCounts {
        total,
        pass,
        fail,
        skip,
        pending,
    })
}

fn read_jasmine_counts(path: &Path) -> Result<ConformanceExecutionCounts> {
    let value = read_json::<serde_json::Value>(path)?;
    Ok(ConformanceExecutionCounts {
        total: json_usize(&value, &["counts", "total"]),
        pass: json_usize(&value, &["counts", "pass"]),
        fail: json_usize(&value, &["counts", "fail"]),
        skip: json_usize(&value, &["counts", "skip"]),
        pending: json_usize(&value, &["counts", "pending"]),
    })
}

fn json_conformance_file_counts(result: &serde_json::Value) -> ConformanceExecutionCounts {
    let Some(assertions) = result
        .get("assertionResults")
        .and_then(|value| value.as_array())
    else {
        return ConformanceExecutionCounts::default();
    };
    json_conformance_assertion_counts(assertions.iter())
}

fn json_conformance_assertion_counts<'a>(
    assertions: impl Iterator<Item = &'a serde_json::Value>,
) -> ConformanceExecutionCounts {
    let mut counts = ConformanceExecutionCounts::default();
    for assertion in assertions {
        counts.total += 1;
        match assertion
            .get("status")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
        {
            "passed" => counts.pass += 1,
            "failed" => counts.fail += 1,
            "pending" | "todo" | "skipped" => counts.skip += 1,
            _ => counts.pending += 1,
        }
    }
    counts.pending = counts
        .total
        .saturating_sub(counts.pass + counts.fail + counts.skip);
    counts
}

fn conformance_coverage_report(
    spec: ConformanceSuiteSpec,
    backend: AliasBackend,
    execution: Option<&ConformanceExecutionResult>,
) -> ConformanceCoverageReport {
    let source = conformance_coverage_kind(spec, backend);
    let default_reason = conformance_coverage_reason(spec, backend).to_string();
    let counts = execution.map(|result| result.counts).unwrap_or_default();
    let files = execution
        .and_then(|result| conformance_coverage_files(result, source, &default_reason).ok())
        .unwrap_or_default();
    let mut counts_by_source = BTreeMap::new();
    for kind in [
        ConformanceCoverageKind::RustBacked,
        ConformanceCoverageKind::ShimBacked,
        ConformanceCoverageKind::Mixed,
    ] {
        counts_by_source.insert(
            kind.as_str().to_string(),
            ConformanceExecutionCounts::default(),
        );
    }
    if files.is_empty() {
        if let Some(bucket) = counts_by_source.get_mut(source.as_str()) {
            *bucket = counts;
        }
    } else {
        for file in &files {
            if let Some(bucket) = counts_by_source.get_mut(file.source.as_str()) {
                bucket.total += file.counts.total;
                bucket.pass += file.counts.pass;
                bucket.fail += file.counts.fail;
                bucket.skip += file.counts.skip;
                bucket.pending += file.counts.pending;
            }
        }
    }
    let rust_backed = counts_by_source
        .get(ConformanceCoverageKind::RustBacked.as_str())
        .copied()
        .unwrap_or_default();
    let report_source = conformance_coverage_report_kind(source, &files);
    let reason = conformance_coverage_report_reason(spec, backend, report_source, &default_reason);
    ConformanceCoverageReport {
        source: report_source,
        reason,
        counts_by_source,
        rust_backed_pass: rust_backed.pass,
        rust_backed_total: rust_backed.total,
        files,
    }
}

fn conformance_coverage_report_reason(
    spec: ConformanceSuiteSpec,
    backend: AliasBackend,
    report_source: ConformanceCoverageKind,
    default_reason: &str,
) -> String {
    match (backend, spec.name, report_source) {
        (AliasBackend::Generated, "vue3-sfc", ConformanceCoverageKind::RustBacked) => {
            "Vue 3 compiler-sfc official tests run through a prepared Vitest suite whose files are all routed to public @vue/compiler-sfc helpers or Rust-backed projection helpers; generated import/API adapters only preserve official test import paths, materialize non-serializable test inputs, and hydrate public result shapes while compiler behavior routes through vuec_node_bridge into Rust."
                .to_string()
        }
        _ => default_reason.to_string(),
    }
}

fn conformance_coverage_report_kind(
    default: ConformanceCoverageKind,
    files: &[ConformanceCoverageFile],
) -> ConformanceCoverageKind {
    let Some(first) = files.first() else {
        return default;
    };
    if files.iter().all(|file| file.source == first.source) {
        first.source
    } else {
        ConformanceCoverageKind::Mixed
    }
}

fn conformance_coverage_kind(
    spec: ConformanceSuiteSpec,
    backend: AliasBackend,
) -> ConformanceCoverageKind {
    match backend {
        AliasBackend::Napi => ConformanceCoverageKind::Mixed,
        AliasBackend::Generated => match spec.name {
            "vue3-core" | "vue3-dom" | "vue3-sfc" | "vue3-ssr" => ConformanceCoverageKind::Mixed,
            _ => ConformanceCoverageKind::RustBacked,
        },
    }
}

fn conformance_coverage_reason(spec: ConformanceSuiteSpec, backend: AliasBackend) -> &'static str {
    match backend {
        AliasBackend::Napi => match spec.name {
            "vue2-compiler" => {
                "Vue 2.6 compiler official tests execute through a prepared Jasmine suite whose public vue-template-compiler package request resolves to the NAPI-backed official package-name alias. Prepared source shims still adapt official internal imports, so this report is mixed harness coverage; failures are real NAPI/Rust compiler parity gaps, not not-wired pending status."
            }
            "vue27-compiler" => {
                "Vue 2.7 compiler official tests execute through a prepared Vitest suite whose public vue-template-compiler package request resolves to the NAPI-backed official package-name alias. Prepared source shims still adapt official internal imports, so this report is mixed harness coverage; failures are real NAPI/Rust compiler parity gaps, not not-wired pending status."
            }
            "vue27-sfc" => {
                "Vue 2.7 compiler-sfc official tests execute through a prepared Vitest suite whose public vue/compiler-sfc package request resolves to the NAPI-backed official package-name alias. Prepared source shims and JavaScript-only callback adapters still participate, so this report is mixed harness coverage; failures are real NAPI/Rust SFC parity gaps, not not-wired pending status."
            }
            "vue3-core" => {
                "Vue 3 compiler-core official tests execute through a prepared Vitest suite whose public @vue/compiler-core package request resolves to the NAPI-backed official package-name alias. Generated source-path shims and internal helper adapters still participate, so this report is mixed harness coverage rather than pure NAPI semantic coverage."
            }
            "vue3-dom" => {
                "Vue 3 compiler-dom official tests execute through a prepared Vitest suite whose public @vue/compiler-dom and @vue/compiler-core package requests resolve to NAPI-backed official package-name aliases. Official TypeScript source, generated source-path shims, and helper adapters still participate, so this report is mixed harness coverage."
            }
            "vue3-sfc" => {
                "Vue 3 compiler-sfc official tests execute through a prepared Vitest suite whose public compiler package requests resolve to NAPI-backed official package-name aliases. Official SFC TypeScript source, generated source-path shims, and JavaScript-only helper adapters still participate, so this report is mixed harness coverage rather than standalone Rust SFC parity."
            }
            "vue3-ssr" => {
                "Vue 3 compiler-ssr official tests execute through a prepared Vitest suite whose public @vue/compiler-ssr and @vue/compiler-core package requests resolve to NAPI-backed official package-name aliases. Official SSR/DOM TypeScript source, generated source-path shims, and helper adapters still participate, so this report is mixed harness coverage."
            }
            _ => "Suite is routed through NAPI-backed official package-name aliases with prepared source shims.",
        },
        AliasBackend::Generated => match spec.name {
            "vue2-compiler" => {
                "Vue 2.6 compiler official tests execute through a prepared Jasmine suite. Generated source-path import shims preserve official internal module requests and route compiler/codeframe calls into the Rust vue-template-compiler alias through vuec_node_bridge; these failures are real Rust compiler parity gaps, not not-wired pending status."
            }
            "vue27-compiler" => {
                "Vue 2.7 compiler official tests execute through a prepared Vitest suite. Generated source-path import shims preserve official internal module requests and route compiler/codeframe calls into the Rust vue-template-compiler alias through vuec_node_bridge; these failures are real Rust compiler parity gaps, not not-wired pending status."
            }
            "vue27-sfc" => {
                "Vue 2.7 compiler-sfc official tests execute through a prepared Vitest suite. Generated source-path import shims preserve official imports and route public vue/compiler-sfc calls into the Rust alias through vuec_node_bridge; compileStyle PostCSS plugin callbacks execute in the JavaScript API adapter because caller-provided plugins are JavaScript functions and cannot be serialized into Rust. Remaining failures are real Vue 2.7 SFC parity gaps, not not-wired pending status."
            }
            "vue3-core" => {
                "Vue 3 compiler-core official tests run through generated import shims and the @vue/compiler-core alias runtime; public APIs call the Rust bridge, while many internal transform/codegen imports still execute JavaScript compatibility semantics in xtask/src/compat.rs."
            }
            "vue3-dom" => {
                "Vue 3 compiler-dom official tests run through a prepared Vitest suite with official DOM source imports, generated compiler-core import shims, and the @vue/compiler-dom alias runtime. Public compile/parse exports call the Rust bridge, but internal DOM transform imports mostly execute official TypeScript source or compatibility adapter code; only explicitly bridged projections count as Rust-backed."
            }
            "vue3-sfc" => {
                "Vue 3 compiler-sfc official tests run through a prepared Vitest suite with official SFC TypeScript source and generated aliases for @vue/compiler-core, @vue/compiler-dom, @vue/compiler-ssr, and @vue/compiler-sfc. The SFC source under test executes mixed official TypeScript logic plus Rust alias bridge calls for compiler dependencies; this runner is conformance harness coverage, not standalone Rust SFC parity."
            }
            "vue3-ssr" => {
                "Vue 3 compiler-ssr official tests run through a prepared Vitest suite with official SSR and DOM source imports, generated compiler-core import shims, and the alias runtime. Public @vue/compiler-ssr exports call the Rust bridge, but prepared SSR source tests execute mixed official TypeScript source, alias adapter code, and Rust bridge projections."
            }
            _ => "Suite is routed through Rust alias package smoke/output paths.",
        },
    }
}

fn conformance_coverage_files(
    execution: &ConformanceExecutionResult,
    source: ConformanceCoverageKind,
    reason: &str,
) -> Result<Vec<ConformanceCoverageFile>> {
    let output_file = PathBuf::from(&execution.output_file);
    let value = read_json::<serde_json::Value>(&output_file)?;
    let mut files = Vec::new();
    if let Some(results) = value.get("testResults").and_then(|value| value.as_array()) {
        for result in results {
            let path = result
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .replace('\\', "/");
            files.extend(conformance_coverage_file_entries(
                &path, result, source, reason,
            ));
        }
    }
    Ok(files)
}

fn conformance_coverage_file_entries(
    path: &str,
    result: &serde_json::Value,
    source: ConformanceCoverageKind,
    reason: &str,
) -> Vec<ConformanceCoverageFile> {
    if path.ends_with("packages/compiler-core/__tests__/transforms/transformElement.spec.ts") {
        let entries = conformance_coverage_transform_element_entries(path, result, reason);
        if !entries.is_empty() {
            return entries;
        }
    }
    if path.ends_with("packages/compiler-core/__tests__/transform.spec.ts") {
        let entries = conformance_coverage_transform_entries(path, result, reason);
        if !entries.is_empty() {
            return entries;
        }
    }

    let counts = json_conformance_file_counts(result);
    let file_source = conformance_coverage_file_kind(path, source);
    vec![ConformanceCoverageFile {
        path: path.to_string(),
        scope: None,
        source: file_source,
        reason: conformance_coverage_file_reason(path, file_source, reason),
        counts,
    }]
}

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

    let mut grouped: BTreeMap<(&'static str, ConformanceCoverageKind), Vec<&serde_json::Value>> =
        BTreeMap::new();
    for assertion in assertions {
        let Some(full_name) = assertion
            .get("fullName")
            .or_else(|| assertion.get("title"))
            .and_then(|value| value.as_str())
        else {
            return Vec::new();
        };
        let (scope, source) = conformance_coverage_transform_element_assertion_kind(full_name);
        grouped.entry((scope, source)).or_default().push(assertion);
    }

    grouped
        .into_iter()
        .map(|((scope, source), assertions)| ConformanceCoverageFile {
            path: path.to_string(),
            scope: Some(scope.to_string()),
            source,
            reason: conformance_coverage_transform_element_reason(scope, source, default_reason),
            counts: json_conformance_assertion_counts(assertions.into_iter()),
        })
        .collect()
}

fn conformance_coverage_transform_element_assertion_kind(
    full_name: &str,
) -> (&'static str, ConformanceCoverageKind) {
    if full_name.starts_with("compiler: v-for ") {
        return ("imported v-for helper", ConformanceCoverageKind::RustBacked);
    }
    if matches!(
        full_name,
        "compiler: element transform directiveTransforms"
            | "compiler: element transform directiveTransform with needRuntime: true"
            | "compiler: element transform directiveTransform with needRuntime: Symbol"
            | "compiler: element transform should process node when node has been replaced"
    ) {
        return ("js callback boundary", ConformanceCoverageKind::Mixed);
    }
    if full_name.starts_with("compiler: element transform ") {
        return (
            "element transform rust suite",
            ConformanceCoverageKind::RustBacked,
        );
    }
    ("unclassified assertions", ConformanceCoverageKind::Mixed)
}

fn conformance_coverage_transform_element_reason(
    scope: &str,
    source: ConformanceCoverageKind,
    default_reason: &str,
) -> String {
    match (scope, source) {
        ("imported v-for helper", ConformanceCoverageKind::RustBacked) => {
            "Official Vue 3 compiler-core transformElement file re-imports parseWithForTransform from the prepared vFor Rust API helper; these duplicated v-for assertions route through vuec_node_bridge command vue3.core.transformForSuite and Rust transformFor/codegen projections."
                .to_string()
        }
        ("element transform rust suite", ConformanceCoverageKind::RustBacked) => {
            "Official Vue 3 compiler-core transformElement file imports a prepared Rust API helper that forwards ordinary parseWithElementTransform/parseWithBind assertions through @vue/compiler-core.__vuecRuntime into vuec_node_bridge command vue3.core.transformElementSuite; the helper only normalizes serializable options, hydrates public AST helper symbols, restores public undefined fields, and emits Rust-projected errors while Rust parser, component resolution, props/children/text projections, runtime-directive materialization, dynamic component resolution, inline template-ref materialization, and root helper projection execute through Rust."
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

    let mut grouped: BTreeMap<(&'static str, ConformanceCoverageKind), Vec<&serde_json::Value>> =
        BTreeMap::new();
    for assertion in assertions {
        let Some(full_name) = assertion
            .get("fullName")
            .or_else(|| assertion.get("title"))
            .and_then(|value| value.as_str())
        else {
            return Vec::new();
        };
        let (scope, source) = conformance_coverage_transform_assertion_kind(full_name);
        grouped.entry((scope, source)).or_default().push(assertion);
    }

    grouped
        .into_iter()
        .map(|((scope, source), assertions)| ConformanceCoverageFile {
            path: path.to_string(),
            scope: Some(scope.to_string()),
            source,
            reason: conformance_coverage_transform_reason(scope, source, default_reason),
            counts: json_conformance_assertion_counts(assertions.into_iter()),
        })
        .collect()
}

fn conformance_coverage_transform_assertion_kind(
    full_name: &str,
) -> (&'static str, ConformanceCoverageKind) {
    if matches!(
        full_name,
        "compiler: transform should inject toString helper for interpolations"
            | "compiler: transform should inject createVNode and Comment for comments"
    ) || full_name.starts_with("compiler: transform root codegenNode ")
    {
        return ("transform rust suite", ConformanceCoverageKind::RustBacked);
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
        return (
            "js transform context boundary",
            ConformanceCoverageKind::Mixed,
        );
    }
    ("unclassified assertions", ConformanceCoverageKind::Mixed)
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
        ("js transform context boundary", ConformanceCoverageKind::Mixed) => {
            "Official Vue 3 compiler-core transform assertion group exercises caller-provided JavaScript NodeTransform callbacks and mutable transform context APIs such as replaceNode, removeNode, hoist, filename/selfName, and onError. These extension points cannot be serialized into the Rust bridge and remain mixed coverage rather than Rust compiler completion evidence."
                .to_string()
        }
        _ => default_reason.to_string(),
    }
}

fn conformance_coverage_file_kind(
    path: &str,
    default: ConformanceCoverageKind,
) -> ConformanceCoverageKind {
    if path.ends_with("packages/compiler-sfc/__tests__/parse.spec.ts")
        || path.ends_with("packages/compiler-sfc/__tests__/rewriteDefault.spec.ts")
        || path.ends_with("packages/compiler-sfc/__tests__/compileStyle.spec.ts")
        || path.ends_with("packages/compiler-sfc/__tests__/cssVars.spec.ts")
        || path.ends_with("packages/compiler-sfc/__tests__/compileScript.spec.ts")
        || path.ends_with("packages/compiler-sfc/__tests__/compileTemplate.spec.ts")
        || path.ends_with("packages/compiler-sfc/__tests__/templateUtils.spec.ts")
        || path.ends_with("packages/compiler-sfc/__tests__/templateTransformAssetUrl.spec.ts")
        || path.ends_with("packages/compiler-sfc/__tests__/templateTransformSrcset.spec.ts")
        || path.ends_with("packages/compiler-sfc/__tests__/compileScript/defineEmits.spec.ts")
        || path.ends_with("packages/compiler-sfc/__tests__/compileScript/defineExpose.spec.ts")
        || path.ends_with("packages/compiler-sfc/__tests__/compileScript/defineModel.spec.ts")
        || path.ends_with("packages/compiler-sfc/__tests__/compileScript/defineOptions.spec.ts")
        || path.ends_with("packages/compiler-sfc/__tests__/compileScript/defineProps.spec.ts")
        || path.ends_with(
            "packages/compiler-sfc/__tests__/compileScript/definePropsDestructure.spec.ts",
        )
        || path.ends_with("packages/compiler-sfc/__tests__/compileScript/defineSlots.spec.ts")
        || path.ends_with("packages/compiler-sfc/__tests__/compileScript/hoistStatic.spec.ts")
        || path.ends_with("packages/compiler-sfc/__tests__/compileScript/importUsageCheck.spec.ts")
        || path.ends_with("packages/compiler-sfc/__tests__/compileScript/resolveType.spec.ts")
    {
        ConformanceCoverageKind::RustBacked
    } else if path.ends_with("packages/compiler-sfc/test/compileStyle.spec.ts") {
        ConformanceCoverageKind::Mixed
    } else if path.ends_with("packages/compiler-sfc/test/compileScript.spec.ts")
        || path.ends_with("packages/compiler-sfc/test/compileTemplate.spec.ts")
        || path.ends_with("packages/compiler-sfc/test/cssVars.spec.ts")
        || path.ends_with("packages/compiler-sfc/test/parseComponent.spec.ts")
        || path.ends_with("packages/compiler-sfc/test/prefixIdentifiers.spec.ts")
        || path.ends_with("packages/compiler-sfc/test/rewriteDefault.spec.ts")
        || path.ends_with("packages/compiler-sfc/test/stylePluginScoped.spec.ts")
    {
        ConformanceCoverageKind::RustBacked
    } else if path.ends_with("packages/compiler-core/__tests__/compile.spec.ts")
        || path.ends_with("packages/compiler-core/__tests__/codegen.spec.ts")
        || path.ends_with("packages/compiler-core/__tests__/parse.spec.ts")
        || path.ends_with("packages/compiler-core/__tests__/scopeId.spec.ts")
        || path.ends_with("packages/compiler-core/__tests__/utils.spec.ts")
        || path
            .ends_with("packages/compiler-core/__tests__/transforms/transformExpressions.spec.ts")
        || path.ends_with("packages/compiler-core/__tests__/transforms/transformSlotOutlet.spec.ts")
        || path.ends_with("packages/compiler-core/__tests__/transforms/transformText.spec.ts")
        || path.ends_with("packages/compiler-core/__tests__/transforms/vBind.spec.ts")
        || path.ends_with("packages/compiler-core/__tests__/transforms/cacheStatic.spec.ts")
        || path.ends_with("packages/compiler-core/__tests__/transforms/vFor.spec.ts")
        || path.ends_with("packages/compiler-core/__tests__/transforms/vIf.spec.ts")
        || path.ends_with("packages/compiler-core/__tests__/transforms/vModel.spec.ts")
        || path.ends_with("packages/compiler-core/__tests__/transforms/vMemo.spec.ts")
        || path.ends_with("packages/compiler-core/__tests__/transforms/vOn.spec.ts")
        || path.ends_with("packages/compiler-core/__tests__/transforms/vOnce.spec.ts")
        || path.ends_with("packages/compiler-core/__tests__/transforms/vSlot.spec.ts")
        || path.ends_with("packages/compiler-dom/__tests__/index.spec.ts")
        || path.ends_with("packages/compiler-dom/__tests__/decoderHtmlBrowser.spec.ts")
        || path.ends_with("packages/compiler-dom/__tests__/parse.spec.ts")
        || path.ends_with("packages/compiler-dom/__tests__/transforms/Transition.spec.ts")
        || path.ends_with("packages/compiler-dom/__tests__/transforms/ignoreSideEffectTags.spec.ts")
        || path.ends_with("packages/compiler-dom/__tests__/transforms/stringifyStatic.spec.ts")
        || path.ends_with("packages/compiler-dom/__tests__/transforms/transformStyle.spec.ts")
        || path.ends_with("packages/compiler-dom/__tests__/transforms/vHtml.spec.ts")
        || path.ends_with("packages/compiler-dom/__tests__/transforms/vModel.spec.ts")
        || path.ends_with("packages/compiler-dom/__tests__/transforms/vOn.spec.ts")
        || path.ends_with("packages/compiler-dom/__tests__/transforms/vShow.spec.ts")
        || path.ends_with("packages/compiler-dom/__tests__/transforms/vText.spec.ts")
        || path.ends_with("packages/compiler-dom/__tests__/transforms/validateHtmlNesting.spec.ts")
        || path.ends_with("packages/compiler-ssr/__tests__/ssrScopeId.spec.ts")
        || path.ends_with("packages/compiler-ssr/__tests__/ssrFallthroughAttrs.spec.ts")
        || path.ends_with("packages/compiler-ssr/__tests__/ssrInjectCssVars.spec.ts")
        || path.ends_with("packages/compiler-ssr/__tests__/ssrElement.spec.ts")
        || path.ends_with("packages/compiler-ssr/__tests__/ssrText.spec.ts")
        || path.ends_with("packages/compiler-ssr/__tests__/ssrPortal.spec.ts")
        || path.ends_with("packages/compiler-ssr/__tests__/ssrSlotOutlet.spec.ts")
        || path.ends_with("packages/compiler-ssr/__tests__/ssrSuspense.spec.ts")
        || path.ends_with("packages/compiler-ssr/__tests__/ssrTransition.spec.ts")
        || path.ends_with("packages/compiler-ssr/__tests__/ssrTransitionGroup.spec.ts")
        || path.ends_with("packages/compiler-ssr/__tests__/ssrComponent.spec.ts")
        || path.ends_with("packages/compiler-ssr/__tests__/ssrVFor.spec.ts")
        || path.ends_with("packages/compiler-ssr/__tests__/ssrVIf.spec.ts")
        || path.ends_with("packages/compiler-ssr/__tests__/ssrVModel.spec.ts")
        || path.ends_with("packages/compiler-ssr/__tests__/ssrVShow.spec.ts")
    {
        ConformanceCoverageKind::RustBacked
    } else {
        default
    }
}

fn conformance_coverage_file_reason(
    path: &str,
    source: ConformanceCoverageKind,
    default_reason: &str,
) -> String {
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
            "Official Vue 3 SFC compileTemplate file imports the public @vue/compiler-sfc API and routes ordinary DOM/SSR template compilation, preprocessing, AST reuse, diagnostics, asset URL transforms, and source maps through vuec_node_bridge into Rust; the generated JavaScript package boundary only materializes caller-provided custom compiler callbacks and hydrates/dehydrates public AST and error shapes."
                .to_string()
        }
        ConformanceCoverageKind::RustBacked
            if path.ends_with("packages/compiler-sfc/__tests__/compileStyle.spec.ts") =>
        {
            "Official Vue 3 SFC compileStyle file imports the public @vue/compiler-sfc API and routes CSS scoped/modules/preprocess compilation through vuec_node_bridge into Rust; the JavaScript package boundary only materializes caller-provided preprocess additionalData callbacks and normalizes public result shape."
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
        ConformanceCoverageKind::Mixed if default_reason.contains("Vue 2.7 compiler-sfc") => {
            "Official file exercises a mixed path: Rust vuec_node_bridge performs SFC style parsing, preprocessing, scoped/CSS-var transforms, maps, and diagnostics, while the generated JavaScript alias adapter executes caller-provided PostCSS plugin callbacks/options and Promise/LazyResult API behavior that cannot cross the JSON bridge."
                .to_string()
        }
        ConformanceCoverageKind::ShimBacked | ConformanceCoverageKind::Mixed => default_reason.to_string(),
    }
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

fn conformance_readiness(
    spec: ConformanceSuiteSpec,
    backend: AliasBackend,
) -> ConformanceReadiness {
    let alias_root = backend.root(spec.version_line);
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
    let mut missing_runner_dependencies = spec
        .runner_dependencies
        .iter()
        .filter(|dependency| !node_dependency_available(&npm_root, dependency))
        .map(|dependency| (*dependency).to_string())
        .collect::<Vec<_>>();
    if missing_runner_dependencies.is_empty() {
        missing_runner_dependencies.extend(conformance_runner_startup_dependency_errors(
            spec, &npm_root,
        ));
    }
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

fn verify_conformance_runner_startup_dependencies(
    spec: ConformanceSuiteSpec,
    node_modules: &Path,
) -> Result<()> {
    let errors = conformance_runner_startup_dependency_errors(spec, node_modules);
    if errors.is_empty() {
        return Ok(());
    }
    anyhow::bail!("{}", errors.join("; "))
}

fn conformance_runner_startup_dependency_errors(
    spec: ConformanceSuiteSpec,
    node_modules: &Path,
) -> Vec<String> {
    conformance_runner_startup_probe_requests(spec)
        .into_iter()
        .filter(|request| node_dependency_available(node_modules, request))
        .filter_map(|request| {
            probe_conformance_runner_startup_dependency(node_modules, request)
                .err()
                .map(|err| format!("runner-native:{request}: {err:#}"))
        })
        .collect()
}

fn conformance_runner_startup_probe_requests(spec: ConformanceSuiteSpec) -> Vec<&'static str> {
    if !spec.runner_dependencies.contains(&"vitest") {
        return Vec::new();
    }
    vec!["rollup", "rolldown"]
}

fn probe_conformance_runner_startup_dependency(node_modules: &Path, request: &str) -> Result<()> {
    let node = resolve_program("node");
    let output = Command::new(node)
        .arg("--input-type=module")
        .arg("-e")
        .arg(RUNNER_STARTUP_PROBE_SCRIPT)
        .env("VUEC_RUNNER_NODE_MODULES", absolute_path(node_modules))
        .env("VUEC_RUNNER_PROBE_REQUEST", request)
        .output()
        .with_context(|| format!("failed to spawn node runner startup probe for {request}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "node runner startup probe failed with status {:?}: {}",
            output.status.code(),
            normalize_command_output(&output.stdout, &output.stderr)
        );
    }
    Ok(())
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
            runner_dependencies: &["@babel/register", "jasmine", "jsdom"],
        },
        ConformanceSuite::Vue27Compiler => ConformanceSuiteSpec {
            name: "vue27-compiler",
            version_line: VersionLine::Vue27,
            relative_test_dirs: &["test/unit/modules/compiler"],
            package_requests: &["vue-template-compiler"],
            runner_dependencies: &["vitest", "esbuild", "typescript", "jsdom"],
        },
        ConformanceSuite::Vue27Sfc => ConformanceSuiteSpec {
            name: "vue27-sfc",
            version_line: VersionLine::Vue27,
            relative_test_dirs: &["packages/compiler-sfc/test"],
            package_requests: &["vue/compiler-sfc"],
            runner_dependencies: &["vitest", "esbuild", "typescript", "jsdom"],
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
            runner_dependencies: &["vitest", "esbuild", "source-map-js", "jsdom"],
        },
        ConformanceSuite::Vue3Sfc => ConformanceSuiteSpec {
            name: "vue3-sfc",
            version_line: VersionLine::Vue3,
            relative_test_dirs: &["packages/compiler-sfc/__tests__"],
            package_requests: &[
                "@vue/compiler-core",
                "@vue/compiler-dom",
                "@vue/compiler-sfc",
                "@vue/compiler-ssr",
            ],
            runner_dependencies: &[
                "@babel/parser",
                "@babel/types",
                "@vue/consolidate",
                "esbuild",
                "estree-walker",
                "hash-sum",
                "lru-cache",
                "magic-string",
                "merge-source-map",
                "minimatch",
                "postcss-modules",
                "postcss-selector-parser",
                "pug",
                "sass",
                "source-map-js",
                "typescript",
                "vitest",
            ],
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
    fn official_lock_rejects_floating_npm_versions() {
        let mut lock = OfficialRevisionsLock {
            vue2_6: BaselineLock {
                repo: "https://github.com/vuejs/vue".into(),
                rev: "612fb89547711cacb030a3893a0065b785802860".into(),
                npm: BTreeMap::from([
                    ("vue".into(), "2.6.14".into()),
                    ("vue-template-compiler".into(), "^2.6.14".into()),
                ]),
                exports: BTreeMap::new(),
            },
            vue2_7: BaselineLock {
                repo: "https://github.com/vuejs/vue".into(),
                rev: "13f4e7dc03e2caed900ac70ff8b8fe58dda45663".into(),
                npm: BTreeMap::from([
                    ("vue".into(), "2.7.16".into()),
                    ("vue-template-compiler".into(), "2.7.16".into()),
                ]),
                exports: BTreeMap::from([(
                    "vue/compiler-sfc".into(),
                    "./compiler-sfc/index.js".into(),
                )]),
            },
            vue3: BaselineLock {
                repo: "https://github.com/vuejs/core".into(),
                rev: "57545e958ae28ed17aa9e0ed321abcd8dc99f752".into(),
                npm: BTreeMap::from([
                    ("vue".into(), "3.5.34".into()),
                    ("@vue/compiler-core".into(), "3.5.34".into()),
                    ("@vue/compiler-dom".into(), "3.5.34".into()),
                    ("@vue/compiler-sfc".into(), "3.5.34".into()),
                    ("@vue/compiler-ssr".into(), "3.5.34".into()),
                ]),
                exports: BTreeMap::new(),
            },
        };

        let violations = validate_official_lock(&lock);
        assert!(violations
            .iter()
            .any(|violation| violation.contains("must be an exact npm package version")));

        lock.vue2_6
            .npm
            .insert("vue-template-compiler".into(), "latest".into());
        let violations = validate_official_lock(&lock);
        assert!(violations
            .iter()
            .any(|violation| violation.contains("must be an exact npm package version")));
    }

    #[test]
    fn official_lock_vendor_validation_rejects_tag_object_revs() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-official-lock-vendor-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let checkout = temp.join("vue2_6");
        fs::create_dir_all(checkout.join("packages/vue-template-compiler")).unwrap();
        run_command("git", &["init"], Some(&checkout)).unwrap();
        fs::write(checkout.join("package.json"), r#"{"version":"2.6.14"}"#).unwrap();
        fs::write(
            checkout
                .join("packages/vue-template-compiler")
                .join("package.json"),
            r#"{"version":"2.6.14"}"#,
        )
        .unwrap();
        run_git(&checkout, &["add", "."]).unwrap();
        run_git(
            &checkout,
            &[
                "-c",
                "user.email=a@b.c",
                "-c",
                "user.name=Vuec",
                "commit",
                "-m",
                "init",
            ],
        )
        .unwrap();
        let commit = git_output(&checkout, &["rev-parse", "HEAD"]).unwrap();
        run_git(
            &checkout,
            &[
                "-c",
                "user.email=a@b.c",
                "-c",
                "user.name=Vuec",
                "tag",
                "-a",
                "v2.6.14",
                "-m",
                "v2.6.14",
            ],
        )
        .unwrap();
        let tag_object = git_output(&checkout, &["rev-parse", "v2.6.14"]).unwrap();
        assert_ne!(tag_object, commit);

        let lock = OfficialRevisionsLock {
            vue2_6: BaselineLock {
                repo: "https://github.com/vuejs/vue".into(),
                rev: tag_object,
                npm: BTreeMap::from([
                    ("vue".into(), "2.6.14".into()),
                    ("vue-template-compiler".into(), "2.6.14".into()),
                ]),
                exports: BTreeMap::new(),
            },
            vue2_7: BaselineLock {
                repo: "https://github.com/vuejs/vue".into(),
                rev: "13f4e7dc03e2caed900ac70ff8b8fe58dda45663".into(),
                npm: BTreeMap::new(),
                exports: BTreeMap::new(),
            },
            vue3: BaselineLock {
                repo: "https://github.com/vuejs/core".into(),
                rev: "57545e958ae28ed17aa9e0ed321abcd8dc99f752".into(),
                npm: BTreeMap::new(),
                exports: BTreeMap::new(),
            },
        };

        let items = validate_official_lock_vendor(&lock, &temp);
        assert!(items.iter().any(|item| {
            item.target == "vue2_6.rev-object"
                && item.status == ReportStatus::Fail
                && item.detail.contains("expected commit")
        }));
        let _ = fs::remove_dir_all(temp);
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
    fn vue3_dom_core_runtime_exports_forward_to_alias_runtime() {
        let function_detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("unwrapTSNode".into()),
            function_arity: Some(1),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let object_detail = ApiExportDetail {
            kind: "object".into(),
            tag: "[object Object]".into(),
            name: None,
            function_arity: None,
            is_async_function: None,
            is_class_like: None,
            own_property_names: vec!["DATA".into(), "SETUP_CONST".into()],
        };
        let target = TargetSpec {
            version_line: VersionLine::Vue3,
            package: "@vue/compiler-dom",
            entry: "index",
            kind: TargetKind::Vue3Dom,
        };

        assert!(
            alias_export_expression(target, "unwrapTSNode", Some(&function_detail))
                .contains("vue3CoreRuntime[\"unwrapTSNode\"].apply")
        );
        assert_eq!(
            alias_export_expression(target, "BindingTypes", Some(&object_detail)),
            "vue3CoreRuntime[\"BindingTypes\"]"
        );
        assert_eq!(
            alias_export_expression(target, "parserOptions", Some(&object_detail)),
            "vue3DomParserOptions"
        );
        assert!(
            alias_export_expression(target, "createSimpleExpression", Some(&function_detail))
                .contains("vue3CoreRuntime[\"createSimpleExpression\"].apply")
        );
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
    fn vue27_sfc_compile_script_alias_hydrates_binding_metadata_shape() {
        let target = TargetSpec {
            version_line: VersionLine::Vue27,
            package: "vue",
            entry: "vue/compiler-sfc",
            kind: TargetKind::Vue27Sfc,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("compileScript".into()),
            function_arity: Some(2),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "compileScript", Some(&detail));

        assert!(expression.contains("hydrateVue27CompileScriptResult"));
        assert!(expression.contains("vue27CompileScriptBridgePayload"));
        assert!(ALIAS_RUNTIME_JS.contains("function hydrateVue27CompileScriptResult"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue27CompileScriptBridgePayload"));
        assert!(ALIAS_RUNTIME_JS.contains("__vuecEmitScriptSetupMarker = false"));
        assert!(ALIAS_RUNTIME_JS.contains("Object.defineProperty(bindings, '__isScriptSetup'"));
    }

    #[test]
    fn vue27_sfc_compile_style_alias_keeps_postcss_callbacks_in_js_adapter() {
        let target = TargetSpec {
            version_line: VersionLine::Vue27,
            package: "vue",
            entry: "vue/compiler-sfc",
            kind: TargetKind::Vue27Sfc,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("compileStyle".into()),
            function_arity: Some(1),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "compileStyle", Some(&detail));

        assert!(expression.contains("vue27StyleBridgePayload"));
        assert!(expression.contains("applyVue27StylePostcssSync"));
        assert!(expression.contains("sfc.vue27.compileStyle"));
        assert!(ALIAS_RUNTIME_JS.contains("function applyVue27StylePostcssSync"));
        assert!(ALIAS_RUNTIME_JS.contains("key !== 'postcssPlugins'"));
        assert!(ALIAS_RUNTIME_JS.contains("key !== 'postcssOptions'"));
    }

    #[test]
    fn vue3_sfc_compile_style_alias_emits_rust_style_warnings() {
        let target = TargetSpec {
            version_line: VersionLine::Vue3,
            package: "@vue/compiler-sfc",
            entry: "@vue/compiler-sfc",
            kind: TargetKind::Vue3Sfc,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("compileStyle".into()),
            function_arity: Some(1),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "compileStyle", Some(&detail));

        assert!(expression.contains("emitVue3StyleWarnings"));
        assert!(expression.contains("normalizeStyleAliasResult"));
        assert!(expression.contains("vue3StyleBridgePayload"));
        assert!(ALIAS_RUNTIME_JS.contains("function emitVue3StyleWarnings"));
        assert!(ALIAS_RUNTIME_JS.contains("function normalizeStyleAliasResult"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue3StyleBridgePayload"));
        assert!(ALIAS_RUNTIME_JS.contains("VUEC_STYLE_DEPRECATED_SCOPED_SELECTOR"));
    }

    #[test]
    fn vue3_sfc_parse_alias_hydrates_hmr_reload_api() {
        let target = TargetSpec {
            version_line: VersionLine::Vue3,
            package: "@vue/compiler-sfc",
            entry: "@vue/compiler-sfc",
            kind: TargetKind::Vue3Sfc,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("parse".into()),
            function_arity: Some(2),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "parse", Some(&detail));

        assert!(expression.contains("hydrateVue3SfcParseResult"));
        assert!(expression.contains("sfc.parse"));
        assert!(expression.contains("vue3SfcParseBridgePayload"));
        assert!(expression.contains("applyVue3SfcCustomCompilerParse"));
        assert!(ALIAS_RUNTIME_JS.contains("function hydrateVue3SfcParseResult"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue3SfcParseBridgePayload"));
        assert!(ALIAS_RUNTIME_JS.contains("function normalizeVue3SfcParseOptionsForBridge"));
        assert!(ALIAS_RUNTIME_JS.contains("function applyVue3SfcCustomCompilerParse"));
        assert!(ALIAS_RUNTIME_JS.contains("options.templateParseOptions"));
        assert!(ALIAS_RUNTIME_JS.contains("key !== 'compiler'"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue3SfcShouldForceReload"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue3SfcTemplateUsedIdentifiers"));
    }

    #[test]
    fn vue3_sfc_compile_script_alias_hydrates_binding_metadata_shape() {
        let target = TargetSpec {
            version_line: VersionLine::Vue3,
            package: "@vue/compiler-sfc",
            entry: "@vue/compiler-sfc",
            kind: TargetKind::Vue3Sfc,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("compileScript".into()),
            function_arity: Some(2),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "compileScript", Some(&detail));

        assert!(expression.contains("sfc.compileScript"));
        assert!(expression.contains("hydrateVue3CompileScriptResult"));
        assert!(expression.contains("vue3CompileScriptBridgePayload"));
        assert!(ALIAS_RUNTIME_JS.contains("function hydrateVue3CompileScriptResult"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue3CompileScriptBridgePayload"));
        assert!(ALIAS_RUNTIME_JS.contains("function throwVue3CompileScriptErrors"));
        assert!(ALIAS_RUNTIME_JS.contains("__vuecEmitScriptSetupMarker = false"));
        assert!(ALIAS_RUNTIME_JS
            .contains("options.__vuecCustomElement = !!options.customElement(filename)"));
        assert!(ALIAS_RUNTIME_JS.contains("delete options.customElement"));
        assert!(ALIAS_RUNTIME_JS.contains("bindings.__propsAliases = result.propsAliases"));
        assert!(ALIAS_RUNTIME_JS.contains("delete result.propsAliases"));
        assert!(ALIAS_RUNTIME_JS.contains("Array.isArray(result.warnings)"));
        assert!(ALIAS_RUNTIME_JS.contains("Object.defineProperty(bindings, '__isScriptSetup'"));
        assert!(ALIAS_RUNTIME_JS.contains("[@vue/compiler-sfc] ${message}"));
    }

    #[test]
    fn vue3_sfc_compile_template_alias_projects_public_api_boundary() {
        let target = TargetSpec {
            version_line: VersionLine::Vue3,
            package: "@vue/compiler-sfc",
            entry: "@vue/compiler-sfc",
            kind: TargetKind::Vue3Sfc,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("compileTemplate".into()),
            function_arity: Some(1),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "compileTemplate", Some(&detail));

        assert!(expression.contains("sfc.compileTemplate"));
        assert!(expression.contains("vue3SfcCompileTemplateBridgePayload"));
        assert!(expression.contains("vue3SfcCustomCompileTemplateResult"));
        assert!(expression.contains("hydrateVue3SfcCompileTemplateResult"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue3SfcCompileTemplateBridgePayload"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue3SfcCustomCompileTemplateResult"));
        assert!(ALIAS_RUNTIME_JS.contains("function hydrateVue3SfcCompileTemplateResult"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3CoreRuntime.dehydrateForBridge(options.ast)"));
        assert!(ALIAS_RUNTIME_JS.contains("compiler.compile(source, compilerOptions)"));
        assert!(ALIAS_RUNTIME_JS.contains("new SyntaxError(message)"));
        assert!(ALIAS_RUNTIME_JS.contains("bridgeOptions.ssrCssVars = options.ssrCssVars"));
    }

    #[test]
    fn vue3_sfc_rewrite_default_alias_routes_to_rust_bridge() {
        let target = TargetSpec {
            version_line: VersionLine::Vue3,
            package: "@vue/compiler-sfc",
            entry: "@vue/compiler-sfc",
            kind: TargetKind::Vue3Sfc,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("rewriteDefault".into()),
            function_arity: Some(3),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "rewriteDefault", Some(&detail));

        assert!(expression.contains("sfc.rewriteDefault"));
        assert!(expression.contains("plugins: a2"));
        assert!(!expression.contains("sfc.vue27.rewriteDefault"));
    }

    #[test]
    fn napi_vue3_sfc_native_alias_hydrates_hmr_reload_api() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let source = fs::read_to_string(
            repo_root
                .join("packages")
                .join("native-aliases")
                .join("@vue")
                .join("compiler-sfc")
                .join("dist")
                .join("compiler-sfc.cjs.js"),
        )
        .unwrap();

        assert!(source.contains("hydrateVue3SfcParseResult(native.parseSfcResult"));
        assert!(source.contains("function hydrateVue3SfcParseResult"));
        assert!(source.contains("function vue3SfcShouldForceReload"));
        assert!(source.contains("descriptor.shouldForceReload = function shouldForceReload"));
    }

    #[test]
    fn napi_vue3_sfc_native_alias_hydrates_compile_script_bindings() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let source = fs::read_to_string(
            repo_root
                .join("packages")
                .join("native-aliases")
                .join("@vue")
                .join("compiler-sfc")
                .join("dist")
                .join("compiler-sfc.cjs.js"),
        )
        .unwrap();

        assert!(source.contains(
            "return hydrateVue3CompileScriptResult(native.compileScript(descriptor || {}, options || {}));"
        ));
        assert!(source.contains("function hydrateVue3CompileScriptResult"));
        assert!(source.contains("function throwVue3CompileScriptErrors"));
        assert!(source.contains("bindings.__propsAliases = result.propsAliases"));
        assert!(source.contains("delete result.propsAliases"));
        assert!(source.contains("Array.isArray(result.warnings)"));
        assert!(source.contains("Object.defineProperty(bindings, '__isScriptSetup'"));
        assert!(source.contains("[@vue/compiler-sfc] ${message}"));
    }

    #[test]
    fn napi_vue3_sfc_native_alias_routes_rewrite_default_to_vue3_native_api() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let source = fs::read_to_string(
            repo_root
                .join("packages")
                .join("native-aliases")
                .join("@vue")
                .join("compiler-sfc")
                .join("dist")
                .join("compiler-sfc.cjs.js"),
        )
        .unwrap();

        assert!(source.contains("function rewriteDefault(source, as, parserPlugins)"));
        assert!(source.contains(
            "return native.rewriteDefaultVue3(String(source || ''), String(as || ''), parserPlugins || []);"
        ));
        assert!(!source.contains(
            "return native.rewriteDefaultVue27(String(source || ''), String(as || ''), parserPlugins || []);"
        ));
    }

    #[test]
    fn vue3_ssr_compile_alias_hydrates_public_ast_helpers() {
        let target = TargetSpec {
            version_line: VersionLine::Vue3,
            package: "@vue/compiler-ssr",
            entry: "@vue/compiler-ssr",
            kind: TargetKind::Vue3Ssr,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("compile".into()),
            function_arity: Some(2),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "compile", Some(&detail));

        assert!(expression.contains("hydrateVue3SsrCompileResult"));
        assert!(expression.contains("vue3.ssr.compile"));
        assert!(ALIAS_RUNTIME_JS.contains("function hydrateVue3SsrCompileResult"));
        assert!(ALIAS_RUNTIME_JS.contains("new Set(result.ast_helpers.map(name => Symbol(name)))"));
    }

    #[test]
    fn napi_vue3_ssr_native_alias_normalizes_custom_element_predicates() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let source = fs::read_to_string(
            repo_root
                .join("packages")
                .join("native-aliases")
                .join("@vue")
                .join("compiler-ssr")
                .join("dist")
                .join("compiler-ssr.cjs.js"),
        )
        .unwrap();

        assert!(source.contains("vue3SsrNativeOptions(options, template)"));
        assert!(source.contains("Object.assign(vue3SsrNativeOptions(options, template)"));
        assert!(source.contains(
            "out.__vuecCustomElements = collectVuePredicateHits(options.isCustomElement, tags);"
        ));
        assert!(!source.contains("native.compileVue3Ssr(String(source || ''), options || {})"));
    }

    #[test]
    fn sfc_compile_style_alias_strips_public_source_map_option() {
        assert!(ALIAS_RUNTIME_JS.contains("key !== 'sourceMap' && key !== 'source_map'"));
        assert!(ALIAS_RUNTIME_JS.contains("key !== 'sourceMap'"));
    }

    #[test]
    fn option_matrix_compile_style_passes_css_source_on_both_sides() {
        assert!(OPTION_MATRIX_PROBE_SCRIPT
            .contains("api.compileStyle(optionObjectWithSource(extractStyleSource(fixture)))"));
        assert!(!OPTION_MATRIX_PROBE_SCRIPT
            .contains("side === 'official' ? extractStyleSource(fixture) : fixture"));
    }

    #[test]
    fn vue27_sfc_compile_style_async_alias_returns_postcss_promise_adapter() {
        let target = TargetSpec {
            version_line: VersionLine::Vue27,
            package: "vue",
            entry: "vue/compiler-sfc",
            kind: TargetKind::Vue27Sfc,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("compileStyleAsync".into()),
            function_arity: Some(1),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "compileStyleAsync", Some(&detail));

        assert!(expression.contains("applyVue27StylePostcssAsync"));
        assert!(expression.contains("sfc.vue27.compileStyleAsync"));
        assert!(ALIAS_RUNTIME_JS.contains("function applyVue27StylePostcssAsync"));
        assert!(
            ALIAS_RUNTIME_JS.contains("return Promise.resolve(normalizeStyleAliasResult(out));")
        );
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
    fn report_metadata_records_lock_versions_and_rust_commit() {
        let lock = OfficialRevisionsLock {
            vue2_6: BaselineLock {
                repo: "https://github.com/vuejs/vue".into(),
                rev: "612fb89547711cacb030a3893a0065b785802860".into(),
                npm: BTreeMap::new(),
                exports: BTreeMap::new(),
            },
            vue2_7: BaselineLock {
                repo: "https://github.com/vuejs/vue".into(),
                rev: "13f4e7dc03e2caed900ac70ff8b8fe58dda45663".into(),
                npm: BTreeMap::new(),
                exports: BTreeMap::new(),
            },
            vue3: BaselineLock {
                repo: "https://github.com/vuejs/core".into(),
                rev: "57545e958ae28ed17aa9e0ed321abcd8dc99f752".into(),
                npm: BTreeMap::new(),
                exports: BTreeMap::new(),
            },
        };
        let metadata =
            ReportMetadata::capture().with_lock_context(Some("lock-hash".into()), Some(&lock));

        assert_eq!(metadata.lock_hash.as_deref(), Some("lock-hash"));
        assert_eq!(
            metadata.official_commits.get("vue2_6").map(String::as_str),
            Some("612fb89547711cacb030a3893a0065b785802860")
        );
        assert_eq!(
            metadata.official_commits.get("vue2_7").map(String::as_str),
            Some("13f4e7dc03e2caed900ac70ff8b8fe58dda45663")
        );
        assert_eq!(
            metadata.official_commits.get("vue3").map(String::as_str),
            Some("57545e958ae28ed17aa9e0ed321abcd8dc99f752")
        );
        assert!(metadata
            .rust_compiler_commit
            .as_deref()
            .map(is_commit_sha)
            .unwrap_or(true));
    }

    #[test]
    fn aggregate_artifact_status_ignores_metadata_payload() {
        let value = serde_json::json!({
            "command": "run_conformance",
            "metadata": {
                "lock_hash": "lock-hash",
                "os": "linux",
                "rustc": "rustc 1.0.0",
                "node": "v22.0.0",
                "official_commits": { "vue3": "57545e958ae28ed17aa9e0ed321abcd8dc99f752" },
                "rust_compiler_commit": "0123456789012345678901234567890123456789",
                "created_unix": 1
            },
            "counts": { "total": 1, "pass": 1, "pending": 0, "fail": 0 },
            "coverage": {
                "source": "rust-backed",
                "counts_by_source": {
                    "rust-backed": { "total": 1, "pass": 1, "pending": 0, "fail": 0, "skip": 0 },
                    "mixed": { "total": 0, "pass": 0, "pending": 0, "fail": 0, "skip": 0 },
                    "shim-backed": { "total": 0, "pass": 0, "pending": 0, "fail": 0, "skip": 0 }
                }
            }
        });

        assert_eq!(report_value_status(&value), ReportStatus::Pass);
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
        assert_eq!(counts.total, 1);
        assert_eq!(counts.fail, 1);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn conformance_item_detail_uses_execution_counts() {
        let readiness = conformance_readiness(
            suite_spec(ConformanceSuite::Vue3Core),
            AliasBackend::Generated,
        );
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
    fn vue3_core_coverage_report_marks_mixed_and_excludes_rust_backed_counts() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-core-coverage-{}",
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
              "testResults": [
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/compile.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/scopeId.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/utils.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/vMemo.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/transformExpressions.spec.ts",
                  "assertionResults": [
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/transformSlotOutlet.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/transformText.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/vOnce.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/vBind.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/vModel.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/vOn.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/vFor.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/vIf.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/cacheStatic.spec.ts",
                  "assertionResults": [
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/transformElement.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "failed" }
                  ]
                }
              ]
            }"#,
        )
        .unwrap();
        let execution = ConformanceExecutionResult {
            status: "failed".into(),
            runner: "vitest".into(),
            prepared_root: "prepared".into(),
            output_file: report.display().to_string(),
            exit_code: Some(1),
            stdout: String::new(),
            stderr: String::new(),
            counts: ConformanceExecutionCounts {
                total: 287,
                pass: 286,
                fail: 1,
                skip: 0,
                pending: 0,
            },
        };

        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue3Core),
            AliasBackend::Generated,
            Some(&execution),
        );

        assert_eq!(coverage.source, ConformanceCoverageKind::Mixed);
        assert_eq!(coverage.rust_backed_pass, 285);
        assert_eq!(coverage.rust_backed_total, 285);
        assert_eq!(
            coverage
                .counts_by_source
                .get("rust-backed")
                .copied()
                .unwrap_or_default()
                .pass,
            285
        );
        assert_eq!(
            coverage
                .counts_by_source
                .get("mixed")
                .copied()
                .unwrap_or_default()
                .pass,
            1
        );
        assert_eq!(
            coverage.files[0].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(
            coverage.files[1].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(
            coverage.files[2].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(
            coverage.files[3].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[3].reason.contains("public baseCompile"));
        assert_eq!(
            coverage.files[4].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[4]
            .reason
            .contains("transformExpressionSuite"));
        assert_eq!(
            coverage.files[5].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[5]
            .reason
            .contains("transformSlotOutletSuite"));
        assert_eq!(
            coverage.files[6].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[6].reason.contains("transformTextSuite"));
        assert_eq!(
            coverage.files[7].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[7].reason.contains("transformOnceSuite"));
        assert_eq!(
            coverage.files[8].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[8].reason.contains("transformBindSuite"));
        assert_eq!(
            coverage.files[9].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[9].reason.contains("transformModelSuite"));
        assert_eq!(
            coverage.files[10].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[10].reason.contains("transformOnSuite"));
        assert_eq!(
            coverage.files[11].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[11].reason.contains("transformForSuite"));
        assert_eq!(
            coverage.files[12].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[12].reason.contains("transformIfSuite"));
        assert_eq!(
            coverage.files[13].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[13].reason.contains("cacheStaticSuite"));
        assert_eq!(coverage.files[14].source, ConformanceCoverageKind::Mixed);
        assert!(coverage.reason.contains("xtask/src/compat.rs"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_core_coverage_splits_transform_element_assertion_groups() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-core-transform-element-coverage-{}",
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
              "testResults": [
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/transformElement.spec.ts",
                  "assertionResults": [
                    {
                      "fullName": "compiler: v-for transform value",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: v-for codegen basic v-for",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: element transform import + resolve component",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: element transform static props",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: element transform directiveTransforms",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: element transform directiveTransform with needRuntime: true",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: element transform should process node when node has been replaced",
                      "status": "failed"
                    }
                  ]
                }
              ]
            }"#,
        )
        .unwrap();
        let execution = ConformanceExecutionResult {
            status: "failed".into(),
            runner: "vitest".into(),
            prepared_root: "prepared".into(),
            output_file: report.display().to_string(),
            exit_code: Some(1),
            stdout: String::new(),
            stderr: String::new(),
            counts: ConformanceExecutionCounts {
                total: 7,
                pass: 6,
                fail: 1,
                skip: 0,
                pending: 0,
            },
        };

        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue3Core),
            AliasBackend::Generated,
            Some(&execution),
        );

        assert_eq!(coverage.source, ConformanceCoverageKind::Mixed);
        assert_eq!(coverage.files.len(), 3);
        assert_eq!(coverage.rust_backed_total, 4);
        assert_eq!(coverage.rust_backed_pass, 4);
        assert_eq!(
            coverage
                .counts_by_source
                .get("mixed")
                .copied()
                .unwrap_or_default(),
            ConformanceExecutionCounts {
                total: 3,
                pass: 2,
                fail: 1,
                skip: 0,
                pending: 0,
            }
        );
        let imported_v_for = coverage
            .files
            .iter()
            .find(|file| file.scope.as_deref() == Some("imported v-for helper"))
            .expect("imported v-for coverage entry");
        assert_eq!(imported_v_for.source, ConformanceCoverageKind::RustBacked);
        assert_eq!(imported_v_for.counts.total, 2);

        let element_suite = coverage
            .files
            .iter()
            .find(|file| file.scope.as_deref() == Some("element transform rust suite"))
            .expect("element transform coverage entry");
        assert_eq!(element_suite.source, ConformanceCoverageKind::RustBacked);
        assert_eq!(element_suite.counts.total, 2);
        assert!(element_suite.reason.contains("transformElementSuite"));

        let callback_boundary = coverage
            .files
            .iter()
            .find(|file| file.scope.as_deref() == Some("js callback boundary"))
            .expect("callback boundary coverage entry");
        assert_eq!(callback_boundary.source, ConformanceCoverageKind::Mixed);
        assert_eq!(callback_boundary.counts.total, 3);
        assert!(callback_boundary
            .reason
            .contains("caller-provided JavaScript"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_core_coverage_splits_transform_assertion_groups() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-core-transform-coverage-{}",
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
              "testResults": [
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transform.spec.ts",
                  "assertionResults": [
                    {
                      "fullName": "compiler: transform context state",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: transform context.replaceNode",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: transform should inject toString helper for interpolations",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: transform root codegenNode root v-for",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: transform root codegenNode multiple children w/ single root + comments",
                      "status": "failed"
                    }
                  ]
                }
              ]
            }"#,
        )
        .unwrap();
        let execution = ConformanceExecutionResult {
            status: "failed".into(),
            runner: "vitest".into(),
            prepared_root: "prepared".into(),
            output_file: report.display().to_string(),
            exit_code: Some(1),
            stdout: String::new(),
            stderr: String::new(),
            counts: ConformanceExecutionCounts {
                total: 5,
                pass: 4,
                fail: 1,
                skip: 0,
                pending: 0,
            },
        };

        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue3Core),
            AliasBackend::Generated,
            Some(&execution),
        );

        assert_eq!(coverage.source, ConformanceCoverageKind::Mixed);
        assert_eq!(coverage.files.len(), 2);
        assert_eq!(coverage.rust_backed_total, 3);
        assert_eq!(coverage.rust_backed_pass, 2);

        let transform_suite = coverage
            .files
            .iter()
            .find(|file| file.scope.as_deref() == Some("transform rust suite"))
            .expect("transform rust coverage entry");
        assert_eq!(transform_suite.source, ConformanceCoverageKind::RustBacked);
        assert_eq!(transform_suite.counts.total, 3);
        assert!(transform_suite.reason.contains("transformSuite"));

        let callback_boundary = coverage
            .files
            .iter()
            .find(|file| file.scope.as_deref() == Some("js transform context boundary"))
            .expect("transform context coverage entry");
        assert_eq!(callback_boundary.source, ConformanceCoverageKind::Mixed);
        assert_eq!(callback_boundary.counts.total, 2);
        assert!(callback_boundary.reason.contains("NodeTransform callbacks"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_core_transform_spec_uses_prepared_rust_api_for_root_codegen() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-core-transform-shim-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let tests = temp
            .join("packages")
            .join("compiler-core")
            .join("__tests__");
        fs::create_dir_all(&tests).unwrap();
        fs::write(
            tests.join("transform.spec.ts"),
            r#"import { baseParse } from '../src/parser'
import { type NodeTransform, transform } from '../src/transform'
import {
  type DirectiveNode,
  type ElementNode,
  type ExpressionNode,
  NodeTypes,
  type VNodeCall,
} from '../src/ast'
import { ErrorCodes, createCompilerError } from '../src/errors'
import {
  CREATE_COMMENT,
  FRAGMENT,
  RENDER_SLOT,
  TO_DISPLAY_STRING,
} from '../src/runtimeHelpers'
import { transformIf } from '../src/transforms/vIf'
import { transformFor } from '../src/transforms/vFor'
import { transformElement } from '../src/transforms/transformElement'
import { transformSlotOutlet } from '../src/transforms/transformSlotOutlet'
import { transformText } from '../src/transforms/transformText'
import { PatchFlags } from '@vue/shared'

describe('compiler: transform', () => {
  test('context state', () => {
    const ast = baseParse(`<div>hello</div>`)
    const plugin: NodeTransform = () => {}
    transform(ast, { nodeTransforms: [plugin] })
    expect(ast).toBeTruthy()
  })

  test('should inject toString helper for interpolations', () => {
    const ast = baseParse(`{{ foo }}`)
    transform(ast, {})
    expect(ast.helpers).toContain(TO_DISPLAY_STRING)
  })

  test('should inject createVNode and Comment for comments', () => {
    const ast = baseParse(`<!--foo-->`)
    transform(ast, {})
    expect(ast.helpers).toContain(CREATE_COMMENT)
  })

  describe('root codegenNode', () => {
    function transformWithCodegen(template: string) {
      const ast = baseParse(template)
      transform(ast, {
        nodeTransforms: [
          transformIf,
          transformFor,
          transformText,
          transformSlotOutlet,
          transformElement,
        ],
      })
      return ast
    }

    test('single element', () => {
      const ast = transformWithCodegen(`<div/>`)
      expect(ast.codegenNode).toMatchObject({ type: NodeTypes.VNODE_CALL })
    })
  })
})
"#,
        )
        .unwrap();

        rewrite_vue3_core_transform_public_api_spec(&temp).unwrap();

        let spec = fs::read_to_string(tests.join("transform.spec.ts")).unwrap();
        assert!(spec.contains("from './transform.rust-api'"));
        assert!(spec.contains("const ast = transformWithCodegen(`{{ foo }}`)"));
        assert!(spec.contains("const ast = transformWithCodegen(`<!--foo-->`)"));
        assert!(!spec.contains("function transformWithCodegen(template: string)"));
        assert!(spec.contains("const plugin: NodeTransform = () => {}"));

        let api = fs::read_to_string(tests.join("transform.rust-api.ts")).unwrap();
        assert!(api.contains("callBridge('vue3.core.transformSuite'"));
        assert!(api.contains("hydrateTransformAst"));
        assert!(api.contains("node.codegenNode = undefined"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue27_sfc_coverage_marks_compile_style_mixed_postcss_boundary() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue27-sfc-coverage-{}",
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
              "testResults": [
                {
                  "name": "F:/repo/prepared/vue27-sfc/packages/compiler-sfc/test/compileScript.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue27-sfc/packages/compiler-sfc/test/compileStyle.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "failed" }
                  ]
                }
              ]
            }"#,
        )
        .unwrap();
        let execution = ConformanceExecutionResult {
            status: "failed".into(),
            runner: "vitest".into(),
            prepared_root: "prepared".into(),
            output_file: report.display().to_string(),
            exit_code: Some(1),
            stdout: String::new(),
            stderr: String::new(),
            counts: ConformanceExecutionCounts {
                total: 4,
                pass: 3,
                fail: 1,
                skip: 0,
                pending: 0,
            },
        };

        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue27Sfc),
            AliasBackend::Generated,
            Some(&execution),
        );

        assert_eq!(coverage.source, ConformanceCoverageKind::Mixed);
        assert_eq!(coverage.rust_backed_pass, 2);
        assert_eq!(coverage.rust_backed_total, 2);
        assert_eq!(
            coverage
                .counts_by_source
                .get("mixed")
                .copied()
                .unwrap_or_default()
                .total,
            2
        );
        assert_eq!(
            coverage.files[0].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(coverage.files[1].source, ConformanceCoverageKind::Mixed);
        assert!(coverage.files[1]
            .reason
            .contains("PostCSS plugin callbacks"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue2_jasmine_coverage_report_reads_per_file_results() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue2-jasmine-coverage-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&temp).unwrap();
        let report = temp.join("jasmine-report.json");
        fs::write(
            &report,
            r#"{
              "counts": { "total": 3, "pass": 1, "fail": 1, "skip": 1, "pending": 0 },
              "testResults": [
                {
                  "name": "F:/repo/prepared/vue2-compiler/test/unit/modules/compiler/codegen.spec.js",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "failed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue2-compiler/test/unit/modules/compiler/parser.spec.js",
                  "assertionResults": [
                    { "status": "skipped" }
                  ]
                }
              ]
            }"#,
        )
        .unwrap();
        let execution = ConformanceExecutionResult {
            status: "failed".into(),
            runner: "jasmine".into(),
            prepared_root: "prepared".into(),
            output_file: report.display().to_string(),
            exit_code: Some(1),
            stdout: String::new(),
            stderr: String::new(),
            counts: ConformanceExecutionCounts {
                total: 3,
                pass: 1,
                fail: 1,
                skip: 1,
                pending: 0,
            },
        };

        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue2Compiler),
            AliasBackend::Generated,
            Some(&execution),
        );

        assert_eq!(coverage.source, ConformanceCoverageKind::RustBacked);
        assert_eq!(coverage.files.len(), 2);
        assert_eq!(coverage.rust_backed_total, 3);
        assert_eq!(coverage.rust_backed_pass, 1);
        assert!(coverage.reason.contains("prepared Jasmine suite"));
        assert!(coverage.reason.contains("not-wired pending status"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn napi_conformance_coverage_marks_mixed_alias_backend() {
        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue2Compiler),
            AliasBackend::Napi,
            None,
        );
        assert_eq!(coverage.source, ConformanceCoverageKind::Mixed);
        assert_eq!(coverage.rust_backed_total, 0);
        assert!(coverage
            .reason
            .contains("NAPI-backed official package-name alias"));
        assert!(coverage.reason.contains("mixed harness coverage"));
    }

    #[test]
    fn vue2_conformance_shims_use_official_runners_and_globs() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue2-shims-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&temp).unwrap();

        write_vue2_compiler_source_shims(&temp, true).unwrap();
        write_vue2_jasmine_runner(&temp).unwrap();
        write_vue27_compiler_conformance_shims(&temp).unwrap();
        let compiler_config = fs::read_to_string(temp.join("vitest.config.ts")).unwrap();
        assert!(compiler_config.contains("environment: 'jsdom'"));
        assert!(compiler_config.contains("include: ['test/unit/modules/compiler/**/*.spec.ts']"));
        assert!(compiler_config.contains(
            "'vue-template-compiler': path.resolve(aliasRoot, 'node_modules/vue-template-compiler/index.js')"
        ));
        let codegen =
            fs::read_to_string(temp.join("src").join("compiler").join("codegen.ts")).unwrap();
        assert!(codegen.contains("callBridge('vue2.generate'"));
        assert!(codegen.contains("export function normalizeVue2AstForBridge"));
        assert!(codegen.contains("events[key] = []"));
        assert!(codegen.contains("function normalizeVue2PublicElementForBridge"));
        assert!(codegen.contains("static_node: Boolean(node.static ?? node.static_node)"));
        assert!(codegen.contains(
            "modifier_order: Array.isArray(handler.modifierOrder || handler.modifier_order)"
        ));
        assert!(codegen.contains("has_modifier_object: Boolean(handler.hasModifierObject"));
        let parser = fs::read_to_string(
            temp.join("src")
                .join("compiler")
                .join("parser")
                .join("index.ts"),
        )
        .unwrap();
        assert!(parser.contains("compiled.element_public_ast"));
        assert!(parser.contains("Object.defineProperty(ast, '__vuecInternal'"));
        assert!(parser.contains("hydrateVue2PublicAst(ast, null"));
        assert!(parser.contains("normalizeVue2OptionsForBridge(options, tags, true)"));
        assert!(parser.contains("__vuecTagNamespaces"));
        assert!(parser.contains("runVue2ModuleTransforms(ast, options, 'preTransformNode')"));
        let optimizer =
            fs::read_to_string(temp.join("src").join("compiler").join("optimizer.ts")).unwrap();
        assert!(optimizer.contains("callBridge('vue2.optimize'"));
        assert!(optimizer.contains("mergeVue2OptimizedAst(ast"));
        assert!(optimizer.contains("__vuecReservedTags"));
        let codeframe =
            fs::read_to_string(temp.join("src").join("compiler").join("codeframe.ts")).unwrap();
        assert!(codeframe.contains("export { generateCodeFrame }"));

        write_vue27_sfc_conformance_shims(&temp).unwrap();
        let sfc_config = fs::read_to_string(temp.join("vitest.config.ts")).unwrap();
        assert!(sfc_config.contains("environment: 'jsdom'"));
        assert!(sfc_config.contains("include: ['packages/compiler-sfc/test/**/*.spec.ts']"));
        assert!(
            sfc_config.find("'vue/compiler-sfc'").unwrap()
                < sfc_config.find("vue: path.resolve").unwrap()
        );

        let runner = fs::read_to_string(temp.join("vuec-jasmine-runner.js")).unwrap();
        assert!(runner.contains("const Jasmine = require('jasmine')"));
        assert!(runner.contains("const { JSDOM } = require('jsdom')"));
        assert!(runner.contains("global.document = dom.window.document"));
        assert!(runner.contains("function vuecInteropDefault(value)"));
        assert!(runner.contains("globalThis.__vuecInteropDefault = vuecInteropDefault"));
        assert!(runner.contains("cache: false"));
        assert!(runner.contains("t.identifier('__vuecInteropDefault')"));
        assert!(runner.contains("testResults: Array.from(testResultsByFile.values())"));
        assert!(runner.contains("compiler-options.spec.js"));
        let vue2_specs = suite_spec(ConformanceSuite::Vue2Compiler);
        assert!(vue2_specs.runner_dependencies.contains(&"jsdom"));
        let setup = fs::read_to_string(temp.join("vuec-vitest-setup.ts")).unwrap();
        assert!(setup.contains("warnMock"));
        assert!(setup.contains("mock.calls"));
        assert!(setup.contains("(console.error as any).mock"));

        let specs = suite_spec(ConformanceSuite::Vue27Compiler);
        assert!(specs.runner_dependencies.contains(&"jsdom"));
        let sfc_specs = suite_spec(ConformanceSuite::Vue27Sfc);
        assert!(sfc_specs.runner_dependencies.contains(&"jsdom"));
        let _ = fs::remove_dir_all(temp);
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
        let transforms_tests = temp
            .join("packages")
            .join("compiler-core")
            .join("__tests__")
            .join("transforms");
        fs::create_dir_all(&transforms_tests).unwrap();
        fs::write(
            transforms_tests.join("vOnce.spec.ts"),
            r#"import {
  type CompilerOptions,
  NodeTypes,
  generate,
  getBaseTransformPreset,
  baseParse as parse,
  transform,
} from '../../src'
import { RENDER_SLOT, SET_BLOCK_TRACKING } from '../../src/runtimeHelpers'

function transformWithOnce(template: string, options: CompilerOptions = {}) {
  const ast = parse(template)
  const [nodeTransforms, directiveTransforms] = getBaseTransformPreset()
  transform(ast, {
    nodeTransforms,
    directiveTransforms,
    ...options,
  })
  return ast
}

test('placeholder', () => {
  expect(transformWithOnce('<div v-once />')).toBeTruthy()
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("vBind.spec.ts"),
            r#"import {
  type CallExpression,
  type CompilerOptions,
  type ElementNode,
  ErrorCodes,
  NodeTypes,
  type ObjectExpression,
  type VNodeCall,
  baseParse as parse,
  transform,
} from '../../src'
import { transformBind } from '../../src/transforms/vBind'
import { transformElement } from '../../src/transforms/transformElement'
import {
  CAMELIZE,
  NORMALIZE_PROPS,
  helperNameMap,
} from '../../src/runtimeHelpers'
import { transformExpression } from '../../src/transforms/transformExpression'
import { transformVBindShorthand } from '../../src/transforms/transformVBindShorthand'

function parseWithVBind(
  template: string,
  options: CompilerOptions = {},
): ElementNode {
  const ast = parse(template)
  transform(ast, {
    nodeTransforms: [
      transformVBindShorthand,
      ...(options.prefixIdentifiers ? [transformExpression] : []),
      transformElement,
    ],
    directiveTransforms: {
      bind: transformBind,
    },
    ...options,
  })
  return ast.children[0] as ElementNode
}

test('placeholder', () => {
  expect(parseWithVBind('<div :id />')).toBeTruthy()
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("vModel.spec.ts"),
            r#"import {
  BindingTypes,
  type CompilerOptions,
  type ComponentNode,
  type ElementNode,
  type ForNode,
  NORMALIZE_PROPS,
  NodeTypes,
  type ObjectExpression,
  type PlainElementNode,
  type VNodeCall,
  generate,
  baseParse as parse,
  transform,
} from '../../src'
import { ErrorCodes } from '../../src/errors'
import { transformModel } from '../../src/transforms/vModel'
import { transformElement } from '../../src/transforms/transformElement'
import { transformExpression } from '../../src/transforms/transformExpression'
import { transformFor } from '../../src/transforms/vFor'
import { trackSlotScopes } from '../../src/transforms/vSlot'
import type { CallExpression } from '@babel/types'

function parseWithVModel(template: string, options: CompilerOptions = {}) {
  const ast = parse(template)

  transform(ast, {
    nodeTransforms: [
      transformFor,
      transformExpression,
      transformElement,
      trackSlotScopes,
    ],
    directiveTransforms: {
      ...options.directiveTransforms,
      model: transformModel,
    },
    ...options,
  })

  return ast
}

test('placeholder', () => {
  expect(parseWithVModel('<input v-model="model" />')).toBeTruthy()
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("vOn.spec.ts"),
            r#"import {
  type CompilerOptions,
  type ElementNode,
  ErrorCodes,
  NodeTypes,
  type ObjectExpression,
  TO_HANDLER_KEY,
  type VNodeCall,
  helperNameMap,
  baseParse as parse,
  transform,
} from '../../src'
import { transformFor } from '../../src/transforms/vFor'
import { transformOn } from '../../src/transforms/vOn'
import { transformElement } from '../../src/transforms/transformElement'
import { transformExpression } from '../../src/transforms/transformExpression'

function parseWithVOn(template: string, options: CompilerOptions = {}) {
  const ast = parse(template, options)
  transform(ast, {
    nodeTransforms: [transformExpression, transformElement, transformFor],
    directiveTransforms: {
      on: transformOn,
    },
    ...options,
  })
  return {
    root: ast,
    node: ast.children[0] as ElementNode,
  }
}

        test('placeholder', () => {
  expect(parseWithVOn('<div @click="foo" />')).toBeTruthy()
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("vFor.spec.ts"),
            r#"import { baseParse as parse } from '../../src/parser'
import { transform } from '../../src/transform'
import { transformIf } from '../../src/transforms/vIf'
import { transformFor } from '../../src/transforms/vFor'
import { transformBind } from '../../src/transforms/vBind'
import { transformElement } from '../../src/transforms/transformElement'
import { transformSlotOutlet } from '../../src/transforms/transformSlotOutlet'
import { transformExpression } from '../../src/transforms/transformExpression'
import {
  ConstantTypes,
  type ElementNode,
  type ForCodegenNode,
  type ForNode,
  type InterpolationNode,
  NodeTypes,
  type RootNode,
  type SimpleExpressionNode,
} from '../../src/ast'
import { ErrorCodes } from '../../src/errors'
import { type CompilerOptions, generate } from '../../src'
import { FRAGMENT, RENDER_LIST, RENDER_SLOT } from '../../src/runtimeHelpers'
import { PatchFlags } from '@vue/shared'
import { createObjectMatcher } from '../testUtils'
import { transformVBindShorthand } from '../../src/transforms/transformVBindShorthand'

export function parseWithForTransform(
  template: string,
  options: CompilerOptions = {},
): {
  root: RootNode
  node: ForNode & { codegenNode: ForCodegenNode }
} {
  const ast = parse(template, options)
  transform(ast, {
    nodeTransforms: [
      transformVBindShorthand,
      transformIf,
      transformFor,
      ...(options.prefixIdentifiers ? [transformExpression] : []),
      transformSlotOutlet,
      transformElement,
    ],
    directiveTransforms: {
      bind: transformBind,
    },
    ...options,
  })
  return {
    root: ast,
    node: ast.children[0] as ForNode & { codegenNode: ForCodegenNode },
  }
}

        test('placeholder', () => {
  expect(parseWithForTransform('<div v-for="i in list" />')).toBeTruthy()
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("transformElement.spec.ts"),
            r#"import {
  BindingTypes,
  type CompilerOptions,
  ErrorCodes,
  type NodeTransform,
  baseCompile,
  baseParse as parse,
  transform,
  transformExpression,
} from '../../src'
import {
  BASE_TRANSITION,
  CREATE_VNODE,
  GUARD_REACTIVE_PROPS,
  KEEP_ALIVE,
  MERGE_PROPS,
  NORMALIZE_CLASS,
  NORMALIZE_PROPS,
  NORMALIZE_STYLE,
  RESOLVE_COMPONENT,
  RESOLVE_DIRECTIVE,
  RESOLVE_DYNAMIC_COMPONENT,
  SUSPENSE,
  TELEPORT,
  TO_HANDLERS,
  helperNameMap,
} from '../../src/runtimeHelpers'
import {
  type DirectiveNode,
  NodeTypes,
  type RootNode,
  type VNodeCall,
  createObjectProperty,
} from '../../src/ast'
import { transformElement } from '../../src/transforms/transformElement'
import { transformStyle } from '../../../compiler-dom/src/transforms/transformStyle'
import { transformOn } from '../../src/transforms/vOn'
import { transformBind } from '../../src/transforms/vBind'
import { PatchFlags } from '@vue/shared'
import { createObjectMatcher } from '../testUtils'
import { transformText } from '../../src/transforms/transformText'
import { parseWithForTransform } from './vFor.spec'

function parseWithElementTransform(
  template: string,
  options: CompilerOptions = {},
): {
  root: RootNode
  node: VNodeCall
} {
  // wrap raw template in an extra div so that it doesn't get turned into a
  // block as root node
  const ast = parse(`<div>${template}</div>`, options)
  transform(ast, {
    nodeTransforms: [transformElement, transformText],
    ...options,
  })
  const codegenNode = (ast as any).children[0].children[0]
    .codegenNode as VNodeCall
  expect(codegenNode.type).toBe(NodeTypes.VNODE_CALL)
  return {
    root: ast,
    node: codegenNode,
  }
}

function parseWithBind(template: string, options?: CompilerOptions) {
  return parseWithElementTransform(template, {
    ...options,
    directiveTransforms: {
      ...options?.directiveTransforms,
      bind: transformBind,
    },
  })
}

describe('compiler: element transform', () => {
  test('should handle <KeepAlive>', () => {
    function assert(tag: string) {
      const root = parse(`<div><${tag}><span /></${tag}></div>`)
      transform(root, {
        nodeTransforms: [transformElement, transformText],
      })
      expect(root.components.length).toBe(0)
      expect(root.helpers).toContain(KEEP_ALIVE)
      const node = (root.children[0] as any).children[0].codegenNode
      expect(node).toMatchObject({
        type: NodeTypes.VNODE_CALL,
        tag: KEEP_ALIVE,
        isBlock: true, // should be forced into a block
        props: undefined,
        // keep-alive should not compile content to slots
        children: [{ type: NodeTypes.ELEMENT, tag: 'span' }],
        // should get a dynamic slots flag to force updates
        patchFlag: PatchFlags.DYNAMIC_SLOTS,
      })
    }

    assert(`keep-alive`)
    assert(`KeepAlive`)
  })

  test('directiveTransforms', () => {
    let _dir: DirectiveNode
    const { node } = parseWithElementTransform(`<div v-foo:bar="hello" />`, {
      directiveTransforms: {
        foo(dir) {
          _dir = dir
          return {
            props: [createObjectProperty(dir.arg!, dir.exp!)],
          }
        },
      },
    })
    expect(node).toBeTruthy()
  })

  test('directiveTransform with needRuntime: true', () => {
    const { root, node } = parseWithElementTransform(
      `<div v-foo:bar="hello" />`,
      {
        directiveTransforms: {
          foo() {
            return {
              props: [],
              needRuntime: true,
            }
          },
        },
      },
    )
    expect(root).toBeTruthy()
    expect(node).toBeTruthy()
  })

  test('directiveTransform with needRuntime: Symbol', () => {
    const { root, node } = parseWithElementTransform(
      `<div v-foo:bar="hello" />`,
      {
        directiveTransforms: {
          foo() {
            return {
              props: [],
              needRuntime: CREATE_VNODE,
            }
          },
        },
      },
    )
    expect(root).toBeTruthy()
    expect(node).toBeTruthy()
  })

  test('<svg> should be forced into blocks', () => {
    const ast = parse(`<div><svg/></div>`)
    transform(ast, {
      nodeTransforms: [transformElement],
    })
    expect((ast as any).children[0].children[0].codegenNode).toMatchObject({
      type: NodeTypes.VNODE_CALL,
      tag: `"svg"`,
      isBlock: true,
    })
  })

  test('<math> should be forced into blocks', () => {
    const ast = parse(`<div><math/></div>`)
    transform(ast, {
      nodeTransforms: [transformElement],
    })
    expect((ast as any).children[0].children[0].codegenNode).toMatchObject({
      type: NodeTypes.VNODE_CALL,
      tag: `"math"`,
      isBlock: true,
    })
  })

  // #938
  test('element with dynamic keys should be forced into blocks', () => {
    const ast = parse(`<div><div :key="foo" /></div>`)
    transform(ast, {
      nodeTransforms: [transformElement],
    })
    expect((ast as any).children[0].children[0].codegenNode).toMatchObject({
      type: NodeTypes.VNODE_CALL,
      tag: `"div"`,
      isBlock: true,
    })
  })

  test('should process node when node has been replaced', () => {
    const customNodeTransform: NodeTransform = () => {}
    expect(customNodeTransform).toBeTruthy()
  })

  test('ref_for marker on static ref', () => {
    expect(parseWithForTransform(`<div v-for="i in l" ref="x"/>`)).toBeTruthy()
  })

  test('placeholder', () => {
    expect(parseWithBind('<div :id="id" />')).toBeTruthy()
    expect(baseCompile('<div />')).toBeTruthy()
  })
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("vIf.spec.ts"),
            r#"import { baseParse as parse } from '../../src/parser'
import { transform } from '../../src/transform'
import { transformIf } from '../../src/transforms/vIf'
import { transformElement } from '../../src/transforms/transformElement'
import { transformSlotOutlet } from '../../src/transforms/transformSlotOutlet'
import {
  type CommentNode,
  type ConditionalExpression,
  type ElementNode,
  ElementTypes,
  type IfBranchNode,
  type IfConditionalExpression,
  type IfNode,
  NodeTypes,
  type SimpleExpressionNode,
  type TextNode,
  type VNodeCall,
} from '../../src/ast'
import { ErrorCodes } from '../../src/errors'
import {
  type CompilerOptions,
  TO_HANDLERS,
  generate,
  transformVBindShorthand,
} from '../../src'
import {
  CREATE_COMMENT,
  FRAGMENT,
  MERGE_PROPS,
  NORMALIZE_PROPS,
  RENDER_SLOT,
} from '../../src/runtimeHelpers'
import { createObjectMatcher } from '../testUtils'

function parseWithIfTransform(
  template: string,
  options: CompilerOptions = {},
  returnIndex: number = 0,
  childrenLen: number = 1,
) {
  const ast = parse(template, options)
  transform(ast, {
    nodeTransforms: [
      transformVBindShorthand,
      transformIf,
      transformSlotOutlet,
      transformElement,
    ],
    ...options,
  })
  if (!options.onError) {
    expect(ast.children.length).toBe(childrenLen)
    for (let i = 0; i < childrenLen; i++) {
      expect(ast.children[i].type).toBe(NodeTypes.IF)
    }
  }
  return {
    root: ast,
    node: ast.children[returnIndex] as IfNode & {
      codegenNode: IfConditionalExpression
    },
  }
}

test('placeholder', () => {
  expect(parseWithIfTransform('<div v-if="ok" />')).toBeTruthy()
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("transformExpressions.spec.ts"),
            r#"import {
  BindingTypes,
  type CompilerOptions,
  ConstantTypes,
  type DirectiveNode,
  type ElementNode,
  type InterpolationNode,
  NodeTypes,
  baseCompile,
  baseParse as parse,
  transform,
} from '../../src'
import { transformIf } from '../../src/transforms/vIf'
import { transformExpression } from '../../src/transforms/transformExpression'
import { PatchFlagNames, PatchFlags } from '../../../shared/src'

function parseWithExpressionTransform(
  template: string,
  options: CompilerOptions = {},
) {
  const ast = parse(template, options)
  transform(ast, {
    prefixIdentifiers: true,
    nodeTransforms: [transformIf, transformExpression],
    ...options,
  })
  return ast.children[0]
}

test('placeholder', () => {
  expect(parseWithExpressionTransform('{{ foo }}')).toBeTruthy()
  expect(baseCompile('<div />')).toBeTruthy()
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("transformSlotOutlet.spec.ts"),
            r#"import {
  type CompilerOptions,
  type ElementNode,
  ErrorCodes,
  NodeTypes,
  baseParse as parse,
  transform,
} from '../../src'
import { transformElement } from '../../src/transforms/transformElement'
import { transformOn } from '../../src/transforms/vOn'
import { transformBind } from '../../src/transforms/vBind'
import { transformExpression } from '../../src/transforms/transformExpression'
import { RENDER_SLOT } from '../../src/runtimeHelpers'
import { transformSlotOutlet } from '../../src/transforms/transformSlotOutlet'

function parseWithSlots(template: string, options: CompilerOptions = {}) {
  const ast = parse(template)
  transform(ast, {
    nodeTransforms: [
      ...(options.prefixIdentifiers ? [transformExpression] : []),
      transformSlotOutlet,
      transformElement,
    ],
    directiveTransforms: {
      on: transformOn,
      bind: transformBind,
    },
    ...options,
  })
  return ast
}

test('placeholder', () => {
  expect(parseWithSlots('<slot />')).toBeTruthy()
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("vSlot.spec.ts"),
            r#"import {
  type CompilerOptions,
  type ComponentNode,
  type ElementNode,
  ErrorCodes,
  type ForNode,
  NodeTypes,
  type ObjectExpression,
  type RenderSlotCall,
  type SimpleExpressionNode,
  type SlotsExpression,
  type VNodeCall,
  generate,
  baseParse as parse,
  transform,
} from '../../src'
import { transformElement } from '../../src/transforms/transformElement'
import { transformOn } from '../../src/transforms/vOn'
import { transformBind } from '../../src/transforms/vBind'
import { transformExpression } from '../../src/transforms/transformExpression'
import { transformSlotOutlet } from '../../src/transforms/transformSlotOutlet'
import {
  trackSlotScopes,
  trackVForSlotScopes,
} from '../../src/transforms/vSlot'
import { CREATE_SLOTS, RENDER_LIST } from '../../src/runtimeHelpers'
import { createObjectMatcher } from '../testUtils'
import { PatchFlags } from '@vue/shared'
import { transformFor } from '../../src/transforms/vFor'
import { transformIf } from '../../src/transforms/vIf'
import { transformText } from '../../src/transforms/transformText'

function parseWithSlots(
  template: string,
  options: CompilerOptions & { transformText?: boolean } = {},
) {
  const ast = parse(template, {
    whitespace: options.whitespace,
  })
  transform(ast, {
    nodeTransforms: [
      transformIf,
      transformFor,
      ...(options.prefixIdentifiers
        ? [trackVForSlotScopes, transformExpression]
        : []),
      transformSlotOutlet,
      transformElement,
      trackSlotScopes,
      ...(options.transformText ? [transformText] : []),
    ],
    directiveTransforms: {
      on: transformOn,
      bind: transformBind,
    },
    ...options,
  })
  return {
    root: ast,
    slots:
      ast.children[0].type === NodeTypes.ELEMENT
        ? ((ast.children[0].codegenNode as VNodeCall)
            .children as SlotsExpression)
        : null,
  }
}

test('placeholder', () => {
  expect(parseWithSlots('<Comp><div/></Comp>')).toBeTruthy()
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("cacheStatic.spec.ts"),
            r#"import {
  type CompilerOptions,
  ConstantTypes,
  type ElementNode,
  type ForNode,
  type IfNode,
  NodeTypes,
  type VNodeCall,
  generate,
  baseParse as parse,
  transform,
} from '../../src'
import {
  FRAGMENT,
  NORMALIZE_CLASS,
  RENDER_LIST,
} from '../../src/runtimeHelpers'
import { transformElement } from '../../src/transforms/transformElement'
import { transformExpression } from '../../src/transforms/transformExpression'
import { transformIf } from '../../src/transforms/vIf'
import { transformFor } from '../../src/transforms/vFor'
import { transformBind } from '../../src/transforms/vBind'
import { transformOn } from '../../src/transforms/vOn'
import { createObjectMatcher } from '../testUtils'
import { transformText } from '../../src/transforms/transformText'
import { PatchFlags } from '@vue/shared'

function transformWithCache(template: string, options: CompilerOptions = {}) {
  const ast = parse(template)
  transform(ast, {
    hoistStatic: true,
    nodeTransforms: [
      transformIf,
      transformFor,
      ...(options.prefixIdentifiers ? [transformExpression] : []),
      transformElement,
      transformText,
    ],
    directiveTransforms: {
      on: transformOn,
      bind: transformBind,
    },
    ...options,
  })
  expect(ast.codegenNode).toMatchObject({
    type: NodeTypes.VNODE_CALL,
    isBlock: true,
  })
  return ast
}

test('placeholder', () => {
  expect(transformWithCache('<div><span/></div>')).toBeTruthy()
})
"#,
        )
        .unwrap();
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
        let v_once_spec = fs::read_to_string(transforms_tests.join("vOnce.spec.ts")).unwrap();
        assert!(v_once_spec.contains("from './vOnce.rust-api'"));
        assert!(!v_once_spec.contains("getBaseTransformPreset"));
        let v_once_api = fs::read_to_string(transforms_tests.join("vOnce.rust-api.ts")).unwrap();
        assert!(v_once_api.contains("callBridge('vue3.core.transformOnceSuite'"));
        assert!(v_once_api.contains("hydrateTransformOnceAst"));
        let v_bind_spec = fs::read_to_string(transforms_tests.join("vBind.spec.ts")).unwrap();
        assert!(v_bind_spec.contains("from './vBind.rust-api'"));
        assert!(!v_bind_spec.contains("baseParse as parse"));
        assert!(!v_bind_spec.contains("transform(ast,"));
        assert!(!v_bind_spec.contains("transformBind } from '../../src/transforms/vBind'"));
        assert!(!v_bind_spec.contains("transformVBindShorthand"));
        let v_bind_api = fs::read_to_string(transforms_tests.join("vBind.rust-api.ts")).unwrap();
        assert!(v_bind_api.contains("callBridge('vue3.core.transformBindSuite'"));
        assert!(v_bind_api.contains("emitErrors"));
        assert!(v_bind_api.contains("hydrateVBindAst"));
        assert!(v_bind_api.contains("__vuecBrowser"));
        let v_model_spec = fs::read_to_string(transforms_tests.join("vModel.spec.ts")).unwrap();
        assert!(v_model_spec.contains("from './vModel.rust-api'"));
        assert!(!v_model_spec.contains("baseParse as parse"));
        assert!(!v_model_spec.contains("transform(ast,"));
        assert!(!v_model_spec.contains("transformModel } from '../../src/transforms/vModel'"));
        assert!(!v_model_spec.contains("trackSlotScopes"));
        let v_model_api = fs::read_to_string(transforms_tests.join("vModel.rust-api.ts")).unwrap();
        assert!(v_model_api.contains("callBridge('vue3.core.transformModelSuite'"));
        assert!(v_model_api.contains("emitErrors"));
        assert!(v_model_api.contains("hydrateVModelAst"));
        let v_on_spec = fs::read_to_string(transforms_tests.join("vOn.spec.ts")).unwrap();
        assert!(v_on_spec.contains("from './vOn.rust-api'"));
        assert!(!v_on_spec.contains("baseParse as parse"));
        assert!(!v_on_spec.contains("transform(ast,"));
        assert!(!v_on_spec.contains("transformOn } from '../../src/transforms/vOn'"));
        let v_on_api = fs::read_to_string(transforms_tests.join("vOn.rust-api.ts")).unwrap();
        assert!(v_on_api.contains("callBridge('vue3.core.transformOnSuite'"));
        assert!(v_on_api.contains("emitErrors"));
        assert!(v_on_api.contains("hydrateVOnAst"));
        assert!(v_on_api.contains("__vuecNativeTags"));
        assert!(v_on_api.contains("delete node.patchFlag"));
        let v_for_spec = fs::read_to_string(transforms_tests.join("vFor.spec.ts")).unwrap();
        assert!(v_for_spec.contains("from './vFor.rust-api'"));
        assert!(v_for_spec.contains("export { parseWithForTransform }"));
        assert!(!v_for_spec.contains("baseParse as parse"));
        assert!(!v_for_spec.contains("transform(ast,"));
        assert!(!v_for_spec.contains("transformFor } from '../../src/transforms/vFor'"));
        assert!(!v_for_spec.contains("transformVBindShorthand"));
        let v_for_api = fs::read_to_string(transforms_tests.join("vFor.rust-api.ts")).unwrap();
        assert!(v_for_api.contains("callBridge('vue3.core.transformForSuite'"));
        assert!(v_for_api.contains("emitErrors"));
        assert!(v_for_api.contains("hydrateVForAst"));
        assert!(v_for_api.contains("node[key] = undefined"));
        let transform_element_spec =
            fs::read_to_string(transforms_tests.join("transformElement.spec.ts")).unwrap();
        assert!(transform_element_spec.contains("from './transformElement.rust-api'"));
        assert!(transform_element_spec.contains("from './vFor.rust-api'"));
        assert!(transform_element_spec.contains("parseWithElementTransformOriginal"));
        assert!(!transform_element_spec.contains("from './vFor.spec'"));
        let transform_element_api =
            fs::read_to_string(transforms_tests.join("transformElement.rust-api.ts")).unwrap();
        assert!(transform_element_api.contains("callBridge('vue3.core.transformElementSuite'"));
        assert!(transform_element_api.contains("emitErrors"));
        assert!(transform_element_api.contains("hydrateTransformElementAst"));
        assert!(transform_element_api.contains("__vuecNativeTags"));
        let v_if_spec = fs::read_to_string(transforms_tests.join("vIf.spec.ts")).unwrap();
        assert!(v_if_spec.contains("from './vIf.rust-api'"));
        assert!(!v_if_spec.contains("baseParse as parse"));
        assert!(!v_if_spec.contains("transform(ast,"));
        assert!(!v_if_spec.contains("transformIf } from '../../src/transforms/vIf'"));
        assert!(!v_if_spec.contains("transformVBindShorthand"));
        let v_if_api = fs::read_to_string(transforms_tests.join("vIf.rust-api.ts")).unwrap();
        assert!(v_if_api.contains("callBridge('vue3.core.transformIfSuite'"));
        assert!(v_if_api.contains("emitErrors"));
        assert!(v_if_api.contains("hydrateVIfAst"));
        assert!(v_if_api.contains("delete node.condition"));
        let expressions_spec =
            fs::read_to_string(transforms_tests.join("transformExpressions.spec.ts")).unwrap();
        assert!(expressions_spec.contains("from './transformExpressions.rust-api'"));
        assert!(!expressions_spec.contains("baseParse as parse"));
        assert!(!expressions_spec.contains("transform(ast,"));
        assert!(!expressions_spec
            .contains("transformExpression } from '../../src/transforms/transformExpression'"));
        assert!(!expressions_spec.contains("transformIf"));
        let expressions_api =
            fs::read_to_string(transforms_tests.join("transformExpressions.rust-api.ts")).unwrap();
        assert!(expressions_api.contains("callBridge('vue3.core.transformExpressionSuite'"));
        assert!(expressions_api.contains("new SyntaxError"));
        assert!(expressions_api.contains("hydrateTransformExpressionsAst"));
        let slot_spec =
            fs::read_to_string(transforms_tests.join("transformSlotOutlet.spec.ts")).unwrap();
        assert!(slot_spec.contains("from './transformSlotOutlet.rust-api'"));
        assert!(!slot_spec.contains("baseParse as parse"));
        assert!(!slot_spec.contains("transform(ast,"));
        assert!(!slot_spec
            .contains("transformSlotOutlet } from '../../src/transforms/transformSlotOutlet'"));
        let slot_api =
            fs::read_to_string(transforms_tests.join("transformSlotOutlet.rust-api.ts")).unwrap();
        assert!(slot_api.contains("callBridge('vue3.core.transformSlotOutletSuite'"));
        assert!(slot_api.contains("emitErrors"));
        assert!(slot_api.contains("hydrateTransformSlotOutletAst"));
        let v_slot_spec = fs::read_to_string(transforms_tests.join("vSlot.spec.ts")).unwrap();
        assert!(v_slot_spec.contains("from './vSlot.rust-api'"));
        assert!(!v_slot_spec.contains("baseParse as parse"));
        assert!(!v_slot_spec.contains("transform(ast,"));
        assert!(!v_slot_spec.contains("trackSlotScopes"));
        assert!(!v_slot_spec.contains("trackVForSlotScopes"));
        assert!(!v_slot_spec.contains("transformSlotOutlet"));
        let v_slot_api = fs::read_to_string(transforms_tests.join("vSlot.rust-api.ts")).unwrap();
        assert!(v_slot_api.contains("callBridge('vue3.core.transformSlotSuite'"));
        assert!(v_slot_api.contains("emitErrors"));
        assert!(v_slot_api.contains("hydrateVSlotAst"));
        assert!(v_slot_api.contains("node.params = undefined"));
        let cache_static_spec =
            fs::read_to_string(transforms_tests.join("cacheStatic.spec.ts")).unwrap();
        assert!(cache_static_spec.contains("from './cacheStatic.rust-api'"));
        assert!(!cache_static_spec.contains("baseParse as parse"));
        assert!(!cache_static_spec.contains("transform(ast,"));
        assert!(!cache_static_spec.contains("../../src/transforms/"));
        let cache_static_api =
            fs::read_to_string(transforms_tests.join("cacheStatic.rust-api.ts")).unwrap();
        assert!(cache_static_api.contains("callBridge('vue3.core.cacheStaticSuite'"));
        assert!(cache_static_api.contains("hydrateCacheStaticAst"));
        assert!(cache_static_api.contains("node.helpers = new Set"));
        assert!(cache_static_api.contains("node[key] = undefined"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_dom_conformance_shims_use_dom_vitest_glob() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-dom-shims-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        write_vue3_core_source_shims(&temp).unwrap();
        let index_spec = temp
            .join("packages")
            .join("compiler-dom")
            .join("__tests__")
            .join("index.spec.ts");
        fs::create_dir_all(index_spec.parent().unwrap()).unwrap();
        fs::write(&index_spec, "import { compile } from '../src'\n").unwrap();
        write_vue3_dom_conformance_shims(&temp).unwrap();

        let config = fs::read_to_string(temp.join("vitest.config.ts")).unwrap();
        assert!(!config.contains("vitest/config"));
        assert!(config.contains("include: ['packages/compiler-dom/__tests__/**/*.spec.ts']"));
        let index_spec = fs::read_to_string(index_spec).unwrap();
        assert!(index_spec.contains("import { compile } from '@vue/compiler-dom'"));

        let transform_style = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("transformStyle.ts"),
        )
        .unwrap();
        assert!(transform_style.contains("export { transformStyle } from '@vue/compiler-dom'"));

        let stringify_static = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("stringifyStatic.ts"),
        )
        .unwrap();
        assert!(stringify_static.contains("__vuecRuntime"));
        assert!(stringify_static.contains("@vue/compiler-core"));
        assert!(stringify_static.contains("StringifyThresholds"));
        assert!(stringify_static.contains("stringifyStatic"));

        let v_html = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("vHtml.ts"),
        )
        .unwrap();
        assert!(v_html.contains("__vuecRuntime"));
        assert!(v_html.contains("@vue/compiler-dom"));
        assert!(v_html.contains("transformVHtml"));

        let v_text = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("vText.ts"),
        )
        .unwrap();
        assert!(v_text.contains("__vuecRuntime"));
        assert!(v_text.contains("@vue/compiler-dom"));
        assert!(v_text.contains("transformVText"));

        let v_show = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("vShow.ts"),
        )
        .unwrap();
        assert!(v_show.contains("__vuecRuntime"));
        assert!(v_show.contains("@vue/compiler-dom"));
        assert!(v_show.contains("transformShow"));

        let v_on = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("vOn.ts"),
        )
        .unwrap();
        assert!(v_on.contains("__vuecRuntime"));
        assert!(v_on.contains("@vue/compiler-dom"));
        assert!(v_on.contains("transformOn"));
        assert!(v_on.contains("V_ON_WITH_MODIFIERS"));

        let v_model = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("vModel.ts"),
        )
        .unwrap();
        assert!(v_model.contains("__vuecRuntime"));
        assert!(v_model.contains("@vue/compiler-dom"));
        assert!(v_model.contains("transformModel"));
        assert!(v_model.contains("V_MODEL_TEXT"));
        let transition = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("Transition.ts"),
        )
        .unwrap();
        assert!(transition.contains("__vuecRuntime"));
        assert!(transition.contains("@vue/compiler-dom"));
        assert!(transition.contains("transformTransition"));
        assert!(transition.contains("TRANSITION"));
        let ignore_side_effect_tags = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("ignoreSideEffectTags.ts"),
        )
        .unwrap();
        assert!(ignore_side_effect_tags.contains("__vuecRuntime"));
        assert!(ignore_side_effect_tags.contains("@vue/compiler-dom"));
        assert!(ignore_side_effect_tags.contains("ignoreSideEffectTags"));
        let decode_html_browser = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("decodeHtmlBrowser.ts"),
        )
        .unwrap();
        assert!(decode_html_browser.contains("__vuecRuntime"));
        assert!(decode_html_browser.contains("@vue/compiler-dom"));
        assert!(decode_html_browser.contains("decodeHtmlBrowser"));
        let validate_html_nesting = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("validateHtmlNesting.ts"),
        )
        .unwrap();
        assert!(validate_html_nesting.contains("__vuecRuntime"));
        assert!(validate_html_nesting.contains("validateHtmlNesting"));
        let html_nesting = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("htmlNesting.ts"),
        )
        .unwrap();
        assert!(html_nesting.contains("__vuecRuntime"));
        assert!(html_nesting.contains("isValidHTMLNesting"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.transformVHtml"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.transformVText"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.transformShow"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.transformOn"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.transformModel"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.transformTransition"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.ignoreSideEffectTags"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.decodeHtmlBrowser"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.validateHtmlNesting"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.isValidHTMLNesting"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.core.stringifyStatic"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_alias_runtime_projects_transform_hoist_to_rust_stringify_option() {
        assert!(ALIAS_RUNTIME_JS.contains(
            "normalized.__vuecStringifyStatic = typeof options.transformHoist === 'function';"
        ));
    }

    #[test]
    fn vue2_alias_runtime_emits_compile_warnings() {
        assert!(ALIAS_RUNTIME_JS.contains("function emitVue2CompileWarnings(result, options)"));
        assert!(ALIAS_RUNTIME_JS.contains("__vuecSuppressWarnings"));
        assert!(ALIAS_RUNTIME_JS.contains("console.error(message)"));
        let target = TargetSpec {
            version_line: VersionLine::Vue26,
            package: "vue-template-compiler",
            entry: "index",
            kind: TargetKind::Vue26Template,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("compile".into()),
            function_arity: Some(2),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        assert!(alias_export_expression(target, "compile", Some(&detail))
            .contains("emitVue2CompileWarnings(__vuecVue2Result, __vuecPayload.options)"));
    }

    #[test]
    fn vue2_generate_code_frame_alias_reads_all_arguments() {
        let target = TargetSpec {
            version_line: VersionLine::Vue27,
            package: "vue-template-compiler",
            entry: "index",
            kind: TargetKind::Vue27Template,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("generateCodeFrame".into()),
            function_arity: Some(1),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "generateCodeFrame", Some(&detail));
        assert!(expression.contains("const a1 = arguments[1];"));
        assert!(expression.contains("const a2 = arguments[2];"));
        assert!(expression.contains("callBridge(\"vue2.generateCodeFrame\""));
    }

    #[test]
    fn vue3_alias_runtime_dehydrates_public_ast_import_paths() {
        assert!(ALIAS_RUNTIME_JS.contains("key === 'imports' || key === 'path'"));
    }

    #[test]
    fn vue3_dom_conformance_coverage_is_mixed() {
        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue3Dom),
            AliasBackend::Generated,
            None,
        );
        assert_eq!(coverage.source, ConformanceCoverageKind::Mixed);
        assert!(coverage.reason.contains("official DOM source imports"));
    }

    #[test]
    fn vue3_dom_coverage_counts_public_parse_as_rust_backed() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-dom-coverage-{}",
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
              "testResults": [
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/index.spec.ts",
                  "assertionResults": [
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/parse.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/transformStyle.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/ignoreSideEffectTags.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/vHtml.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/vText.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/vShow.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/vOn.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/vModel.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/Transition.spec.ts",
                  "assertionResults": [
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/validateHtmlNesting.spec.ts",
                  "assertionResults": [
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/decoderHtmlBrowser.spec.ts",
                  "assertionResults": [
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/stringifyStatic.spec.ts",
                  "assertionResults": [
                    { "status": "failed" }
                  ]
                }
              ]
            }"#,
        )
        .unwrap();
        let execution = ConformanceExecutionResult {
            status: "failed".into(),
            runner: "vitest".into(),
            prepared_root: "prepared".into(),
            output_file: report.display().to_string(),
            exit_code: Some(1),
            stdout: String::new(),
            stderr: String::new(),
            counts: ConformanceExecutionCounts {
                total: 24,
                pass: 23,
                fail: 1,
                skip: 0,
                pending: 0,
            },
        };

        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue3Dom),
            AliasBackend::Generated,
            Some(&execution),
        );

        assert_eq!(coverage.source, ConformanceCoverageKind::RustBacked);
        assert_eq!(coverage.rust_backed_pass, 23);
        assert_eq!(coverage.rust_backed_total, 24);
        assert_eq!(
            coverage.files[0].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(
            coverage.files[1].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(
            coverage.files[2].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(
            coverage.files[3].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(
            coverage.files[4].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(
            coverage.files[5].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(
            coverage.files[6].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(
            coverage.files[7].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(
            coverage.files[8].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(
            coverage.files[9].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(
            coverage.files[10].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(
            coverage.files[11].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(
            coverage.files[12].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[0]
            .reason
            .contains("routed through vuec_node_bridge"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_sfc_conformance_shims_use_sfc_vitest_glob() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-sfc-shims-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        write_vue3_core_source_shims(&temp).unwrap();
        write_vue3_sfc_conformance_shims(&temp).unwrap();

        let config = fs::read_to_string(temp.join("vitest.config.ts")).unwrap();
        assert!(!config.contains("vitest/config"));
        assert!(config.contains("oxc:"));
        assert!(config.contains("target: 'es2020'"));
        assert!(config.contains("fileParallelism: false"));
        assert!(config.contains("maxWorkers: 1"));
        assert!(config.contains("include: ['packages/compiler-sfc/__tests__/**/*.spec.ts']"));
        assert!(config.contains(
            "'@vue/compiler-core': path.resolve(aliasRoot, 'node_modules/@vue/compiler-core/index.js')"
        ));
        assert!(config.contains(
            "'@vue/compiler-dom': path.resolve(aliasRoot, 'node_modules/@vue/compiler-dom/index.js')"
        ));
        assert!(config.contains(
            "'@vue/compiler-ssr': path.resolve(aliasRoot, 'node_modules/@vue/compiler-ssr/dist/compiler-ssr.cjs.js')"
        ));
        assert!(config.contains(
            "'@vue/compiler-sfc': path.resolve(aliasRoot, 'node_modules/@vue/compiler-sfc/dist/compiler-sfc.cjs.js')"
        ));
        assert!(config
            .contains("'hash-sum': path.resolve(npmRoot, 'node_modules/hash-sum/hash-sum.js')"));
        assert!(config.contains(
            "'lru-cache': path.resolve(npmRoot, 'node_modules/lru-cache/dist/esm/index.js')"
        ));
        assert!(config
            .contains("'postcss': path.resolve(npmRoot, 'node_modules/postcss/lib/postcss.mjs')"));
        assert!(config.contains(
            "'@babel/parser': path.resolve(npmRoot, 'node_modules/@babel/parser/lib/index.js')"
        ));
        let package_json = fs::read_to_string(temp.join("package.json")).unwrap();
        assert!(package_json.contains("\"type\": \"module\""));
        let transform_element = fs::read_to_string(
            temp.join("packages")
                .join("compiler-core")
                .join("src")
                .join("transforms")
                .join("transformElement.ts"),
        )
        .unwrap();
        assert!(transform_element.contains("__vuecRuntime"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_sfc_compile_template_patch_projects_asset_options() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-sfc-asset-patch-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&temp).unwrap();
        let path = temp.join("compileTemplate.ts");
        fs::write(
            &path,
            "compile({\r\n    mode: 'module',\r\n    ...compilerOptions,\r\n    hmr: !isProd,\r\n})\r\n",
        )
        .unwrap();

        patch_vue3_sfc_compile_template_asset_bridge(&path).unwrap();
        patch_vue3_sfc_compile_template_asset_bridge(&path).unwrap();

        let patched = fs::read_to_string(&path).unwrap();
        assert_eq!(patched.matches("transformAssetUrls:").count(), 1);
        assert!(patched.contains("normalizeOptions(transformAssetUrls)"));
        assert!(patched.contains("(compilerOptions as any).transformAssetUrls"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_sfc_public_api_specs_rewrite_imports_to_public_alias() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-sfc-public-api-import-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let tests = temp.join("packages").join("compiler-sfc").join("__tests__");
        fs::create_dir_all(&tests).unwrap();
        fs::write(
            tests.join("parse.spec.ts"),
            "import { parse } from '../src'\nimport { compileScript } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("rewriteDefault.spec.ts"),
            "import { rewriteDefault } from '../src'\nimport { rewriteDefaultAST } from '../src/rewriteDefault'\n",
        )
        .unwrap();
        fs::write(
            tests.join("compileStyle.spec.ts"),
            "import { compileStyle, compileStyleAsync } from '../src/compileStyle'\n",
        )
        .unwrap();
        fs::write(
            tests.join("compileTemplate.spec.ts"),
            "import {\n  type SFCTemplateCompileOptions,\n  compileTemplate,\n} from '../src/compileTemplate'\nimport { type SFCTemplateBlock, parse } from '../src/parse'\nimport { compileScript } from '../src'\nimport { getPositionInCode } from './utils'\n",
        )
        .unwrap();
        fs::write(
            tests.join("cssVars.spec.ts"),
            "import { compileStyle, parse } from '../src'\nimport { assertCode, compileSFCScript, mockId } from './utils'\n",
        )
        .unwrap();
        fs::write(
            tests.join("compileScript.spec.ts"),
            "import { assertCode, compileSFCScript as compile } from './utils'\n",
        )
        .unwrap();
        let compile_script_tests = tests.join("compileScript");
        fs::create_dir_all(&compile_script_tests).unwrap();
        for compile_script_spec in [
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
            fs::write(
                compile_script_tests.join(compile_script_spec),
                "import { assertCode, compileSFCScript as compile } from '../utils'\n",
            )
            .unwrap();
        }
        fs::write(
            compile_script_tests.join("resolveType.spec.ts"),
            format!(
                "{}\ndescribe('resolveType', () => {{\n  test('type literal', () => {{\n    const {{ props, calls }} = resolve(`defineProps<{{ foo: number; (e: 'save'): void }}>()`)\n    expect(props).toStrictEqual({{ foo: ['Number'] }})\n    expect(calls?.length).toBe(1)\n    expect(UNKNOWN_TYPE).toBe('Unknown')\n  }})\n}})\n\n{}",
                VUE3_SFC_RESOLVE_TYPE_INTERNAL_IMPORTS,
                VUE3_SFC_RESOLVE_TYPE_INTERNAL_HELPER
            ),
        )
        .unwrap();
        fs::write(
            tests.join("templateUtils.spec.ts"),
            "import {\n  isDataUrl,\n  isExternalUrl,\n  isRelativeUrl,\n} from '../src/template/templateUtils'\n",
        )
        .unwrap();
        fs::write(
            tests.join("templateTransformAssetUrl.spec.ts"),
            format!(
                "{}\ndescribe('asset url', () => {{ compileWithAssetUrls('<img src=\"./x.png\"/>') }})\n",
                VUE3_SFC_ASSET_TRANSFORM_INTERNAL_HELPER
            ),
        )
        .unwrap();
        fs::write(
            tests.join("templateTransformSrcset.spec.ts"),
            format!(
                "{}\ndescribe('srcset', () => {{ compileWithSrcset('<img srcset=\"./x.png 2x\"/>') }})\n",
                VUE3_SFC_SRCSET_TRANSFORM_INTERNAL_HELPER
            ),
        )
        .unwrap();
        fs::write(
            tests.join("utils.ts"),
            "import { compileScript, parse } from '../src'\n",
        )
        .unwrap();

        rewrite_vue3_sfc_public_api_spec_imports(&temp).unwrap();
        rewrite_vue3_sfc_public_api_spec_imports(&temp).unwrap();

        let parse_spec = fs::read_to_string(tests.join("parse.spec.ts")).unwrap();
        assert!(parse_spec.contains("import { parse } from '@vue/compiler-sfc'"));
        assert!(parse_spec.contains("import { compileScript } from '../src'"));
        assert_eq!(parse_spec.matches("@vue/compiler-sfc").count(), 1);

        let rewrite_default_spec =
            fs::read_to_string(tests.join("rewriteDefault.spec.ts")).unwrap();
        assert!(rewrite_default_spec.contains("import { rewriteDefault } from '@vue/compiler-sfc'"));
        assert!(rewrite_default_spec
            .contains("import { rewriteDefaultAST } from '../src/rewriteDefault'"));
        assert_eq!(rewrite_default_spec.matches("@vue/compiler-sfc").count(), 1);

        let compile_style_spec = fs::read_to_string(tests.join("compileStyle.spec.ts")).unwrap();
        assert!(compile_style_spec
            .contains("import { compileStyle, compileStyleAsync } from '@vue/compiler-sfc'"));
        assert_eq!(compile_style_spec.matches("@vue/compiler-sfc").count(), 1);

        let compile_template_spec =
            fs::read_to_string(tests.join("compileTemplate.spec.ts")).unwrap();
        assert!(compile_template_spec.contains("from '@vue/compiler-sfc'"));
        assert!(compile_template_spec.contains("import { compileScript } from '@vue/compiler-sfc'"));
        assert!(compile_template_spec.contains("from './utils.public-api'"));
        assert!(!compile_template_spec.contains("../src/compileTemplate"));
        assert!(!compile_template_spec.contains("../src/parse"));
        assert_eq!(
            compile_template_spec.matches("@vue/compiler-sfc").count(),
            3
        );

        let css_vars_spec = fs::read_to_string(tests.join("cssVars.spec.ts")).unwrap();
        assert!(css_vars_spec.contains("import { compileStyle, parse } from '@vue/compiler-sfc'"));
        assert!(
            css_vars_spec.contains("from './utils.public-api'"),
            "cssVars should use a dedicated helper so shared utils.ts remains scoped to mixed files"
        );
        assert_eq!(css_vars_spec.matches("@vue/compiler-sfc").count(), 1);
        let root_compile_script_spec =
            fs::read_to_string(tests.join("compileScript.spec.ts")).unwrap();
        assert!(root_compile_script_spec.contains("from './utils.public-api'"));
        assert!(!root_compile_script_spec.contains("from './utils'"));
        let shared_utils = fs::read_to_string(tests.join("utils.ts")).unwrap();
        assert!(shared_utils.contains("from '../src'"));
        let public_utils = fs::read_to_string(tests.join("utils.public-api.ts")).unwrap();
        assert!(public_utils.contains("from '@vue/compiler-sfc'"));
        assert!(public_utils.contains("export function compileSFCScript"));
        assert!(public_utils.contains("__vuecEmitScriptSetupMarker: false"));
        assert!(public_utils.contains("babelParse(code"));
        assert!(public_utils.contains("export function getPositionInCode"));
        let template_utils_spec = fs::read_to_string(tests.join("templateUtils.spec.ts")).unwrap();
        assert!(template_utils_spec.contains("from './templateUtils.rust-api'"));
        assert!(!template_utils_spec.contains("../src/template/templateUtils"));
        let template_utils_api =
            fs::read_to_string(tests.join("templateUtils.rust-api.ts")).unwrap();
        assert!(template_utils_api.contains("from '@vue/compiler-sfc'"));
        assert!(template_utils_api.contains("sfc.templateUtils.isRelativeUrl"));
        assert!(template_utils_api.contains("sfc.templateUtils.isExternalUrl"));
        assert!(template_utils_api.contains("sfc.templateUtils.isDataUrl"));
        let asset_transform_spec =
            fs::read_to_string(tests.join("templateTransformAssetUrl.spec.ts")).unwrap();
        assert!(asset_transform_spec.contains("from './templateTransforms.public-api'"));
        assert!(asset_transform_spec.contains("compileWithAssetUrls"));
        assert!(!asset_transform_spec.contains("@vue/compiler-core"));
        assert!(!asset_transform_spec.contains("../src/template/transformAssetUrl"));
        assert!(!asset_transform_spec.contains("../../compiler-dom/src/transforms/stringifyStatic"));
        let srcset_transform_spec =
            fs::read_to_string(tests.join("templateTransformSrcset.spec.ts")).unwrap();
        assert!(srcset_transform_spec.contains("from './templateTransforms.public-api'"));
        assert!(srcset_transform_spec.contains("compileWithSrcset"));
        assert!(!srcset_transform_spec.contains("@vue/compiler-core"));
        assert!(!srcset_transform_spec.contains("../src/template/transformSrcset"));
        assert!(
            !srcset_transform_spec.contains("../../compiler-dom/src/transforms/stringifyStatic")
        );
        let template_transforms_api =
            fs::read_to_string(tests.join("templateTransforms.public-api.ts")).unwrap();
        assert!(template_transforms_api.contains("compileTemplate"));
        assert!(template_transforms_api.contains("export function compileWithAssetUrls"));
        assert!(template_transforms_api.contains("export function compileWithSrcset"));
        assert!(template_transforms_api.contains("compilerOptions"));
        assert!(template_transforms_api.contains("transformAssetUrls"));
        assert!(template_transforms_api.contains("img: []"));
        for compile_script_spec in [
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
            let spec = fs::read_to_string(compile_script_tests.join(compile_script_spec)).unwrap();
            assert!(spec.contains("from '../utils.public-api'"));
            assert!(!spec.contains("from '../utils'"));
        }
        let resolve_type_spec =
            fs::read_to_string(compile_script_tests.join("resolveType.spec.ts")).unwrap();
        assert!(resolve_type_spec.contains("from './resolveType.rust-api'"));
        assert!(!resolve_type_spec.contains("../../src/script/resolveType"));
        assert!(!resolve_type_spec.contains("../../src/script/context"));
        assert!(!resolve_type_spec.contains("../../src/script/utils"));
        assert!(!resolve_type_spec.contains("registerTS(() => ts)"));
        assert!(!resolve_type_spec.contains("function resolve("));
        let resolve_type_api =
            fs::read_to_string(compile_script_tests.join("resolveType.rust-api.ts")).unwrap();
        assert!(resolve_type_api.contains("from '@vue/compiler-sfc'"));
        assert!(resolve_type_api.contains("__vuecRuntime"));
        assert!(resolve_type_api.contains("sfc.resolveType"));
        assert!(resolve_type_api.contains("globalTypeFiles"));
        assert!(resolve_type_api.contains("materializeFiles"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_sfc_conformance_coverage_is_mixed() {
        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue3Sfc),
            AliasBackend::Generated,
            None,
        );
        assert_eq!(coverage.source, ConformanceCoverageKind::Mixed);
        assert!(coverage.reason.contains("official SFC TypeScript source"));
        assert!(coverage.reason.contains("not standalone Rust SFC parity"));
    }

    #[test]
    fn vue3_sfc_coverage_marks_public_parse_and_rewrite_default_rust_backed() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-sfc-coverage-{}",
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
              "testResults": [
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/parse.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/rewriteDefault.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileStyle.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/cssVars.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript.spec.ts",
                  "assertionResults": [
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileTemplate.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/templateUtils.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/templateTransformAssetUrl.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/templateTransformSrcset.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/defineEmits.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/defineExpose.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/defineModel.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/defineOptions.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/defineProps.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/definePropsDestructure.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/defineSlots.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/hoistStatic.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/importUsageCheck.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/resolveType.spec.ts",
                  "assertionResults": [
                    { "status": "passed" }
                  ]
                }
              ]
            }"#,
        )
        .unwrap();
        let execution = ConformanceExecutionResult {
            status: "failed".into(),
            runner: "vitest".into(),
            prepared_root: "prepared".into(),
            output_file: report.display().to_string(),
            exit_code: Some(1),
            stdout: String::new(),
            stderr: String::new(),
            counts: ConformanceExecutionCounts {
                total: 199,
                pass: 199,
                fail: 0,
                skip: 0,
                pending: 0,
            },
        };

        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue3Sfc),
            AliasBackend::Generated,
            Some(&execution),
        );

        assert_eq!(coverage.source, ConformanceCoverageKind::RustBacked);
        assert_eq!(coverage.rust_backed_pass, 199);
        assert_eq!(coverage.rust_backed_total, 199);
        assert_eq!(
            coverage.files[0].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(
            coverage.files[1].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(
            coverage.files[2].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[2]
            .reason
            .contains("additionalData callbacks"));
        assert_eq!(
            coverage.files[3].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[3].reason.contains("Babel syntax assertion"));
        assert_eq!(
            coverage.files[4].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[4]
            .reason
            .contains("Rust vuec_sfc compileScript implementation"));
        assert_eq!(
            coverage.files[5].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[5]
            .reason
            .contains("custom compiler callbacks"));
        assert_eq!(
            coverage.files[6].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[6].reason.contains("vuec_vue3_asset"));
        assert_eq!(
            coverage.files[7].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[7].reason.contains("asset/srcset transform"));
        assert_eq!(
            coverage.files[8].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[8].reason.contains("asset/srcset transform"));
        for file in coverage.files.iter().skip(9).take(9) {
            assert_eq!(file.source, ConformanceCoverageKind::RustBacked);
            assert!(file
                .reason
                .contains("Rust vuec_sfc compileScript implementation"));
        }
        assert_eq!(
            coverage.files[18].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[18].reason.contains("resolve_vue3_type"));
        assert_eq!(
            coverage
                .counts_by_source
                .get("rust-backed")
                .copied()
                .unwrap_or_default()
                .pass,
            199
        );
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_ssr_conformance_shims_use_ssr_vitest_glob() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-ssr-shims-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        write_vue3_core_source_shims(&temp).unwrap();
        write_vue3_ssr_conformance_shims(&temp).unwrap();

        let config = fs::read_to_string(temp.join("vitest.config.ts")).unwrap();
        assert!(!config.contains("vitest/config"));
        assert!(config.contains("include: ['packages/compiler-ssr/__tests__/**/*.spec.ts']"));
        assert!(config.contains(
            "'@vue/compiler-dom': path.resolve(root, 'packages/compiler-dom/src/index.ts')"
        ));
        assert!(config.contains(
            "'@vue/compiler-ssr': path.resolve(aliasRoot, 'node_modules/@vue/compiler-ssr/dist/compiler-ssr.cjs.js')"
        ));
        assert!(config.contains("'packages/compiler-core/src/transform': path.resolve(root, 'packages/compiler-core/src/transform.ts')"));

        let transform = fs::read_to_string(
            temp.join("packages")
                .join("compiler-core")
                .join("src")
                .join("transform.ts"),
        )
        .unwrap();
        assert!(transform.contains("export * from \"@vue/compiler-core\""));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_ssr_rust_backed_specs_route_compile_to_public_alias() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-ssr-rust-backed-routing-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let tests = temp.join("packages").join("compiler-ssr").join("__tests__");
        fs::create_dir_all(&tests).unwrap();
        fs::write(
            tests.join("ssrText.spec.ts"),
            "import { compile } from '../src'\nimport { getCompiledString } from './utils'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrVIf.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrVFor.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrScopeId.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrFallthroughAttrs.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrInjectCssVars.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrVShow.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrVModel.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrElement.spec.ts"),
            "import { compile } from '../src'\nimport { getCompiledString } from './utils'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrSlotOutlet.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrPortal.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrSuspense.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrTransition.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrTransitionGroup.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrComponent.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(tests.join("utils.ts"), "import { compile } from '../src'\n").unwrap();

        rewrite_vue3_ssr_rust_backed_public_compile_imports(&temp).unwrap();

        let spec = fs::read_to_string(tests.join("ssrText.spec.ts")).unwrap();
        let vif_spec = fs::read_to_string(tests.join("ssrVIf.spec.ts")).unwrap();
        let vfor_spec = fs::read_to_string(tests.join("ssrVFor.spec.ts")).unwrap();
        let scope_id_spec = fs::read_to_string(tests.join("ssrScopeId.spec.ts")).unwrap();
        let fallthrough_attrs_spec =
            fs::read_to_string(tests.join("ssrFallthroughAttrs.spec.ts")).unwrap();
        let inject_css_vars_spec =
            fs::read_to_string(tests.join("ssrInjectCssVars.spec.ts")).unwrap();
        let vshow_spec = fs::read_to_string(tests.join("ssrVShow.spec.ts")).unwrap();
        let vmodel_spec = fs::read_to_string(tests.join("ssrVModel.spec.ts")).unwrap();
        let element_spec = fs::read_to_string(tests.join("ssrElement.spec.ts")).unwrap();
        let slot_outlet_spec = fs::read_to_string(tests.join("ssrSlotOutlet.spec.ts")).unwrap();
        let portal_spec = fs::read_to_string(tests.join("ssrPortal.spec.ts")).unwrap();
        let suspense_spec = fs::read_to_string(tests.join("ssrSuspense.spec.ts")).unwrap();
        let transition_spec = fs::read_to_string(tests.join("ssrTransition.spec.ts")).unwrap();
        let transition_group_spec =
            fs::read_to_string(tests.join("ssrTransitionGroup.spec.ts")).unwrap();
        let component_spec = fs::read_to_string(tests.join("ssrComponent.spec.ts")).unwrap();
        let utils = fs::read_to_string(tests.join("utils.ts")).unwrap();
        let rust_text_utils = fs::read_to_string(tests.join("utils.rust-ssr-text.ts")).unwrap();
        assert!(spec.contains("from '@vue/compiler-ssr'"));
        assert!(spec.contains("from './utils.rust-ssr-text'"));
        assert!(vif_spec.contains("from '@vue/compiler-ssr'"));
        assert!(vfor_spec.contains("from '@vue/compiler-ssr'"));
        assert!(scope_id_spec.contains("from '@vue/compiler-ssr'"));
        assert!(fallthrough_attrs_spec.contains("from '@vue/compiler-ssr'"));
        assert!(inject_css_vars_spec.contains("from '@vue/compiler-ssr'"));
        assert!(vshow_spec.contains("from '@vue/compiler-ssr'"));
        assert!(vmodel_spec.contains("from '@vue/compiler-ssr'"));
        assert!(element_spec.contains("from '@vue/compiler-ssr'"));
        assert!(element_spec.contains("from './utils.rust-ssr-text'"));
        assert!(slot_outlet_spec.contains("from '@vue/compiler-ssr'"));
        assert!(portal_spec.contains("from '@vue/compiler-ssr'"));
        assert!(suspense_spec.contains("from '@vue/compiler-ssr'"));
        assert!(transition_spec.contains("from '@vue/compiler-ssr'"));
        assert!(transition_group_spec.contains("from '@vue/compiler-ssr'"));
        assert!(component_spec.contains("from '@vue/compiler-ssr'"));
        assert!(utils.contains("from '../src'"));
        assert!(!utils.contains("from '@vue/compiler-ssr'"));
        assert!(rust_text_utils.contains("from '@vue/compiler-ssr'"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_ssr_conformance_coverage_is_mixed() {
        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue3Ssr),
            AliasBackend::Generated,
            None,
        );
        assert_eq!(coverage.source, ConformanceCoverageKind::Mixed);
        assert!(coverage
            .reason
            .contains("official SSR and DOM source imports"));
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
    fn official_install_marker_requires_current_platform() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-platform-marker-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&temp).unwrap();
        let marker = temp.join("official-install.json");
        let specs = vec!["vue@3.5.34".to_string()];

        write_json(
            &marker,
            &serde_json::json!({
                "packages": specs,
                "platform": npm_install_platform_marker(),
            }),
        )
        .unwrap();
        assert!(official_install_marker_matches(&marker, &specs));

        write_json(
            &marker,
            &serde_json::json!({
                "packages": specs,
                "platform": "other-os-other-arch",
            }),
        )
        .unwrap();
        assert!(!official_install_marker_matches(&marker, &specs));

        write_json(
            &marker,
            &serde_json::json!({
                "packages": specs,
            }),
        )
        .unwrap();
        assert!(!official_install_marker_matches(&marker, &specs));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn runtime_smoke_dependency_specs_use_locked_jsdom_versions() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-runtime-smoke-deps-{}",
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
                "jsdom": "^29.1.1"
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
  jsdom@29.1.1: {}
"#,
        )
        .unwrap();

        let specs = runtime_smoke_dependency_specs(VersionLine::Vue3, &temp).unwrap();
        assert_eq!(specs, vec!["jsdom@29.1.1"]);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn runtime_smoke_required_dependencies_cover_vue3_ssr_runtime() {
        assert_eq!(
            runtime_smoke_required_node_dependencies(VersionLine::Vue26),
            ["vue", "jsdom"]
        );
        assert_eq!(
            runtime_smoke_required_node_dependencies(VersionLine::Vue27),
            ["vue", "jsdom"]
        );
        assert_eq!(
            runtime_smoke_required_node_dependencies(VersionLine::Vue3),
            ["vue", "@vue/compiler-ssr", "@vue/server-renderer", "jsdom"]
        );
    }

    #[test]
    fn conformance_targets_include_suite_package_requests() {
        let targets = conformance_targets(&[ConformanceSuite::Vue3Dom]);
        let requests = targets
            .into_iter()
            .map(api_require_request)
            .collect::<Vec<_>>();
        assert_eq!(requests, vec!["@vue/compiler-core", "@vue/compiler-dom"]);

        let sfc_targets = conformance_targets(&[ConformanceSuite::Vue3Sfc]);
        let sfc_requests = sfc_targets
            .into_iter()
            .map(api_require_request)
            .collect::<Vec<_>>();
        assert_eq!(
            sfc_requests,
            vec![
                "@vue/compiler-core",
                "@vue/compiler-dom",
                "@vue/compiler-ssr",
                "@vue/compiler-sfc",
            ]
        );
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
                "@babel/parser": "^7.29.3",
                "@babel/types": "^7.29.0",
                "@vue/consolidate": "1.0.0",
                "estree-walker": "^2.0.2",
                "vitest": "^4.1.5",
                "esbuild": "^0.28.0",
                "hash-sum": "^2.0.0",
                "jsdom": "^29.1.1",
                "lru-cache": "11.5.0",
                "magic-string": "^0.30.21",
                "merge-source-map": "^1.1.0",
                "minimatch": "~10.2.5",
                "postcss-modules": "^6.0.1",
                "postcss-selector-parser": "^7.1.1",
                "pug": "^3.0.4",
                "sass": "^1.99.0",
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
  '@babel/parser@7.29.3': {}
  '@babel/types@7.29.0': {}
  '@vue/consolidate@1.0.0': {}
  esbuild@0.28.0: {}
  estree-walker@2.0.2: {}
  hash-sum@2.0.0: {}
  jsdom@29.1.1: {}
  lru-cache@11.5.0: {}
  magic-string@0.30.21: {}
  merge-source-map@1.1.0: {}
  minimatch@10.2.5: {}
  postcss-modules@6.0.1(postcss@8.5.14): {}
  postcss-selector-parser@7.1.1: {}
  pug@3.0.4: {}
  sass@1.99.0: {}
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
        let dom_specs = runner_dependency_specs(suite_spec(ConformanceSuite::Vue3Dom), &temp)
            .unwrap()
            .unwrap();
        assert_eq!(
            dom_specs,
            vec![
                "esbuild@0.28.0",
                "jsdom@29.1.1",
                "source-map-js@1.2.1",
                "vitest@4.1.5"
            ]
        );
        let sfc_specs = runner_dependency_specs(suite_spec(ConformanceSuite::Vue3Sfc), &temp)
            .unwrap()
            .unwrap();
        assert_eq!(
            sfc_specs,
            vec![
                "@babel/parser@7.29.3",
                "@babel/types@7.29.0",
                "@vue/consolidate@1.0.0",
                "esbuild@0.28.0",
                "estree-walker@2.0.2",
                "hash-sum@2.0.0",
                "lru-cache@11.5.0",
                "magic-string@0.30.21",
                "merge-source-map@1.1.0",
                "minimatch@10.2.5",
                "postcss-modules@6.0.1",
                "postcss-selector-parser@7.1.1",
                "pug@3.0.4",
                "sass@1.99.0",
                "source-map-js@1.2.1",
                "typescript@5.6.3",
                "vitest@4.1.5",
            ]
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
        let fallback_root = temp.join("vue2_7");
        fs::create_dir_all(&fallback_root).unwrap();
        fs::write(
            fallback_root.join("package.json"),
            r#"{
              "devDependencies": {
                "jsdom": "^19.0.0"
              }
            }"#,
        )
        .unwrap();
        fs::write(
            fallback_root.join("pnpm-lock.yaml"),
            r#"
packages:
  .
snapshots:
  /jsdom@19.0.0: {}
"#,
        )
        .unwrap();

        let specs = runner_dependency_specs(suite_spec(ConformanceSuite::Vue2Compiler), &temp)
            .unwrap()
            .unwrap();
        assert_eq!(
            specs,
            vec!["@babel/register@^7.0.0", "jasmine@^2.99.0", "jsdom@19.0.0"]
        );
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
            official_revision: Some("612fb89547711cacb030a3893a0065b785802860".into()),
        }
    }
}
