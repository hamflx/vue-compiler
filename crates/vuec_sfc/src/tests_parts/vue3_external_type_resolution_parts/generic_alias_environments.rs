#[test]
fn vue3_external_generic_aliases_share_their_definition_environment() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
interface PrivateBase { privateValue: string }
export type Box<T> = PrivateBase & { box: T }
export type Pair<T> = PrivateBase & { pair: T }
export interface Wrapped<T> extends PrivateBase { wrapped: T }
"#,
    )
    .expect("write generic type module");

    let context = vue3_external_type_context_from_path(
        &types,
        &mut BTreeSet::new(),
        &Vue3TypeResolverContext::default(),
    )
    .expect("load generic type context");
    let environments = context
        .generic_type_aliases
        .values()
        .map(|alias| match &alias.scope {
            Vue3GenericTypeScope::Captured(environment) => environment.clone(),
            Vue3GenericTypeScope::Local => panic!("external alias retained a local scope"),
        })
        .collect::<Vec<_>>();

    assert_eq!(environments.len(), 3);
    assert!(environments
        .iter()
        .skip(1)
        .all(|environment| std::sync::Arc::ptr_eq(&environments[0], environment)));
    assert!(environments[0]
        .props_type_declarations
        .contains_key("PrivateBase"));
    assert_eq!(
        environments[0].definition_filename.as_deref(),
        Some(normalize_path_string(&types).as_str())
    );
}

#[test]
fn vue3_external_generic_aliases_resolve_in_their_definition_scope() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    let relative = dir.path().join("relative.ts");
    std::fs::write(
        &relative,
        "export interface RelativeBase { relativeValue: string }",
    )
    .expect("write relative generic dependency");
    std::fs::write(
        &types,
        r#"
interface PrivateBase { privateValue: string }
export type Box<T> = PrivateBase & import('./relative').RelativeBase & { box: T }
export interface Wrapped<T> extends PrivateBase { wrapped: T }
"#,
    )
    .expect("write scoped generic aliases");

    let filename = dir.path().join("component").join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type { Box, Wrapped } from '../types'
type PrivateBase = { privateValue: number }
defineProps<Box<boolean> & Wrapped<number>>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("privateValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("relativeValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("box: { type: Boolean, required: true }"));
    assert!(script
        .content
        .contains("wrapped: { type: Number, required: true }"));
    let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
    assert!(deps.contains(&normalize_path_string(&types)));
    assert!(deps.contains(&normalize_path_string(&relative)));
}

#[test]
fn vue3_external_generic_aliases_ignore_export_space_aliases() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
interface Local { localValue: string }
interface Other { otherValue: number }
export { Other as Local }
export type Box<T> = Local & { value: T }
"#,
    )
    .expect("write export-space alias types");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type { Box } from './types'
defineProps<Box<boolean>>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("localValue: { type: String, required: true }"));
    assert!(!script.content.contains("otherValue:"));
    assert!(script
        .content
        .contains("value: { type: Boolean, required: true }"));
}

#[test]
fn vue3_default_export_generic_interfaces_capture_their_definition_scope() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
interface PrivateBase { privateValue: string }
export default interface Box<T> extends PrivateBase { value: T }
"#,
    )
    .expect("write default generic interface");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type Box from './types'
defineProps<Box<number>>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("privateValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("value: { type: Number, required: true }"));
}

#[test]
fn vue3_parent_generic_aliases_keep_their_scope_inside_namespaces() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
export namespace Nested {
  interface Base { innerValue: number }
  export type Props = Box<boolean>
}
type Box<T> = Base & { value: T }
interface Base { outerValue: string }
"#,
    )
    .expect("write namespace generic types");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Types from './types'
defineProps<Types.Nested.Props>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("outerValue: { type: String, required: true }"));
    assert!(!script.content.contains("innerValue:"));
    assert!(script
        .content
        .contains("value: { type: Boolean, required: true }"));
}

#[test]
fn vue3_deferred_namespaces_preserve_outer_forward_references() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
export type Props = Nested.Inner
export namespace Nested {
  export interface Inner { nestedValue: string }
}
"#,
    )
    .expect("write forward namespace types");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type { Props } from './types'
defineProps<Props>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("nestedValue: { type: String, required: true }"));
}

