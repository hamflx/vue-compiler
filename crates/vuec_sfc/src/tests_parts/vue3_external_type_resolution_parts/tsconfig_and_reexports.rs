    #[test]
    fn vue3_compile_script_resolves_tsconfig_path_macro_types_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let types_pkg = node_modules.join("vuec-tsconfig-pkg");
        std::fs::create_dir_all(&types_pkg).expect("create package");
        std::fs::write(types_pkg.join("package.json"), r#"{"types":"index.d.ts"}"#)
            .expect("write package manifest");
        std::fs::write(
            types_pkg.join("index.d.ts"),
            "export type PackageProps = { packaged: boolean }",
        )
        .expect("write package types");

        std::fs::create_dir_all(dir.path().join("web")).expect("create web dir");
        std::fs::create_dir_all(dir.path().join("empty")).expect("create empty dir");
        std::fs::create_dir_all(dir.path().join("tsconfigs")).expect("create tsconfigs dir");
        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        std::fs::create_dir_all(dir.path().join("src").join("views")).expect("create views dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "files": [],
                "compilerOptions": {
                    "paths": {
                        "bar": ["./pp.ts"]
                    }
                },
                "references": [
                    { "path": "./tsconfig.app.json" },
                    { "path": "./web" },
                    { "path": "./empty" },
                    { "path": "./noexists-should-ignore" }
                ]
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.path().join("tsconfig.app.json"),
            r#"{
                "include": ["**/*.ts", "**/*.vue"],
                "extends": ["./tsconfigs/base.json"]
            }"#,
        )
        .expect("write app tsconfig");
        std::fs::write(
            dir.path().join("tsconfigs").join("base.json"),
            r#"{
                "compilerOptions": {
                    "paths": {
                        "@/*": ["${configDir}/src/*"]
                    }
                },
                "include": ["${configDir}/src/**/*.ts", "${configDir}/src/**/*.vue"]
            }"#,
        )
        .expect("write base tsconfig");
        std::fs::write(
            dir.path().join("web").join("tsconfig.json"),
            r#"{
                "include": ["../**/*.ts", "../**/*.vue"],
                "compilerOptions": {
                    "composite": true,
                    "paths": {
                        "user": ["../user.ts"]
                    }
                },
                "references": [
                    { "path": "../tsconfig.json" }
                ]
            }"#,
        )
        .expect("write web tsconfig");
        std::fs::write(
            dir.path().join("empty").join("tsconfig.json"),
            r#"{"compilerOptions":{"composite":true}}"#,
        )
        .expect("write empty tsconfig");
        std::fs::write(
            dir.path().join("pp.ts"),
            "export type PathProps = { bar: string }",
        )
        .expect("write root path type");
        std::fs::write(
            dir.path().join("user.ts"),
            "export type UserProps = { user: string }",
        )
        .expect("write referenced type");
        std::fs::write(
            dir.path().join("src").join("types.ts"),
            "export type BaseProps = { foo?: string; count: number }",
        )
        .expect("write configDir type");
        std::fs::write(
            dir.path().join("src").join("views").join("Aliased.vue"),
            "<script lang=\"ts\">export type VueProps = { fromVue: string }</script>",
        )
        .expect("write aliased vue");

        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { PackageProps } from 'vuec-tsconfig-pkg'
