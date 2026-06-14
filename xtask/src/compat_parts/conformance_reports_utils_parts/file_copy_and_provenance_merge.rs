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

fn remove_vitest_provenance_sidecars(prepared_root: &Path) -> Result<()> {
    if !prepared_root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(prepared_root)
        .with_context(|| format!("failed to read {}", prepared_root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.starts_with("vuec-provenance.") && name.ends_with(".ndjson") {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

fn merge_vitest_provenance_sidecars(
    output_file: &Path,
    absolute_output_file: &Path,
    prepared_root: &Path,
) -> Result<()> {
    let report_path = if output_file.exists() {
        output_file
    } else {
        absolute_output_file
    };
    if !report_path.exists() || !prepared_root.exists() {
        return Ok(());
    }

    let sidecars = vitest_provenance_sidecars(prepared_root)?;
    if sidecars.is_empty() {
        return Ok(());
    }

    let mut report = read_json::<serde_json::Value>(report_path)?;
    for sidecar in sidecars {
        let data = fs::read_to_string(&sidecar)
            .with_context(|| format!("failed to read {}", sidecar.display()))?;
        for (index, line) in data.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let record: serde_json::Value = serde_json::from_str(line).with_context(|| {
                format!(
                    "failed to parse provenance sidecar {} line {}",
                    sidecar.display(),
                    index + 1
                )
            })?;
            merge_vitest_provenance_record(&mut report, &record);
        }
    }

    write_json(report_path, &report)?;
    Ok(())
}

fn vitest_provenance_sidecars(prepared_root: &Path) -> Result<Vec<PathBuf>> {
    let mut sidecars = Vec::new();
    for entry in fs::read_dir(prepared_root)
        .with_context(|| format!("failed to read {}", prepared_root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.starts_with("vuec-provenance.") && name.ends_with(".ndjson") {
            sidecars.push(path);
        }
    }
    sidecars.sort();
    Ok(sidecars)
}

fn merge_vitest_provenance_record(report: &mut serde_json::Value, record: &serde_json::Value) {
    let markers = conformance_record_markers(record);
    if markers.is_empty() {
        return;
    }
    let test_path = normalize_conformance_path(
        record
            .get("testPath")
            .and_then(|value| value.as_str())
            .unwrap_or_default(),
    );
    let full_name = record
        .get("fullName")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let title = record
        .get("title")
        .and_then(|value| value.as_str())
        .unwrap_or_default();

    let Some(results) = report
        .get_mut("testResults")
        .and_then(|value| value.as_array_mut())
    else {
        return;
    };

    let file_index = results
        .iter()
        .position(|result| vitest_result_matches_record_path(result, &test_path));
    let Some(index) = file_index else {
        return;
    };
    let result = &mut results[index];
    if merge_provenance_markers_into_matching_assertion(result, full_name, title, &markers) {
        return;
    }
    merge_provenance_markers(result, &markers);
}

fn conformance_record_markers(record: &serde_json::Value) -> Vec<String> {
    let mut markers = Vec::new();
    collect_conformance_runtime_markers(record.get("markers"), &mut markers);
    markers
}

fn normalize_conformance_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn vitest_result_matches_record_path(result: &serde_json::Value, test_path: &str) -> bool {
    if test_path.is_empty() {
        return false;
    }
    let result_path = normalize_conformance_path(
        result
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or_default(),
    );
    if result_path.is_empty() {
        return false;
    }
    result_path == test_path
        || result_path.ends_with(test_path)
        || test_path.ends_with(&result_path)
}

fn merge_provenance_markers_into_matching_assertion(
    result: &mut serde_json::Value,
    full_name: &str,
    title: &str,
    markers: &[String],
) -> bool {
    let Some(assertions) = result
        .get_mut("assertionResults")
        .and_then(|value| value.as_array_mut())
    else {
        return false;
    };
    let full_name = normalize_conformance_test_name(full_name);
    let title = normalize_conformance_test_name(title);
    let assertion_index = assertions.iter().position(|assertion| {
        let assertion_full_name =
            normalize_conformance_test_name(&vitest_assertion_full_name(assertion));
        let assertion_title = normalize_conformance_test_name(
            assertion
                .get("title")
                .and_then(|value| value.as_str())
                .unwrap_or_default(),
        );
        (!full_name.is_empty() && assertion_full_name == full_name)
            || (!title.is_empty() && assertion_title == title)
            || (!full_name.is_empty() && assertion_title == full_name)
    });
    let Some(index) = assertion_index else {
        return false;
    };
    merge_provenance_markers(&mut assertions[index], markers);
    true
}

fn vitest_assertion_full_name(assertion: &serde_json::Value) -> String {
    if let Some(full_name) = assertion.get("fullName").and_then(|value| value.as_str()) {
        return full_name.to_string();
    }
    let mut parts = Vec::new();
    if let Some(ancestors) = assertion
        .get("ancestorTitles")
        .and_then(|value| value.as_array())
    {
        for ancestor in ancestors {
            if let Some(value) = ancestor.as_str() {
                parts.push(value);
            }
        }
    }
    if let Some(title) = assertion.get("title").and_then(|value| value.as_str()) {
        parts.push(title);
    }
    parts.join(" ")
}

fn normalize_conformance_test_name(name: &str) -> String {
    name.replace(" > ", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn merge_provenance_markers(target: &mut serde_json::Value, markers: &[String]) {
    if !target.is_object() {
        return;
    }
    if target.get("coverageProvenance").is_none() {
        target["coverageProvenance"] = serde_json::Value::Array(Vec::new());
    }
    let Some(values) = target
        .get_mut("coverageProvenance")
        .and_then(|value| value.as_array_mut())
    else {
        return;
    };
    for marker in markers {
        if !values.iter().any(|value| value.as_str() == Some(marker)) {
            values.push(serde_json::Value::String(marker.clone()));
        }
    }
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
