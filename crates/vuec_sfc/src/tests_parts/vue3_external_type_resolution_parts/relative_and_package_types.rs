    fn write_vue3_bundler_config(root: &Path) {
        std::fs::write(
            root.join("tsconfig.json"),
            r#"{"compilerOptions":{"module":"ESNext","moduleResolution":"Bundler"}}"#,
        )
        .expect("write Bundler config");
    }

    fn write_vue3_typescript_version(root: &Path, version: &str) {
        let package = root.join("node_modules").join("typescript");
        std::fs::create_dir_all(&package).expect("create TypeScript package directory");
        std::fs::write(
            package.join("package.json"),
            serde_json::json!({ "version": version }).to_string(),
        )
        .expect("write TypeScript package manifest");
    }

    fn write_vue3_node_next_config(root: &Path) {
        std::fs::write(
            root.join("tsconfig.json"),
            r#"{"compilerOptions":{"module":"NodeNext","moduleResolution":"NodeNext"}}"#,
        )
        .expect("write NodeNext config");
    }

    const VUE3_EXACT_PACKAGE_MAP_TARGET_CASES: [(&str, &str, Option<&str>); 6] = [
        ("explicit-ts", "explicit-ts.js", Some("explicit-ts.ts")),
        (
            "explicit-dts",
            "explicit-dts.js",
            Some("explicit-dts.d.ts"),
        ),
        ("extensionless-ts", "extensionless-ts", None),
        ("extensionless-dts", "extensionless-dts", None),
        ("directory-manifest", "directory-manifest", None),
        ("directory-index", "directory-index", None),
    ];

    fn vue3_exact_package_map(key_prefix: &str, target_dir: &str) -> serde_json::Value {
        serde_json::Value::Object(
            VUE3_EXACT_PACKAGE_MAP_TARGET_CASES
                .iter()
                .map(|(key, target, _)| {
                    (
                        format!("{key_prefix}{key}"),
                        serde_json::Value::String(format!("./{target_dir}/{target}")),
                    )
                })
                .collect(),
        )
    }

    fn write_vue3_exact_package_map_targets(root: &Path) {
        std::fs::create_dir_all(root).expect("create package-map target directory");
        for name in [
            "explicit-ts.ts",
            "explicit-dts.d.ts",
            "extensionless-ts.ts",
            "extensionless-dts.d.ts",
        ] {
            std::fs::write(root.join(name), "export {};").expect("write package-map target");
        }

        let manifest_directory = root.join("directory-manifest");
        std::fs::create_dir_all(&manifest_directory).expect("create manifest target directory");
        std::fs::write(
            manifest_directory.join("package.json"),
            r#"{"types":"./entry.d.ts"}"#,
        )
        .expect("write nested target manifest");
        std::fs::write(manifest_directory.join("entry.d.ts"), "export {};")
            .expect("write nested manifest decoy");

        let index_directory = root.join("directory-index");
        std::fs::create_dir_all(&index_directory).expect("create index target directory");
        for name in ["index.ts", "index.d.ts"] {
            std::fs::write(index_directory.join(name), "export {};")
                .expect("write directory index decoy");
        }
    }

    fn assert_vue3_exact_package_map_targets(
        importer: &Path,
        source_prefix: &str,
        target_root: &Path,
    ) {
        for module_resolution in [
            Vue3TypeModuleResolutionKind::Node16,
            Vue3TypeModuleResolutionKind::NodeNext,
            Vue3TypeModuleResolutionKind::Bundler,
        ] {
            for resolution_mode in [
                Vue3TypeResolutionMode::Import,
                Vue3TypeResolutionMode::Require,
            ] {
                let resolver = Vue3TypeResolverContext {
                    typescript_version: (6, 0, 3).into(),
                    module_resolution,
                    ..Vue3TypeResolverContext::default()
                };
                for (case, _, expected) in VUE3_EXACT_PACKAGE_MAP_TARGET_CASES {
                    let expected = expected.map(|expected| target_root.join(expected));
                    assert_eq!(
                        resolve_vue3_type_import_with_mode(
                            &importer.to_string_lossy(),
                            &format!("{source_prefix}{case}"),
                            resolution_mode,
                            &resolver,
                        ),
                        expected,
                        "{module_resolution:?} {resolution_mode:?} package-map target {source_prefix}{case}",
                    );
                }
            }
        }
    }

    #[test]
    fn vue3_compile_script_resolves_relative_imported_macro_types_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("foo.ts"),
            "export interface Props { foo: string }",
        )
        .expect("write foo type");
        std::fs::create_dir_all(dir.path().join("bar")).expect("create bar dir");
        std::fs::write(
            dir.path().join("bar").join("index.tsx"),
            "export type ExtraProps = { count?: number }",
        )
        .expect("write bar type");
        std::fs::write(
            dir.path().join("events.d.ts"),
            "type E = { (e: 'save'): void }; export { E as Emits }",
        )
        .expect("write emits type");
        std::fs::write(
            dir.path().join("model.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write model type");
        std::fs::write(
            dir.path().join("unused.ts"),
            "export type Unused = { nope: string }",
        )
        .expect("write unused type");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import { Props } from './foo'
import { ExtraProps } from './bar'
import type { Emits } from './events'
import type { ModelValue } from './model'
import type { Unused } from './unused'
const props = defineProps<Props & ExtraProps>()
const emit = defineEmits<Emits>()
const model = defineModel<ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: String, required: true }"));
        assert!(script
            .content
            .contains("count: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            normalize_path_string(&dir.path().join("foo.ts")),
            normalize_path_string(&dir.path().join("bar").join("index.tsx")),
            normalize_path_string(&dir.path().join("events.d.ts")),
            normalize_path_string(&dir.path().join("model.ts")),
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script
            .deps
            .iter()
            .any(|dep| dep.contains("unused") || dep.contains('\\')));
    }

    #[test]
    fn vue3_nodenext_inline_commonjs_imports_allow_extensionless_paths() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "module": "NodeNext",
                    "moduleResolution": "NodeNext"
                }
            }"#,
        )
        .expect("write NodeNext config");
        std::fs::write(
            dir.path().join("extensionless.ts"),
            "export interface ExtensionlessProps { extensionlessFile: string }",
        )
        .expect("write extensionless decoy");
        let directory = dir.path().join("directory");
        std::fs::create_dir_all(&directory).expect("create directory decoy");
        std::fs::write(
            directory.join("index.ts"),
            "export interface DirectoryProps { directoryIndex: number }",
        )
        .expect("write directory decoy");
        let explicit = dir.path().join("explicit.ts");
        std::fs::write(
            &explicit,
            "export interface ExplicitProps { explicitReplacement: boolean }",
        )
        .expect("write explicit replacement target");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { ExtensionlessProps } from './extensionless'
import type { DirectoryProps } from './directory'
import type { ExplicitProps } from './explicit.js'
defineProps<ExtensionlessProps & DirectoryProps & ExplicitProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("extensionlessFile"));
        assert!(script.content.contains("directoryIndex"));
        assert!(script.content.contains("explicitReplacement"));
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [
                normalize_path_string(&dir.path().join("extensionless.ts")),
                normalize_path_string(&directory.join("index.ts")),
                normalize_path_string(&explicit),
            ]
            .into_iter()
            .collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn vue3_classic_imports_resolve_ancestor_files_before_type_packages() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        let component_dir = source_dir.join("components");
        std::fs::create_dir_all(&component_dir).expect("create component directory");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{"compilerOptions":{"moduleResolution":"Classic"}}"#,
        )
        .expect("write Classic config");
        let ancestor = source_dir.join("shared.ts");
        std::fs::write(
            &ancestor,
            "export interface SharedProps { classicAncestor: string }",
        )
        .expect("write Classic ancestor type");

        let package = dir.path().join("node_modules").join("shared");
        std::fs::create_dir_all(&package).expect("create package decoy");
        std::fs::write(package.join("package.json"), r#"{"types":"index.d.ts"}"#)
            .expect("write package decoy manifest");
        std::fs::write(
            package.join("index.d.ts"),
            "export interface SharedProps { wrongPackage: never }",
        )
        .expect("write package decoy type");

        let filename = component_dir.join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { SharedProps } from 'shared'
defineProps<SharedProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("classicAncestor: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(!script.content.contains("wrongPackage"));
        assert_eq!(script.deps, vec![normalize_path_string(&ancestor)]);
    }

    #[test]
    fn vue3_module_suffixes_respect_configured_order_for_relative_imports() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "moduleSuffixes": [".native", ".web", ""]
                }
            }"#,
        )
        .expect("write project config");
        let native = dir.path().join("ordered.native.ts");
        std::fs::write(
            &native,
            "export interface OrderedProps { nativeValue: string }",
        )
        .expect("write first configured suffix target");
        std::fs::write(
            dir.path().join("ordered.web.ts"),
            "export interface OrderedProps { wrongWebOrder: never }",
        )
        .expect("write later configured suffix target");
        std::fs::write(
            dir.path().join("ordered.ts"),
            "export interface OrderedProps { wrongOrderFallback: never }",
        )
        .expect("write unsuffixed order decoy");
        let directory_decoy = dir.path().join("ordered");
        std::fs::create_dir_all(&directory_decoy).expect("create same-name directory decoy");
        std::fs::write(
            directory_decoy.join("package.json"),
            r#"{"types":"index.d.ts"}"#,
        )
        .expect("write same-name directory manifest");
        std::fs::write(
            directory_decoy.join("index.d.ts"),
            "export interface OrderedProps { wrongDirectoryOrder: never }",
        )
        .expect("write same-name directory type decoy");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { OrderedProps } from './ordered'
defineProps<OrderedProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("nativeValue: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(!script.content.contains("wrongWebOrder"), "{}", script.content);
        assert!(
            !script.content.contains("wrongOrderFallback"),
            "{}",
            script.content
        );
        assert!(!script.content.contains("wrongDirectoryOrder"));
        assert_eq!(script.deps, vec![normalize_path_string(&native)]);
    }

    #[test]
    fn vue3_module_suffixes_do_not_add_an_implicit_empty_fallback() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "moduleSuffixes": [".native"]
                }
            }"#,
        )
        .expect("write project config");
        std::fs::write(
            dir.path().join("fallback.ts"),
            "export interface FallbackProps { implicitFallback: string }",
        )
        .expect("write forbidden unsuffixed fallback");
        let fallback_filename = dir.path().join("Fallback.vue");
        let fallback_source = r#"<script setup lang="ts">
import type { FallbackProps } from './fallback'
defineProps<FallbackProps>()
</script>"#;
        let mut fallback_compiler = SfcCompiler::new();
        let fallback_descriptor =
            fallback_compiler.parse(fallback_filename.to_string_lossy(), fallback_source);
        let fallback_script = fallback_compiler.compile_script(
            &fallback_descriptor,
            SfcScriptCompileOptions::default(),
        );

        assert!(
            fallback_script
                .errors
                .iter()
                .any(|error| error.contains("./fallback")),
            "{:?}",
            fallback_script.errors
        );
        assert!(
            !fallback_script.content.contains("implicitFallback"),
            "{}",
            fallback_script.content
        );
        assert!(fallback_script.deps.is_empty(), "{:?}", fallback_script.deps);
    }

    #[test]
    fn vue3_module_suffixes_are_inherited_and_overridden_as_a_list() {
        let dir = tempfile::tempdir().expect("temp dir");
        let config_dir = dir.path().join("configs");
        let inherited_dir = dir.path().join("inherited");
        let overridden_dir = dir.path().join("overridden");
        for directory in [&config_dir, &inherited_dir, &overridden_dir] {
            std::fs::create_dir_all(directory).expect("create module suffixes directory");
        }
        std::fs::write(
            config_dir.join("base.json"),
            r#"{
                "compilerOptions": {
                    "moduleSuffixes": [".base", ""]
                }
            }"#,
        )
        .expect("write base config");
        std::fs::write(
            inherited_dir.join("tsconfig.json"),
            r#"{"extends":"../configs/base.json"}"#,
        )
        .expect("write inherited config");
        std::fs::write(
            overridden_dir.join("tsconfig.json"),
            r#"{
                "extends":"../configs/base.json",
                "compilerOptions": {
                    "moduleSuffixes": [".project", ""]
                }
            }"#,
        )
        .expect("write overriding config");

        let inherited_target = inherited_dir.join("entry.base.ts");
        std::fs::write(
            &inherited_target,
            "export interface InheritedProps { inheritedValue: string }",
        )
        .expect("write inherited suffix target");
        std::fs::write(
            inherited_dir.join("entry.ts"),
            "export interface InheritedProps { wrongInheritedFallback: never }",
        )
        .expect("write inherited suffix decoy");
        let inherited_filename = inherited_dir.join("Comp.vue");
        let inherited_source = r#"<script setup lang="ts">
import type { InheritedProps } from './entry'
defineProps<InheritedProps>()
</script>"#;
        let mut inherited_compiler = SfcCompiler::new();
        let inherited_descriptor = inherited_compiler.parse(
            inherited_filename.to_string_lossy(),
            inherited_source,
        );
        let inherited_script = inherited_compiler.compile_script(
            &inherited_descriptor,
            SfcScriptCompileOptions::default(),
        );

        assert!(
            inherited_script.errors.is_empty(),
            "{:?}",
            inherited_script.errors
        );
        assert!(
            inherited_script
                .content
                .contains("inheritedValue: { type: String, required: true }"),
            "{}",
            inherited_script.content
        );
        assert!(
            !inherited_script.content.contains("wrongInheritedFallback"),
            "{}",
            inherited_script.content
        );
        assert_eq!(
            inherited_script.deps,
            vec![normalize_path_string(&inherited_target)]
        );

        let overridden_target = overridden_dir.join("entry.project.ts");
        std::fs::write(
            &overridden_target,
            "export interface OverriddenProps { overriddenValue: number }",
        )
        .expect("write overriding suffix target");
        std::fs::write(
            overridden_dir.join("entry.base.ts"),
            "export interface OverriddenProps { wrongBaseSuffix: never }",
        )
        .expect("write overridden base suffix decoy");
        std::fs::write(
            overridden_dir.join("entry.ts"),
            "export interface OverriddenProps { wrongOverrideFallback: never }",
        )
        .expect("write overridden fallback decoy");
        let overridden_filename = overridden_dir.join("Comp.vue");
        let overridden_source = r#"<script setup lang="ts">
import type { OverriddenProps } from './entry'
defineProps<OverriddenProps>()
</script>"#;
        let mut overridden_compiler = SfcCompiler::new();
        let overridden_descriptor = overridden_compiler.parse(
            overridden_filename.to_string_lossy(),
            overridden_source,
        );
        let overridden_script = overridden_compiler.compile_script(
            &overridden_descriptor,
            SfcScriptCompileOptions::default(),
        );

        assert!(
            overridden_script.errors.is_empty(),
            "{:?}",
            overridden_script.errors
        );
        assert!(
            overridden_script
                .content
                .contains("overriddenValue: { type: Number, required: true }"),
            "{}",
            overridden_script.content
        );
        assert!(!overridden_script.content.contains("wrongBaseSuffix"));
        assert!(!overridden_script.content.contains("wrongOverrideFallback"));
        assert_eq!(
            overridden_script.deps,
            vec![normalize_path_string(&overridden_target)]
        );
    }

    #[test]
    fn vue3_module_suffixes_apply_to_tsconfig_paths_targets_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        std::fs::create_dir_all(&source_dir).expect("create project source directory");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "moduleSuffixes": [".platform", ""],
                    "paths": {
                        "@vuec/*": ["./src/*"]
                    }
                }
            }"#,
        )
        .expect("write paths config");
        let platform_target = source_dir.join("aliased.platform.ts");
        std::fs::write(
            &platform_target,
            "export interface AliasedProps { platformValue: boolean }",
        )
        .expect("write suffixed paths target");
        std::fs::write(
            source_dir.join("aliased.ts"),
            "export interface AliasedProps { wrongPathsFallback: never }",
        )
        .expect("write unsuffixed paths decoy");

        let filename = source_dir.join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { AliasedProps } from '@vuec/aliased'
