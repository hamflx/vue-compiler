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
                if let Err(err) = sync_git_checkout(&baseline.repo, &baseline.rev, &dir, true) {
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

pub fn export_api(scope: &SelectionArgs, out_dir: &Path) -> JsonReport {
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
            let path = target.api_manifest_path_in(out_dir, side.as_str());
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