#[test]
fn vue3_deferred_namespaces_capture_refreshed_parent_aliases() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    let base = dir.path().join("base.ts");
    std::fs::write(
        &base,
        "export interface ExternalBase { baseValue: string }",
    )
    .expect("write deferred namespace base");
    std::fs::write(
        &types,
        r#"
import type { ExternalBase } from './base'
export namespace Nested {
  export type Props = Alias
}
type Alias = ExternalBase & LocalBase & { aliasValue: boolean }
interface LocalBase { localValue: number }
"#,
    )
    .expect("write deferred namespace aliases");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Types from './types'
defineProps<Types.Nested.Props>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("baseValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("localValue: { type: Number, required: true }"));
    assert!(script
        .content
        .contains("aliasValue: { type: Boolean, required: true }"));
    let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
    assert!(deps.contains(&normalize_path_string(&types)));
    assert!(deps.contains(&normalize_path_string(&base)));
}

#[test]
fn vue3_ambient_namespaces_capture_forward_parent_aliases() {
    let dir = tempfile::tempdir().expect("temp dir");
    let global = dir.path().join("global.d.ts");
    std::fs::write(
        &global,
        r#"
declare namespace Ambient {
  type Props = Alias
}
declare type Alias = Base & { aliasValue: boolean }
declare interface Base { baseValue: string }
"#,
    )
    .expect("write ambient namespace aliases");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<Ambient.Props>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(
        &descriptor,
        SfcScriptCompileOptions {
            global_type_files: vec![global.to_string_lossy().to_string()],
            ..SfcScriptCompileOptions::default()
        },
    );

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("baseValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("aliasValue: { type: Boolean, required: true }"));
}

#[test]
fn vue3_global_files_merge_generic_interfaces_with_fragment_scopes_in_any_order() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first_leaf = dir.path().join("first-leaf.ts");
    let second_leaf = dir.path().join("second-leaf.ts");
    std::fs::write(
        &first_leaf,
        "export interface FirstLeaf { firstLeafValue: string }",
    )
    .expect("write first generic global dependency");
    std::fs::write(
        &second_leaf,
        "export interface SecondLeaf { secondLeafValue: number }",
    )
    .expect("write second generic global dependency");

    let first = dir.path().join("first-global.d.ts");
    let second = dir.path().join("second-global.d.ts");
    std::fs::write(
        &first,
        r#"
import type { FirstLeaf } from './first-leaf'
type LocalBase = FirstLeaf & { firstPrivateValue: boolean }
export {}
declare global {
  interface Shared<T> extends LocalBase { firstValue: T }
}
"#,
    )
    .expect("write first generic global fragment");
    std::fs::write(
        &second,
        r#"
import type { SecondLeaf } from './second-leaf'
type LocalBase = SecondLeaf & { secondPrivateValue: boolean }
export {}
declare global {
  interface Shared<T> extends LocalBase { secondValue: T }
}
"#,
    )
    .expect("write second generic global fragment");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<Shared<string>>()
</script>"#;
    for files in [
        vec![first.clone(), second.clone()],
        vec![second.clone(), first.clone()],
    ] {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: files
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect(),
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for (name, runtime) in [
            ("firstLeafValue", "String"),
            ("secondLeafValue", "Number"),
            ("firstPrivateValue", "Boolean"),
            ("secondPrivateValue", "Boolean"),
            ("firstValue", "String"),
            ("secondValue", "String"),
        ] {
            assert!(
                script.content.contains(&format!(
                    "{name}: {{ type: {runtime}, required: true }}"
                )),
                "missing {name} for {files:?}: {}",
                script.content
            );
        }
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [
                normalize_path_string(&first),
                normalize_path_string(&second),
                normalize_path_string(&first_leaf),
                normalize_path_string(&second_leaf),
            ]
            .into_iter()
            .collect()
        );
    }
}

#[test]
fn vue3_global_files_capture_merged_interfaces_in_generic_dependents() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first = dir.path().join("first-global.d.ts");
    let second = dir.path().join("second-global.d.ts");
    std::fs::write(
        &first,
        r#"
export {}
declare global {
  interface Shared<T> { firstValue: T }
  type Consumer<T> = Shared<T> & { consumerValue: T }
}
"#,
    )
    .expect("write generic global consumer");
    std::fs::write(
        &second,
        r#"
export {}
declare global {
  interface Shared<T> { secondValue: T }
}
"#,
    )
    .expect("write later generic global fragment");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<Consumer<string>>()