defineProps<AliasedProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("platformValue: { type: Boolean, required: true }"),
            "{}",
            script.content
        );
        assert!(!script.content.contains("wrongPathsFallback"));
        assert_eq!(
            script.deps,
            vec![normalize_path_string(&platform_target)]
        );
    }

    #[test]
    fn vue3_module_suffixes_preserve_extension_order_and_arbitrary_extensions() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{"compilerOptions":{"moduleSuffixes":[".native",""]}}"#,
        )
        .expect("write module suffix config");
        let tsx = dir.path().join("ordered.tsx");
        let arbitrary = dir.path().join("theme.d.css.native.ts");
        let esm = dir.path().join("module.native.mts");
        let cjs = dir.path().join("legacy.native.cts");
        let appended_mjs = dir.path().join("fallback.mjs.native.ts");
        let appended_js = dir.path().join("script.js.native.ts");
        let appended_arbitrary = dir.path().join("raw.css.native.ts");
        let jsx = dir.path().join("view.native.tsx");
        std::fs::write(
            &tsx,
            "export interface TsxProps { extensionOrder: string }",
        )
        .expect("write earlier plain extension target");
        std::fs::write(
            dir.path().join("ordered.native.d.ts"),
            "export interface TsxProps { wrongSuffixMajorOrder: never }",
        )
        .expect("write later suffixed extension decoy");
        std::fs::write(
            &arbitrary,
            "export interface CssProps { arbitraryExtension: boolean }",
        )
        .expect("write arbitrary extension declaration");
        std::fs::write(
            &esm,
            "export interface EsmProps { esmExtension: number }",
        )
        .expect("write mjs overlay source");
        std::fs::write(
            &cjs,
            "export interface CjsProps { cjsExtension?: string }",
        )
        .expect("write cjs overlay source");
        std::fs::write(
            &appended_mjs,
            "export interface AppendedMjsProps { appendedMjs: string }",
        )
        .expect("write appended mjs declaration");
        std::fs::write(
            dir.path().join("fallback.native.ts"),
            "export interface AppendedMjsProps { wrongMjsStem: never }",
        )
        .expect("write wrong mjs stem decoy");
        std::fs::write(
            &appended_js,
            "export interface AppendedJsProps { appendedJs: number }",
        )
        .expect("write appended js declaration");
        std::fs::write(
            &appended_arbitrary,
            "export interface AppendedCssProps { appendedCss: boolean }",
        )
        .expect("write appended arbitrary declaration");
        std::fs::write(
            &jsx,
            "export interface JsxProps { jsxPrefersTsx: string }",
        )
        .expect("write jsx tsx replacement");
        std::fs::write(
            dir.path().join("view.native.ts"),
            "export interface JsxProps { wrongJsxTsOrder: never }",
        )
        .expect("write jsx ts replacement decoy");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { TsxProps } from './ordered'
import type { CssProps } from './theme.css'
import type { EsmProps } from './module.mjs'
import type { CjsProps } from './legacy.cjs'
import type { AppendedMjsProps } from './fallback.mjs'
import type { AppendedJsProps } from './script.js'
import type { AppendedCssProps } from './raw.css'
import type { JsxProps } from './view.jsx'
defineProps<TsxProps & CssProps & EsmProps & CjsProps & AppendedMjsProps & AppendedJsProps & AppendedCssProps & JsxProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("extensionOrder: { type: String, required: true }"));
        assert!(script
            .content
            .contains("arbitraryExtension: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("esmExtension: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("cjsExtension: { type: String, required: false }"));
        assert!(script
            .content
            .contains("appendedMjs: { type: String, required: true }"));
        assert!(script
            .content
            .contains("appendedJs: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("appendedCss: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("jsxPrefersTsx: { type: String, required: true }"));
        assert!(!script.content.contains("wrongSuffixMajorOrder"));
        assert!(!script.content.contains("wrongMjsStem"));
        assert!(!script.content.contains("wrongJsxTsOrder"));
        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            tsx,
            arbitrary,
            esm,
            cjs,
            appended_mjs,
            appended_js,
            appended_arbitrary,
            jsx,
        ]
            .iter()
            .map(|path| normalize_path_string(path))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
    }

    #[test]
    fn vue3_module_suffixes_apply_before_compound_package_type_extensions() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir
            .path()
            .join("node_modules")
            .join("vuec-module-suffix-types");
        std::fs::create_dir_all(&package).expect("create package directory");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{"compilerOptions":{"moduleSuffixes":[".native",""]}}"#,
        )
        .expect("write project config");
        std::fs::write(package.join("package.json"), r#"{"types":"index.d.ts"}"#)
            .expect("write package manifest");
        let preferred = package.join("index.native.d.ts");
        std::fs::write(
            &preferred,
            "export interface PackageProps { compoundSuffix: string }",
        )
        .expect("write correctly suffixed package type");
        std::fs::write(
            package.join("index.d.native.ts"),
            "export interface PackageProps { wrongCompoundPlacement: never }",
        )
        .expect("write incorrectly suffixed package decoy");
        std::fs::write(
            package.join("index.d.ts"),
            "export interface PackageProps { wrongPackageFallback: never }",
        )
        .expect("write unsuffixed package decoy");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { PackageProps } from 'vuec-module-suffix-types'
defineProps<PackageProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("compoundSuffix: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(!script.content.contains("wrongCompoundPlacement"));
        assert!(!script.content.contains("wrongPackageFallback"));
        assert_eq!(script.deps, vec![normalize_path_string(&preferred)]);
    }

    #[test]
    fn vue3_module_suffixes_empty_and_pre_4_7_configs_use_normal_resolution() {
        let dir = tempfile::tempdir().expect("temp dir");
        let empty_project = dir.path().join("empty");
        let legacy_project = dir.path().join("legacy");
        std::fs::create_dir_all(&empty_project).expect("create empty suffix project");
        std::fs::create_dir_all(legacy_project.join("node_modules").join("typescript"))
            .expect("create legacy TypeScript package");

        std::fs::write(
            empty_project.join("tsconfig.json"),
            r#"{"compilerOptions":{"moduleSuffixes":[]}}"#,
        )
        .expect("write empty suffix config");
        let empty_target = empty_project.join("plain.ts");
        std::fs::write(
            &empty_target,
            "export interface EmptyProps { emptyListValue: string }",
        )
        .expect("write empty suffix target");
        let empty_filename = empty_project.join("Comp.vue");
        let empty_source = r#"<script setup lang="ts">
import type { EmptyProps } from './plain'
defineProps<EmptyProps>()
</script>"#;
        let mut empty_compiler = SfcCompiler::new();
        let empty_descriptor =
            empty_compiler.parse(empty_filename.to_string_lossy(), empty_source);
        let empty_script = empty_compiler
            .compile_script(&empty_descriptor, SfcScriptCompileOptions::default());

        assert!(empty_script.errors.is_empty(), "{:?}", empty_script.errors);
        assert!(
            empty_script
                .content
                .contains("emptyListValue: { type: String, required: true }"),
            "{}",
            empty_script.content
        );
        assert_eq!(
            empty_script.deps,
            vec![normalize_path_string(&empty_target)]
        );

        std::fs::write(
            legacy_project.join("tsconfig.json"),
            r#"{"compilerOptions":{"moduleSuffixes":[".native"]}}"#,
        )
        .expect("write legacy suffix config");
        std::fs::write(
            legacy_project
                .join("node_modules")
                .join("typescript")
                .join("package.json"),
            r#"{"version":"4.6.4"}"#,
        )
        .expect("write legacy TypeScript manifest");
        let legacy_target = legacy_project.join("plain.ts");
        std::fs::write(
            &legacy_target,
            "export interface LegacyProps { legacyValue: number }",
        )
        .expect("write legacy unsuffixed target");
        std::fs::write(
            legacy_project.join("plain.native.ts"),
            "export interface LegacyProps { wrongLegacySuffix: never }",
        )
        .expect("write ignored legacy suffix decoy");
        let legacy_filename = legacy_project.join("Comp.vue");
        let legacy_source = r#"<script setup lang="ts">
import type { LegacyProps } from './plain'
defineProps<LegacyProps>()
</script>"#;
        let mut legacy_compiler = SfcCompiler::new();
        let legacy_descriptor =
            legacy_compiler.parse(legacy_filename.to_string_lossy(), legacy_source);
        let legacy_script = legacy_compiler
            .compile_script(&legacy_descriptor, SfcScriptCompileOptions::default());

        assert!(
            legacy_script.errors.is_empty(),
            "{:?}",
            legacy_script.errors
        );
        assert!(
            legacy_script
                .content
                .contains("legacyValue: { type: Number, required: true }"),
            "{}",
            legacy_script.content
        );
        assert!(!legacy_script.content.contains("wrongLegacySuffix"));
        assert_eq!(
            legacy_script.deps,
            vec![normalize_path_string(&legacy_target)]
        );
    }

    #[test]
    fn vue3_compile_script_resolves_bare_package_macro_types_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        write_vue3_bundler_config(dir.path());
        let node_modules = dir.path().join("node_modules");
        let types_pkg = node_modules.join("vuec-types-pkg");
        let types_dist = types_pkg.join("dist");
        std::fs::create_dir_all(&types_dist).expect("create types package");
        std::fs::write(
            types_pkg.join("package.json"),
            r#"{"types":"dist/index.d.ts"}"#,
        )
        .expect("write types package manifest");
        std::fs::write(
            types_dist.join("index.d.ts"),
            "export interface Props { root: string }\nexport { ExtraProps } from './extra'\nexport type Events = { (e: 'save'): void }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write types package root");
        std::fs::write(
            types_dist.join("extra.d.ts"),
            "export type ExtraProps = { extra?: number }",
        )
        .expect("write types package extra");
        std::fs::write(
            types_dist.join("model.d.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write types package model");

        let facade_pkg = node_modules.join("vuec-facade-pkg");
        std::fs::create_dir_all(&facade_pkg).expect("create facade package");
        std::fs::write(facade_pkg.join("package.json"), r#"{"types":"index.d.ts"}"#)
            .expect("write facade manifest");
        std::fs::write(
            facade_pkg.join("index.d.ts"),
            "export { Props as FacadeProps } from 'vuec-types-pkg'",
        )
        .expect("write facade types");

        let exports_pkg = node_modules.join("vuec-exports-pkg");
        std::fs::create_dir_all(exports_pkg.join("types").join("feature"))
            .expect("create exports package");
        std::fs::create_dir_all(exports_pkg.join("types").join("internal"))
            .expect("create specific exports package path");
        std::fs::write(
            exports_pkg.join("package.json"),
            r#"{"exports":{".":{"types":"./types/index.d.ts","default":"./dist/index.js"},"./feature/*":{"types":"./types/feature/*.d.ts","default":"./dist/feature/*.js"},"./feature/internal/*":{"types":"./types/internal/*.d.ts","default":"./dist/internal/*.js"}}}"#,
        )
        .expect("write exports manifest");
        std::fs::write(
            exports_pkg.join("types").join("index.d.ts"),
            "export namespace Nested { export type Props = { flag: boolean } }",
        )
        .expect("write exports root types");
        std::fs::write(
            exports_pkg.join("types").join("feature").join("item.d.ts"),
            "export type FeatureProps = { feature: boolean }",
        )
        .expect("write exports feature types");
        std::fs::write(
            exports_pkg.join("types").join("internal").join("item.d.ts"),
            "export type InternalProps = { internal: boolean }",
        )
        .expect("write specific exports feature types");

        let ambient_pkg = node_modules.join("@types").join("vuec-ambient");
        std::fs::create_dir_all(&ambient_pkg).expect("create @types package");
        std::fs::write(
            ambient_pkg.join("index.d.ts"),
            "export type AmbientProps = { ambient: string }",
        )
        .expect("write @types package");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { ExtraProps, Events } from 'vuec-types-pkg'
import type { FacadeProps } from 'vuec-facade-pkg'
import type { FeatureProps } from 'vuec-exports-pkg/feature/item'
import type { InternalProps } from 'vuec-exports-pkg/feature/internal/item'
import type { AmbientProps } from 'vuec-ambient'
import * as Exported from 'vuec-exports-pkg'
const props = defineProps<FacadeProps & ExtraProps & FeatureProps & InternalProps & AmbientProps & Exported.Nested.Props>()
const emit = defineEmits<Events>()
const model = defineModel<import('vuec-types-pkg').ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("root: { type: String, required: true }"),
            "{}\ndeps: {:?}",
            script.content,
            script.deps
        );
        assert!(script
            .content
            .contains("extra: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("feature: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("internal: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("ambient: { type: String, required: true }"));
        assert!(script
            .content
            .contains("flag: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            types_dist.join("index.d.ts"),
            types_dist.join("extra.d.ts"),
            types_dist.join("model.d.ts"),
            facade_pkg.join("index.d.ts"),
            exports_pkg.join("types").join("index.d.ts"),
            exports_pkg.join("types").join("feature").join("item.d.ts"),
            exports_pkg.join("types").join("internal").join("item.d.ts"),
            ambient_pkg.join("index.d.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_dependency_packages_resolve_self_name_imports_with_their_own_mode() {
        let dir = tempfile::tempdir().expect("temp dir");
        write_vue3_node_next_config(dir.path());
        let node_modules = dir.path().join("node_modules");
        let module_package = node_modules.join("@vuec").join("self-module");
        let commonjs_package = node_modules.join("vuec-self-commonjs");
        for package in [&module_package, &commonjs_package] {
            std::fs::create_dir_all(package.join("types").join("import"))
                .expect("create import type directory");
            std::fs::create_dir_all(package.join("types").join("require"))
                .expect("create require type directory");
        }
        std::fs::write(
            module_package.join("package.json"),
            r#"{
                "name":"@vuec/self-module",
                "type":"module",
                "exports":{
                    ".":{"types":"./types/index.d.ts"},
                    "./feature/*":{"types":{
                        "import":"./types/import/*.d.ts",
                        "require":"./types/require/*.d.ts"
                    }}
                }
            }"#,
        )
        .expect("write module self-reference manifest");
        std::fs::write(
            commonjs_package.join("package.json"),
            r#"{
                "name":"vuec-self-commonjs",
                "type":"commonjs",
                "exports":{
                    ".":{"types":"./types/index.d.ts"},
                    "./feature/*":{"types":{
                        "import":"./types/import/*.d.ts",
                        "require":"./types/require/*.d.ts"
                    }}
                }
            }"#,
        )
        .expect("write CommonJS self-reference manifest");
        std::fs::write(
            module_package.join("types").join("index.d.ts"),
            concat!(
                "export { FeatureProps as ModuleStaticProps } from '@vuec/self-module/feature/item'\n",
                "export type ModuleDynamicProps = import('@vuec/self-module/feature/item').DynamicProps",
            ),
        )
        .expect("write module self-reference root");
        std::fs::write(
            commonjs_package.join("types").join("index.d.ts"),
            concat!(
                "export { FeatureProps as CommonJsStaticProps } from 'vuec-self-commonjs/feature/item'\n",
                "export type CommonJsDynamicProps = import('vuec-self-commonjs/feature/item').DynamicProps",
            ),
        )
        .expect("write CommonJS self-reference root");
        std::fs::write(
            module_package
                .join("types")
                .join("import")
                .join("item.d.ts"),
            concat!(
                "export interface FeatureProps { moduleStatic: string }\n",
                "export interface DynamicProps { moduleDynamic: boolean }",
            ),
        )
        .expect("write module import target");
        std::fs::write(
            module_package
                .join("types")
                .join("require")
                .join("item.d.ts"),
            concat!(
                "export interface FeatureProps { wrongModuleStatic: never }\n",
                "export interface DynamicProps { wrongModuleDynamic: never }",
            ),
        )
        .expect("write module require decoy");
        std::fs::write(
            commonjs_package
                .join("types")
                .join("require")
                .join("item.d.ts"),
            concat!(
                "export interface FeatureProps { commonjsStatic: number }\n",
                "export interface DynamicProps { commonjsDynamic?: string }",
            ),
        )
        .expect("write CommonJS require target");
        std::fs::write(
            commonjs_package
                .join("types")
                .join("import")
                .join("item.d.ts"),
            concat!(
                "export interface FeatureProps { wrongCommonJsStatic: never }\n",
                "export interface DynamicProps { wrongCommonJsDynamic: never }",
            ),
        )
        .expect("write CommonJS import decoy");
        let nested_module_decoy = module_package
            .join("types")
            .join("node_modules")
            .join("@vuec")
            .join("self-module");
        let nested_commonjs_decoy = commonjs_package
            .join("types")
            .join("node_modules")
            .join("vuec-self-commonjs");
        for decoy in [&nested_module_decoy, &nested_commonjs_decoy] {
            std::fs::create_dir_all(decoy).expect("create nested same-name decoy");
            std::fs::write(
                decoy.join("package.json"),
                r#"{"exports":{"./feature/*":{"types":"./*.d.ts"}}}"#,
            )
            .expect("write nested same-name decoy manifest");
            std::fs::write(
                decoy.join("item.d.ts"),
                concat!(
                    "export interface FeatureProps { nestedDecoyStatic: never }\n",
                    "export interface DynamicProps { nestedDecoyDynamic: never }",
                ),
            )
            .expect("write nested same-name decoy target");
        }

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { ModuleStaticProps, ModuleDynamicProps } from '@vuec/self-module'
import type { CommonJsStaticProps, CommonJsDynamicProps } from 'vuec-self-commonjs'
defineProps<ModuleStaticProps & ModuleDynamicProps & CommonJsStaticProps & CommonJsDynamicProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for expected in [
            "moduleStatic: { type: String, required: true }",
            "moduleDynamic: { type: Boolean, required: true }",
            "commonjsStatic: { type: Number, required: true }",
            "commonjsDynamic: { type: String, required: false }",
        ] {
            assert!(script.content.contains(expected), "{}", script.content);
        }
        assert!(!script.content.contains("wrongModule"), "{}", script.content);
        assert!(!script.content.contains("wrongCommonJs"), "{}", script.content);
        assert!(!script.content.contains("nestedDecoy"), "{}", script.content);

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            module_package.join("types").join("index.d.ts"),
            module_package
                .join("types")
                .join("import")
                .join("item.d.ts"),
            commonjs_package.join("types").join("index.d.ts"),
            commonjs_package
                .join("types")
                .join("require")
                .join("item.d.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
    }

    #[test]
    fn vue3_dependency_self_name_export_condition_falls_back_within_self_package() {
        let dir = tempfile::tempdir().expect("temp dir");
        let outer_package = dir.path().join("node_modules").join("vuec-self-blocked");
        let nested_package = dir
            .path()
            .join("node_modules")
            .join("container")
            .join("node_modules")
            .join("vuec-self-blocked");
        std::fs::create_dir_all(&outer_package).expect("create outer decoy package");
        std::fs::create_dir_all(&nested_package).expect("create nested self package");
        std::fs::write(
            outer_package.join("package.json"),
            r#"{"exports":{"./private":{"types":"./private.d.ts"}}}"#,
        )
        .expect("write outer decoy manifest");
        std::fs::write(
            outer_package.join("private.d.ts"),
            "export interface PrivateProps { leaked: string }",
        )
        .expect("write outer decoy type");
        std::fs::write(
            nested_package.join("package.json"),
            r#"{
                "name":"vuec-self-blocked",
                "exports":{
                    ".":{"types":"./index.d.ts"},
                    "./private":{"types":null,"default":"./leak.d.ts"}
                }
            }"#,
        )
        .expect("write nested self manifest");
        let importer = nested_package.join("index.d.ts");
        std::fs::write(&importer, "export {};").expect("write nested importer");
        std::fs::write(
            nested_package.join("leak.d.ts"),
            "export interface PrivateProps { selfLeak: string }",
        )
        .expect("write self fallback target");
        let outside_resolver = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_type_import(
                &dir.path().join("outside.ts").to_string_lossy(),
                "vuec-self-blocked/private",
                &outside_resolver,
            ),
            Some(outer_package.join("private.d.ts"))
        );
        let resolver = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..Vue3TypeResolverContext::default()
        };

        assert_eq!(
            resolve_vue3_type_import(
                &importer.to_string_lossy(),
                "vuec-self-blocked/private",
                &resolver,
            ),
            Some(nested_package.join("leak.d.ts"))
        );
        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.metadata_files_read, 1);
        assert!(!resolver.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn vue3_self_name_failures_only_block_outer_packages_for_terminal_null_targets() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package_name = "vuec-self-fallback-boundary";
        let project = dir.path().join("project");
        let source_dir = project.join("src");
        let dependency = project.join("node_modules").join(package_name);
        std::fs::create_dir_all(&source_dir).expect("create project source directory");
        std::fs::create_dir_all(&dependency).expect("create outer same-name dependency");
        std::fs::write(
            dependency.join("package.json"),
            r#"{
                "exports": {
                    ".": "./outer.d.ts",
                    "./feature": "./outer.d.ts"
                }
            }"#,
        )
        .expect("write outer same-name manifest");
        let outer_target = dependency.join("outer.d.ts");
        std::fs::write(
            &outer_target,
            "export interface OuterProps { outer: string }",
        )
        .expect("write outer same-name target");
        let importer = source_dir.join("index.d.mts");
        std::fs::write(&importer, "export {};").expect("write project importer");

        let cases = [
            (
                (5, 9, 3),
                format!("{package_name}/feature"),
                serde_json::json!({ "./feature": null }),
                Some(outer_target.clone()),
            ),
            (
                (6, 0, 0),
                format!("{package_name}/feature"),
                serde_json::json!({ "./feature": null }),
                None,
            ),
            (
                (6, 0, 0),
                package_name.to_string(),
                serde_json::json!({ ".": null }),
                Some(outer_target.clone()),
            ),
            (
                (6, 0, 0),
                format!("{package_name}/feature"),
                serde_json::json!({ "./feature": "./missing.d.ts" }),
                Some(outer_target.clone()),
            ),
            (
                (6, 0, 0),
                format!("{package_name}/feature"),
                serde_json::json!({ "./feature": "../invalid.d.ts" }),
                Some(outer_target.clone()),
            ),
            (
                (6, 0, 0),
                format!("{package_name}/feature"),
                serde_json::json!({ "./feature": [] }),
                Some(outer_target.clone()),
            ),
            (
                (6, 0, 0),
                format!("{package_name}/feature"),
                serde_json::json!({ "./other": "./local.d.ts" }),
                Some(outer_target.clone()),
            ),
        ];
        for (version, source, exports, expected) in cases {
            std::fs::write(
                project.join("package.json"),
                serde_json::json!({ "name": package_name, "exports": exports }).to_string(),
            )
            .expect("write project package manifest");
            let resolver = Vue3TypeResolverContext {
                typescript_version: version.into(),
                module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
                ..Vue3TypeResolverContext::default()
            };

            assert_eq!(
                resolve_vue3_type_import(&importer.to_string_lossy(), &source, &resolver),
                expected,
                "TypeScript {version:?}, source {source}, exports {exports}"
            );
            assert!(!resolver.external_type_session.metadata_is_blocked());
        }
    }

    #[test]
    fn vue3_bare_package_active_null_export_uses_typescript_fallback() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir
            .path()
            .join("node_modules")
            .join("vuec-null-conditional");
        std::fs::create_dir_all(&package).expect("create package");
        std::fs::write(
            package.join("package.json"),
            r#"{
                "exports":{
                    "./private":{"types":null,"default":"./leak.d.ts"}
                }
            }"#,
        )
        .expect("write package manifest");
        std::fs::write(
            package.join("leak.d.ts"),
            "export interface PrivateProps { leaked: string }",
        )
        .expect("write fallback target");
        let resolver = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..Vue3TypeResolverContext::default()
        };

        assert_eq!(
            resolve_vue3_type_import(
                &dir.path().join("outside.ts").to_string_lossy(),
                "vuec-null-conditional/private",
                &resolver,
            ),
            Some(package.join("leak.d.ts"))
        );
        assert!(!resolver.external_type_session.metadata_is_blocked());
    }

    #[cfg(unix)]
    #[test]
    fn vue3_bare_package_exact_targets_preserve_literal_stars() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir
            .path()
            .join("node_modules")
            .join("vuec-literal-star-target");
        std::fs::create_dir_all(&package).expect("create package");
        std::fs::write(
            package.join("package.json"),
            r#"{"exports":{".":{"types":"./literal*.d.ts"}}}"#,
        )
        .expect("write package manifest");
        let target = package.join("literal*.d.ts");
        std::fs::write(&target, "export interface LiteralStar { value: string }")
            .expect("write literal-star target");
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_import(
                &dir.path().join("outside.ts").to_string_lossy(),
                "vuec-literal-star-target",
                &resolver,
            ),
            Some(target)
        );
        assert!(!resolver.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn vue3_project_self_name_exports_map_emitted_targets_and_resolution_modes_to_sources() {
        let dir = tempfile::tempdir().expect("temp dir");
        write_vue3_typescript_version(dir.path(), "5.3.0");
        let source_dir = dir.path().join("src");
        let output_dir = dir.path().join("dist");
        let declaration_dir = dir.path().join("declarations");
        for directory in [&source_dir, &output_dir, &declaration_dir] {
            std::fs::create_dir_all(directory).expect("create project directory");
        }
        std::fs::write(
            dir.path().join("package.json"),
            r#"{
                "name":"vuec-project-self",
                "type":"module",
                "exports":{
                    ".":{"types":{
                        "import":"./dist/root.js",
                        "require":"./declarations/root.d.cts"
                    }},
                    "./feature":{"types":{
                        "import":"./dist/feature.mjs",
                        "require":"./declarations/feature.d.cts"
                    }}
                }
            }"#,
        )
        .expect("write project self-reference manifest");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "module": "ESNext",
                    "moduleResolution": "Bundler",
                    "rootDir": "./src",
                    "outDir": "./dist",
                    "declarationDir": "./declarations"
                }
            }"#,
        )
        .expect("write project config");
        let targets = [
            (
                source_dir.join("root.ts"),
                "export interface ImportRootProps { importRoot: string }",
            ),
            (
                source_dir.join("root.cts"),
                "export interface RequireRootProps { requireRoot: number }",
            ),
            (
                source_dir.join("feature.mts"),
                "export interface ImportFeatureProps { importFeature: boolean }",
            ),
            (
                source_dir.join("feature.cts"),
                "export interface RequireFeatureProps { requireFeature?: string }",
            ),
        ];
        for (path, source) in &targets {
            std::fs::write(path, source).expect("write project self-reference source");
        }
        for (path, source) in [
            (
                output_dir.join("root.d.ts"),
                "export interface ImportRootProps { wrongOutputRoot: never }",
            ),
            (
                output_dir.join("feature.d.mts"),
                "export interface ImportFeatureProps { wrongOutputFeature: never }",
            ),
            (
                declaration_dir.join("root.d.cts"),
                "export interface RequireRootProps { wrongDeclarationRoot: never }",
            ),
            (
                declaration_dir.join("feature.d.cts"),
                "export interface RequireFeatureProps { wrongDeclarationFeature: never }",
            ),
        ] {
            std::fs::write(path, source).expect("write emitted self-reference decoy");
        }

        let filename = source_dir.join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { ImportRootProps } from 'vuec-project-self'
