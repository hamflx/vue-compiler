fn verify_napi_api() -> Result<compat::JsonReport> {
    let targets = [
        NapiApiTarget {
            version_line: "vue2_6",
            package: "vue-template-compiler",
            entry: "index",
            alias: NapiApiAlias::Vue2TemplateCompiler {
                template_variant: "vue2_6",
            },
        },
        NapiApiTarget {
            version_line: "vue2_7",
            package: "vue-template-compiler",
            entry: "index",
            alias: NapiApiAlias::Vue2TemplateCompiler {
                template_variant: "vue2_7",
            },
        },
        NapiApiTarget {
            version_line: "vue2_7",
            package: "vue/compiler-sfc",
            entry: "vue/compiler-sfc",
            alias: NapiApiAlias::PackageTemplate {
                source: "packages/native-aliases/vue",
                package_subpath: &["vue"],
                manifest_package: "vue",
                manifest_file: "vue_compiler-sfc.json",
                package_json_subpath: &["vue"],
                types_base_subpath: &["vue"],
            },
        },
        NapiApiTarget {
            version_line: "vue3",
            package: "@vue/compiler-core",
            entry: "index",
            alias: NapiApiAlias::PackageTemplate {
                source: "packages/native-aliases/@vue/compiler-core",
                package_subpath: &["@vue", "compiler-core"],
                manifest_package: "_vue_compiler-core",
                manifest_file: "index.json",
                package_json_subpath: &["@vue", "compiler-core"],
                types_base_subpath: &["@vue", "compiler-core"],
            },
        },
        NapiApiTarget {
            version_line: "vue3",
            package: "@vue/compiler-ssr",
            entry: "index",
            alias: NapiApiAlias::PackageTemplate {
                source: "packages/native-aliases/@vue/compiler-ssr",
                package_subpath: &["@vue", "compiler-ssr"],
                manifest_package: "_vue_compiler-ssr",
                manifest_file: "index.json",
                package_json_subpath: &["@vue", "compiler-ssr"],
                types_base_subpath: &["@vue", "compiler-ssr"],
            },
        },
        NapiApiTarget {
            version_line: "vue3",
            package: "@vue/compiler-sfc",
            entry: "index",
            alias: NapiApiAlias::PackageTemplate {
                source: "packages/native-aliases/@vue/compiler-sfc",
                package_subpath: &["@vue", "compiler-sfc"],
                manifest_package: "_vue_compiler-sfc",
                manifest_file: "index.json",
                package_json_subpath: &["@vue", "compiler-sfc"],
                types_base_subpath: &["@vue", "compiler-sfc"],
            },
        },
        NapiApiTarget {
            version_line: "vue3",
            package: "@vue/compiler-dom",
            entry: "index",
            alias: NapiApiAlias::PackageTemplate {
                source: "packages/native-aliases/@vue/compiler-dom",
                package_subpath: &["@vue", "compiler-dom"],
                manifest_package: "_vue_compiler-dom",
                manifest_file: "index.json",
                package_json_subpath: &["@vue", "compiler-dom"],
                types_base_subpath: &["@vue", "compiler-dom"],
            },
        },
    ];
    let mut violations = Vec::new();
    let mut created = Vec::new();
    let mut items = Vec::new();

    let build_failure = build_napi_crate()
        .err()
        .map(|err| format!("failed to build vuec_napi: {err:#}"));
    if let Some(err) = &build_failure {
        violations.push(err.clone());
    }

    for target in targets {
        let mut target_violations = Vec::new();
        let root = PathBuf::from("target")
            .join("napi-api")
            .join(target.version_line)
            .join(target.target_dir_name());
        let binding_path = root
            .join("node_modules")
            .join("@vuec-rs")
            .join("native")
            .join("vuec_napi.node");
        if build_failure.is_none() {
            match prepare_napi_api_tree(&root, target) {
                Ok(paths) => {
                    created.extend(paths.into_iter().map(|path| path.display().to_string()))
                }
                Err(err) => target_violations.push(format!(
                    "{} failed to prepare NAPI API tree: {err:#}",
                    target.display()
                )),
            }
        }
        if build_failure.is_none() && target_violations.is_empty() {
            match copy_napi_binding(&binding_path) {
                Ok(path) => created.push(path.display().to_string()),
                Err(err) => target_violations.push(format!(
                    "{} failed to install NAPI binding: {err:#}",
                    target.display()
                )),
            }
        }

        let detail = if build_failure.is_none() && target_violations.is_empty() {
            match run_napi_api_probe(&root, target) {
                Ok(detail) => detail,
                Err(err) => {
                    target_violations.push(format!(
                        "{} NAPI API diff failed: {err:#}",
                        target.display()
                    ));
                    "NAPI API diff did not pass".into()
                }
            }
        } else {
            "NAPI API diff did not run".into()
        };
        let status = if build_failure.is_none() && target_violations.is_empty() {
            compat::ReportStatus::Pass
        } else {
            compat::ReportStatus::Fail
        };
        violations.extend(target_violations);
        items.push(compat::ReportItem::new(
            target.display(),
            status,
            detail,
            Some(root),
        ));
    }

    Ok(compat::JsonReport::new(
        "verify_napi_api",
        if violations.is_empty() {
            compat::ReportStatus::Pass
        } else {
            compat::ReportStatus::Fail
        },
    )
    .with_items(items)
    .with_created(created)
    .with_violations(violations)
    .with_note("compares official API manifests against NAPI-backed official package-name aliases"))
}