</script>"#;
    for files in [
        vec![first.clone(), second.clone()],
        vec![second.clone(), first.clone()],
    ] {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: files
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect(),
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for name in ["firstValue", "secondValue", "consumerValue"] {
            assert!(
                script
                    .content
                    .contains(&format!("{name}: {{ type: String, required: true }}")),
                "missing {name} for {files:?}: {}",
                script.content
            );
        }
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [normalize_path_string(&first), normalize_path_string(&second)]
                .into_iter()
                .collect()
        );
    }
}

#[test]
fn vue3_global_namespaces_capture_merged_interfaces_in_generic_dependents() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first = dir.path().join("first-global.d.ts");
    let second = dir.path().join("second-global.d.ts");
    std::fs::write(
        &first,
        r#"
declare namespace SharedNamespace {
  interface Shared<T> { firstValue: T }
  type Consumer<T> = Shared<T> & { consumerValue: T }
}
"#,
    )
    .expect("write namespace generic consumer");
    std::fs::write(
        &second,
        r#"
declare namespace SharedNamespace {
  interface Shared<T> { secondValue: T }
}
"#,
    )
    .expect("write later namespace generic fragment");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<SharedNamespace.Consumer<string>>()
</script>"#;
    for files in [
        vec![first.clone(), second.clone()],
        vec![second.clone(), first.clone()],
    ] {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: files
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect(),
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for name in ["firstValue", "secondValue", "consumerValue"] {
            assert!(
                script
                    .content
                    .contains(&format!("{name}: {{ type: String, required: true }}")),
                "missing {name} for {files:?}: {}",
                script.content
            );
        }
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [normalize_path_string(&first), normalize_path_string(&second)]
                .into_iter()
                .collect()
        );
    }
}

#[test]
fn vue3_global_generic_merge_stabilizes_with_exact_context_build_budget() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first = dir.path().join("first-global.d.ts");
    let second = dir.path().join("second-global.d.ts");
    std::fs::write(&first, "interface Shared<T> { firstValue: T }")
        .expect("write first generic fragment");
    std::fs::write(&second, "interface Shared<T> { secondValue: T }")
        .expect("write second generic fragment");
    let filename = dir.path().join("Comp.vue");
    let global_files = [first, second]
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    let exact = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_context_builds: 2,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    let context = vue3_global_type_context(
        &filename.to_string_lossy(),
        &global_files,
        &exact,
    );
    assert_eq!(
        context
            .generic_type_aliases
            .get("Shared")
            .map(|alias| alias.interface_fragments.len()),
        Some(2)
    );
    assert_eq!(exact.external_type_session.stats().context_builds, 2);

    let rejected = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_context_builds: 1,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    let context = vue3_global_type_context(
        &filename.to_string_lossy(),
        &global_files,
        &rejected,
    );
    assert_eq!(context, Vue27TypeContext::default());
    assert_eq!(rejected.external_type_session.stats().context_builds, 1);
}

#[test]
fn vue3_incompatible_generic_global_interfaces_fail_closed_in_any_order() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first = dir.path().join("first-global.d.ts");
    let second = dir.path().join("second-global.d.ts");
    std::fs::write(&first, "interface Shared<T> { firstValue: T }")
        .expect("write first generic interface");
    std::fs::write(
        &second,
        "interface Shared<T, U> { secondValue: U }",
    )
    .expect("write incompatible generic interface");
    let filename = dir.path().join("Comp.vue");
    let expected_deps = [normalize_path_string(&first), normalize_path_string(&second)]
        .into_iter()
        .collect::<BTreeSet<_>>();

    for files in [
        vec![first.clone(), second.clone()],
        vec![second.clone(), first.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );

        assert!(!vue3_type_context_has_name(&context, "Shared"));
        assert!(!context.generic_type_aliases.contains_key("Shared"));
        assert!(context.silent_unresolved_type_names.contains("Shared"));
        assert_eq!(context.type_deps.get("Shared"), Some(&expected_deps));
    }
}