import type { ImportFeatureProps } from 'vuec-project-self/feature'
import type { RequireRootProps } from 'vuec-project-self' with { "resolution-mode": "require" }
import type { RequireFeatureProps } from 'vuec-project-self/feature' with { "resolution-mode": "require" }
defineProps<ImportRootProps & ImportFeatureProps & RequireRootProps & RequireFeatureProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for expected in [
            "importRoot: { type: String, required: true }",
            "importFeature: { type: Boolean, required: true }",
            "requireRoot: { type: Number, required: true }",
            "requireFeature: { type: String, required: false }",
        ] {
            assert!(script.content.contains(expected), "{}", script.content);
        }
        assert!(!script.content.contains("wrongOutput"), "{}", script.content);
        assert!(
            !script.content.contains("wrongDeclaration"),
            "{}",
            script.content
        );
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            targets
                .iter()
                .map(|(path, _)| normalize_path_string(path))
                .collect::<BTreeSet<_>>()
        );
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_project_self_name_emit_paths_accept_windows_separators() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        let output_dir = dir.path().join("dist");
        let declaration_dir = dir.path().join("declarations");
        for directory in [&source_dir, &output_dir, &declaration_dir] {
            std::fs::create_dir_all(directory).expect("create project directory");
        }
        std::fs::write(
            dir.path().join("package.json"),
            r#"{
                "name":"vuec-project-windows-paths",
                "exports":{
                    ".":{"types":"./dist/root.js"},
                    "./feature":{"types":"./declarations/feature.d.ts"}
                }
            }"#,
        )
        .expect("write project self-reference manifest");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "module": "ESNext",
                    "moduleResolution": "Bundler",
                    "rootDir": ".\\src",
                    "outDir": ".\\dist",
                    "declarationDir": ".\\declarations"
                }
            }"#,
        )
        .expect("write project config with Windows separators");
        let root = source_dir.join("root.ts");
        let feature = source_dir.join("feature.ts");
        std::fs::write(
            &root,
            "export interface RootProps { windowsRoot: string }",
        )
        .expect("write project root source");
        std::fs::write(
            &feature,
            "export interface FeatureProps { windowsFeature?: number }",
        )
        .expect("write project feature source");
        std::fs::write(
            output_dir.join("root.d.ts"),
            "export interface RootProps { wrongOutputRoot: never }",
        )
        .expect("write output root decoy");
        std::fs::write(
            declaration_dir.join("feature.d.ts"),
            "export interface FeatureProps { wrongDeclarationFeature: never }",
        )
        .expect("write declaration feature decoy");

        let filename = source_dir.join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { RootProps } from 'vuec-project-windows-paths'
import type { FeatureProps } from 'vuec-project-windows-paths/feature'
defineProps<RootProps & FeatureProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for expected in [
            "windowsRoot: { type: String, required: true }",
            "windowsFeature: { type: Number, required: false }",
        ] {
            assert!(script.content.contains(expected), "{}", script.content);
        }
        assert!(!script.content.contains("wrongOutput"), "{}", script.content);
        assert!(
            !script.content.contains("wrongDeclaration"),
            "{}",
            script.content
        );
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [root, feature]
                .iter()
                .map(|path| normalize_path_string(path))
                .collect::<BTreeSet<_>>()
        );
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_project_self_name_exports_require_typescript_4_7() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        let dependency = dir.path().join("node_modules").join("vuec-versioned-self");
        std::fs::create_dir_all(&source_dir).expect("create project source directory");
        std::fs::create_dir_all(&dependency).expect("create fallback dependency");
        std::fs::write(
            dir.path().join("package.json"),
            r#"{
                "name":"vuec-versioned-self",
                "exports":{
                    "./feature":{"types":"./src/local.d.ts"},
                    "./excluded":null
                }
            }"#,
        )
        .expect("write project self-reference manifest");
        let local = source_dir.join("local.d.ts");
        std::fs::write(&local, "export interface Props { local: string }")
            .expect("write local self-reference target");
        std::fs::write(
            dependency.join("package.json"),
            r#"{"types":"feature.d.ts"}"#,
        )
        .expect("write fallback dependency manifest");
        let fallback = dependency.join("feature.d.ts");
        std::fs::write(&fallback, "export interface Props { fallback: number }")
            .expect("write fallback dependency target");
        let excluded_fallback = dependency.join("excluded.d.ts");
        let missing_fallback = dependency.join("missing.d.ts");
        for path in [&excluded_fallback, &missing_fallback] {
            std::fs::write(
                path,
                "export interface Props { packageFallback: boolean }",
            )
            .expect("write same-name package fallback");
        }
        let importer = source_dir.join("index.d.ts");
        std::fs::write(&importer, "export {};").expect("write project importer");

        let legacy = Vue3TypeResolverContext {
            typescript_version: (4, 6, 0).into(),
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..Vue3TypeResolverContext::default()
        };
        let current = Vue3TypeResolverContext {
            typescript_version: (4, 7, 0).into(),
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..Vue3TypeResolverContext::default()
        };
        let importer = importer.to_string_lossy();

        assert_eq!(
            resolve_vue3_type_import_with_mode(
                &importer,
                "vuec-versioned-self/feature",
                Vue3TypeResolutionMode::Require,
                &legacy,
            ),
            Some(fallback)
        );
        assert_eq!(
            resolve_vue3_type_import_with_mode(
                &importer,
                "vuec-versioned-self/feature",
                Vue3TypeResolutionMode::Require,
                &current,
            ),
            Some(local)
        );
        assert_eq!(
            resolve_vue3_type_import_with_mode(
                &importer,
                "vuec-versioned-self/excluded",
                Vue3TypeResolutionMode::Require,
                &current,
            ),
            Some(excluded_fallback)
        );
        assert_eq!(
            resolve_vue3_type_import_with_mode(
                &importer,
                "vuec-versioned-self/missing",
                Vue3TypeResolutionMode::Require,
                &current,
            ),
            Some(missing_fallback)
        );
        assert!(!legacy.external_type_session.metadata_is_blocked());
        assert!(!current.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn vue3_project_self_name_without_exports_uses_bare_package_lookup() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        let dependency = dir
            .path()
            .join("node_modules")
            .join("vuec-project-no-exports");
        std::fs::create_dir_all(&source_dir).expect("create project source directory");
        std::fs::create_dir_all(&dependency).expect("create dependency package");
        std::fs::write(
            dir.path().join("package.json"),
            r#"{
                "name":"vuec-project-no-exports",
                "types":"./src/local-decoy.ts"
            }"#,
        )
        .expect("write project manifest without exports");
        let local_decoy = source_dir.join("local-decoy.ts");
        std::fs::write(
            &local_decoy,
            "export interface ProjectProps { wrongLocalSelf: never }",
        )
        .expect("write local package decoy");
        std::fs::write(
            dependency.join("package.json"),
            r#"{"types":"index.d.ts"}"#,
        )
        .expect("write dependency manifest");
        let dependency_entry = dependency.join("index.d.ts");
        std::fs::write(
            &dependency_entry,
            "export interface ProjectProps { dependencyValue: string }",
        )
        .expect("write dependency entry");

        let filename = source_dir.join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { ProjectProps } from 'vuec-project-no-exports'
