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

    let evidence_groups = vec![
        ReportEvidenceGroup::new("official-conformance-gate", items.clone()).with_note(if locked {
            "official/API/option/output/lock gate; this group determines summarize_compat top-level status"
        } else {
            "official/API/option/output gate; this group determines summarize_compat top-level status"
        }),
        ReportEvidenceGroup::new(
            "production-corpus-evidence",
            production_corpus_evidence_items(root),
        )
        .with_note(
            "external project corpus evidence; reported separately and never overwrites official conformance target status",
        ),
    ];

    let mut report = JsonReport::new("summarize_compat", aggregate_status(&items));
    report.metadata = metadata;
    report
        .with_items(items)
        .with_evidence_groups(evidence_groups)
        .with_violations(violations)
        .with_note(if locked {
            "summary status is the official conformance gate: lock validation plus API, option, output, and official conformance artifacts; production corpus evidence is reported in evidence_groups separately"
        } else {
            "summary status is the official conformance gate: API, option, output, and official conformance artifacts; production corpus evidence is reported in evidence_groups separately"
        })
}

fn production_corpus_evidence_items(root: &Path) -> Vec<ReportItem> {
    [
        (
            "production-corpus::vue2_6-projects",
            root.join("target")
                .join("external")
                .join("vue2-project-corpus")
                .join("verify_vue2_project_corpus.json"),
        ),
        (
            "production-corpus::vue2_7-projects",
            root.join("target")
                .join("external")
                .join("vue27-project-corpus")
                .join("verify_vue27_project_corpus.json"),
        ),
    ]
    .into_iter()
    .map(|(target, path)| production_corpus_evidence_item(target, path))
    .collect()
}

fn production_corpus_evidence_item(target: &str, path: PathBuf) -> ReportItem {
    let Ok(data) = fs::read_to_string(&path) else {
        return ReportItem::new(
            target,
            ReportStatus::Pending,
            "production corpus evidence report is missing; this is separate from official conformance",
            Some(path),
        );
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&data) else {
        return ReportItem::new(
            target,
            ReportStatus::Fail,
            "production corpus evidence report is invalid JSON; this is separate from official conformance",
            Some(path),
        );
    };

    let status = report_value_status(&value);
    let project_vue_version = value
        .get("project_vue_version")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let compiler_version_line = value
        .get("compiler_version_line")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let counts = value.get("counts").and_then(|value| value.as_object());
    let total = counts
        .and_then(|counts| counts.get("total"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let pass = counts
        .and_then(|counts| counts.get("pass"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let pending = counts
        .and_then(|counts| counts.get("pending"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let fail = counts
        .and_then(|counts| counts.get("fail"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0);

    ReportItem::new(
        target,
        status,
        format!(
            "production corpus evidence, not official conformance; project_vue_version={}, compiler_version_line={}, projects pass={}, pending={}, fail={}, total={}",
            project_vue_version, compiler_version_line, pass, pending, fail, total
        ),
        Some(path),
    )
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
    if statuses.contains(&ReportStatus::Fail) {
        ReportStatus::Fail
    } else if statuses.contains(&ReportStatus::Pending) {
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
