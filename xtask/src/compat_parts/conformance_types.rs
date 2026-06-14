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
    #[serde(skip_serializing_if = "Option::is_none")]
    prepared_manifest_file: Option<String>,
    output_file: String,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    counts: ConformanceExecutionCounts,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AliasRuntimeFragmentManifestEntry {
    order: u32,
    name: String,
    role: String,
    source: String,
    source_anchor: String,
    execution_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    migration_note: Option<String>,
}

impl From<AliasRuntimeFragmentSpec> for AliasRuntimeFragmentManifestEntry {
    fn from(spec: AliasRuntimeFragmentSpec) -> Self {
        Self {
            order: spec.order,
            name: spec.name.to_string(),
            role: spec.role.to_string(),
            source: spec.source.to_string(),
            source_anchor: spec.source_anchor.to_string(),
            execution_path: spec.execution_path.to_string(),
            migration_note: spec.migration_note.map(str::to_string),
        }
    }
}

fn alias_runtime_fragment_manifest_entries() -> Vec<AliasRuntimeFragmentManifestEntry> {
    ALIAS_RUNTIME_FRAGMENT_SPECS
        .iter()
        .copied()
        .map(AliasRuntimeFragmentManifestEntry::from)
        .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PreparedTestManifest {
    schema_version: u32,
    suite: String,
    official_test_origin: String,
    #[serde(default)]
    alias_runtime_fragments: Vec<AliasRuntimeFragmentManifestEntry>,
    entries: Vec<PreparedTestManifestEntry>,
}

impl PreparedTestManifest {
    fn new(suite: &str) -> Self {
        Self {
            schema_version: 1,
            suite: suite.to_string(),
            official_test_origin: "unmodified-official".into(),
            alias_runtime_fragments: alias_runtime_fragment_manifest_entries(),
            entries: Vec::new(),
        }
    }

    fn push(&mut self, entry: PreparedTestManifestEntry) {
        self.entries.push(entry);
        self.official_test_origin = self.derived_origin().to_string();
    }

    fn derived_origin(&self) -> &'static str {
        if self.entries.is_empty() {
            "unmodified-official"
        } else {
            "prepared-official"
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PreparedTestManifestEntry {
    original_path: String,
    prepared_path: String,
    rewrite_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    helper_path: Option<String>,
    related_bridge_commands: Vec<String>,
    expected_provenance: PreparedTestProvenanceExpectation,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PreparedTestProvenanceExpectation {
    test_origin: String,
    execution_path: String,
    api_surface: String,
    adapter_roles: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct PreparedTestManifestReport {
    official_test_origin: String,
    manifest_file: String,
    entry_count: usize,
    alias_runtime_fragments: Vec<AliasRuntimeFragmentManifestEntry>,
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
    summary: BTreeMap<String, ConformanceExecutionCounts>,
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
    #[serde(flatten)]
    provenance: ConformanceCoverageProvenance,
    reason: String,
    counts: ConformanceExecutionCounts,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct ConformanceCoverageProvenance {
    test_origin: String,
    execution_path: String,
    api_surface: String,
    adapter_roles: Vec<String>,
    bridge_commands: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    runtime_markers: Vec<String>,
}

impl ConformanceCoverageProvenance {
    fn new(
        test_origin: &str,
        execution_path: &str,
        api_surface: &str,
        adapter_roles: &[&str],
        bridge_commands: &[&str],
    ) -> Self {
        Self {
            test_origin: test_origin.to_string(),
            execution_path: execution_path.to_string(),
            api_surface: api_surface.to_string(),
            adapter_roles: adapter_roles.iter().copied().map(str::to_string).collect(),
            bridge_commands: bridge_commands
                .iter()
                .copied()
                .map(str::to_string)
                .collect(),
            runtime_markers: Vec::new(),
        }
    }

    fn from_prepared_expectation(entry: &PreparedTestManifestEntry) -> Self {
        let adapter_roles = canonical_adapter_roles(&entry.expected_provenance.adapter_roles);
        let api_surface = canonical_bridge_api_surface(
            &entry.related_bridge_commands,
            &entry.expected_provenance.api_surface,
        );
        let execution_path =
            canonical_execution_path(&entry.expected_provenance, &adapter_roles, &api_surface);
        Self {
            test_origin: entry.expected_provenance.test_origin.clone(),
            execution_path,
            api_surface,
            adapter_roles,
            bridge_commands: entry.related_bridge_commands.clone(),
            runtime_markers: Vec::new(),
        }
    }

    fn with_runtime_markers(mut self, markers: Vec<String>) -> Self {
        let mut has_callback_boundary = false;
        let mut has_semantic_js = false;
        for marker in &markers {
            if let Some(command) = marker.strip_prefix("bridge:") {
                push_unique_string(&mut self.bridge_commands, command);
            }
            if marker_is_callback_boundary(marker) {
                push_unique_string(&mut self.adapter_roles, "callback-materialization");
                has_callback_boundary = true;
            }
            if marker_is_semantic_js(marker) {
                push_unique_string(&mut self.adapter_roles, "semantic-shim");
                has_semantic_js = true;
            }
        }
        self.api_surface = canonical_bridge_api_surface(&self.bridge_commands, &self.api_surface);
        if has_callback_boundary {
            self.execution_path = "mixed-js-callback-boundary".into();
        } else if has_semantic_js {
            self.execution_path = "shim-backed-semantic-js".into();
        }
        self.runtime_markers = markers;
        self
    }

    fn legacy_source(&self) -> ConformanceCoverageKind {
        if self
            .adapter_roles
            .iter()
            .any(|role| role == "callback-materialization")
            || self.api_surface == "suite-only-bridge-command"
            || matches!(
                self.execution_path.as_str(),
                "hybrid-js-adapter-rust-projection" | "mixed-js-callback-boundary"
            )
        {
            return ConformanceCoverageKind::Mixed;
        }
        if self
            .adapter_roles
            .iter()
            .any(|role| role == "semantic-shim")
            || self.execution_path == "shim-backed-semantic-js"
        {
            return ConformanceCoverageKind::ShimBacked;
        }
        if matches!(
            self.execution_path.as_str(),
            "pure-rust-public-api" | "rust-bridge-shape-adapter"
        ) {
            return ConformanceCoverageKind::RustBacked;
        }
        ConformanceCoverageKind::Mixed
    }
}