defineProps<ProjectProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("dependencyValue: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(!script.content.contains("wrongLocalSelf"), "{}", script.content);
        assert_eq!(script.deps, vec![normalize_path_string(&dependency_entry)]);
        assert!(!script.deps.contains(&normalize_path_string(&local_decoy)));
    }

    #[test]
    fn vue3_dependency_self_name_without_exports_uses_legacy_package_lookup() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir.path().join("node_modules").join("vuec-self-legacy");
        std::fs::create_dir_all(&package).expect("create legacy self package");
        let importer = package.join("index.d.ts");
        let leaf = package.join("leaf.d.ts");
        std::fs::write(&importer, "export {};").expect("write legacy self importer");
        std::fs::write(&leaf, "export interface LegacyProps { value: string }")
            .expect("write legacy self leaf");

        for exports in [
            serde_json::Value::Null,
            serde_json::json!(false),
            serde_json::json!(0),
            serde_json::json!(-0.0),
            serde_json::json!(""),
        ] {
            let manifest = serde_json::json!({
                "name": "vuec-self-legacy",
                "exports": exports,
            });
            std::fs::write(package.join("package.json"), manifest.to_string())
                .expect("write legacy self manifest");
            let resolver = Vue3TypeResolverContext::default();
            assert_eq!(
                resolve_vue3_type_import(
                    &importer.to_string_lossy(),
                    "vuec-self-legacy/leaf",
                    &resolver,
                ),
                Some(leaf.clone()),
                "{manifest}"
            );
            assert!(!resolver.external_type_session.metadata_is_blocked());
        }
    }

    #[test]
    fn vue3_node_esm_package_root_index_fallback_requires_a_legacy_manifest() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let source_dir = dir.path().join("src");
        std::fs::create_dir_all(&source_dir).expect("create source directory");
        let importer = source_dir.join("entry.mts");
        std::fs::write(&importer, "export {};").expect("write importer");

        let cases = [
            ("no-manifest", None, false),
            (
                "missing-exports",
                Some(r#"{"types":"missing.d.ts"}"#),
                true,
            ),
            (
                "null-exports",
                Some(r#"{"types":"missing.d.ts","exports":null}"#),
                true,
            ),
            (
                "false-exports",
                Some(r#"{"types":"missing.d.ts","exports":false}"#),
                false,
            ),
            (
                "zero-exports",
                Some(r#"{"types":"missing.d.ts","exports":0}"#),
                false,
            ),
            (
                "negative-zero-exports",
                Some(r#"{"types":"missing.d.ts","exports":-0.0}"#),
                false,
            ),
            (
                "empty-string-exports",
                Some(r#"{"types":"missing.d.ts","exports":""}"#),
                false,
            ),
        ];
        let mut fixtures = Vec::new();
        for (case, manifest, node_esm_uses_package_index) in cases {
            let package_name = format!("vuec-esm-root-{case}");
            let package = node_modules.join(&package_name);
            std::fs::create_dir_all(&package).expect("create package directory");
            if let Some(manifest) = manifest {
                std::fs::write(package.join("package.json"), manifest)
                    .expect("write package manifest");
            }
            let package_index = package.join("index.d.ts");
            std::fs::write(&package_index, "export interface RootProps {}").expect("write index");

            let types_package = node_modules.join("@types").join(&package_name);
            std::fs::create_dir_all(&types_package).expect("create @types package");
            std::fs::write(types_package.join("package.json"), r#"{"exports":null}"#)
                .expect("write @types manifest");
            let types_index = types_package.join("index.d.ts");
            std::fs::write(&types_index, "export interface RootProps {}").expect("write @types index");

            fixtures.push((
                package_name,
                package_index,
                types_index,
                node_esm_uses_package_index,
            ));
        }

        for module_resolution in [
            Vue3TypeModuleResolutionKind::Node16,
            Vue3TypeModuleResolutionKind::NodeNext,
        ] {
            let resolver = Vue3TypeResolverContext {
                typescript_version: (6, 0, 3).into(),
                module_resolution,
                ..Vue3TypeResolverContext::default()
            };
            for (package_name, package_index, types_index, node_esm_uses_package_index) in
                fixtures.iter()
            {
                assert_eq!(
                    resolve_vue3_type_import_with_mode(
                        &importer.to_string_lossy(),
                        package_name,
                        Vue3TypeResolutionMode::Import,
                        &resolver,
                    ),
                    Some(if *node_esm_uses_package_index {
                        package_index.clone()
                    } else {
                        types_index.clone()
                    }),
                    "Node ESM root fallback for {package_name}"
                );
                assert_eq!(
                    resolve_vue3_type_import_with_mode(
                        &importer.to_string_lossy(),
                        package_name,
                        Vue3TypeResolutionMode::Require,
                        &resolver,
                    ),
                    Some(package_index.clone()),
                    "CommonJS root fallback for {package_name}"
                );
            }
            assert!(!resolver.external_type_session.metadata_is_blocked());
        }

        let bundler = Vue3TypeResolverContext {
            typescript_version: (6, 0, 3).into(),
            module_resolution: Vue3TypeModuleResolutionKind::Bundler,
            ..Vue3TypeResolverContext::default()
        };
        for (package_name, package_index, _, _) in fixtures.iter() {
            assert_eq!(
                resolve_vue3_type_import_with_mode(
                    &importer.to_string_lossy(),
                    package_name,
                    Vue3TypeResolutionMode::Import,
                    &bundler,
                ),
                Some(package_index.clone()),
                "Bundler root fallback for {package_name}"
            );
        }
        assert!(!bundler.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn vue3_node_esm_package_subpaths_require_explicit_files_or_package_entries() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir
            .path()
            .join("node_modules")
            .join("vuec-esm-subpath-fallback");
        let folder = package.join("folder");
        let nested = package.join("nested");
        for directory in [&package, &folder, &nested] {
            std::fs::create_dir_all(directory).expect("create package fixture");
        }
        std::fs::write(package.join("package.json"), "{}").expect("write package manifest");
        let extensionless = package.join("extensionless.d.ts");
        let explicit = package.join("explicit.d.ts");
        let folder_index = folder.join("index.d.ts");
        let nested_index = nested.join("index.d.ts");
        for target in [&extensionless, &explicit, &folder_index, &nested_index] {
            std::fs::write(target, "export interface SubpathProps {}").expect("write type file");
        }
        std::fs::write(
            nested.join("package.json"),
            r#"{"types":"index.d.ts"}"#,
        )
        .expect("write nested package manifest");
        let importer = dir.path().join("entry.mts");
        std::fs::write(&importer, "export {};").expect("write importer");

        for module_resolution in [
            Vue3TypeModuleResolutionKind::Node16,
            Vue3TypeModuleResolutionKind::NodeNext,
        ] {
            let resolver = Vue3TypeResolverContext {
                typescript_version: (6, 0, 3).into(),
                module_resolution,
                ..Vue3TypeResolverContext::default()
            };
            assert_eq!(
                resolve_vue3_type_import_with_mode(
                    &importer.to_string_lossy(),
                    "vuec-esm-subpath-fallback/extensionless",
                    Vue3TypeResolutionMode::Import,
                    &resolver,
                ),
                None
            );
            assert_eq!(
                resolve_vue3_type_import_with_mode(
                    &importer.to_string_lossy(),
                    "vuec-esm-subpath-fallback/explicit.js",
                    Vue3TypeResolutionMode::Import,
                    &resolver,
                ),
                Some(explicit.clone())
            );
            assert_eq!(
                resolve_vue3_type_import_with_mode(
                    &importer.to_string_lossy(),
                    "vuec-esm-subpath-fallback/folder",
                    Vue3TypeResolutionMode::Import,
                    &resolver,
                ),
                None
            );
            assert_eq!(
                resolve_vue3_type_import_with_mode(
                    &importer.to_string_lossy(),
                    "vuec-esm-subpath-fallback/nested",
                    Vue3TypeResolutionMode::Import,
                    &resolver,
                ),
                Some(nested_index.clone())
            );
            for (source, expected) in [
                ("vuec-esm-subpath-fallback/extensionless", &extensionless),
                ("vuec-esm-subpath-fallback/folder", &folder_index),
            ] {
                assert_eq!(
                    resolve_vue3_type_import_with_mode(
                        &importer.to_string_lossy(),
                        source,
                        Vue3TypeResolutionMode::Require,
                        &resolver,
                    ),
                    Some(expected.clone())
                );
            }
            assert!(!resolver.external_type_session.metadata_is_blocked());
        }

        let bundler = Vue3TypeResolverContext {
            typescript_version: (6, 0, 3).into(),
            module_resolution: Vue3TypeModuleResolutionKind::Bundler,
            ..Vue3TypeResolverContext::default()
        };
        for (source, expected) in [
            ("vuec-esm-subpath-fallback/extensionless", extensionless),
            ("vuec-esm-subpath-fallback/folder", folder_index),
            ("vuec-esm-subpath-fallback/nested", nested_index),
        ] {
            assert_eq!(
                resolve_vue3_type_import_with_mode(
                    &importer.to_string_lossy(),
                    source,
                    Vue3TypeResolutionMode::Import,
                    &bundler,
                ),
                Some(expected)
            );
        }
        assert!(!bundler.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn vue3_package_subpath_fallback_respects_root_exports_property_presence() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let importer = dir.path().join("entry.mts");
        std::fs::write(&importer, "export {};").expect("write importer");
        let mut fixtures = Vec::new();
        for (case, exports) in [
            ("null", serde_json::Value::Null),
            ("false", serde_json::json!(false)),
            ("zero", serde_json::json!(0)),
            ("negative-zero", serde_json::json!(-0.0)),
            ("empty-string", serde_json::json!("")),
        ] {
            let package_name = format!("vuec-root-exports-{case}");
            let package = node_modules.join(&package_name);
            let nested = package.join("nested");
            std::fs::create_dir_all(&nested).expect("create nested package");
            std::fs::write(
                package.join("package.json"),
                serde_json::json!({ "exports": exports }).to_string(),
            )
            .expect("write root package manifest");
            std::fs::write(
                nested.join("package.json"),
                r#"{"types":"entry.d.ts"}"#,
            )
            .expect("write nested package manifest");
            let nested_entry = nested.join("entry.d.ts");
            let nested_index = nested.join("index.d.ts");
            std::fs::write(&nested_entry, "export interface NestedProps {}").expect("write entry");
            std::fs::write(&nested_index, "export interface NestedProps {}").expect("write index");
            fixtures.push((package_name, nested_entry, nested_index));
        }

        for module_resolution in [
            Vue3TypeModuleResolutionKind::Node16,
            Vue3TypeModuleResolutionKind::NodeNext,
        ] {
            let resolver = Vue3TypeResolverContext {
                typescript_version: (6, 0, 3).into(),
                module_resolution,
                ..Vue3TypeResolverContext::default()
            };
            for (package_name, _, nested_index) in fixtures.iter() {
                let source = format!("{package_name}/nested");
                assert_eq!(
                    resolve_vue3_type_import_with_mode(
                        &importer.to_string_lossy(),
                        &source,
                        Vue3TypeResolutionMode::Import,
                        &resolver,
                    ),
                    None,
                    "Node ESM must ignore the nested manifest for {package_name}"
                );
                assert_eq!(
                    resolve_vue3_type_import_with_mode(
                        &importer.to_string_lossy(),
                        &source,
                        Vue3TypeResolutionMode::Require,
                        &resolver,
                    ),
                    Some(nested_index.clone()),
                    "CommonJS must use index instead of the nested manifest for {package_name}"
                );
            }
            assert!(!resolver.external_type_session.metadata_is_blocked());
        }

        let bundler = Vue3TypeResolverContext {
            typescript_version: (6, 0, 3).into(),
            module_resolution: Vue3TypeModuleResolutionKind::Bundler,
            ..Vue3TypeResolverContext::default()
        };
        let bundler_exports_disabled = Vue3TypeResolverContext {
            typescript_version: (6, 0, 3).into(),
            module_resolution: Vue3TypeModuleResolutionKind::Bundler,
            resolve_package_json_exports: Some(false),
            ..Vue3TypeResolverContext::default()
        };
        for (package_name, nested_entry, nested_index) in fixtures.iter() {
            let source = format!("{package_name}/nested");
            assert_eq!(
                resolve_vue3_type_import_with_mode(
                    &importer.to_string_lossy(),
                    &source,
                    Vue3TypeResolutionMode::Import,
                    &bundler,
                ),
                Some(nested_index.clone()),
                "Bundler must ignore the nested manifest for {package_name}"
            );
            assert_eq!(
                resolve_vue3_type_import_with_mode(
                    &importer.to_string_lossy(),
                    &source,
                    Vue3TypeResolutionMode::Import,
                    &bundler_exports_disabled,
                ),
                Some(nested_entry.clone()),
                "disabled exports must restore nested manifest lookup for {package_name}"
            );
        }

        let legacy_package_name = "vuec-root-without-exports";
        let legacy_nested = node_modules.join(legacy_package_name).join("nested");
        std::fs::create_dir_all(&legacy_nested).expect("create legacy nested package");
        std::fs::write(
            node_modules
                .join(legacy_package_name)
                .join("package.json"),
            "{}",
        )
        .expect("write legacy root manifest");
        std::fs::write(
            legacy_nested.join("package.json"),
            r#"{"types":"entry.d.ts"}"#,
        )
        .expect("write legacy nested manifest");
        let legacy_entry = legacy_nested.join("entry.d.ts");
        std::fs::write(&legacy_entry, "export interface NestedProps {}").expect("write legacy entry");
        let node_next = Vue3TypeResolverContext {
            typescript_version: (6, 0, 3).into(),
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_type_import_with_mode(
                &importer.to_string_lossy(),
                "vuec-root-without-exports/nested",
                Vue3TypeResolutionMode::Import,
                &node_next,
            ),
            Some(legacy_entry)
        );
        for resolver in [&bundler, &bundler_exports_disabled, &node_next] {
            assert!(!resolver.external_type_session.metadata_is_blocked());
        }
    }

    #[test]
    fn vue3_dependency_package_imports_resolve_modes_patterns_external_targets_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        write_vue3_bundler_config(dir.path());
        let package = dir.path().join("node_modules").join("vuec-imports-package");
        let deep = package.join("deep");
        let import_types = package.join("types").join("import");
        let require_types = package.join("types").join("require");
        let external = package
            .join("node_modules")
            .join("vuec-imports-external");
        let decoy = deep
            .join("node_modules")
            .join("vuec-imports-external");
        for directory in [&deep, &import_types, &require_types, &external, &decoy] {
            std::fs::create_dir_all(directory).expect("create package imports fixture");
        }
        std::fs::write(
            package.join("package.json"),
            r##"{
                "name":"vuec-imports-package",
                "type":"module",
                "exports":{
                    ".":{"types":"./deep/index.d.mts"},
                    "./commonjs":{"types":"./deep/commonjs.d.cts"}
                },
                "imports":{
                    "#feature/exact":{
                        "types":{
                            "import":"./types/import-exact.d.mts",
                            "require":"./types/require-exact.d.cts"
                        }
                    },
                    "#feature/*":{
                        "types":{
                            "import":"./types/import/*.d.mts",
                            "require":"./types/require/*.d.cts"
                        }
                    },
                    "#external":{"types":"vuec-imports-external"}
                }
            }"##,
        )
        .expect("write package imports manifest");
        let module_entry = deep.join("index.d.mts");
        let commonjs_entry = deep.join("commonjs.d.cts");
        std::fs::write(
            &module_entry,
            r#"
import type { ImportExact } from '#feature/exact'
import type { ImportPattern } from '#feature/item'
import type { ImportExternal } from '#external'
export interface ModuleProps extends ImportExact, ImportPattern, ImportExternal {}
"#,
        )
        .expect("write module imports entry");
        std::fs::write(
            &commonjs_entry,
            r#"
import type { RequireExact } from '#feature/exact'
import type { RequirePattern } from '#feature/item'
import type { RequireExternal } from '#external'
export interface CommonJsProps extends RequireExact, RequirePattern, RequireExternal {}
"#,
        )
        .expect("write CommonJS imports entry");
        let import_exact = package.join("types").join("import-exact.d.mts");
        let require_exact = package.join("types").join("require-exact.d.cts");
        let import_pattern = import_types.join("item.d.mts");
        let require_pattern = require_types.join("item.d.cts");
        for (path, source) in [
            (
                &import_exact,
                "export interface ImportExact { importExact: string }",
            ),
            (
                &require_exact,
                "export interface RequireExact { requireExact: number }",
            ),
            (
                &import_pattern,
                "export interface ImportPattern { importPattern: boolean }",
            ),
            (
                &require_pattern,
                "export interface RequirePattern { requirePattern: string }",
            ),
        ] {
            std::fs::write(path, source).expect("write package imports target");
        }
        std::fs::write(
            import_types.join("exact.d.mts"),
            "export interface ImportExact { wrongPatternExact: never }",
        )
        .expect("write import exact pattern decoy");
        std::fs::write(
            require_types.join("exact.d.cts"),
            "export interface RequireExact { wrongPatternExact: never }",
        )
        .expect("write require exact pattern decoy");
        std::fs::write(
            external.join("package.json"),
            r#"{
                "exports":{
                    ".":{
                        "types":{
                            "import":"./import.d.mts",
                            "require":"./require.d.cts"
                        }
                    }
                }
            }"#,
        )
        .expect("write external target manifest");
        let import_external = external.join("import.d.mts");
        let require_external = external.join("require.d.cts");
        std::fs::write(
            &import_external,
            "export interface ImportExternal { importExternal: number }",
        )
        .expect("write import external target");
        std::fs::write(
            &require_external,
            "export interface RequireExternal { requireExternal: boolean }",
        )
        .expect("write require external target");
        std::fs::write(decoy.join("package.json"), r#"{"types":"index.d.ts"}"#)
            .expect("write deep external decoy manifest");
        std::fs::write(
            decoy.join("index.d.ts"),
            "export interface ImportExternal { wrongDeepExternal: never }\n\
             export interface RequireExternal { wrongDeepExternal: never }",
        )
        .expect("write deep external decoy");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { ModuleProps } from 'vuec-imports-package'
import type { CommonJsProps } from 'vuec-imports-package/commonjs'
defineProps<ModuleProps & CommonJsProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for expected in [
            "importExact: { type: String, required: true }",
            "importPattern: { type: Boolean, required: true }",
            "importExternal: { type: Number, required: true }",
            "requireExact: { type: Number, required: true }",
            "requirePattern: { type: String, required: true }",
            "requireExternal: { type: Boolean, required: true }",
        ] {
            assert!(script.content.contains(expected), "{}", script.content);
        }
        assert!(!script.content.contains("wrong"), "{}", script.content);
        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            module_entry,
            commonjs_entry,
            import_exact,
            require_exact,
            import_pattern,
            require_pattern,
            import_external,
            require_external,
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
    }

    #[test]
    fn vue3_project_package_imports_resolve_direct_source_targets_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        write_vue3_bundler_config(dir.path());
        let source_dir = dir.path().join("src");
        std::fs::create_dir_all(&source_dir).expect("create project source directory");
        std::fs::write(
            dir.path().join("package.json"),
            r##"{
                "imports": {
                    "#project-props": "./src/project-props.ts",
                    "#external-props": "vuec-project-import-external"
                }
            }"##,
        )
        .expect("write project imports manifest");
        let project_props = source_dir.join("project-props.ts");
        std::fs::write(
            &project_props,
            "export interface ProjectProps { projectValue: string }",
        )
        .expect("write project imports target");
        let external_package = dir
            .path()
            .join("node_modules")
            .join("vuec-project-import-external");
        let external_decoy = source_dir
            .join("node_modules")
            .join("vuec-project-import-external");
        for package in [&external_package, &external_decoy] {
            std::fs::create_dir_all(package).expect("create external imports target");
            std::fs::write(package.join("package.json"), r#"{"types":"index.d.ts"}"#)
                .expect("write external imports manifest");
        }
        let external_props = external_package.join("index.d.ts");
        std::fs::write(
            &external_props,
            "export interface ExternalProps { externalValue: number }",
        )
        .expect("write external imports target");
        std::fs::write(
            external_decoy.join("index.d.ts"),
            "export interface ExternalProps { wrongExternalRoot: never }",
        )
        .expect("write external imports decoy");

        let filename = source_dir.join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { ProjectProps } from '#project-props'
import type { ExternalProps } from '#external-props'
defineProps<ProjectProps & ExternalProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("projectValue: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(
            script
                .content
                .contains("externalValue: { type: Number, required: true }"),
            "{}",
            script.content
        );
        assert!(!script.content.contains("wrongExternalRoot"));
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [project_props, external_props]
                .iter()
                .map(|path| normalize_path_string(path))
                .collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn vue3_project_tsconfig_paths_precede_package_maps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        std::fs::create_dir_all(&source_dir).expect("create project source directory");
        std::fs::write(
            dir.path().join("package.json"),
            r##"{
                "name":"vuec-path-priority",
                "imports":{"#choice":"./src/imports-choice.ts"},
                "exports":{"./choice":{"types":"./src/self-choice.ts"}}
            }"##,
        )
        .expect("write project package maps");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r##"{
                "compilerOptions":{
                    "paths":{
                        "#choice":["./src/paths-choice.ts"],
                        "vuec-path-priority/choice":["./src/paths-self-choice.ts"]
                    }
                }
            }"##,
        )
        .expect("write project config");
        let paths_choice = source_dir.join("paths-choice.ts");
        std::fs::write(
            &paths_choice,
            "export interface ChoiceProps { pathsValue: string }",
        )
        .expect("write paths target");
        let paths_self_choice = source_dir.join("paths-self-choice.ts");
        std::fs::write(
            &paths_self_choice,
            "export interface SelfChoiceProps { selfPathsValue: number }",
        )
        .expect("write self-name paths target");
        std::fs::write(
            source_dir.join("imports-choice.ts"),
            "export interface ChoiceProps { wrongImportsPriority: never }",
        )
        .expect("write imports decoy");
        std::fs::write(
            source_dir.join("self-choice.ts"),
            "export interface SelfChoiceProps { wrongSelfPriority: never }",
        )
        .expect("write self-name decoy");

        let filename = source_dir.join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { ChoiceProps } from '#choice'
import type { SelfChoiceProps } from 'vuec-path-priority/choice'
defineProps<ChoiceProps & SelfChoiceProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("pathsValue: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(
            script
                .content
                .contains("selfPathsValue: { type: Number, required: true }"),
            "{}",
            script.content
        );
        assert!(!script.content.contains("wrongImportsPriority"));
        assert!(!script.content.contains("wrongSelfPriority"));
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [paths_choice, paths_self_choice]
                .iter()
                .map(|path| normalize_path_string(path))
                .collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn vue3_project_package_imports_map_emitted_targets_back_to_sources() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        std::fs::create_dir_all(&source_dir).expect("create project source directory");
        std::fs::write(
            dir.path().join("package.json"),
            r##"{
                "imports": {
                    "#javascript": "./dist/javascript.js",
                    "#declaration": "./declarations/declaration.d.ts",
                    "#module": "./dist/module.mjs",
                    "#commonjs": "./dist/commonjs.cjs"
                }
            }"##,
        )
        .expect("write project imports manifest");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "module": "ESNext",
                    "moduleResolution": "Bundler",
                    "rootDir": "./src",
                    "outDir": "./dist",
                    "declarationDir": "./declarations"
                }
            }"#,
        )
        .expect("write project config");
        std::fs::write(
            source_dir.join("javascript.ts"),
            "export interface JavaScriptProps { wrongTsPriority: never }",
        )
        .expect("write lower-priority project source target");
        let targets = [
            (
                source_dir.join("javascript.tsx"),
                "export interface JavaScriptProps { javascriptValue: string }",
            ),
            (
                source_dir.join("declaration.ts"),
                "export interface DeclarationProps { declarationValue: number }",
            ),
            (
                source_dir.join("module.mts"),
                "export interface ModuleProps { moduleValue: boolean }",
            ),
            (
                source_dir.join("commonjs.cts"),
                "export interface CommonJsProps { commonJsValue: string }",
            ),
        ];
        for (path, source) in &targets {
            std::fs::write(path, source).expect("write project source target");
        }

        let filename = source_dir.join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { JavaScriptProps } from '#javascript'