import type { PathProps } from 'bar'
import type { UserProps } from 'user'
import type { BaseProps } from '@/types.ts'
import type { VueProps } from '@/views/Aliased.vue'
const props = defineProps<PackageProps & PathProps & UserProps & BaseProps & VueProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for expected_prop in [
            "packaged: { type: Boolean, required: true }",
            "bar: { type: String, required: true }",
            "user: { type: String, required: true }",
            "foo: { type: String, required: false }",
            "count: { type: Number, required: true }",
            "fromVue: { type: String, required: true }",
        ] {
            assert!(script.content.contains(expected_prop), "{}", script.content);
        }

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            types_pkg.join("index.d.ts"),
            dir.path().join("pp.ts"),
            dir.path().join("user.ts"),
            dir.path().join("src").join("types.ts"),
            dir.path().join("src").join("views").join("Aliased.vue"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_tsconfig_jsonc_paths_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        std::fs::create_dir_all(dir.path().join("src").join("base")).expect("create base dir");
        std::fs::create_dir_all(dir.path().join("config")).expect("create config dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                // Root path mapping.
                "compilerOptions": {
                    "paths": {
                        "root-alias": ["./root.ts",],
                    },
                },
                "references": [
                    { "path": "./tsconfig.app.json", },
                ],
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.path().join("tsconfig.app.json"),
            r#"{
                "extends": [
                    "./config/base.json", // inherited alias
                ],
                "compilerOptions": {
                    "paths": {
                        "app-alias": ["./app.ts",],
                    },
                },
            }"#,
        )
        .expect("write app tsconfig");
        std::fs::write(
            dir.path().join("config").join("base.json"),
            r#"{
                /* ${configDir} should still resolve from the referencing config. */
                "compilerOptions": {
                    "paths": {
                        "@base/*": ["${configDir}/src/base/*",],
                    },
                },
            }"#,
        )
        .expect("write base tsconfig");
        std::fs::write(
            dir.path().join("root.ts"),
            "export type RootProps = { root: string }",
        )
        .expect("write root type");
        std::fs::write(
            dir.path().join("app.ts"),
            "export type AppProps = { app?: number }",
        )
        .expect("write app type");
        std::fs::write(
            dir.path().join("src").join("base").join("types.ts"),
            "export type BaseProps = { base: boolean }",
        )
        .expect("write base type");

        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { RootProps } from 'root-alias'
import type { AppProps } from 'app-alias'
import type { BaseProps } from '@base/types'
defineProps<RootProps & AppProps & BaseProps>()
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
            .contains("app: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("base: { type: Boolean, required: true }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            dir.path().join("root.ts"),
            dir.path().join("app.ts"),
            dir.path().join("src").join("base").join("types.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_package_tsconfig_extends_paths_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        let scoped_config_pkg = dir
            .path()
            .join("node_modules")
            .join("@vuec")
            .join("tsconfig");
        std::fs::create_dir_all(&scoped_config_pkg).expect("create scoped config package");
        std::fs::write(
            scoped_config_pkg.join("package.json"),
            r#"{"tsconfig":"base.json"}"#,
        )
        .expect("write scoped config package manifest");
        std::fs::write(
            scoped_config_pkg.join("base.json"),
            r#"{
                // Package config entries may be JSONC.
                "compilerOptions": {
                    "paths": {
                        "pkg-root": ["${configDir}/root.ts",],
                    },
                },
            }"#,
        )
        .expect("write scoped package config");

        let preset_pkg = dir.path().join("node_modules").join("vuec-tsconfig-preset");
        std::fs::create_dir_all(&preset_pkg).expect("create preset package");
        std::fs::write(
            preset_pkg.join("shared.json"),
            r#"{
                "compilerOptions": {
                    "paths": {
                        "pkg-shared": ["${configDir}/shared.ts"]
                    }
                }
            }"#,
        )
        .expect("write preset subpath config");

        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "extends": ["@vuec/tsconfig", "vuec-tsconfig-preset/shared"],
                "compilerOptions": {
                    "paths": {
                        "local-alias": ["./local.ts"]
                    }
                }
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.path().join("root.ts"),
            "export type RootProps = { root: string }",
        )
        .expect("write root type");
        std::fs::write(
            dir.path().join("shared.ts"),
            "export type SharedProps = { shared?: number }",
        )
        .expect("write shared type");
        std::fs::write(
            dir.path().join("local.ts"),
            "export type LocalProps = { local: boolean }",
        )
        .expect("write local type");

        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { RootProps } from 'pkg-root'
