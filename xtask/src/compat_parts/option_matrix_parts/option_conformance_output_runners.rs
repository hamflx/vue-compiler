pub fn generate_option_matrix(scope: &SelectionArgs, out_dir: &Path) -> JsonReport {
    let targets = select_targets(scope);
    let mut created = Vec::new();
    let mut items = Vec::new();
    for target in targets {
        let path = target.option_matrix_path_in(out_dir);
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
        let request = api_require_request(target);
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
            let official_probe = run_option_probe(OptionProbeRequest {
                side: "official",
                target,
                root: &official_root,
                request: &request,
                method: &row.method,
                fixture_source: &row.fixture_source,
                fixture_id: &row.fixture_id,
                option_name: &row.option_name,
                option_path: &row.option_path,
                input_kind: &row.input_kind,
                option_value: row.option_value.as_ref(),
            });
            let rust_probe = run_option_probe(OptionProbeRequest {
                side: backend.option_side(),
                target,
                root: &rust_root,
                request: &request,
                method: &row.method,
                fixture_source: &row.fixture_source,
                fixture_id: &row.fixture_id,
                option_name: &row.option_name,
                option_path: &row.option_path,
                input_kind: &row.input_kind,
                option_value: row.option_value.as_ref(),
            });
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
        let prepared_test_manifest = execution_result
            .as_ref()
            .and_then(|result| result.prepared_manifest_file.as_deref())
            .and_then(prepared_test_manifest_report);
        let official_test_origin = prepared_test_manifest
            .as_ref()
            .map(|manifest| manifest.official_test_origin.as_str())
            .unwrap_or("manifest-missing");
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
            "official_test_origin": official_test_origin,
            "prepared_test_manifest": prepared_test_manifest,
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

pub fn generate_output_contract(scope: &SelectionArgs, out_dir: &Path) -> JsonReport {
    let targets = select_targets(scope);
    let mut created = Vec::new();
    let mut items = Vec::new();
    for target in targets {
        let path = target.output_contract_path_in(out_dir);
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