import type { DeclarationProps } from '#declaration'
import type { ModuleProps } from '#module'
import type { CommonJsProps } from '#commonjs'
defineProps<JavaScriptProps & DeclarationProps & ModuleProps & CommonJsProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for expected in [
            "javascriptValue: { type: String, required: true }",
            "declarationValue: { type: Number, required: true }",
            "moduleValue: { type: Boolean, required: true }",
            "commonJsValue: { type: String, required: true }",
        ] {
            assert!(script.content.contains(expected), "{}", script.content);
        }
        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = targets
            .iter()
            .map(|(path, _)| normalize_path_string(path))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.content.contains("wrongTsPriority"), "{}", script.content);
    }

    #[test]
    fn vue3_project_package_imports_inherit_emit_paths_from_their_declaring_configs() {
        let dir = tempfile::tempdir().expect("temp dir");
        let config_dir = dir.path().join("configs");
        let source_dir = dir.path().join("sources");
        std::fs::create_dir_all(&config_dir).expect("create config directory");
        std::fs::create_dir_all(&source_dir).expect("create source directory");
        std::fs::write(
            dir.path().join("package.json"),
            r##"{
                "imports": {
                    "#output": "./dist/output.js",
                    "#declaration": "./declarations/declaration.d.ts"
                }
            }"##,
        )
        .expect("write project imports manifest");
        std::fs::write(
            config_dir.join("base.json"),
            r#"{
                "compilerOptions": {
                    "rootDir": "../sources",
                    "outDir": "../base-dist"
                }
            }"#,
        )
        .expect("write base config");
        std::fs::write(
            config_dir.join("declarations.json"),
            r#"{
                "compilerOptions": {
                    "declarationDir": "../declarations"
                }
            }"#,
        )
        .expect("write declaration config");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "extends": [
                    "./configs/base.json",
                    "./configs/declarations.json"
                ],
                "compilerOptions": {
                    "module": "ESNext",
                    "moduleResolution": "Bundler",
                    "outDir": "./dist"
                }
            }"#,
        )
        .expect("write project config");
        let output = source_dir.join("output.ts");
        let declaration = source_dir.join("declaration.ts");
        std::fs::write(
            &output,
            "export interface OutputProps { outputValue: string }",
        )
        .expect("write inherited output source");
        std::fs::write(
            &declaration,
            "export interface DeclarationProps { declarationValue: number }",
        )
        .expect("write inherited declaration source");

        let filename = source_dir.join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { OutputProps } from '#output'
