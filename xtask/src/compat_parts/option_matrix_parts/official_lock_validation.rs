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
    if scope.all {
        return all_targets().to_vec();
    }
    let has_filter =
        scope.version_line.is_some() || scope.package.is_some() || scope.entry.is_some();
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
    if targets.is_empty() && !has_filter {
        targets.extend_from_slice(all_targets());
    }
    targets
}