#[test]
fn vue3_global_generic_dependency_chains_reach_a_fixed_point() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first = dir.path().join("first-global.d.ts");
    let second = dir.path().join("second-global.d.ts");
    let third = dir.path().join("third-global.d.ts");
    std::fs::write(
        &first,
        r#"
interface Shared<T> { firstValue: T }
type First<T> = Second<T> & { consumerValue: T }
"#,
    )
    .expect("write first generic alias");
    std::fs::write(
        &second,
        "type Second<T> = Shared<T> & { middleValue: T }",
    )
    .expect("write second generic alias");
    std::fs::write(&third, "interface Shared<T> { thirdValue: T }")
        .expect("write generic chain leaf");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<First<string>>()
</script>"#;
    for files in [
        vec![first.clone(), second.clone(), third.clone()],
        vec![third.clone(), second.clone(), first.clone()],
    ] {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: files
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect(),
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for name in ["consumerValue", "middleValue", "firstValue", "thirdValue"] {
            assert!(
                script
                    .content
                    .contains(&format!("{name}: {{ type: String, required: true }}")),
                "missing {name} for {files:?}: {}",
                script.content
            );
        }
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [
                normalize_path_string(&first),
                normalize_path_string(&second),
                normalize_path_string(&third),
            ]
            .into_iter()
            .collect()
        );
    }
}

#[test]
fn vue3_global_merge_rejects_incompatible_complete_type_parameter_lists() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cases = [
        (
            "interface Shared<T extends string> { firstValue: T }",
            "interface Shared<T extends number> { secondValue: T }",
            "Shared",
        ),
        (
            "interface Shared<T = string> { firstValue: T }",
            "interface Shared<T = number> { secondValue: T }",
            "Shared",
        ),
        (
            "interface Shared<in T> { firstValue: T }",
            "interface Shared<out T> { secondValue: T }",
            "Shared",
        ),
        (
            "declare class Shared<T extends string> { firstValue: T }",
            "interface Shared<T extends number> { secondValue: T }",
            "Shared",
        ),
        (
            "interface Shared { firstValue: string }",
            "interface Shared<T> { secondValue: T }",
            "Shared",
        ),
        (
            "declare namespace Box { interface Shared<T extends string> { firstValue: T } }",
            "declare namespace Box { interface Shared<T extends number> { secondValue: T } }",
            "Box.Shared",
        ),
    ];

    for (index, (first_source, second_source, name)) in cases.into_iter().enumerate() {
        let first = dir.path().join(format!("first-{index}.d.ts"));
        let second = dir.path().join(format!("second-{index}.d.ts"));
        std::fs::write(&first, first_source).expect("write first declaration");
        std::fs::write(&second, second_source).expect("write second declaration");
        let expected_deps = [normalize_path_string(&first), normalize_path_string(&second)]
            .into_iter()
            .collect::<BTreeSet<_>>();
        for files in [
            vec![first.clone(), second.clone()],
            vec![second.clone(), first.clone()],
        ] {
            let context = vue3_global_type_context(
                &dir.path().join("Comp.vue").to_string_lossy(),
                &files
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect::<Vec<_>>(),
                &Vue3TypeResolverContext::default(),
            );
            assert!(!vue3_type_context_has_name(&context, name), "case {index}");
            assert!(
                context.silent_unresolved_type_names.contains(name),
                "case {index}"
            );
            assert_eq!(context.type_deps.get(name), Some(&expected_deps));
        }
    }
}

#[test]
fn vue3_global_merge_compares_generic_parameters_structurally() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first = dir.path().join("first-global.d.ts");
    let second = dir.path().join("second-global.d.ts");
    std::fs::write(
        &first,
        "interface Shared<T extends ({ label: 'ok'; count: 1_000 }) = { label: 'ok'; count: 1_000 }> { firstValue: T }",
    )
    .expect("write first generic interface");
    std::fs::write(
        &second,
        "interface Shared<T /* trivia */ extends {label: \"ok\", count: 1000} = {label: \"ok\", count: 1000}> { secondValue: T }",
    )
    .expect("write second generic interface");

    for files in [
        vec![first.clone(), second.clone()],
        vec![second.clone(), first.clone()],
    ] {
        let context = vue3_global_type_context(
            &dir.path().join("Comp.vue").to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        assert!(!context.silent_unresolved_type_names.contains("Shared"));
        assert_eq!(
            context
                .generic_type_aliases
                .get("Shared")
                .map(|alias| alias.interface_fragments.len()),
            Some(2)
        );
    }
}

#[test]
fn vue3_global_generic_fixed_point_can_revisit_the_same_files() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first = dir.path().join("first-global.d.ts");
    let second = dir.path().join("second-global.d.ts");
    std::fs::write(
        &first,
        r#"
type First<T> = Second<T> & { firstValue: T }
type Third<T> = Leaf<T> & { thirdValue: T }
"#,
    )
    .expect("write alternating generic declarations");
    std::fs::write(
        &second,
        r#"
type Second<T> = Third<T> & { secondValue: T }
interface Leaf<T> { leafValue: T }
"#,
    )
    .expect("write alternating generic dependencies");
    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<First<string>>()
</script>"#;

    for files in [
        vec![first.clone(), second.clone()],
        vec![second.clone(), first.clone()],
    ] {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: files
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect(),
                ..SfcScriptCompileOptions::default()
            },
        );
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for name in ["firstValue", "secondValue", "thirdValue", "leafValue"] {
            assert!(
                script
                    .content
                    .contains(&format!("{name}: {{ type: String, required: true }}")),
                "missing {name} for {files:?}: {}",
                script.content
            );
        }
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [normalize_path_string(&first), normalize_path_string(&second)]
                .into_iter()
                .collect()
        );
    }
}

#[test]
fn vue3_incompatible_same_file_generic_interfaces_do_not_pollute_dependents() {
    let dir = tempfile::tempdir().expect("temp dir");
    let filename = dir.path().join("Comp.vue");
    for (index, declarations) in [
        r#"
interface Shared<T extends string> { firstValue: T }
interface Shared<T extends number> { leakedValue: T }
type Uses<T extends string> = Shared<T>
"#,
        r#"
interface Shared<T extends number> { leakedValue: T }
interface Shared<T extends string> { firstValue: T }
type Uses<T extends string> = Shared<T>
"#,
    ]
    .into_iter()
    .enumerate()
    {
        let types = dir.path().join(format!("global-{index}.d.ts"));
        std::fs::write(&types, declarations)
            .expect("write incompatible same-file declarations");
        let global_files = vec![types.to_string_lossy().to_string()];
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &global_files,
            &Vue3TypeResolverContext::default(),
        );
        assert!(context.silent_unresolved_type_names.contains("Shared"));
        assert!(context.silent_unresolved_type_names.contains("Uses"));
        assert!(!context.generic_type_aliases.contains_key("Uses"));
        assert!(!context.props_type_declarations.contains_key("Uses"));

        let source = r#"<script setup lang="ts">
defineProps<Uses<string>>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: global_files,
                ..SfcScriptCompileOptions::default()
            },
        );
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(!script.content.contains("firstValue"), "{}", script.content);
        assert!(!script.content.contains("leakedValue"), "{}", script.content);
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            BTreeSet::from([normalize_path_string(&types)])
        );
    }
}

#[test]
fn vue3_global_generic_merge_keeps_shared_ambient_constraint_identity() {
    let dir = tempfile::tempdir().expect("temp dir");
    let base = dir.path().join("base.d.ts");
    let first = dir.path().join("first.d.ts");
    let second = dir.path().join("second.d.ts");
    std::fs::write(&base, "interface Base { baseValue: string }").expect("write base");
    std::fs::write(
        &first,
        "interface Shared<T extends Base> { firstValue: T }",
    )
    .expect("write first fragment");
    std::fs::write(
        &second,
        "interface Shared<T extends Base> { secondValue: T }",
    )
    .expect("write second fragment");
    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<Shared<Base>>()
</script>"#;

    for files in [
        vec![base.clone(), first.clone(), second.clone()],
        vec![second.clone(), first.clone(), base.clone()],
    ] {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: files
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect(),
                ..SfcScriptCompileOptions::default()
            },
        );
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for name in ["firstValue", "secondValue"] {
            assert!(
                script
                    .content
                    .contains(&format!("{name}: {{ type: Object, required: true }}")),
                "missing {name} for {files:?}: {}",
                script.content
            );
        }
    }
}

#[test]
fn vue3_global_generic_merge_distinguishes_imported_constraint_scopes() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first_type = dir.path().join("first-type.d.ts");
    let second_type = dir.path().join("second-type.d.ts");
    let first = dir.path().join("first-global.d.ts");
    let second = dir.path().join("second-global.d.ts");
    std::fs::write(&first_type, "export interface ConstraintType { first: string }")
        .expect("write first constraint");
    std::fs::write(
        &second_type,
        "export interface ConstraintType { second: number }",
    )
    .expect("write second constraint");
    std::fs::write(
        &first,
        r#"
import type { ConstraintType as Constraint } from './first-type'
declare global {
  interface Shared<T extends Constraint> { firstValue: T }
  interface ScopedProperty { value: Constraint }
}
export {}
"#,
    )
    .expect("write first global fragment");
    std::fs::write(
        &second,
        r#"
import type { ConstraintType as Constraint } from './second-type'
declare global {
  interface Shared<T extends Constraint> { secondValue: T }
  interface ScopedProperty { value: Constraint }
}
export {}
"#,
    )
    .expect("write second global fragment");
    let filename = dir.path().join("Comp.vue");

    for files in [
        vec![first.clone(), second.clone()],
        vec![second.clone(), first.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        assert!(context.silent_unresolved_type_names.contains("Shared"));
        assert!(!vue3_type_context_has_name(&context, "Shared"));
        assert!(context
            .silent_unresolved_type_names
            .contains("ScopedProperty"));
        assert!(!vue3_type_context_has_name(&context, "ScopedProperty"));
    }
}

#[test]
fn vue3_global_merge_recognizes_shared_import_binding_identity() {
    let dir = tempfile::tempdir().expect("temp dir");
    let constraint = dir.path().join("constraint.d.ts");
    let unrelated = dir.path().join("unrelated.d.ts");
    let first = dir.path().join("first-global.d.ts");
    let second = dir.path().join("second-global.d.ts");
    std::fs::write(
        &constraint,
        "export interface ConstraintType { common: string }",
    )
    .expect("write shared constraint");
    std::fs::write(&unrelated, "export interface Unrelated { other: number }")
        .expect("write unrelated import");
    std::fs::write(
        &first,
        r#"
import type { ConstraintType as Constraint } from './constraint'
declare global {
  interface Shared<T extends Constraint> { firstValue: T }
  interface ScopedProperty { value: Constraint }
}
export {}
"#,
    )
    .expect("write first global fragment");
    std::fs::write(
        &second,
        r#"
import type { ConstraintType as Constraint } from './constraint'
import type { Unrelated } from './unrelated'
declare global {
  interface Shared<T extends Constraint> { secondValue: T }
  interface ScopedProperty { value: Constraint }
}
export {}
"#,
    )
    .expect("write second global fragment");
    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<Shared<{ common: string }> & ScopedProperty>()
</script>"#;

    for files in [
        vec![first.clone(), second.clone()],
        vec![second.clone(), first.clone()],
    ] {
        let global_files = files
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &global_files,
            &Vue3TypeResolverContext::default(),
        );
        assert!(!context.silent_unresolved_type_names.contains("Shared"));
        assert!(!context
            .silent_unresolved_type_names
            .contains("ScopedProperty"));

        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: global_files,
                ..SfcScriptCompileOptions::default()
            },
        );
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for name in ["firstValue", "secondValue", "value"] {
            assert!(
                script
                    .content
                    .contains(&format!("{name}: {{ type: Object, required: true }}")),
                "missing {name} for {files:?}: {}",
                script.content
            );
        }
    }
}

#[test]
fn vue3_global_generic_classes_merge_with_compatible_interfaces() {
    let dir = tempfile::tempdir().expect("temp dir");
    let class = dir.path().join("class.d.ts");
    let interface = dir.path().join("interface.d.ts");
    std::fs::write(
        &class,
        "declare class Shared<T> {}\ntype Consumer<T> = Shared<T>",
    )
        .expect("write generic class");
    std::fs::write(
        &interface,
        "interface Shared<T> { interfaceValue: T }",
    )
    .expect("write generic interface");
    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<Consumer<string>>()
</script>"#;

    for files in [
        vec![class.clone(), interface.clone()],
        vec![interface.clone(), class.clone()],
    ] {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: files
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect(),
                ..SfcScriptCompileOptions::default()
            },
        );
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("interfaceValue: { type: String, required: true }"),
            "missing merged value for {files:?}: {}",
            script.content
        );
    }
}