import type { DeclarationProps } from '#declaration'
defineProps<OutputProps & DeclarationProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("outputValue: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(
            script
                .content
                .contains("declarationValue: { type: Number, required: true }"),
            "{}",
            script.content
        );
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [output, declaration]
                .iter()
                .map(|path| normalize_path_string(path))
                .collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn vue3_project_package_input_candidates_match_typescript_emit_extensions() {
        let path = Path::new("project/dist/entry.d.ts");
        assert_eq!(
            vue3_possible_project_input_paths(path),
            [
                "project/dist/entry.tsx",
                "project/dist/entry.ts",
                "project/dist/entry.jsx",
                "project/dist/entry.js",
            ]
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>()
        );
        assert_eq!(
            vue3_possible_project_input_paths(Path::new("project/dist/entry.d.mts")),
            ["project/dist/entry.mts", "project/dist/entry.mjs"]
                .iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            vue3_possible_project_input_paths(Path::new("project/dist/entry.d.cts")),
            ["project/dist/entry.cts", "project/dist/entry.cjs"]
                .iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        );
        assert!(vue3_possible_project_input_paths(Path::new("project/dist/entry.ts")).is_empty());
    }

    #[test]
    fn vue3_dependency_exports_targets_resolve_only_exact_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        let package_name = "vuec-exact-dependency-exports";
        let package = dir.path().join("node_modules").join(package_name);
        let targets = package.join("targets");
        std::fs::create_dir_all(&source_dir).expect("create source directory");
        std::fs::create_dir_all(&package).expect("create dependency package");
        write_vue3_exact_package_map_targets(&targets);
        std::fs::write(
            package.join("package.json"),
            serde_json::json!({
                "exports": vue3_exact_package_map("./", "targets")
            })
            .to_string(),
        )
        .expect("write dependency exports manifest");
        let importer = source_dir.join("entry.ts");
        std::fs::write(&importer, "export {};").expect("write dependency importer");

        assert_vue3_exact_package_map_targets(
            &importer,
            &format!("{package_name}/"),
            &targets,
        );
    }

    #[test]
    fn vue3_relative_package_import_targets_resolve_only_exact_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        let targets = dir.path().join("import-targets");
        std::fs::create_dir_all(&source_dir).expect("create source directory");
        write_vue3_exact_package_map_targets(&targets);
        std::fs::write(
            dir.path().join("package.json"),
            serde_json::json!({
                "name": "vuec-exact-relative-imports",
                "imports": vue3_exact_package_map("#", "import-targets")
            })
            .to_string(),
        )
        .expect("write package imports manifest");
        let importer = source_dir.join("entry.ts");
        std::fs::write(&importer, "export {};").expect("write package imports importer");

        assert_vue3_exact_package_map_targets(&importer, "#", &targets);
    }

    #[test]
    fn vue3_self_name_export_targets_resolve_only_exact_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        let package_name = "vuec-exact-self-name";
        let targets = dir.path().join("self-targets");
        std::fs::create_dir_all(&source_dir).expect("create source directory");
        write_vue3_exact_package_map_targets(&targets);
        std::fs::write(
            dir.path().join("package.json"),
            serde_json::json!({
                "name": package_name,
                "exports": vue3_exact_package_map("./", "self-targets")
            })
            .to_string(),
        )
        .expect("write self-name exports manifest");
        let importer = source_dir.join("entry.ts");
        std::fs::write(&importer, "export {};").expect("write self-name importer");

        assert_vue3_exact_package_map_targets(
            &importer,
            &format!("{package_name}/"),
            &targets,
        );
    }

    #[test]
    fn vue3_package_relative_targets_normalize_windows_separators() {
        let resolver = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            vue3_package_exports_type_target(
                &serde_json::json!({ "types": "./types\\index.d.ts" }),
                None,
                &resolver,
            )
            .as_deref(),
            Some("./types/index.d.ts")
        );
        assert_eq!(
            vue3_package_exports_type_target(
                &serde_json::json!({
                    "./feature/*": { "types": "./types\\*.d.ts" }
                }),
                Some("feature/item"),
                &resolver,
            )
            .as_deref(),
            Some("./types/item.d.ts")
        );
        assert_eq!(
            vue3_package_exports_type_target(
                &serde_json::json!({ "./legacy/": { "types": "./types\\" } }),
                Some("legacy/item.d.ts"),
                &resolver,
            )
            .as_deref(),
            Some("./types/item.d.ts")
        );
        assert!(vue3_package_exports_type_target(
            &serde_json::json!({ "types": ".\\types\\index.d.ts" }),
            None,
            &resolver,
        )
        .is_none());
        assert!(!vue3_package_import_external_target_is_safe(
            "vuec-external\\feature"
        ));

        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir.path().join("node_modules").join("vuec-backslash-targets");
        let types = package.join("types");
        std::fs::create_dir_all(&types).expect("create package types directory");
        std::fs::write(
            package.join("package.json"),
            r##"{
                "name":"vuec-backslash-targets",
                "exports":{
                    ".":{"types":"./types\\index.d.ts"},
                    "./feature/*":{"types":"./types\\*.d.ts"}
                },
                "imports":{"#local":{"types":"./types\\index.d.ts"}}
            }"##,
        )
        .expect("write backslash target manifest");
        let index = types.join("index.d.ts");
        let item = types.join("item.d.ts");
        let importer = package.join("source.d.mts");
        std::fs::write(&index, "export interface Index {}").expect("write root type target");
        std::fs::write(&item, "export interface Item {}").expect("write pattern type target");
        std::fs::write(&importer, "export {};").expect("write package importer");

        assert_eq!(
            resolve_vue3_package_json_type_entry(&package, None, &resolver),
            Vue3PackageJsonTypeResolution::Resolved(index.clone())
        );
        assert_eq!(
            resolve_vue3_package_json_type_entry(&package, Some("feature/item"), &resolver),
            Vue3PackageJsonTypeResolution::Resolved(item)
        );
        assert_eq!(
            resolve_vue3_type_import(&importer.to_string_lossy(), "#local", &resolver),
            Some(index)
        );
    }

    #[test]
    fn vue3_package_exports_selects_the_most_specific_pattern() {
        let resolver = Vue3TypeResolverContext::default();
        let invalid_array_fallback = serde_json::json!([
            "../invalid.d.ts",
            "./valid.d.ts"
        ]);
        assert_eq!(
            vue3_package_exports_type_target(&invalid_array_fallback, None, &resolver).as_deref(),
            Some("./valid.d.ts")
        );
        let null_array_fallback = serde_json::json!([null, "./valid.d.ts"]);
        assert_eq!(
            vue3_package_exports_type_target(&null_array_fallback, None, &resolver).as_deref(),
            Some("./valid.d.ts")
        );
        assert!(vue3_package_exports_type_target(
            &serde_json::json!([null]),
            None,
            &resolver,
        )
        .is_none());
        assert!(vue3_package_exports_type_target(
            &serde_json::json!([]),
            None,
            &resolver,
        )
        .is_none());
        for conditional_fallback in [
            serde_json::json!({ "types": null, "default": "./valid.d.ts" }),
            serde_json::json!({ "types": [], "default": "./valid.d.ts" }),
            serde_json::json!({ "types": [null, []], "default": "./valid.d.ts" }),
        ] {
            assert_eq!(
                vue3_package_exports_type_target(&conditional_fallback, None, &resolver)
                    .as_deref(),
                Some("./valid.d.ts")
            );
        }
        let nested_conditional_fallback = serde_json::json!({
            "types": { "import": null, "default": "./inner.d.ts" },
            "default": "./outer.d.ts"
        });
        assert_eq!(
            vue3_package_exports_type_target(&nested_conditional_fallback, None, &resolver)
                .as_deref(),
            Some("./inner.d.ts")
        );
        let conditional_array_fallback = serde_json::json!([
            { "node": null, "default": "./inner.d.ts" },
            "./outer.d.ts"
        ]);
        assert_eq!(
            vue3_package_exports_type_target(&conditional_array_fallback, None, &resolver)
                .as_deref(),
            Some("./inner.d.ts")
        );
        assert_eq!(
            vue3_package_exports_type_target(
                &serde_json::json!({ ".": "./literal*.d.ts" }),
                None,
                &resolver,
            )
            .as_deref(),
            Some("./literal*.d.ts")
        );
        let legacy_prefix = serde_json::json!({
            "./legacy/": { "types": "./types/" }
        });
        assert_eq!(
            vue3_package_exports_type_target(
                &legacy_prefix,
                Some("legacy/item.d.ts"),
                &resolver,
            )
            .as_deref(),
            Some("./types/item.d.ts")
        );
        let invalid_legacy_prefix = serde_json::json!({
            "./legacy/": { "types": "./types/index.d.ts" }
        });
        assert!(vue3_package_exports_type_target(
            &invalid_legacy_prefix,
            Some("legacy/item.d.ts"),
            &resolver,
        )
        .is_none());
        let pattern_over_prefix = serde_json::json!({
            "./legacy/": { "types": "./prefix/" },
            "./legacy/*": { "types": "./pattern/*.d.ts" }
        });
        assert_eq!(
            vue3_package_exports_type_target(
                &pattern_over_prefix,
                Some("legacy/item"),
                &resolver,
            )
            .as_deref(),
            Some("./pattern/item.d.ts")
        );

        let exports = serde_json::json!({
            "./feature/*": { "types": "./generic/*.d.ts" },
            "./feature/internal/*": { "types": "./internal/*.d.ts" },
            "./feature/*.js": { "types": "./javascript/*.d.ts" },
            "./feature/exact.js": { "types": "./exact.d.ts" }
        });
        assert_eq!(
            vue3_package_exports_type_target(
                &exports,
                Some("feature/internal/item"),
                &resolver,
            )
            .as_deref(),
            Some("./internal/item.d.ts")
        );
        assert_eq!(
            vue3_package_exports_type_target(&exports, Some("feature/item.js"), &resolver)
                .as_deref(),
            Some("./javascript/item.d.ts")
        );
        assert_eq!(
            vue3_package_exports_type_target(&exports, Some("feature/exact.js"), &resolver)
                .as_deref(),
            Some("./exact.d.ts")
        );

        let exclusions = serde_json::json!({
            "./*": { "types": "./broad/*.d.ts" },
            "./private/*": null,
            "./private/exact": null
        });
        assert!(vue3_package_exports_type_target(
            &exclusions,
            Some("private/item"),
            &resolver,
        )
        .is_none());
        assert!(vue3_package_exports_type_target(
            &exclusions,
            Some("private/exact"),
            &resolver,
        )
        .is_none());

        let invalid_pattern = serde_json::json!({
            "./feature/*": { "types": "./broad/*.d.ts" },
            "./feature/*/*": { "types": "./invalid/*.d.ts" }
        });
        assert_eq!(
            vue3_package_exports_type_target(
                &invalid_pattern,
                Some("feature/one/two"),
                &resolver,
            )
            .as_deref(),
            Some("./broad/one/two.d.ts")
        );
        assert!(vue3_package_export_pattern_capture("./feature/*/*", "./feature/one/two")
            .is_none());
    }

    #[test]
    fn vue3_package_null_targets_follow_typescript_version_fallback_semantics() {
        let legacy = Vue3TypeResolverContext {
            typescript_version: (5, 9, 3).into(),
            ..Vue3TypeResolverContext::default()
        };
        let current = Vue3TypeResolverContext {
            typescript_version: (6, 0, 0).into(),
            ..Vue3TypeResolverContext::default()
        };
        for (target, legacy_target) in [
            (
                serde_json::json!([null, "./array.d.ts"]),
                "./array.d.ts",
            ),
            (
                serde_json::json!({ "types": null, "default": "./condition.d.ts" }),
                "./condition.d.ts",
            ),
            (
                serde_json::json!({
                    "types": [null, "./nested-array.d.ts"],
                    "default": "./outer.d.ts"
                }),
                "./nested-array.d.ts",
            ),
            (
                serde_json::json!({
                    "types": { "import": null, "default": "./inner.d.ts" },
                    "default": "./outer.d.ts"
                }),
                "./inner.d.ts",
            ),
        ] {
            assert_eq!(
                vue3_package_exports_type_target(&target, None, &legacy).as_deref(),
                Some(legacy_target),
                "legacy target: {target}"
            );
            assert!(
                vue3_package_exports_type_target(&target, None, &current).is_none(),
                "TypeScript 6 target: {target}"
            );
        }

        for target in [
            serde_json::json!({ "types": [], "default": "./fallback.d.ts" }),
            serde_json::json!({ "types": true, "default": "./fallback.d.ts" }),
            serde_json::json!(["../invalid.d.ts", "./fallback.d.ts"]),
        ] {
            assert_eq!(
                vue3_package_exports_type_target(&target, None, &current).as_deref(),
                Some("./fallback.d.ts"),
                "TypeScript 6 non-null fallback: {target}"
            );
        }
    }

    #[test]
    fn vue3_package_null_target_resolution_changes_in_typescript_6() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir
            .path()
            .join("node_modules")
            .join("vuec-versioned-null-targets");
        std::fs::create_dir_all(&package).expect("create package");
        std::fs::write(
            package.join("package.json"),
            r##"{
                "name": "vuec-versioned-null-targets",
                "exports": {
                    "./condition-null": { "types": null, "default": "./ok.d.mts" },
                    "./array-null": { "types": [null, "./ok.d.mts"] },
                    "./empty-array": { "types": [], "default": "./ok.d.mts" },
                    "./invalid-target": { "types": true, "default": "./ok.d.mts" }
                },
                "imports": {
                    "#condition-null": { "types": null, "default": "./ok.d.mts" },
                    "#array-null": { "types": [null, "./ok.d.mts"] },
                    "#empty-array": { "types": [], "default": "./ok.d.mts" },
                    "#invalid-target": { "types": true, "default": "./ok.d.mts" }
                }
            }"##,
        )
        .expect("write package manifest");
        let target = package.join("ok.d.mts");
        let importer = package.join("index.d.mts");
        let outside = dir.path().join("outside.d.mts");
        std::fs::write(&target, "export interface Ok { value: string }")
            .expect("write target");
        std::fs::write(&importer, "export {};").expect("write package importer");

        for (version, null_target_resolves) in [((5, 9, 3), true), ((6, 0, 0), false)] {
            let resolver = Vue3TypeResolverContext {
                typescript_version: version.into(),
                module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
                ..Vue3TypeResolverContext::default()
            };
            let expected_null_target = null_target_resolves.then_some(target.clone());
            for subpath in ["condition-null", "array-null"] {
                assert_eq!(
                    resolve_vue3_type_import(
                        &outside.to_string_lossy(),
                        &format!("vuec-versioned-null-targets/{subpath}"),
                        &resolver,
                    ),
                    expected_null_target,
                    "exports target with TypeScript {version:?}: {subpath}"
                );
                assert_eq!(
                    resolve_vue3_type_import(
                        &importer.to_string_lossy(),
                        &format!("#{subpath}"),
                        &resolver,
                    ),
                    expected_null_target,
                    "imports target with TypeScript {version:?}: {subpath}"
                );
            }
            for subpath in ["empty-array", "invalid-target"] {
                assert_eq!(
                    resolve_vue3_type_import(
                        &outside.to_string_lossy(),
                        &format!("vuec-versioned-null-targets/{subpath}"),
                        &resolver,
                    ),
                    Some(target.clone()),
                    "exports fallback with TypeScript {version:?}: {subpath}"
                );
                assert_eq!(
                    resolve_vue3_type_import(
                        &importer.to_string_lossy(),
                        &format!("#{subpath}"),
                        &resolver,
                    ),
                    Some(target.clone()),
                    "imports fallback with TypeScript {version:?}: {subpath}"
                );
            }
            assert!(!resolver.external_type_session.metadata_is_blocked());
        }
    }

    #[test]
    fn vue3_package_exports_follow_typescript_object_shape_fallbacks() {
        let resolver = Vue3TypeResolverContext {
            typescript_version: (6, 0, 0).into(),
            ..Vue3TypeResolverContext::default()
        };
        for mixed_keys in [
            serde_json::json!({
                ".": "./root.d.ts",
                "./feature": "./feature.d.ts",
                "types": "./conditional.d.ts"
            }),
            serde_json::json!({
                "types": "./conditional.d.ts",
                ".": "./root.d.ts",
                "./feature": "./feature.d.ts"
            }),
        ] {
            assert_eq!(
                vue3_package_exports_type_target(&mixed_keys, None, &resolver).as_deref(),
                Some("./root.d.ts")
            );
            assert!(vue3_package_exports_type_target(
                &mixed_keys,
                Some("feature"),
                &resolver,
            )
            .is_none());
        }

        let numeric_condition = serde_json::json!({
            "types": "./valid.d.ts",
            "0": "./invalid.d.ts"
        });
        assert_eq!(
            vue3_package_exports_type_target(&numeric_condition, None, &resolver).as_deref(),
            Some("./valid.d.ts")
        );
        let nested_numeric_condition = serde_json::json!({
            "types": { "0": "./invalid.d.ts" },
            "default": "./fallback.d.ts"
        });
        assert_eq!(
            vue3_package_exports_type_target(&nested_numeric_condition, None, &resolver).as_deref(),
            Some("./fallback.d.ts")
        );
        let numeric_condition_in_array = serde_json::json!([
            { "4294967294": "./invalid.d.ts" },
            "./fallback.d.ts"
        ]);
        assert_eq!(
            vue3_package_exports_type_target(&numeric_condition_in_array, None, &resolver)
                .as_deref(),
            Some("./fallback.d.ts")
        );

        for condition in ["00", "-0", "4294967295", "1e0"] {
            let conditions = serde_json::json!({
                (condition): "./inactive.d.ts",
                "default": "./valid.d.ts"
            });
            assert_eq!(
                vue3_package_exports_type_target(&conditions, None, &resolver).as_deref(),
                Some("./valid.d.ts"),
                "{condition}"
            );
        }

        for conditions in [
            serde_json::json!({
                ".": {
                    "./unknown": "../invalid.d.ts",
                    "types": "./valid.d.ts"
                }
            }),
            serde_json::json!({
                ".": {
                    "types": "./valid.d.ts",
                    "./unknown": "../invalid.d.ts"
                }
            }),
        ] {
            assert_eq!(
                vue3_package_exports_type_target(&conditions, None, &resolver).as_deref(),
                Some("./valid.d.ts")
            );
        }
    }

    #[test]
    fn vue3_package_exports_select_only_the_requested_resolution_mode() {
        let resolver = Vue3TypeResolverContext::default();
        let exports = serde_json::json!({
            ".": {
                "types": {
                    "import": "./import.d.mts",
                    "require": "./require.d.cts"
                }
            },
            "./feature/*": {
                "types": {
                    "import": "./import/*.d.mts",
                    "require": "./require/*.d.cts"
                }
            }
        });

        for (mode, root, pattern) in [
            (
                Vue3TypeResolutionMode::Import,
                "./import.d.mts",
                "./import/item.d.mts",
            ),
            (
                Vue3TypeResolutionMode::Require,
                "./require.d.cts",
                "./require/item.d.cts",
            ),
        ] {
            assert_eq!(
                vue3_package_exports_type_target_with_mode(&exports, None, mode, &resolver)
                    .as_deref(),
                Some(root),
            );
            assert_eq!(
                vue3_package_exports_type_target_with_mode(
                    &exports,
                    Some("feature/item"),
                    mode,
                    &resolver,
                )
                .as_deref(),
                Some(pattern),
            );
        }

        let require_only = serde_json::json!({ ".": { "require": "./require.d.cts" } });
        assert!(vue3_package_exports_type_target(
            &require_only,
            None,
            &resolver,
        )
        .is_none());
    }

    #[test]
    fn vue3_resolution_mode_attributes_drive_imported_and_re_exported_macro_types() {
        let dir = tempfile::tempdir().expect("temp dir");
        write_vue3_bundler_config(dir.path());
        write_vue3_typescript_version(dir.path(), "5.3.0");
        let package = dir
            .path()
            .join("node_modules")
            .join("vuec-resolution-mode-attributes");
        std::fs::create_dir_all(&package).expect("create conditional package");
        std::fs::write(
            package.join("package.json"),
            r#"{
                "exports": {
                    ".": {
                        "types": {
                            "import": "./import.d.mts",
                            "require": "./require.d.cts"
                        }
                    }
                }
            }"#,
        )
        .expect("write conditional package manifest");
        let import_entry = package.join("import.d.mts");
        let require_entry = package.join("require.d.cts");
        std::fs::write(
            &import_entry,
            "export interface CommonJsImportedSource { commonJsImported: string }",
        )
        .expect("write import condition types");
        std::fs::write(
            &require_entry,
            r#"
export interface DirectRequired { directRequired: string }
export interface NamedRequiredSource { namedRequired: number }
export interface AllRequired { allRequired: boolean }
export interface CommonJsDefaultImportedSource { commonJsDefaultImported: number }
export interface CommonJsDefaultImportTypeSource { commonJsDefaultImportType: boolean }
export interface ImportTypeRequired { importTypeRequired: string }
"#,
        )
        .expect("write require condition types");

        let named_bridge = dir.path().join("named-bridge.d.ts");
        let all_bridge = dir.path().join("all-bridge.d.ts");
        let commonjs_bridge = dir.path().join("commonjs-bridge.d.cts");
        let commonjs_default_bridge = dir.path().join("commonjs-default-bridge.d.cts");
        std::fs::write(
            &named_bridge,
            r#"export type { NamedRequiredSource as NamedRequired } from 'vuec-resolution-mode-attributes' with { "resolution-mode": "require" }"#,
        )
        .expect("write named require bridge");
        std::fs::write(
            &all_bridge,
            r#"export type * from 'vuec-resolution-mode-attributes' with { "resolution-mode": "require" }"#,
        )
        .expect("write export-all require bridge");
        std::fs::write(
            &commonjs_bridge,
            r#"export type { CommonJsImportedSource as CommonJsImported } from 'vuec-resolution-mode-attributes' with { "resolution-mode": "import" }"#,
        )
        .expect("write CommonJS import bridge");
        std::fs::write(
            &commonjs_default_bridge,
            r#"
import type { CommonJsDefaultImportedSource } from 'vuec-resolution-mode-attributes'
export type CommonJsDefaultImported = CommonJsDefaultImportedSource
export type CommonJsDefaultImportType = import('vuec-resolution-mode-attributes').CommonJsDefaultImportTypeSource
"#,
        )
        .expect("write CommonJS default-mode bridge");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { DirectRequired } from 'vuec-resolution-mode-attributes' with { "resolution-mode": "require" }
