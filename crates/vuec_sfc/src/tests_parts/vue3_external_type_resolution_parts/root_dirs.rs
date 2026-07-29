#[test]
fn vue3_root_dirs_use_longest_prefix_then_configured_order() {
    let dir = tempfile::tempdir().expect("temp dir");
    let src = dir.path().join("src");
    let nested = src.join("nested");
    let generated = dir.path().join("generated");
    let alternate = dir.path().join("alternate");
    for directory in [&nested, &generated, &alternate] {
        std::fs::create_dir_all(directory).expect("create rootDirs fixture directory");
    }
    std::fs::write(
        dir.path().join("tsconfig.json"),
        r#"{
            "compilerOptions": {
                "rootDirs": ["generated", "src", "src/nested", "alternate"]
            }
        }"#,
    )
    .expect("write rootDirs config");
    let importer = nested.join("Comp.vue");
    let generated_target = generated.join("target.ts");
    let shallow_decoy = src.join("target.ts");
    let alternate_decoy = alternate.join("target.ts");
    let native_target = nested.join("native.ts");
    let generated_native_decoy = generated.join("native.ts");
    std::fs::write(&generated_target, "export interface GeneratedProps {}")
        .expect("write ordered root target");
    std::fs::write(&shallow_decoy, "export interface ShallowProps {}")
        .expect("write shallow root decoy");
    std::fs::write(&alternate_decoy, "export interface AlternateProps {}")
        .expect("write alternate root decoy");
    std::fs::write(&native_target, "export interface NativeProps {}")
        .expect("write native root target");
    std::fs::write(
        &generated_native_decoy,
        "export interface GeneratedNativeProps {}",
    )
    .expect("write generated native decoy");

    let filename = importer.to_string_lossy();
    let resolver = vue3_type_resolver_context_for_filename(&filename);
    assert_eq!(
        resolver.root_dirs.as_ref(),
        [
            generated.clone(),
            src.clone(),
            nested.clone(),
            alternate.clone(),
        ]
    );
    assert_eq!(
        resolve_vue3_type_import(&filename, "./target", &resolver),
        Some(generated_target.clone()),
    );
    assert_eq!(
        resolve_vue3_type_import(&filename, "./native", &resolver),
        Some(native_target.clone()),
    );
    assert_eq!(
        resolve_vue3_type_import(&filename, "./target", &resolver),
        Some(generated_target),
    );
    assert_eq!(
        resolve_vue3_type_import(&filename, "./native", &resolver),
        Some(native_target),
    );
    assert_eq!(
        resolver
            .external_type_session
            .stats()
            .resolution_cache_hits,
        2,
    );
    assert!(!resolver.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_root_dirs_inherit_origin_templates_and_clear_overrides() {
    let dir = tempfile::tempdir().expect("temp dir");
    for (name, override_source, should_resolve) in [
        ("inherited", "", true),
        ("null", r#", "compilerOptions": { "rootDirs": null }"#, false),
        ("empty", r#", "compilerOptions": { "rootDirs": [] }"#, false),
    ] {
        let fixture = dir.path().join(name);
        let configs = fixture.join("configs");
        let project = fixture.join("project");
        let src = project.join("src");
        let generated = fixture.join("generated");
        for directory in [&configs, &src, &generated] {
            std::fs::create_dir_all(directory).expect("create inherited rootDirs directory");
        }
        std::fs::write(
            configs.join("base.json"),
            r#"{"compilerOptions":{"rootDirs":["../project/src","../generated"]}}"#,
        )
        .expect("write inherited rootDirs base config");
        std::fs::write(
            project.join("tsconfig.json"),
            format!(r#"{{"extends":"../configs/base.json"{override_source}}}"#),
        )
        .expect("write inherited rootDirs project config");
        let target = generated.join("target.ts");
        std::fs::write(&target, "export interface InheritedProps {}")
            .expect("write inherited rootDirs target");
        let filename = src.join("Comp.vue").to_string_lossy().to_string();
        let resolver = vue3_type_resolver_context_for_filename(&filename);

        assert_eq!(
            resolve_vue3_type_import(&filename, "./target", &resolver),
            should_resolve.then_some(target),
            "{name}",
        );
        assert_eq!(resolver.root_dirs.is_empty(), !should_resolve, "{name}");
        assert!(!resolver.external_type_session.metadata_is_blocked(), "{name}");
    }

    let template = dir.path().join("template");
    let configs = template.join("configs");
    let project = template.join("project");
    let src = project.join("src");
    let generated = project.join("generated");
    for directory in [&configs, &src, &generated] {
        std::fs::create_dir_all(directory).expect("create templated rootDirs directory");
    }
    std::fs::write(
        configs.join("base.json"),
        r#"{
            "compilerOptions": {
                "rootDirs": ["${configDir}/src", "${configDir}/generated"]
            }
        }"#,
    )
    .expect("write templated rootDirs base config");
    std::fs::write(
        project.join("tsconfig.json"),
        r#"{"extends":"../configs/base.json"}"#,
    )
    .expect("write templated rootDirs project config");
    let target = generated.join("target.ts");
    std::fs::write(&target, "export interface TemplatedProps {}")
        .expect("write templated rootDirs target");
    let filename = src.join("Comp.vue").to_string_lossy().to_string();
    let resolver = vue3_type_resolver_context_for_filename(&filename);

    assert_eq!(resolver.root_dirs.as_ref(), [src, generated]);
    assert_eq!(
        resolve_vue3_type_import(&filename, "./target", &resolver),
        Some(target),
    );
    assert!(!resolver.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_root_dirs_config_is_validated_and_bounded() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("tsconfig.json"),
        r#"{"compilerOptions":{"rootDirs":["src","generated","alternate"]}}"#,
    )
    .expect("write bounded rootDirs config");
    let filename = dir.path().join("src").join("Comp.vue");
    let exact = Vue3TypeResolverContext {
        external_type_session: Vue3ExternalTypeLoadSession::with_limits(
            Vue3ExternalTypeLoadLimits {
                max_metadata_fanout_entries: 3,
                ..Vue3ExternalTypeLoadLimits::default()
            },
        ),
        ..Vue3TypeResolverContext::default()
    };
    let options = vue3_tsconfig_type_resolver_options(&filename.to_string_lossy(), &exact)
        .expect("parse rootDirs at exact fanout limit");
    assert_eq!(options.root_dirs.len(), 3);
    assert_eq!(exact.external_type_session.stats().metadata_fanout_entries, 3);
    assert!(!exact.external_type_session.metadata_is_blocked());

    let short = Vue3TypeResolverContext {
        external_type_session: Vue3ExternalTypeLoadSession::with_limits(
            Vue3ExternalTypeLoadLimits {
                max_metadata_fanout_entries: 2,
                ..Vue3ExternalTypeLoadLimits::default()
            },
        ),
        ..Vue3TypeResolverContext::default()
    };
    assert!(vue3_tsconfig_type_resolver_options(&filename.to_string_lossy(), &short).is_none());
    assert_eq!(short.external_type_session.stats().metadata_fanout_entries, 2);
    assert!(short.external_type_session.metadata_is_blocked());

    std::fs::write(
        dir.path().join("tsconfig.json"),
        r#"{"compilerOptions":{"rootDirs":["src",1]}}"#,
    )
    .expect("write invalid rootDirs config");
    let invalid = Vue3TypeResolverContext::default();
    assert!(vue3_tsconfig_type_resolver_options(&filename.to_string_lossy(), &invalid).is_none());
    assert!(invalid.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_root_dirs_resolution_fanout_is_bounded() {
    let dir = tempfile::tempdir().expect("temp dir");
    let src = dir.path().join("src");
    let missing = dir.path().join("missing");
    let generated = dir.path().join("generated");
    std::fs::create_dir_all(&src).expect("create rootDirs source directory");
    std::fs::create_dir_all(&generated).expect("create rootDirs generated directory");
    let target = generated.join("target.ts");
    std::fs::write(&target, "export interface BoundedProps {}")
        .expect("write bounded rootDirs target");
    let filename = src.join("Comp.vue").to_string_lossy().to_string();
    let root_dirs: std::sync::Arc<[PathBuf]> =
        std::sync::Arc::from([src, missing, generated]);

    let exact = Vue3TypeResolverContext {
        root_dirs: root_dirs.clone(),
        external_type_session: Vue3ExternalTypeLoadSession::with_limits(
            Vue3ExternalTypeLoadLimits {
                max_metadata_fanout_entries: 3,
                ..Vue3ExternalTypeLoadLimits::default()
            },
        ),
        ..Vue3TypeResolverContext::default()
    };
    assert_eq!(
        resolve_vue3_type_import(&filename, "./target", &exact),
        Some(target),
    );
    assert_eq!(exact.external_type_session.stats().metadata_fanout_entries, 3);
    assert!(!exact.external_type_session.metadata_is_blocked());

    let short = Vue3TypeResolverContext {
        root_dirs,
        external_type_session: Vue3ExternalTypeLoadSession::with_limits(
            Vue3ExternalTypeLoadLimits {
                max_metadata_fanout_entries: 2,
                ..Vue3ExternalTypeLoadLimits::default()
            },
        ),
        ..Vue3TypeResolverContext::default()
    };
    assert!(resolve_vue3_type_import(&filename, "./target", &short).is_none());
    assert_eq!(short.external_type_session.stats().metadata_fanout_entries, 2);
    assert!(short.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_root_dirs_isolate_resolution_and_context_caches() {
    let dir = tempfile::tempdir().expect("temp dir");
    let src = dir.path().join("src");
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    for directory in [&src, &first, &second] {
        std::fs::create_dir_all(directory).expect("create cache rootDirs directory");
    }
    let root = src.join("root.ts");
    let first_target = first.join("target.ts");
    let second_target = second.join("target.ts");
    std::fs::write(&root, "export { Props } from './target'")
        .expect("write cache root module");
    std::fs::write(
        &first_target,
        "export interface Props { firstValue: string }",
    )
    .expect("write first rootDirs target");
    std::fs::write(
        &second_target,
        "export interface Props { secondValue: number }",
    )
    .expect("write second rootDirs target");
    let session = Vue3ExternalTypeLoadSession::default();
    let first_resolver = Vue3TypeResolverContext {
        root_dirs: std::sync::Arc::from([src.clone(), first]),
        external_type_session: session.clone(),
        ..Vue3TypeResolverContext::default()
    };
    let second_resolver = Vue3TypeResolverContext {
        root_dirs: std::sync::Arc::from([src, second]),
        external_type_session: session,
        ..Vue3TypeResolverContext::default()
    };
    assert_ne!(first_resolver, second_resolver);
    let filename = root.to_string_lossy();

    for _ in 0..2 {
        assert_eq!(
            resolve_vue3_type_import(&filename, "./target", &first_resolver),
            Some(first_target.clone()),
        );
        assert_eq!(
            resolve_vue3_type_import(&filename, "./target", &second_resolver),
            Some(second_target.clone()),
        );
    }
    assert_eq!(
        first_resolver
            .external_type_session
            .stats()
            .resolution_cache_hits,
        2,
    );

    let first_context = vue3_external_type_context_from_path(
        &root,
        &mut BTreeSet::new(),
        &first_resolver,
    )
    .expect("load first rootDirs context");
    let second_context = vue3_external_type_context_from_path(
        &root,
        &mut BTreeSet::new(),
        &second_resolver,
    )
    .expect("load second rootDirs context");
    assert_eq!(
        first_context.type_sources.get("Props"),
        Some(&normalize_path_string(&first_target)),
    );
    assert_eq!(
        second_context.type_sources.get("Props"),
        Some(&normalize_path_string(&second_target)),
    );
}

#[test]
fn vue3_compile_script_resolves_root_dirs_types_and_dependencies() {
    let dir = tempfile::tempdir().expect("temp dir");
    let components = dir.path().join("src").join("components");
    let generated = dir.path().join("generated");
    std::fs::create_dir_all(&components).expect("create rootDirs component directory");
    std::fs::create_dir_all(&generated).expect("create generated types directory");
    std::fs::write(
        dir.path().join("tsconfig.json"),
        r#"{"compilerOptions":{"rootDirs":["src","generated"]}}"#,
    )
    .expect("write rootDirs integration config");
    let types = generated.join("types.ts");
    let leaf = generated.join("leaf.ts");
    std::fs::write(
        &types,
        "import type { LeafProps } from './leaf'; export interface RootProps extends LeafProps { count?: number }",
    )
    .expect("write rootDirs integration types");
    std::fs::write(&leaf, "export interface LeafProps { label: string }")
        .expect("write rootDirs integration leaf");
    let filename = components.join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type { RootProps } from '../types'
defineProps<RootProps>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(
        script
            .content
            .contains("label: { type: String, required: true }")
    );
    assert!(
        script
            .content
            .contains("count: { type: Number, required: false }")
    );
    assert_eq!(
        script.deps.iter().cloned().collect::<BTreeSet<_>>(),
        [types, leaf]
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect(),
    );
}