#[derive(Clone, Copy)]
struct NapiApiTarget {
    version_line: &'static str,
    package: &'static str,
    entry: &'static str,
    alias: NapiApiAlias,
}

#[derive(Clone, Copy)]
enum NapiApiAlias {
    Vue2TemplateCompiler {
        template_variant: &'static str,
    },
    PackageTemplate {
        source: &'static str,
        package_subpath: &'static [&'static str],
        manifest_package: &'static str,
        manifest_file: &'static str,
        package_json_subpath: &'static [&'static str],
        types_base_subpath: &'static [&'static str],
    },
}

impl NapiApiTarget {
    fn display(self) -> String {
        format!("{}::{}/{}", self.version_line, self.package, self.entry)
    }

    fn target_dir_name(self) -> &'static str {
        match self.alias {
            NapiApiAlias::Vue2TemplateCompiler { .. } => "vue-template-compiler",
            NapiApiAlias::PackageTemplate {
                manifest_package, ..
            } => manifest_package,
        }
    }

    fn source_path(self) -> PathBuf {
        match self.alias {
            NapiApiAlias::Vue2TemplateCompiler { .. } => {
                PathBuf::from("packages/native-aliases/vue-template-compiler")
            }
            NapiApiAlias::PackageTemplate { source, .. } => PathBuf::from(source),
        }
    }

    fn package_subpath(self) -> &'static [&'static str] {
        match self.alias {
            NapiApiAlias::Vue2TemplateCompiler { .. } => &["vue-template-compiler"],
            NapiApiAlias::PackageTemplate {
                package_subpath, ..
            } => package_subpath,
        }
    }

    fn package_json_subpath(self) -> &'static [&'static str] {
        match self.alias {
            NapiApiAlias::Vue2TemplateCompiler { .. } => &["vue-template-compiler"],
            NapiApiAlias::PackageTemplate {
                package_json_subpath,
                ..
            } => package_json_subpath,
        }
    }

    fn types_base_subpath(self) -> &'static [&'static str] {
        match self.alias {
            NapiApiAlias::Vue2TemplateCompiler { .. } => &["vue-template-compiler"],
            NapiApiAlias::PackageTemplate {
                types_base_subpath, ..
            } => types_base_subpath,
        }
    }

    fn official_manifest_path(self) -> PathBuf {
        let (manifest_package, manifest_file) = match self.alias {
            NapiApiAlias::Vue2TemplateCompiler { .. } => ("vue-template-compiler", "index.json"),
            NapiApiAlias::PackageTemplate {
                manifest_package, ..
            } => (manifest_package, self.manifest_file_name()),
        };
        PathBuf::from("compat")
            .join("api")
            .join("official")
            .join(self.version_line)
            .join(manifest_package)
            .join(manifest_file)
    }

    fn manifest_file_name(self) -> &'static str {
        match self.alias {
            NapiApiAlias::Vue2TemplateCompiler { .. } => "index.json",
            NapiApiAlias::PackageTemplate { manifest_file, .. } => manifest_file,
        }
    }
}

fn verify_napi_platform() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut created = Vec::new();
    let platform_root = PathBuf::from("target").join("napi-platform");
    let node_modules = platform_root.join("node_modules");
    let native_package_dir = node_modules.join("@vuec-rs").join("native");
    let platform_package = current_platform_package_name();
    let platform_package_dir =
        platform_package_path(&node_modules, platform_package.unwrap_or("unsupported"));

    match build_napi_crate() {
        Ok(()) => {}
        Err(err) => violations.push(format!("failed to build vuec_napi: {err:#}")),
    }

    if platform_package.is_none() {
        violations.push(format!(
            "unsupported NAPI platform package for os={} arch={}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
    }

    if violations.is_empty() {
        match prepare_napi_platform_tree(&platform_root, platform_package.unwrap()) {
            Ok(paths) => created.extend(paths.into_iter().map(|path| path.display().to_string())),
            Err(err) => {
                violations.push(format!("failed to prepare NAPI platform package: {err:#}"))
            }
        }
    }

    if violations.is_empty() {
        match copy_napi_binding(&platform_package_dir.join("vuec_napi.node")) {
            Ok(path) => created.push(path.display().to_string()),
            Err(err) => {
                violations.push(format!("failed to install platform NAPI binding: {err:#}"))
            }
        }
    }

    let smoke_output = if violations.is_empty() {
        match run_napi_platform_smoke(&platform_root) {
            Ok(output) => Some(output),
            Err(err) => {
                violations.push(format!("NAPI platform package smoke failed: {err:#}"));
                None
            }
        }
    } else {
        None
    };

    let item_status = if violations.is_empty() {
        compat::ReportStatus::Pass
    } else {
        compat::ReportStatus::Fail
    };
    Ok(
        compat::JsonReport::new("verify_napi_platform", item_status)
            .with_items(vec![compat::ReportItem::new(
                platform_package.unwrap_or("unsupported-platform"),
                item_status,
                smoke_output.unwrap_or_else(|| "NAPI platform package smoke did not run".into()),
                Some(native_package_dir),
            )])
            .with_created(created)
            .with_violations(violations)
            .with_note("builds vuec_napi, installs the current optional platform package under target/napi-platform, and verifies @vuec-rs/native loads from that package instead of a local .node"),
    )
}

fn verify_napi_alias() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut created = Vec::new();
    let alias_root = PathBuf::from("target").join("napi-alias");
    let node_modules = alias_root.join("node_modules");
    let native_package_dir = node_modules.join("@vuec-rs").join("native");
    let binding_path = native_package_dir.join("vuec_napi.node");

    match build_napi_crate() {
        Ok(()) => {}
        Err(err) => violations.push(format!("failed to build vuec_napi: {err:#}")),
    }

    if violations.is_empty() {
        match prepare_napi_alias_tree(&alias_root) {
            Ok(paths) => created.extend(paths.into_iter().map(|path| path.display().to_string())),
            Err(err) => violations.push(format!("failed to prepare NAPI alias packages: {err:#}")),
        }
    }

    if violations.is_empty() {
        match copy_napi_binding(&binding_path) {
            Ok(path) => created.push(path.display().to_string()),
            Err(err) => violations.push(format!("failed to install NAPI binding: {err:#}")),
        }
    }

    let smoke_output = if violations.is_empty() {
        match run_napi_alias_smoke(&alias_root) {
            Ok(output) => Some(output),
            Err(err) => {
                violations.push(format!("NAPI alias smoke failed: {err:#}"));
                None
            }
        }
    } else {
        None
    };

    let item_status = if violations.is_empty() {
        compat::ReportStatus::Pass
    } else {
        compat::ReportStatus::Fail
    };
    Ok(
        compat::JsonReport::new("verify_napi_alias", item_status)
            .with_items(vec![compat::ReportItem::new(
                "official-package-name-napi-alias",
                item_status,
                smoke_output.unwrap_or_else(|| "NAPI alias smoke did not run".into()),
                Some(alias_root),
            )])
            .with_created(created)
            .with_violations(violations)
            .with_note("builds vuec_napi, installs @vuec-rs/native plus official package-name alias templates under target/napi-alias, and requires them from Node"),
    )
}

fn verify_napi() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut created = Vec::new();
    let package_dir = PathBuf::from("packages/native");
    let binding_path = package_dir.join("vuec_napi.node");

    match build_napi_crate() {
        Ok(()) => {}
        Err(err) => violations.push(format!("failed to build vuec_napi: {err:#}")),
    }

    if violations.is_empty() {
        match copy_napi_binding(&binding_path) {
            Ok(path) => created.push(path.display().to_string()),
            Err(err) => violations.push(format!("failed to install NAPI binding: {err:#}")),
        }
    }

    let smoke_output = if violations.is_empty() {
        match run_native_smoke(&package_dir) {
            Ok(output) => Some(output),
            Err(err) => {
                violations.push(format!("native smoke failed: {err:#}"));
                None
            }
        }
    } else {
        None
    };

    let item_status = if violations.is_empty() {
        compat::ReportStatus::Pass
    } else {
        compat::ReportStatus::Fail
    };
    let item_detail = smoke_output.unwrap_or_else(|| "NAPI smoke did not run".into());
    Ok(
        compat::JsonReport::new("verify_napi", item_status)
            .with_items(vec![compat::ReportItem::new(
                "@vuec-rs/native",
                item_status,
                item_detail,
                Some(binding_path),
            )])
            .with_created(created)
            .with_violations(violations)
            .with_note("builds vuec_napi, installs packages/native/vuec_napi.node, and runs the Node loader smoke"),
    )
}