import type { NamedRequired } from './named-bridge'
import type { AllRequired } from './all-bridge'
import type { CommonJsImported } from './commonjs-bridge.d.cts'
import type { CommonJsDefaultImported, CommonJsDefaultImportType } from './commonjs-default-bridge.d.cts'
type ImportTypeRequired = import('vuec-resolution-mode-attributes', { with: { "resolution-mode": `require` } }).ImportTypeRequired
defineProps<DirectRequired & NamedRequired & AllRequired & CommonJsImported & CommonJsDefaultImported & CommonJsDefaultImportType & ImportTypeRequired>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("directRequired: { type: String, required: true }"));
        assert!(script
            .content
            .contains("namedRequired: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("allRequired: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("commonJsImported: { type: String, required: true }"));
        assert!(script
            .content
            .contains("commonJsDefaultImported: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("commonJsDefaultImportType: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("importTypeRequired: { type: String, required: true }"));
        for dependency in [
            import_entry,
            require_entry,
            named_bridge,
            all_bridge,
            commonjs_bridge,
            commonjs_default_bridge,
        ] {
            assert!(
                script.deps.contains(&normalize_path_string(&dependency)),
                "missing dependency {}",
                dependency.display()
            );
        }
    }

    #[test]
    fn vue3_commonjs_bundler_inline_imports_use_require_conditions() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let typescript = node_modules.join("typescript");
        let package = node_modules.join("vuec-inline-commonjs-conditions");
        let dynamic_package = node_modules.join("vuec-inline-commonjs-dynamic");
        std::fs::create_dir_all(&typescript).expect("create TypeScript package");
        std::fs::create_dir_all(&package).expect("create conditional package");
        std::fs::create_dir_all(&dynamic_package).expect("create dynamic conditional package");
        std::fs::write(
            typescript.join("package.json"),
            r#"{"name":"typescript","version":"6.0.3"}"#,
        )
        .expect("write TypeScript package manifest");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "module": "CommonJS",
                    "moduleResolution": "Bundler"
                }
            }"#,
        )
        .expect("write CommonJS Bundler config");
        std::fs::write(
            package.join("package.json"),
            r#"{
                "exports": {
                    ".": {
                        "types": {
                            "import": "./import.d.mts",
                            "require": "./require.d.cts"
                        }
                    }
                }
            }"#,
        )
        .expect("write conditional package manifest");
        std::fs::write(
            dynamic_package.join("package.json"),
            r#"{
                "exports": {
                    ".": {
                        "types": {
                            "import": "./import.d.mts",
                            "require": "./require.d.cts"
                        }
                    }
                }
            }"#,
        )
        .expect("write dynamic conditional package manifest");
        let import_entry = package.join("import.d.mts");
        let require_entry = package.join("require.d.cts");
        let dynamic_import_entry = dynamic_package.join("import.d.mts");
        let dynamic_require_entry = dynamic_package.join("require.d.cts");
        std::fs::write(
            &import_entry,
            r#"
export interface StaticProps { wrongStaticImport: never }
export interface DynamicProps { wrongDynamicImport: never }
export interface NormalStaticProps { wrongNormalStaticImport: never }
export interface NormalDynamicProps { wrongNormalDynamicImport: never }
export interface ExternalVueLeafProps { wrongExternalVueImport: never }
"#,
        )
        .expect("write import condition types");
        std::fs::write(
            &require_entry,
            r#"
export interface StaticProps { staticRequire: string }
export interface DynamicProps { dynamicRequire: number }
export interface NormalStaticProps { normalStaticRequire: boolean }
export interface NormalDynamicProps { normalDynamicRequire: object }
export interface ExternalVueLeafProps { externalVueRequire: string }
"#,
        )
        .expect("write require condition types");
        std::fs::write(
            &dynamic_import_entry,
            "declare global { interface DynamicGlobalProps { wrongRuntimeImport: never } } export {}",
        )
        .expect("write dynamic import condition types");
        std::fs::write(
            &dynamic_require_entry,
            "declare global { interface DynamicGlobalProps { runtimeRequire: boolean } } export {}",
        )
        .expect("write dynamic require condition types");
        let bridge = dir.path().join("Bridge.vue");
        std::fs::write(
            &bridge,
            r#"<script lang="ts">
import type { ExternalVueLeafProps } from 'vuec-inline-commonjs-conditions'
export interface ExternalVueProps extends ExternalVueLeafProps {}
</script>"#,
        )
        .expect("write imported SFC");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script lang="ts">
import type { NormalStaticProps } from 'vuec-inline-commonjs-conditions'
type NormalDynamicProps = import('vuec-inline-commonjs-conditions').NormalDynamicProps
interface NormalProps extends NormalStaticProps, NormalDynamicProps {}
</script>
<script setup lang="ts">
import type { StaticProps } from 'vuec-inline-commonjs-conditions'
import type { ExternalVueProps } from './Bridge.vue'
type DynamicProps = import('vuec-inline-commonjs-conditions').DynamicProps
void import('vuec-inline-commonjs-dynamic')
defineProps<StaticProps & DynamicProps & DynamicGlobalProps & NormalProps & ExternalVueProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("staticRequire: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(
            script
                .content
                .contains("dynamicRequire: { type: Number, required: true }"),
            "{}",
            script.content
        );
        assert!(!script.content.contains("wrongStaticImport"));
        assert!(!script.content.contains("wrongDynamicImport"));
        assert!(
            script
                .content
                .contains("normalStaticRequire: { type: Boolean, required: true }"),
            "{}",
            script.content
        );
        assert!(
            script
                .content
                .contains("normalDynamicRequire: { type: Object, required: true }"),
            "{}",
            script.content
        );
        assert!(!script.content.contains("wrongNormalStaticImport"));
        assert!(!script.content.contains("wrongNormalDynamicImport"));
        assert!(
            script
                .content
                .contains("externalVueRequire: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(!script.content.contains("wrongExternalVueImport"));
        assert!(
            script
                .content
                .contains("runtimeRequire: { type: Boolean, required: true }"),
            "{}",
            script.content
        );
        assert!(!script.content.contains("wrongRuntimeImport"));
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [
                normalize_path_string(&require_entry),
                normalize_path_string(&dynamic_require_entry),
                normalize_path_string(&bridge),
            ]
                .into_iter()
                .collect::<BTreeSet<_>>()
        );
        assert!(!script.deps.contains(&normalize_path_string(&import_entry)));
        assert!(!script
            .deps
            .contains(&normalize_path_string(&dynamic_import_entry)));
    }

    #[test]
    fn vue3_inline_resolution_modes_follow_effective_module_emit() {
        let source_type = oxc_span::SourceType::ts();
        for (module_resolution, module, expected) in [
            (
                Vue3TypeModuleResolutionKind::Bundler,
                Vue3TypeModuleKind::CommonJs,
                (
                    Vue3TypeResolutionMode::Require,
                    Vue3TypeResolutionMode::Require,
                ),
            ),
            (
                Vue3TypeModuleResolutionKind::Bundler,
                Vue3TypeModuleKind::EcmaScript,
                (
                    Vue3TypeResolutionMode::Import,
                    Vue3TypeResolutionMode::Import,
                ),
            ),
            (
                Vue3TypeModuleResolutionKind::Bundler,
                Vue3TypeModuleKind::Preserve,
                (
                    Vue3TypeResolutionMode::Import,
                    Vue3TypeResolutionMode::Import,
                ),
            ),
            (
                Vue3TypeModuleResolutionKind::Node16,
                Vue3TypeModuleKind::Node16,
                (
                    Vue3TypeResolutionMode::Require,
                    Vue3TypeResolutionMode::Import,
                ),
            ),
            (
                Vue3TypeModuleResolutionKind::NodeNext,
                Vue3TypeModuleKind::NodeNext,
                (
                    Vue3TypeResolutionMode::Require,
                    Vue3TypeResolutionMode::Import,
                ),
            ),
        ] {
            let resolver = Vue3TypeResolverContext {
                typescript_version: (6, 0, 3).into(),
                module_resolution,
                module: Some(module),
                ..Vue3TypeResolverContext::default()
            };
            assert_eq!(
                vue3_inline_type_resolution_modes(source_type, &resolver),
                expected,
                "{module_resolution:?} with {module:?}"
            );
        }

        let resolver = Vue3TypeResolverContext {
            typescript_version: (6, 0, 3).into(),
            module_resolution: Vue3TypeModuleResolutionKind::Bundler,
            module: Some(Vue3TypeModuleKind::CommonJs),
            resolve_package_json_exports: Some(false),
            resolve_package_json_imports: Some(false),
            ..Vue3TypeResolverContext::default()
        };
        assert!(resolver.package_json_features().self_name);
        assert_eq!(
            vue3_inline_type_resolution_modes(source_type, &resolver),
            (
                Vue3TypeResolutionMode::Import,
                Vue3TypeResolutionMode::Import,
            )
        );
    }

    #[test]
    fn vue3_bundler_disabled_package_maps_self_references_use_import_conditions() {
        let dir = tempfile::tempdir().expect("temp dir");
        let typescript = dir.path().join("node_modules").join("typescript");
        std::fs::create_dir_all(&typescript).expect("create TypeScript package");
        std::fs::write(
            typescript.join("package.json"),
            r#"{"name":"typescript","version":"6.0.3"}"#,
        )
        .expect("write TypeScript package manifest");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "module": "CommonJS",
                    "moduleResolution": "Bundler",
                    "resolvePackageJsonExports": false,
                    "resolvePackageJsonImports": false
                }
            }"#,
        )
        .expect("write CommonJS Bundler config");
        std::fs::write(
            dir.path().join("package.json"),
            r#"{
                "name": "vuec-self-mode-disabled",
                "exports": {
                    "./feature": {
                        "types": {
                            "import": "./import.d.mts",
                            "require": "./require.d.cts"
                        }
                    }
                }
            }"#,
        )
        .expect("write project package manifest");
        let import_entry = dir.path().join("import.d.mts");
        let require_entry = dir.path().join("require.d.cts");
        std::fs::write(
            &import_entry,
            "export interface SelfProps { selfImport: string }",
        )
        .expect("write import self-reference target");
        std::fs::write(
            &require_entry,
            "export interface SelfProps { wrongSelfRequire: never }",
        )
        .expect("write require self-reference decoy");

        let source_dir = dir.path().join("src");
        std::fs::create_dir_all(&source_dir).expect("create source directory");
        let filename = source_dir.join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { SelfProps } from 'vuec-self-mode-disabled/feature'
defineProps<SelfProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("selfImport: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(!script.content.contains("wrongSelfRequire"));
        assert_eq!(script.deps, vec![normalize_path_string(&import_entry)]);
        assert!(!script.deps.contains(&normalize_path_string(&require_entry)));
    }

    #[test]
    fn vue3_package_type_drives_transitive_resolution_modes() {
        let dir = tempfile::tempdir().expect("temp dir");
        write_vue3_node_next_config(dir.path());
        let node_modules = dir.path().join("node_modules");
        let conditional = node_modules.join("vuec-package-type-conditional");
        std::fs::create_dir_all(&conditional).expect("create conditional package");
        std::fs::write(
            conditional.join("package.json"),
            r#"{
                "exports": {
                    ".": {
                        "types": {
                            "import": "./import.d.mts",
                            "require": "./require.d.cts"
                        }
                    }
                }
            }"#,
        )
        .expect("write conditional package manifest");
        let import_entry = conditional.join("import.d.mts");
        let require_entry = conditional.join("require.d.cts");
        std::fs::write(
            &import_entry,
            r#"
export interface ImportDirect { importDirect: string }
export interface ImportType { importType: number }
export interface ImportNamed { importNamed: boolean }
export interface ImportAll { importAll: bigint }
export interface ImportExplicit { importExplicit: symbol }
export interface ImportBoundary { importBoundary: object }
export interface ImportGeneric { importGeneric: string }
export interface WrongRequireDirect { wrongRequireDirect: string }
"#,
        )
        .expect("write import condition types");
        std::fs::write(
            &require_entry,
            r#"
export interface RequireDirect { requireDirect: string }
export interface RequireType { requireType: number }
export interface RequireNamed { requireNamed: boolean }
export interface RequireAll { requireAll: bigint }
export interface RequireExplicit { requireExplicit: symbol }
export interface RequireBoundary { requireBoundary: object }
export interface RequireGeneric { requireGeneric: string }
export interface WrongImportDirect { wrongImportDirect: string }
"#,
        )
        .expect("write require condition types");

        let commonjs_bridge = node_modules.join("vuec-commonjs-type-bridge");
        std::fs::create_dir_all(&commonjs_bridge).expect("create CommonJS bridge");
        std::fs::write(
            commonjs_bridge.join("package.json"),
            r#"{"type":"commonjs","types":"index.d.ts"}"#,
        )
        .expect("write CommonJS bridge manifest");
        std::fs::write(
            commonjs_bridge.join("index.d.ts"),
            r#"
import type { RequireDirect } from 'vuec-package-type-conditional'
export interface CommonJsDirect extends RequireDirect {}
export type CommonJsImportType = import('vuec-package-type-conditional').RequireType
export type CommonJsGeneric<T> = T & import('vuec-package-type-conditional').RequireGeneric
export type { RequireNamed as CommonJsNamed } from 'vuec-package-type-conditional'
export * from 'vuec-package-type-conditional'
"#,
        )
        .expect("write CommonJS bridge types");

        let module_bridge = node_modules.join("vuec-module-type-bridge");
        std::fs::create_dir_all(&module_bridge).expect("create module bridge");
        std::fs::write(
            module_bridge.join("package.json"),
            r#"{"type":"module","types":"index.d.ts"}"#,
        )
        .expect("write module bridge manifest");
        std::fs::write(
            module_bridge.join("index.d.ts"),
            r#"
import type { ImportDirect } from 'vuec-package-type-conditional'
export interface ModuleDirect extends ImportDirect {}
export type ModuleImportType = import('vuec-package-type-conditional').ImportType
export type { ImportNamed as ModuleNamed } from 'vuec-package-type-conditional'
export * from 'vuec-package-type-conditional'
"#,
        )
        .expect("write module bridge types");

        let explicit_commonjs = node_modules.join("vuec-explicit-commonjs-bridge");
        std::fs::create_dir_all(&explicit_commonjs).expect("create explicit CommonJS bridge");
        std::fs::write(
            explicit_commonjs.join("package.json"),
            r#"{"type":"module","types":"index.d.cts"}"#,
        )
        .expect("write explicit CommonJS bridge manifest");
        std::fs::write(
            explicit_commonjs.join("index.d.cts"),
            "export type ExplicitCommonJs = import('vuec-package-type-conditional').RequireExplicit",
        )
        .expect("write explicit CommonJS bridge types");

        let explicit_module = node_modules.join("vuec-explicit-module-bridge");
        std::fs::create_dir_all(&explicit_module).expect("create explicit module bridge");
        std::fs::write(
            explicit_module.join("package.json"),
            r#"{"type":"commonjs","types":"index.d.mts"}"#,
        )
        .expect("write explicit module bridge manifest");
        std::fs::write(
            explicit_module.join("index.d.mts"),
            "export type ExplicitModule = import('vuec-package-type-conditional').ImportExplicit",
        )
        .expect("write explicit module bridge types");

        let nested_boundary = node_modules.join("vuec-nested-package-boundary");
        let nested = nested_boundary.join("nested");
        std::fs::create_dir_all(&nested).expect("create nested package boundary");
        std::fs::write(
            nested_boundary.join("package.json"),
            r#"{"type":"module","types":"nested/index.d.ts"}"#,
        )
        .expect("write outer module package manifest");
        std::fs::write(nested.join("package.json"), "{}")
            .expect("write empty nested package manifest");
        std::fs::write(
            nested.join("index.d.ts"),
            "export type BoundaryType = import('vuec-package-type-conditional').RequireBoundary",
        )
        .expect("write nested boundary types");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { CommonJsDirect, CommonJsGeneric, CommonJsImportType, CommonJsNamed, RequireAll } from 'vuec-commonjs-type-bridge'
import type { ModuleDirect, ModuleImportType, ModuleNamed, ImportAll } from 'vuec-module-type-bridge'
import type { ExplicitCommonJs } from 'vuec-explicit-commonjs-bridge'
import type { ExplicitModule } from 'vuec-explicit-module-bridge'
import type { BoundaryType } from 'vuec-nested-package-boundary'
defineProps<CommonJsDirect & CommonJsGeneric<{ genericLocal: number }> & CommonJsImportType & CommonJsNamed & RequireAll & ModuleDirect & ModuleImportType & ModuleNamed & ImportAll & ExplicitCommonJs & ExplicitModule & BoundaryType>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for property in [
            "requireDirect",
            "requireType",
            "requireNamed",
            "requireAll",
            "importDirect",
            "importType",
            "importNamed",
            "importAll",
            "requireExplicit",
            "requireGeneric",
            "genericLocal",
            "importExplicit",
            "requireBoundary",
        ] {
            assert!(
                script.content.contains(&format!("{property}: {{ type:")),
                "missing {property}: {}",
                script.content
            );
        }
        assert!(!script.content.contains("wrongRequireDirect:"));
        assert!(!script.content.contains("wrongImportDirect:"));
        assert!(!script.content.contains("importGeneric:"));
        assert!(!script.content.contains("importBoundary:"));
        for dependency in [
            import_entry,
            require_entry,
            commonjs_bridge.join("index.d.ts"),
            module_bridge.join("index.d.ts"),
            explicit_commonjs.join("index.d.cts"),
            explicit_module.join("index.d.mts"),
            nested.join("index.d.ts"),
        ] {
            assert!(
                script.deps.contains(&normalize_path_string(&dependency)),
                "missing dependency {}",
                dependency.display()
            );
        }
    }

    #[test]
    fn vue3_package_types_version_selector_supports_node_semver_ranges() {
        for selector in [
            "*",
            "<=5.0",
            "~5.0",
            "^4.8 || >=5.0",
            "5.0 - 5.9",
            ">=4.8 <5.3",
            "5.x",
            "5.*",
        ] {
            assert!(
                vue3_package_types_version_selector_matches(selector),
                "{selector}"
            );
        }

        for selector in ["", ">=5.1", "<5.0", "4.x", "4.*", "5.1 - 5.9"] {
            assert!(
                !vue3_package_types_version_selector_matches(selector),
                "{selector}"
            );
        }
    }

    #[test]
    fn vue3_package_types_versions_targets_accept_windows_separators_cross_platform() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir
            .path()
            .join("node_modules")
            .join("vuec-typesversions-windows-paths");
        let target = package.join("versioned").join("feature").join("item.d.ts");
        std::fs::create_dir_all(target.parent().expect("target parent"))
            .expect("create versioned package directory");
        std::fs::write(
            package.join("package.json"),
            r#"{
                "types": "index.d.ts",
                "typesVersions": {
                    "*": {
                        "feature/*": ["versioned\\feature\\*.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write versioned package manifest");
        std::fs::write(
            package.join("index.d.ts"),
            "export interface VersionedProps { wrongFallback: never }",
        )
        .expect("write versioned fallback types");
        std::fs::write(
            &target,
            "export interface VersionedProps { windowsTarget: string }",
        )
        .expect("write versioned Windows target");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { VersionedProps } from 'vuec-typesversions-windows-paths/feature/item'
defineProps<VersionedProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("windowsTarget: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(!script.content.contains("wrongFallback"));
        assert_eq!(script.deps, vec![normalize_path_string(&target)]);
    }

    #[test]
    fn vue3_package_root_fields_normalize_windows_separators_before_version_matching() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let mut expected_deps = Vec::new();
        for (package_name, manifest, relative_target, declaration) in [
            (
                "vuec-windows-types-field",
                r#"{"types":".\\declarations\\index.d.ts"}"#,
                "declarations/index.d.ts",
                "export interface TypesFieldProps { typesField: string }",
            ),
            (
                "vuec-windows-typings-field",
                r#"{"typings":".\\declarations\\index.d.ts"}"#,
                "declarations/index.d.ts",
                "export interface TypingsFieldProps { typingsField: number }",
            ),
            (
                "vuec-windows-main-field",
                r#"{"main":".\\dist\\index.js"}"#,
                "dist/index.d.ts",
                "export interface MainFieldProps { mainField: boolean }",
            ),
        ] {
            let package = node_modules.join(package_name);
            let target = package.join(relative_target);
            std::fs::create_dir_all(target.parent().expect("package target parent"))
                .expect("create package root field fixture");
            std::fs::write(package.join("package.json"), manifest)
                .expect("write package root field manifest");
            std::fs::write(&target, declaration).expect("write package root field declaration");
            expected_deps.push(target);
        }

        let versioned_package = node_modules.join("vuec-windows-versioned-root");
        let fallback = versioned_package.join("types").join("index.d.ts");
        let versioned = versioned_package.join("versioned").join("index.d.ts");
        for target in [&fallback, &versioned] {
            std::fs::create_dir_all(target.parent().expect("versioned target parent"))
                .expect("create versioned root fixture");
        }
        std::fs::write(
            versioned_package.join("package.json"),
            r#"{
                "types": ".\\types\\index.d.ts",
                "typesVersions": {
                    "*": { "types/*": ["versioned\\*"] }
                }
            }"#,
        )
        .expect("write versioned root manifest");
        std::fs::write(
            &fallback,
            "export interface VersionedRootProps { wrongFallback: never }",
        )
        .expect("write versioned root fallback");
        std::fs::write(
            &versioned,
            "export interface VersionedRootProps { versionedRoot?: string }",
        )
        .expect("write versioned root target");
        expected_deps.push(versioned);

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { TypesFieldProps } from 'vuec-windows-types-field'
import type { TypingsFieldProps } from 'vuec-windows-typings-field'
import type { MainFieldProps } from 'vuec-windows-main-field'
import type { VersionedRootProps } from 'vuec-windows-versioned-root'
defineProps<TypesFieldProps & TypingsFieldProps & MainFieldProps & VersionedRootProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for expected in [
            "typesField: { type: String, required: true }",
            "typingsField: { type: Number, required: true }",
            "mainField: { type: Boolean, required: true }",
            "versionedRoot: { type: String, required: false }",
        ] {
            assert!(script.content.contains(expected), "{}", script.content);
        }
        assert!(!script.content.contains("wrongFallback"));
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            expected_deps
                .iter()
                .map(|path| normalize_path_string(path))
                .collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn vue3_compile_script_resolves_package_types_versions_type_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let versioned_pkg = node_modules.join("vuec-typesversions-pkg");
        std::fs::create_dir_all(versioned_pkg.join("dist")).expect("create dist types");
        std::fs::create_dir_all(versioned_pkg.join("future").join("feature"))
            .expect("create future types");
        std::fs::create_dir_all(versioned_pkg.join("ts5").join("feature"))
            .expect("create ts5 types");
        std::fs::create_dir_all(versioned_pkg.join("legacy").join("feature"))
            .expect("create legacy types");
        std::fs::write(
            versioned_pkg.join("package.json"),
            r#"{
                "types": "dist/index.d.ts",
                "typesVersions": {
                    ">=5.1": {
                        "dist/index.d.ts": ["future/index.d.ts"],
                        "feature/*": ["future/feature/*.d.ts"]
                    },
                    "^4.8 || 5.x": {
                        "dist/index.d.ts": ["ts5/index.d.ts"],
                        "feature/*": ["ts5/feature/*.d.ts"]
                    },
                    "*": {
                        "dist/index.d.ts": ["legacy/index.d.ts"],
                        "feature/*": ["legacy/feature/*.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write versioned package manifest");
        std::fs::write(
            versioned_pkg.join("dist").join("index.d.ts"),
            "export interface RootProps { fallbackRoot: string }",
        )
        .expect("write fallback root types");
        std::fs::write(
            versioned_pkg.join("legacy").join("index.d.ts"),
            "export interface RootProps { legacyRoot: string }",
        )
        .expect("write legacy root types");
        std::fs::write(
            versioned_pkg
                .join("legacy")
                .join("feature")
                .join("item.d.ts"),
            "export type FeatureProps = { legacyFeature: string }",
        )
        .expect("write legacy feature types");
        std::fs::write(
            versioned_pkg.join("future").join("index.d.ts"),
            "export interface RootProps { futureRoot: string }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write future root types");
        std::fs::write(
            versioned_pkg
                .join("future")
                .join("feature")
                .join("item.d.ts"),
            "export type FeatureProps = { futureFeature: string }",
        )
        .expect("write future feature types");
        std::fs::write(
            versioned_pkg.join("future").join("model.d.ts"),
            "export type ModelValue = number",
        )
        .expect("write future model types");
        std::fs::write(
            versioned_pkg.join("ts5").join("index.d.ts"),
            "export interface RootProps { root: string }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write ts5 root types");
        std::fs::write(
            versioned_pkg.join("ts5").join("feature").join("item.d.ts"),
            "export type FeatureProps = { feature?: number }",
        )
        .expect("write ts5 feature types");
        std::fs::write(
            versioned_pkg.join("ts5").join("model.d.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write ts5 model types");

        let ambient_pkg = node_modules
            .join("@types")
            .join("vuec-typesversions-ambient");
        std::fs::create_dir_all(ambient_pkg.join("ts5")).expect("create @types versioned");
        std::fs::write(
            ambient_pkg.join("package.json"),
            r#"{
                "types": "index.d.ts",
                "typesVersions": {
                    "~5.0": {
                        "index.d.ts": ["ts5/index.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write @types package manifest");
        std::fs::write(
            ambient_pkg.join("index.d.ts"),
            "export type AmbientProps = { ambientFallback: number }",
        )
        .expect("write fallback @types");
        std::fs::write(
            ambient_pkg.join("ts5").join("index.d.ts"),
            "export type AmbientProps = { ambient: boolean }",
        )
        .expect("write ts5 @types");

        let type_root_pkg = dir.path().join("typings").join("versioned-global");
        std::fs::create_dir_all(type_root_pkg.join("ts5")).expect("create type root package");
        std::fs::write(
            type_root_pkg.join("package.json"),
            r#"{
                "types": "index.d.ts",
                "typesVersions": {
                    "5.0 - 5.9": {
                        "index.d.ts": ["ts5/index.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write type root package manifest");
        std::fs::write(
            type_root_pkg.join("index.d.ts"),
            "declare interface TypeRootGlobalProps { typeRootFallback: number }",
        )
        .expect("write fallback type root global");
        std::fs::write(
            type_root_pkg.join("ts5").join("index.d.ts"),
            "declare interface TypeRootGlobalProps { typeRoot: string }",
        )
        .expect("write ts5 type root global");

        let ordered_pkg = node_modules.join("vuec-typesversions-ordered");
        std::fs::create_dir_all(ordered_pkg.join("first")).expect("create first ordered types");
        std::fs::create_dir_all(ordered_pkg.join("second")).expect("create second ordered types");
        std::fs::create_dir_all(ordered_pkg.join("fallback"))
            .expect("create fallback ordered types");
        std::fs::write(
            ordered_pkg.join("package.json"),
            r#"{
                "types": "index.d.ts",
                "typesVersions": {
                    ">=4.8": {
                        "index.d.ts": ["first/index.d.ts"]
                    },
                    ">=5.0": {
                        "index.d.ts": ["second/index.d.ts"]
                    },
                    "*": {
                        "index.d.ts": ["fallback/index.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write ordered package manifest");
        std::fs::write(
            ordered_pkg.join("index.d.ts"),
            "export type OrderedProps = { orderedFallbackRoot: boolean }",
        )
        .expect("write ordered root fallback");
        std::fs::write(
            ordered_pkg.join("first").join("index.d.ts"),
            "export type OrderedProps = { orderedFirst: string }",
        )
        .expect("write first ordered types");
        std::fs::write(
            ordered_pkg.join("second").join("index.d.ts"),
            "export type OrderedProps = { orderedSecond: number }",
        )
        .expect("write second ordered types");
        std::fs::write(
            ordered_pkg.join("fallback").join("index.d.ts"),
            "export type OrderedProps = { orderedFallback: boolean }",
        )
        .expect("write fallback ordered types");

        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "files": [],
                "compilerOptions": {
                    "types": ["versioned-global"],
                    "typeRoots": ["./typings"]
                }
            }"#,
        )
        .expect("write tsconfig");

        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { RootProps } from 'vuec-typesversions-pkg'
import type { FeatureProps } from 'vuec-typesversions-pkg/feature/item'
import type { AmbientProps } from 'vuec-typesversions-ambient'
import type { OrderedProps } from 'vuec-typesversions-ordered'
defineProps<RootProps & FeatureProps & AmbientProps & TypeRootGlobalProps & OrderedProps>()
defineModel<import('vuec-typesversions-pkg').ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("root: { type: String, required: true }"));
        assert!(script
            .content
            .contains("feature: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("ambient: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("typeRoot: { type: String, required: true }"));
        assert!(script
            .content
            .contains("orderedFirst: { type: String, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));
        assert!(!script.content.contains("fallbackRoot"));
        assert!(!script.content.contains("futureRoot"));
        assert!(!script.content.contains("futureFeature"));
        assert!(!script.content.contains("legacyRoot"));
        assert!(!script.content.contains("legacyFeature"));
        assert!(!script.content.contains("ambientFallback"));
        assert!(!script.content.contains("typeRootFallback"));
        assert!(!script.content.contains("orderedSecond"));
        assert!(!script.content.contains("orderedFallback"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            versioned_pkg.join("ts5").join("index.d.ts"),
            versioned_pkg.join("ts5").join("feature").join("item.d.ts"),
            versioned_pkg.join("ts5").join("model.d.ts"),
            ambient_pkg.join("ts5").join("index.d.ts"),
            type_root_pkg.join("ts5").join("index.d.ts"),
            ordered_pkg.join("first").join("index.d.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_package_types_versions_from_project_typescript() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let typescript_pkg = node_modules.join("typescript");
        std::fs::create_dir_all(&typescript_pkg).expect("create typescript package");
        std::fs::write(
            typescript_pkg.join("package.json"),
            r#"{"version":"5.2.0"}"#,
        )
        .expect("write typescript manifest");

        let versioned_pkg = node_modules.join("vuec-typesversions-project-ts");
        std::fs::create_dir_all(versioned_pkg.join("dist")).expect("create dist types");
        std::fs::create_dir_all(versioned_pkg.join("ts52").join("feature"))
            .expect("create ts52 types");
        std::fs::create_dir_all(versioned_pkg.join("ts50").join("feature"))
            .expect("create ts50 types");
        std::fs::create_dir_all(versioned_pkg.join("legacy").join("feature"))
            .expect("create legacy types");
        std::fs::write(
            versioned_pkg.join("package.json"),
            r#"{
                "types": "dist/index.d.ts",
                "typesVersions": {
                    ">=5.1": {
                        "dist/index.d.ts": ["ts52/index.d.ts"],
                        "feature/*": ["ts52/feature/*.d.ts"]
                    },
                    ">=5.0": {
                        "dist/index.d.ts": ["ts50/index.d.ts"],
                        "feature/*": ["ts50/feature/*.d.ts"]
                    },
                    "*": {
                        "dist/index.d.ts": ["legacy/index.d.ts"],
                        "feature/*": ["legacy/feature/*.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write versioned package manifest");
        std::fs::write(
            versioned_pkg.join("dist").join("index.d.ts"),
            "export interface Props { fallbackRoot: string }",
        )
        .expect("write dist fallback types");
        std::fs::write(
            versioned_pkg.join("legacy").join("index.d.ts"),
            "export interface Props { legacyRoot: string }",
        )
        .expect("write legacy root types");
        std::fs::write(
            versioned_pkg
                .join("legacy")
                .join("feature")
                .join("item.d.ts"),
            "export type FeatureProps = { legacyFeature: string }",
        )
        .expect("write legacy feature types");
        std::fs::write(
            versioned_pkg.join("ts50").join("index.d.ts"),
            "export interface Props { baselineRoot: string }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write ts50 root types");
        std::fs::write(
            versioned_pkg.join("ts50").join("feature").join("item.d.ts"),
            "export type FeatureProps = { baselineFeature: boolean }",
        )
        .expect("write ts50 feature types");
        std::fs::write(
            versioned_pkg.join("ts50").join("model.d.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write ts50 model types");
        std::fs::write(
            versioned_pkg.join("ts52").join("index.d.ts"),
            "export interface Props { futureRoot: string }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write ts52 root types");
        std::fs::write(
            versioned_pkg.join("ts52").join("feature").join("item.d.ts"),
            "export type FeatureProps = { futureFeature?: number }",
        )
        .expect("write ts52 feature types");
        std::fs::write(
            versioned_pkg.join("ts52").join("model.d.ts"),
            "export type ModelValue = number",
        )
        .expect("write ts52 model types");

        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { Props } from 'vuec-typesversions-project-ts'
import type { FeatureProps } from 'vuec-typesversions-project-ts/feature/item'
defineProps<Props & FeatureProps>()
defineModel<import('vuec-typesversions-project-ts').ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("futureRoot: { type: String, required: true }"));
        assert!(script
            .content
            .contains("futureFeature: { type: Number, required: false }"));
        assert!(script.content.contains("\"modelValue\": { type: Number },"));
        assert!(!script.content.contains("baselineRoot"));
        assert!(!script.content.contains("baselineFeature"));
        assert!(!script.content.contains("legacyRoot"));
        assert!(!script.content.contains("legacyFeature"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            versioned_pkg.join("ts52").join("index.d.ts"),
            versioned_pkg.join("ts52").join("feature").join("item.d.ts"),
            versioned_pkg.join("ts52").join("model.d.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }
