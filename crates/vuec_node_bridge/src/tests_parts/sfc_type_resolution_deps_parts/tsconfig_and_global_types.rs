    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_tsconfig_path_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-tsconfig-path-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src").join("components")).expect("create components");
        std::fs::create_dir_all(dir.join("src").join("views")).expect("create views");
        std::fs::create_dir_all(dir.join("tsconfigs")).expect("create tsconfigs");
        std::fs::write(
            dir.join("tsconfig.json"),
            r#"{
                "files": [],
                "references": [{ "path": "./tsconfig.app.json" }],
                "compilerOptions": {
                    "paths": {
                        "bar": ["./pp.ts"]
                    }
                }
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.join("tsconfig.app.json"),
            r#"{
                "extends": ["./tsconfigs/base.json"]
            }"#,
        )
        .expect("write app tsconfig");
        std::fs::write(
            dir.join("tsconfigs").join("base.json"),
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
        std::fs::write(dir.join("pp.ts"), "export type PathProps = { bar: string }")
            .expect("write path type");
        std::fs::write(
            dir.join("src").join("types.ts"),
            "export type BaseProps = { foo?: string; count: number }",
        )
        .expect("write aliased type");
        std::fs::write(
            dir.join("src").join("views").join("Aliased.vue"),
            "<script lang=\"ts\">export type VueProps = { fromVue: string }</script>",
        )
        .expect("write aliased vue");

        let filename = dir.join("src").join("components").join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { PathProps } from 'bar'\n",
                    "import type { BaseProps } from '@/types.ts'\n",
                    "import type { VueProps } from '@/views/Aliased.vue'\n",
                    "defineProps<PathProps & BaseProps & VueProps>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("bar: { type: String, required: true }"));
        assert!(content.contains("foo: { type: String, required: false }"));
        assert!(content.contains("count: { type: Number, required: true }"));
        assert!(content.contains("fromVue: { type: String, required: true }"));

        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            dir.join("pp.ts"),
            dir.join("src").join("types.ts"),
            dir.join("src").join("views").join("Aliased.vue"),
        ]
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_tsconfig_jsonc_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-tsconfig-jsonc-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src").join("components")).expect("create components");
        std::fs::create_dir_all(dir.join("src").join("base")).expect("create base");
        std::fs::create_dir_all(dir.join("config")).expect("create config");
        std::fs::write(
            dir.join("tsconfig.json"),
            r#"{
                // Root alias survives comments and trailing commas.
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
            dir.join("tsconfig.app.json"),
            r#"{
                "extends": [
                    "./config/base.json", // inherited aliases
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
            dir.join("config").join("base.json"),
            r#"{
                /* ${configDir} resolves from the referencing config directory. */
                "compilerOptions": {
                    "paths": {
                        "@base/*": ["${configDir}/src/base/*",],
                    },
                },
            }"#,
        )
        .expect("write base tsconfig");
        std::fs::write(
            dir.join("root.ts"),
            "export type RootProps = { root: string }",
        )
        .expect("write root type");
        std::fs::write(
            dir.join("app.ts"),
            "export type AppProps = { app?: number }",
        )
        .expect("write app type");
        std::fs::write(
            dir.join("src").join("base").join("types.ts"),
            "export type BaseProps = { base: boolean }",
        )
        .expect("write base type");

        let filename = dir.join("src").join("components").join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { RootProps } from 'root-alias'\n",
                    "import type { AppProps } from 'app-alias'\n",
                    "import type { BaseProps } from '@base/types'\n",
                    "defineProps<RootProps & AppProps & BaseProps>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("root: { type: String, required: true }"));
        assert!(content.contains("app: { type: Number, required: false }"));
        assert!(content.contains("base: { type: Boolean, required: true }"));

        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            dir.join("root.ts"),
            dir.join("app.ts"),
            dir.join("src").join("base").join("types.ts"),
        ]
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_package_tsconfig_extends_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-package-tsconfig-extends-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src").join("components")).expect("create components");

        let scoped_config_pkg = dir.join("node_modules").join("@vuec").join("tsconfig");
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

        let preset_pkg = dir.join("node_modules").join("vuec-tsconfig-preset");
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
            dir.join("tsconfig.json"),
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
            dir.join("root.ts"),
            "export type RootProps = { root: string }",
        )
        .expect("write root type");
        std::fs::write(
            dir.join("shared.ts"),
            "export type SharedProps = { shared?: number }",
        )
        .expect("write shared type");
        std::fs::write(
            dir.join("local.ts"),
            "export type LocalProps = { local: boolean }",
        )
        .expect("write local type");

        let filename = dir.join("src").join("components").join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { RootProps } from 'pkg-root'\n",
                    "import type { SharedProps } from 'pkg-shared'\n",
                    "import type { LocalProps } from 'local-alias'\n",
                    "defineProps<RootProps & SharedProps & LocalProps>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("root: { type: String, required: true }"));
        assert!(content.contains("shared: { type: Number, required: false }"));
        assert!(content.contains("local: { type: Boolean, required: true }"));

        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            dir.join("root.ts"),
            dir.join("shared.ts"),
            dir.join("local.ts"),
        ]
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_global_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-global-type-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let global = dir.join("global.d.ts");
        std::fs::write(
            &global,
            concat!(
                "declare interface GlobalProps { msg: string }\n",
                "declare type GlobalModel = boolean | string"
            ),
        )
        .expect("write global types");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "defineProps<GlobalProps>()\n",
                    "defineModel<GlobalModel>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy(),
                "options": {
                    "globalTypeFiles": [global.to_string_lossy()]
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = global.to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("msg: { type: String, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Boolean, String] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_discovers_tsconfig_global_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-tsconfig-global-type-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src").join("components")).expect("create components");
        std::fs::create_dir_all(dir.join("types").join("nested")).expect("create types");
        std::fs::create_dir_all(dir.join("config")).expect("create config");
        std::fs::create_dir_all(dir.join("project")).expect("create project");
        std::fs::write(
            dir.join("tsconfig.json"),
            r#"{
                "files": ["./types/root.d.ts"],
                "include": ["./types/**/*.ts", "./src/**/*.vue"],
                "extends": "./config/base.json",
                "references": [{ "path": "./project" }]
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.join("config").join("base.json"),
            r#"{
                "files": ["${configDir}/types/base.d.ts"]
            }"#,
        )
        .expect("write base tsconfig");
        std::fs::write(
            dir.join("project").join("tsconfig.json"),
            r#"{
                "files": ["../types/ref.d.ts"]
            }"#,
        )
        .expect("write referenced tsconfig");
        std::fs::write(
            dir.join("types").join("root.d.ts"),
            "declare interface RootGlobalProps { root: string }",
        )
        .expect("write root global");
        std::fs::write(
            dir.join("types").join("nested").join("included.d.ts"),
            "declare interface IncludedGlobalProps { included?: number }",
        )
        .expect("write included global");
        std::fs::write(
            dir.join("types").join("base.d.ts"),
            "declare interface BaseGlobalProps { base: boolean }",
        )
        .expect("write base global");
        std::fs::write(
            dir.join("types").join("ref.d.ts"),
            "declare type RefGlobalModel = boolean | string",
        )
        .expect("write referenced global");
        std::fs::write(
            dir.join("src").join("ignored.d.ts"),
            "declare interface IgnoredByVueInclude { ignored: string }",
        )
        .expect("write ignored global");

        let filename = dir.join("src").join("components").join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "defineProps<RootGlobalProps & IncludedGlobalProps & BaseGlobalProps>()\n",
                    "defineModel<RefGlobalModel>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("root: { type: String, required: true }"));
        assert!(content.contains("included: { type: Number, required: false }"));
        assert!(content.contains("base: { type: Boolean, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Boolean, String] },"));
        assert!(!content.contains("ignored: { type: String, required: true }"));

        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            dir.join("types").join("root.d.ts"),
            dir.join("types").join("nested").join("included.d.ts"),
            dir.join("types").join("base.d.ts"),
            dir.join("types").join("ref.d.ts"),
        ]
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!deps.iter().any(|dep| dep.contains("ignored")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_discovers_tsconfig_types_type_roots_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-tsconfig-types-type-roots-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src").join("components")).expect("create components");
        std::fs::create_dir_all(dir.join("typings").join("chosen")).expect("create chosen");
        std::fs::create_dir_all(dir.join("typings").join("@scope").join("tool"))
            .expect("create scoped");
        std::fs::create_dir_all(dir.join("typings").join("ignored")).expect("create ignored");
        std::fs::create_dir_all(dir.join("base-types").join("base-root")).expect("create base");
        std::fs::create_dir_all(dir.join("node_modules").join("@types").join("defaulted"))
            .expect("create default @types");
        std::fs::create_dir_all(dir.join("config")).expect("create config");
        std::fs::create_dir_all(dir.join("project")).expect("create project");
        std::fs::write(
            dir.join("tsconfig.json"),
            r#"{
                "extends": "./config/base.json",
                "compilerOptions": {
                    "types": ["chosen", "@scope/tool"],
                    "typeRoots": ["./typings"]
                },
                "references": [{ "path": "./project" }]
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.join("config").join("base.json"),
            r#"{
                "compilerOptions": {
                    "typeRoots": ["${configDir}/base-types"]
                }
            }"#,
        )
        .expect("write base tsconfig");
        std::fs::write(dir.join("project").join("tsconfig.json"), "{}")
            .expect("write referenced tsconfig");
        std::fs::write(
            dir.join("typings").join("chosen").join("index.d.ts"),
            "declare interface ChosenGlobalProps { chosen: string }",
        )
        .expect("write chosen global");
        std::fs::write(
            dir.join("typings")
                .join("@scope")
                .join("tool")
                .join("index.d.ts"),
            "declare type ScopedGlobalModel = number | boolean",
        )
        .expect("write scoped global");
        std::fs::write(
            dir.join("typings").join("ignored").join("index.d.ts"),
            "declare interface IgnoredTypeRootGlobalProps { ignored: string }",
        )
        .expect("write ignored global");
        std::fs::write(
            dir.join("base-types").join("base-root").join("index.d.ts"),
            "declare interface BaseRootGlobalProps { baseRoot?: number }",
        )
        .expect("write base root global");
        std::fs::write(
            dir.join("node_modules")
                .join("@types")
                .join("defaulted")
                .join("index.d.ts"),
            "declare interface DefaultTypesGlobalProps { defaulted: boolean }",
        )
        .expect("write default @types global");

        let filename = dir.join("src").join("components").join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "defineProps<ChosenGlobalProps & BaseRootGlobalProps & DefaultTypesGlobalProps>()\n",
                    "defineModel<ScopedGlobalModel>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("chosen: { type: String, required: true }"));
        assert!(content.contains("baseRoot: { type: Number, required: false }"));
        assert!(content.contains("defaulted: { type: Boolean, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Number, Boolean] },"));
        assert!(!content.contains("ignored: { type: String"));

        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            dir.join("base-types").join("base-root").join("index.d.ts"),
            dir.join("typings").join("chosen").join("index.d.ts"),
            dir.join("typings")
                .join("@scope")
                .join("tool")
                .join("index.d.ts"),
            dir.join("node_modules")
                .join("@types")
                .join("defaulted")
                .join("index.d.ts"),
        ]
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!deps.iter().any(|dep| dep.contains("ignored")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_respects_empty_configured_tsconfig_type_roots() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-empty-tsconfig-type-roots-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src").join("components")).expect("create components");
        std::fs::create_dir_all(dir.join("node_modules").join("@types").join("defaulted"))
            .expect("create default @types");
        std::fs::write(
            dir.join("node_modules")
                .join("@types")
                .join("defaulted")
                .join("index.d.ts"),
            "declare interface DefaultTypesGlobalProps { defaulted: boolean }",
        )
        .expect("write default @types global");
        std::fs::write(
            dir.join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "typeRoots": ["./missing"]
                }
            }"#,
        )
        .expect("write tsconfig");

        let filename = dir.join("src").join("components").join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "defineProps<DefaultTypesGlobalProps>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let errors = compiled["errors"].as_array().unwrap();
        assert!(errors
            .iter()
            .any(|error| error.as_str().is_some_and(|message| {
                message.contains("Unresolvable type reference or unsupported built-in utility type")
            })));
        assert!(compiled["deps"].as_array().unwrap().is_empty());
        assert!(!compiled["content"]
            .as_str()
            .unwrap_or_default()
            .contains("defaulted: { type: Boolean"));

        let _ = std::fs::remove_dir_all(&dir);
    }