import type { SharedProps } from 'pkg-shared'
import type { LocalProps } from 'local-alias'
defineProps<RootProps & SharedProps & LocalProps>()
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
            .contains("shared: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("local: { type: Boolean, required: true }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            dir.path().join("root.ts"),
            dir.path().join("shared.ts"),
            dir.path().join("local.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_tsconfig_jsonc_preserves_string_literal_contents() {
        let value = vue3_parse_tsconfig_jsonc(
            r#"{
                "compilerOptions": {
                    "baseUrl": "./src,not-trailing",
                    "paths": {
                        "url/*": [
                            "./literal//slash/*",
                            "./literal/*block*/segment/*",
                        ],
                    },
                },
            }"#,
        )
        .expect("parse jsonc tsconfig");
        let compiler_options = value
            .get("compilerOptions")
            .and_then(serde_json::Value::as_object)
            .expect("compiler options");
        assert_eq!(
            compiler_options
                .get("baseUrl")
                .and_then(serde_json::Value::as_str),
            Some("./src,not-trailing")
        );
        let targets = compiler_options
            .get("paths")
            .and_then(|paths| paths.get("url/*"))
            .and_then(serde_json::Value::as_array)
            .expect("paths target");
        let targets = targets
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>();
        assert_eq!(
            targets,
            vec!["./literal//slash/*", "./literal/*block*/segment/*"]
        );
    }

    #[test]
    fn vue3_compile_script_resolves_relative_re_exported_macro_types_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("leaf.ts"),
            "export type LeafProps = { leaf?: number }",
        )
        .expect("write leaf type");
        std::fs::write(
            dir.path().join("events.ts"),
            "export type LeafEmits = { (e: 'save'): void }",
        )
        .expect("write events type");
        std::fs::write(
            dir.path().join("model.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write model type");
        std::fs::write(
            dir.path().join("bar.ts"),
            "export { LeafProps as BarProps } from './leaf'\nexport * from './events'",
        )
        .expect("write bar type");
        std::fs::write(
            dir.path().join("foo.ts"),
            "export { BarProps as Props } from './bar'\nexport { LeafEmits as Emits } from './bar'\nexport * from './model'",
        )
        .expect("write foo type");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { Props, Emits, ModelValue } from './foo'
const props = defineProps<Props>()
const emit = defineEmits<Emits>()
const model = defineModel<ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("leaf: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = ["foo.ts", "bar.ts", "leaf.ts", "events.ts", "model.ts"]
            .into_iter()
            .map(|name| normalize_path_string(&dir.path().join(name)))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_relative_default_type_imports_and_re_exports() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("direct_base.ts"),
            "export interface DirectBase { inherited?: string }",
        )
        .expect("write direct base type");
        std::fs::write(
            dir.path().join("direct.ts"),
            "import type { DirectBase } from './direct_base'\nexport default interface DirectProps extends DirectBase { direct?: boolean }",
        )
        .expect("write direct type");
        std::fs::write(
            dir.path().join("alias.ts"),
            "type AliasProps = { alias: string }; export default AliasProps",
        )
        .expect("write alias type");
        std::fs::write(
            dir.path().join("leaf.ts"),
            "export default interface LeafProps { leaf: string }",
        )
        .expect("write leaf type");
        std::fs::write(
            dir.path().join("facade.ts"),
            "export { default } from './leaf'",
        )
        .expect("write default facade");
        std::fs::write(
            dir.path().join("named.ts"),
            "export interface NamedProps { named: number }",
        )
        .expect("write named type");
        std::fs::write(
            dir.path().join("default_named.ts"),
            "export { NamedProps as default } from './named'",
        )
        .expect("write named default facade");
        std::fs::write(
            dir.path().join("renamed.ts"),
            "export { default as RenamedProps } from './alias'",
        )
        .expect("write renamed default facade");
        std::fs::write(
            dir.path().join("events.ts"),
            "type Events = { (e: 'save'): void }; export default Events",
        )
        .expect("write events type");
        std::fs::write(
            dir.path().join("model.ts"),
            "type ModelValue = boolean | string; export default ModelValue",
        )
        .expect("write model type");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import DirectProps from './direct'
import AliasProps from './alias'
import FacadeProps from './facade'
import NamedDefaultProps from './default_named'
import { RenamedProps } from './renamed'
import Events from './events'
import ModelValue from './model'
const props = defineProps<DirectProps & AliasProps & FacadeProps & NamedDefaultProps & RenamedProps>()
const emit = defineEmits<Events>()
const model = defineModel<ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("direct: { type: Boolean, required: false }"));
        assert!(script
            .content
            .contains("inherited: { type: String, required: false }"));
        assert!(script
            .content
            .contains("alias: { type: String, required: true }"));
        assert!(script
            .content
            .contains("leaf: { type: String, required: true }"));
        assert!(script
            .content
            .contains("named: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            "direct_base.ts",
            "direct.ts",
            "alias.ts",
            "leaf.ts",
            "facade.ts",
            "named.ts",
            "default_named.ts",
            "renamed.ts",
            "events.ts",
            "model.ts",
        ]
        .into_iter()
        .map(|name| normalize_path_string(&dir.path().join(name)))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }
