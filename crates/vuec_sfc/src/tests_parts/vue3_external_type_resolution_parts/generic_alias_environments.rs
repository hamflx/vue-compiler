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
interface Base { outerValue: string }
type Box<T> = Base & { value: T }
export namespace Nested {
  interface Base { innerValue: number }
  export type Props = Box<boolean>
}
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
