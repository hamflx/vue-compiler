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
#[serde(rename_all = "snake_case")]
pub enum VersionLine {
    Vue26,
    Vue27,
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

    fn api_exports(&self) -> &'static [&'static str] {
        match self.kind {
            TargetKind::Vue26Template => &["compile", "compileToFunctions"],
            TargetKind::Vue27Template => &["compile", "compileToFunctions"],
            TargetKind::Vue27Sfc => &["parse", "compileScript", "compileTemplate", "compileStyle"],
            TargetKind::Vue3Core => &["baseParse", "baseCompile"],
            TargetKind::Vue3Dom => &["compile", "parse"],
            TargetKind::Vue3Ssr => &["compile"],
            TargetKind::Vue3Sfc => &["parse", "compileScript", "compileTemplate", "compileStyle"],
        }
    }

    fn option_categories(&self) -> &'static [&'static str] {
        match self.kind {
            TargetKind::Vue26Template => &[
                "modules",
                "directives",
                "warn",
                "outputSourceRange",
                "comments",
                "delimiters",
                "whitespace/preserveWhitespace",
                "decode newlines",
                "platform predicates",
                "codeframe",
            ],
            TargetKind::Vue27Template => &[
                "modules",
                "directives",
                "warn",
                "outputSourceRange",
                "comments",
                "delimiters",
                "whitespace/preserveWhitespace",
                "decode newlines",
                "platform predicates",
                "codeframe",
                "2.7 delta",
            ],
            TargetKind::Vue27Sfc => &[
                "parse",
                "compileTemplate",
                "compileScript",
                "compileStyle",
                "source map",
                "id/scope",
                "TS/script setup",
                "CSS modules/preprocess",
            ],
            TargetKind::Vue3Core => &[
                "prefixIdentifiers",
                "mode",
                "hoistStatic",
                "cacheHandlers",
                "scopeId",
                "slotted",
                "isTS",
                "expressionPlugins",
                "nodeTransforms",
                "directiveTransforms",
                "transformAssetUrls",
                "ssrCssVars",
                "hmr",
                "source map",
                "error position",
            ],
            TargetKind::Vue3Dom => &[
                "parser options",
                "entity decode",
                "custom element",
                "v-model variants",
                "event modifiers",
                "binding modifiers",
                "asset URL",
            ],
            TargetKind::Vue3Ssr => &[
                "SSR mode",
                "scope",
                "slotted",
                "teleport",
                "suspense",
                "SSR helpers",
            ],
            TargetKind::Vue3Sfc => &[
                "parse",
                "compileTemplate",
                "compileScript",
                "compileStyle",
                "transformAssetUrls",
                "ssrCssVars",
                "HMR",
                "source map",
                "preprocess",
                "macro/type resolve",
            ],
        }
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
    version_line: VersionLine,
    package: String,
    entry: String,
    exports: Vec<String>,
    status: String,
    source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct OptionMatrixFile {
    version_line: VersionLine,
    package: String,
    entry: String,
    required_options: Vec<String>,
    fixture_ids: Vec<String>,
    status: String,
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
    for target in targets {
        let path = target.relative_api_manifest_path(if scope.rust { "rust" } else { "official" });
        let manifest = ManifestFile {
            version_line: target.version_line,
            package: target.package.to_string(),
            entry: target.entry.to_string(),
            exports: target
                .api_exports()
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            status: "pending".into(),
            source: if scope.rust {
                "rust-spec"
            } else {
                "official-spec"
            }
            .into(),
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = write_json(&path, &manifest);
        created.push(path.display().to_string());
        items.push(ReportItem::new(
            target.display(),
            ReportStatus::Pending,
            format!("{} exports captured", target.api_exports().len()),
            Some(path),
        ));
    }
    JsonReport::new("export_api", ReportStatus::Pending)
        .with_scope(scope)
        .with_items(items)
        .with_created(created)
        .with_note("API manifest generation is wired to the compatibility spec, not the final compiler backends yet")
}

pub fn diff_api(scope: &SelectionArgs) -> JsonReport {
    let targets = select_targets(scope);
    let mut items = Vec::new();
    for target in targets {
        items.push(ReportItem::new(
            target.display(),
            ReportStatus::Pending,
            "official and rust manifests are not yet backed by final compiler packages",
            None,
        ));
    }
    JsonReport::new("diff_api", ReportStatus::Pending)
        .with_scope(scope)
        .with_items(items)
        .with_note("diff is scaffolded; real package parity will replace the spec-only manifests")
}

pub fn generate_option_matrix(scope: &SelectionArgs) -> JsonReport {
    let targets = select_targets(scope);
    let mut created = Vec::new();
    let mut items = Vec::new();
    for target in targets {
        let path = target.relative_option_matrix_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let matrix = OptionMatrixFile {
            version_line: target.version_line,
            package: target.package.to_string(),
            entry: target.entry.to_string(),
            required_options: target
                .option_categories()
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            fixture_ids: Vec::new(),
            status: "pending".into(),
        };
        let _ = write_json(&path, &matrix);
        created.push(path.display().to_string());
        items.push(ReportItem::new(
            target.display(),
            ReportStatus::Pass,
            format!("{} option categories seeded", matrix.required_options.len()),
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
                let expected: Vec<String> = target
                    .option_categories()
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect();
                if matrix.required_options == expected && matrix.version_line == target.version_line
                {
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
    let mut items = Vec::new();
    for target in targets {
        let path = target.relative_option_matrix_path();
        let detail = if path.exists() {
            "option matrix available; runtime execution not yet wired to final compiler backends"
        } else {
            "option matrix missing"
        };
        items.push(ReportItem::new(
            target.display(),
            ReportStatus::Pending,
            detail,
            Some(path),
        ));
    }
    JsonReport::new("run_option_matrix", ReportStatus::Pending)
        .with_scope(scope)
        .with_items(items)
        .with_note("option matrix execution is scaffolded and will become pass/fail once compiler backends land")
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
    for target in targets {
        let path = target.relative_output_contract_path();
        items.push(ReportItem::new(
            target.display(),
            ReportStatus::Pending,
            if path.exists() {
                "output contract file available; runtime parity not yet wired"
            } else {
                "output contract file missing"
            },
            Some(path),
        ));
    }
    JsonReport::new("run_output_contract", ReportStatus::Pending)
        .with_scope(scope)
        .with_items(items)
        .with_note("output contract execution is scaffolded and will become pass/fail once compiler backends land")
}

pub fn verify_npm_alias(scope: &SelectionArgs) -> JsonReport {
    let targets = select_targets(scope);
    let items = targets
        .into_iter()
        .map(|target| {
            ReportItem::new(
                target.display(),
                ReportStatus::Pending,
                "npm alias wiring is documented but final package publishing is not yet implemented",
                None,
            )
        })
        .collect::<Vec<_>>();
    JsonReport::new("verify_npm_alias", ReportStatus::Pending)
        .with_scope(scope)
        .with_items(items)
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
