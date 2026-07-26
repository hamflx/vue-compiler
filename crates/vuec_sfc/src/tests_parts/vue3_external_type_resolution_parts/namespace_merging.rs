#[test]
fn vue3_sibling_namespaces_reach_a_fixed_point_and_preserve_deps() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    let leaf = dir.path().join("leaf.ts");
    std::fs::write(
        &leaf,
        "export interface Leaf { leafValue: string }",
    )
    .expect("write namespace dependency leaf");
    std::fs::write(
        &types,
        r#"
import type { Leaf } from './leaf'
export namespace First {
  export type Props = Alias
}
type Alias = Second.Middle
export namespace Second {
  export type Middle = Third.Base
}
export namespace Third {
  export type Base = Leaf & { terminalValue: boolean }
}
"#,
    )
    .expect("write sibling namespace chain");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Types from './types'
defineProps<Types.First.Props>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("leafValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("terminalValue: { type: Boolean, required: true }"));
    let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(
        deps,
        [normalize_path_string(&types), normalize_path_string(&leaf)]
            .into_iter()
            .collect()
    );
    assert!(!script
        .deps
        .iter()
        .any(|dependency| dependency.contains('\\')));
}

#[test]
fn vue3_namespace_local_forward_dependency_chains_reach_a_fixed_point() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    let leaf = dir.path().join("leaf.ts");
    std::fs::write(
        &leaf,
        "export interface Leaf { leafValue: string }",
    )
    .expect("write local namespace dependency leaf");
    std::fs::write(
        &types,
        r#"
import type { Leaf } from './leaf'
export namespace Nested {
  export type Props = Middle
  export type Middle = Leaf
}
"#,
    )
    .expect("write local namespace dependency chain");

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
        .contains("leafValue: { type: String, required: true }"));
    assert_eq!(
        script.deps.iter().cloned().collect::<BTreeSet<_>>(),
        [normalize_path_string(&types), normalize_path_string(&leaf)]
            .into_iter()
            .collect()
    );
}

#[test]
fn vue3_namespace_projection_refreshes_declared_function_returns() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
export declare function makeValue(): Values.Value
export namespace Values {
  export type Value = string
}
"#,
    )
    .expect("write namespace-dependent function return");

    let context = vue3_external_type_context_from_path(
        &types,
        &mut BTreeSet::new(),
        &Vue3TypeResolverContext::default(),
    )
    .expect("load namespace-dependent function context");
    assert_eq!(
        context
            .return_type_runtime_type_declarations
            .get("makeValue"),
        Some(&vec!["String".to_string()])
    );

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import { makeValue } from './types'
defineProps<{ value: ReturnType<typeof makeValue> }>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("value: { type: String, required: true }"));
    assert_eq!(
        script.deps.iter().cloned().collect::<BTreeSet<_>>(),
        [normalize_path_string(&types)].into_iter().collect()
    );
}

#[test]
fn vue3_namespace_projection_refreshes_function_props_options() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
export type DirectOptions = {
  directFile: { type: StringConstructor, required: true }
}
export declare function directOptions(): DirectOptions
export declare function declaredOptions(): Values.DeclaredOptions
export const valueOptions = (): Values.ValueOptions => ({
  valueFile: { type: String, required: true }
})
export namespace Values {
  export type DeclaredOptions = {
    declaredFile: { type: StringConstructor, required: true }
  }
  export type ValueOptions = {
    valueFile: { type: StringConstructor, required: true }
  }
}
"#,
    )
    .expect("write namespace-dependent props options functions");

    let context = vue3_external_type_context_from_path(
        &types,
        &mut BTreeSet::new(),
        &Vue3TypeResolverContext::default(),
    )
    .expect("load namespace-dependent props options context");
    assert!(context
        .return_type_props_options_declarations
        .contains_key("declaredOptions"));
    assert!(context
        .return_type_props_options_declarations
        .contains_key("directOptions"));
    assert!(context
        .return_type_props_options_declarations
        .contains_key("valueOptions"));

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type { declaredOptions, directOptions, valueOptions } from './types'
type Props =
  ExtractPropTypes<ReturnType<typeof directOptions>> &
  ExtractPropTypes<ReturnType<typeof declaredOptions>> &
  ExtractPropTypes<ReturnType<typeof valueOptions>>
defineProps<Props>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("directFile: { type: String, required: true }"));
    assert!(script
        .content
        .contains("declaredFile: { type: String, required: true }"));
    assert!(script
        .content
        .contains("valueFile: { type: String, required: true }"));
}

#[test]
fn vue3_split_namespaces_share_exported_types_in_both_directions() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
export namespace Merged {
  export type Forward = Later & { forwardValue: boolean }
  export interface Earlier { earlierValue: string }
}
export namespace Merged {
  export interface Later { laterValue: number }
  export type Props = Forward & Earlier
}
"#,
    )
    .expect("write split namespace types");

    let context = vue3_external_type_context_from_path(
        &types,
        &mut BTreeSet::new(),
        &Vue3TypeResolverContext::default(),
    )
    .expect("load split namespace context");
    for name in [
        "Merged.Forward",
        "Merged.Earlier",
        "Merged.Later",
        "Merged.Props",
    ] {
        assert!(context.declared_types.contains_key(name), "missing {name}");
    }

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Types from './types'
defineProps<Types.Merged.Props>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("forwardValue: { type: Boolean, required: true }"));
    assert!(script
        .content
        .contains("earlierValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("laterValue: { type: Number, required: true }"));
}

#[test]
fn vue3_interfaces_and_namespaces_merge_in_both_source_orders() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
export interface Before { beforeValue: string }
export namespace Before {
  export interface Meta { beforeMetaValue: boolean }
}
export namespace After {
  export interface Meta { afterMetaValue: number }
}
export interface After { afterValue: string }
"#,
    )
    .expect("write interface namespace merges");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Types from './types'
defineProps<Types.Before & Types.After & {
  beforeMeta: Types.Before.Meta
  afterMeta: Types.After.Meta
}>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("beforeValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("afterValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("beforeMeta: { type: Object, required: true }"));
    assert!(script
        .content
        .contains("afterMeta: { type: Object, required: true }"));
}

#[test]
fn vue3_namespace_classes_merge_with_interfaces_in_both_source_orders() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
export namespace PlainInterfaceFirst {
  export interface Item { plainInterfaceFirst: string }
}
export namespace PlainInterfaceFirst { export class Item {} }
export namespace PlainClassFirst { export class Item {} }
export namespace PlainClassFirst {
  export interface Item { plainClassFirst: number }
}
export namespace GenericInterfaceFirst {
  export interface Item<T> { genericInterfaceFirst: T }
}
export namespace GenericInterfaceFirst { export class Item<T> {} }
export namespace GenericClassFirst { export class Item<T> {} }
export namespace GenericClassFirst {
  export interface Item<T> { genericClassFirst: T }
}
"#,
    )
    .expect("write class interface namespace merges");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Types from './types'
defineProps<
  Types.PlainInterfaceFirst.Item &
  Types.PlainClassFirst.Item &
  Types.GenericInterfaceFirst.Item<boolean> &
  Types.GenericClassFirst.Item<string>
>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("plainInterfaceFirst: { type: String, required: true }"));
    assert!(script
        .content
        .contains("plainClassFirst: { type: Number, required: true }"));
    assert!(script
        .content
        .contains("genericInterfaceFirst: { type: Boolean, required: true }"));
    assert!(script
        .content
        .contains("genericClassFirst: { type: String, required: true }"));
}

#[test]
fn vue3_external_modules_with_empty_exports_hide_private_types() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        "export {}\ninterface Secret { leakedValue: string }",
    )
    .expect("write private external type");

    let context = vue3_external_type_context_from_path(
        &types,
        &mut BTreeSet::new(),
        &Vue3TypeResolverContext::default(),
    )
    .expect("load private external type context");
    assert!(!context.declared_types.contains_key("Secret"));
    assert!(!context.props_type_declarations.contains_key("Secret"));

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Types from './types'
defineProps<Types.Secret>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(!script.errors.is_empty());
    assert!(!script.content.contains("leakedValue:"));
}

#[test]
fn vue3_split_namespaces_merge_interface_declarations() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
export namespace Merged {
  export interface Props { firstValue: string }
}
export namespace Merged {
  export interface Props { secondValue: number }
}
"#,
    )
    .expect("write merged namespace interfaces");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Types from './types'
defineProps<Types.Merged.Props>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("firstValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("secondValue: { type: Number, required: true }"));
}

#[test]
fn vue3_split_namespaces_merge_generic_interfaces_with_block_scopes() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
export namespace Merged {
  interface Hidden { firstPrivate: number }
  export interface Props<T> extends Hidden { first: T }
}
export namespace Merged {
  interface Hidden { secondPrivate: boolean }
  export interface Props<T> extends Hidden { second: T }
}
"#,
    )
    .expect("write merged generic namespace interfaces");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Types from './types'
defineProps<Types.Merged.Props<string>>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    for prop in ["first", "second"] {
        assert!(script.content.contains(&format!(
            "{prop}: {{ type: String, required: true }}"
        )));
    }
    assert!(script
        .content
        .contains("firstPrivate: { type: Number, required: true }"));
    assert!(script
        .content
        .contains("secondPrivate: { type: Boolean, required: true }"));
}

#[test]
fn vue3_same_namespace_block_merges_generic_interfaces() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
export namespace Merged {
  export interface Props<T> { first: T }
  export interface Props<T> { second: T }
}
"#,
    )
    .expect("write same-block generic namespace interfaces");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Types from './types'
defineProps<Types.Merged.Props<string>>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    for prop in ["first", "second"] {
        assert!(script.content.contains(&format!(
            "{prop}: {{ type: String, required: true }}"
        )));
    }
}

#[test]
fn vue3_split_namespaces_merge_enum_runtime_types() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
export namespace Merged {
  export enum Kind { Text = 'text' }
}
export namespace Merged {
  export enum Kind { Numeric = 1 }
  export interface Props { kind: Kind }
}
"#,
    )
    .expect("write merged namespace enums");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Types from './types'
defineProps<Types.Merged.Props>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("kind: { type: [String, Number], required: true }"));
}

#[test]
fn vue3_split_namespaces_keep_private_members_block_local() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
interface Hidden { outerValue: string }
export namespace Merged {
  interface Hidden { privateValue: number }
  export interface Internal extends Hidden {}
}
export namespace Merged {
  export interface Props extends Hidden { visibleValue: boolean }
}
"#,
    )
    .expect("write namespace private types");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Types from './types'
defineProps<Types.Merged.Props>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("outerValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("visibleValue: { type: Boolean, required: true }"));
    assert!(!script.content.contains("privateValue:"));
}

#[test]
fn vue3_ambient_split_namespaces_merge_interface_declarations() {
    let dir = tempfile::tempdir().expect("temp dir");
    let global = dir.path().join("global.d.ts");
    std::fs::write(
        &global,
        r#"
declare namespace Ambient {
  interface Props { firstValue: string }
}
declare namespace Ambient {
  interface Props { secondValue: number }
}
"#,
    )
    .expect("write ambient merged namespace interfaces");

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
        .contains("firstValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("secondValue: { type: Number, required: true }"));
}

#[test]
fn vue3_nested_split_namespaces_share_exported_types() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
export namespace Root {
  export namespace Child {
    export interface Props extends Later { firstValue: string }
  }
}
export namespace Root {
  export namespace Child {
    export interface Later { laterValue: number }
  }
}
"#,
    )
    .expect("write nested split namespaces");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Types from './types'
defineProps<Types.Root.Child.Props>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("firstValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("laterValue: { type: Number, required: true }"));
}

#[test]
fn vue3_dotted_split_namespaces_share_exported_types() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
export namespace Root.Child {
  export interface Props extends Later { firstValue: string }
}
export namespace Root.Child {
  export interface Later { laterValue: number }
}
"#,
    )
    .expect("write dotted split namespaces");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Types from './types'
defineProps<Types.Root.Child.Props>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("firstValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("laterValue: { type: Number, required: true }"));
}

#[test]
fn vue3_nested_ambient_namespaces_inherit_ambient_visibility() {
    let dir = tempfile::tempdir().expect("temp dir");
    let global = dir.path().join("global.d.ts");
    std::fs::write(
        &global,
        r#"
declare namespace Ambient {
  namespace Nested {
    interface Props { nestedValue: string }
  }
}
"#,
    )
    .expect("write nested ambient namespace");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<Ambient.Nested.Props>()
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
        .contains("nestedValue: { type: String, required: true }"));
}

#[test]
fn vue3_global_augmentation_namespaces_are_ambient() {
    let dir = tempfile::tempdir().expect("temp dir");
    let global = dir.path().join("global.d.ts");
    std::fs::write(
        &global,
        r#"
export {}
declare global {
  namespace Augmented {
    interface Props { globalValue: number }
  }
}
"#,
    )
    .expect("write global namespace augmentation");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<Augmented.Props>()
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
        .contains("globalValue: { type: Number, required: true }"));
}

#[test]
fn vue3_global_augmentation_blocks_share_declaration_groups() {
    let dir = tempfile::tempdir().expect("temp dir");
    let global = dir.path().join("global.d.ts");
    std::fs::write(
        &global,
        r#"
export {}
declare global {
  namespace Shared {
    interface Props { firstValue: string }
  }
  interface RootProps { firstRoot: boolean }
}
declare global {
  namespace Shared {
    interface Props { secondValue: number }
  }
  interface RootProps { secondRoot: string }
}
"#,
    )
    .expect("write split global augmentations");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<Shared.Props & RootProps>()
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
        .contains("firstValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("secondValue: { type: Number, required: true }"));
    assert!(script
        .content
        .contains("firstRoot: { type: Boolean, required: true }"));
    assert!(script
        .content
        .contains("secondRoot: { type: String, required: true }"));
}

#[test]
fn vue3_global_augmentation_uses_module_lexical_types_and_preserves_deps() {
    let dir = tempfile::tempdir().expect("temp dir");
    let leaf = dir.path().join("leaf.ts");
    std::fs::write(
        &leaf,
        "export interface Leaf { leafValue: string }",
    )
    .expect("write global augmentation dependency");
    let global = dir.path().join("global.d.ts");
    std::fs::write(
        &global,
        r#"
import type { Leaf } from './leaf'
export {}
declare global {
  interface GlobalBase extends Leaf { globalValue: boolean }
  interface GlobalProps extends LocalAlias {}
}
type LocalAlias = GlobalBase & { localValue: number }
"#,
    )
    .expect("write lexical global augmentation");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<GlobalProps>()
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
        .contains("leafValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("globalValue: { type: Boolean, required: true }"));
    assert!(script
        .content
        .contains("localValue: { type: Number, required: true }"));
    assert_eq!(
        script.deps.iter().cloned().collect::<BTreeSet<_>>(),
        [normalize_path_string(&global), normalize_path_string(&leaf)]
            .into_iter()
            .collect()
    );
}

#[test]
fn vue3_global_augmentation_keeps_module_lexical_types_private() {
    let dir = tempfile::tempdir().expect("temp dir");
    let global = dir.path().join("global.d.ts");
    std::fs::write(
        &global,
        r#"
export {}
interface ModuleOnly { hiddenValue: string }
namespace ModuleNamespace {
  interface AmbientBase { ambientValue: boolean }
}
interface Shared { moduleSharedValue: Date }
enum SharedEnum { Module = 'module' }
declare global {
  interface Shared { globalSharedValue: string }
  enum SharedEnum { Global = 1 }
  interface GlobalOnly extends ModuleOnly, ModuleNamespace.AmbientBase, Shared {
    visibleValue: number
    enumValue: SharedEnum
  }
}
"#,
    )
    .expect("write isolated global augmentation");
    let resolver = Vue3TypeResolverContext::default();
    let context = vue3_global_type_context(
        &dir.path().join("Comp.vue").to_string_lossy(),
        &[global.to_string_lossy().to_string()],
        &resolver,
    );

    let names = vue3_type_context_names(&context);
    assert!(names.contains("GlobalOnly"));
    assert!(!names.contains("ModuleOnly"));
    assert!(!names.iter().any(|name| name.starts_with("ModuleNamespace")));
    let props = context
        .props_type_declarations
        .get("GlobalOnly")
        .expect("global augmentation props");
    let keys = props
        .members
        .iter()
        .map(|prop| prop.key.as_str())
        .collect::<BTreeSet<_>>();
    assert!(keys.contains("hiddenValue"));
    assert!(keys.contains("ambientValue"));
    assert!(keys.contains("globalSharedValue"));
    assert!(keys.contains("visibleValue"));
    assert!(!keys.contains("moduleSharedValue"));
    let enum_prop = props
        .members
        .iter()
        .find(|prop| prop.key == "enumValue")
        .expect("global enum prop");
    assert_eq!(enum_prop.types, ["Number"]);
}

#[test]
fn vue3_global_namespace_and_module_namespace_import_keep_separate_scopes() {
    let dir = tempfile::tempdir().expect("temp dir");
    let base = dir.path().join("base-global.d.ts");
    std::fs::write(
        &base,
        "declare namespace Types { interface BaseOnly { baseValue: Date } }",
    )
    .expect("write base global namespace");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        "export interface ModuleOnly { moduleValue: string }",
    )
    .expect("write module namespace import");
    let global = dir.path().join("global.d.ts");
    std::fs::write(
        &global,
        r#"
import type * as Types from './types'
export {}
type ModuleAlias = Types.ModuleOnly
type BaseLeakAlias = Types.BaseOnly
declare global {
  namespace Types {
    interface GlobalOnly { globalValue: number }
  }
  interface GlobalProps extends Types.GlobalOnly {}
  interface ModuleProps extends ModuleAlias {}
  interface LeakedProps extends Types.ModuleOnly {}
  interface BaseLeakedProps extends BaseLeakAlias {}
}
"#,
    )
    .expect("write colliding namespace scopes");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<GlobalProps & ModuleProps>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(
        &descriptor,
        SfcScriptCompileOptions {
            global_type_files: vec![
                base.to_string_lossy().to_string(),
                global.to_string_lossy().to_string(),
            ],
            ..SfcScriptCompileOptions::default()
        },
    );

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("globalValue: { type: Number, required: true }"));
    assert!(script
        .content
        .contains("moduleValue: { type: String, required: true }"));
    assert_eq!(
        script.deps.iter().cloned().collect::<BTreeSet<_>>(),
        [normalize_path_string(&global), normalize_path_string(&types)]
            .into_iter()
            .collect()
    );
    let context = vue3_global_type_context(
        &filename.to_string_lossy(),
        &[
            base.to_string_lossy().to_string(),
            global.to_string_lossy().to_string(),
        ],
        &Vue3TypeResolverContext::default(),
    );
    let leaked = context
        .props_type_declarations
        .get("LeakedProps")
        .expect("unresolved shadowed namespace member");
    assert!(!leaked.errors.is_empty());
    assert!(!leaked.members.iter().any(|prop| prop.key == "moduleValue"));
    let base_leaked = context
        .props_type_declarations
        .get("BaseLeakedProps")
        .expect("shadowed base namespace member");
    assert!(!base_leaked.errors.is_empty());
    assert!(!base_leaked
        .members
        .iter()
        .any(|prop| prop.key == "baseValue"));
}

#[test]
fn vue3_global_module_imports_exactly_shadow_base_type_projections() {
    let dir = tempfile::tempdir().expect("temp dir");
    let base = dir.path().join("base-global.d.ts");
    std::fs::write(
        &base,
        r#"
interface Foo { leakedValue: string }
interface MissingFoo { missingLeakValue: number }
"#,
    )
    .expect("write base global aliases");
    let types = dir.path().join("types.ts");
    std::fs::write(&types, "export type Foo = string").expect("write primitive import");
    let global = dir.path().join("global.d.ts");
    std::fs::write(
        &global,
        r#"
import type { Foo } from './types'
import type { MissingFoo } from './missing'
export {}
type ImportedAlias = Foo
type MissingAlias = MissingFoo
declare global {
  interface ImportedProps { value: ImportedAlias }
  interface ImportedLeak extends Foo {}
  interface MissingLeak extends MissingAlias {}
}
"#,
    )
    .expect("write exact import shadows");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<ImportedProps>()
</script>"#;
    let files = vec![
        base.to_string_lossy().to_string(),
        global.to_string_lossy().to_string(),
    ];
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(
        &descriptor,
        SfcScriptCompileOptions {
            global_type_files: files.clone(),
            ..SfcScriptCompileOptions::default()
        },
    );

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("value: { type: String, required: true }"));
    assert_eq!(
        script.deps.iter().cloned().collect::<BTreeSet<_>>(),
        [normalize_path_string(&global), normalize_path_string(&types)]
            .into_iter()
            .collect()
    );

    let context = vue3_global_type_context(
        &filename.to_string_lossy(),
        &files,
        &Vue3TypeResolverContext::default(),
    );
    let imported_leak = context
        .props_type_declarations
        .get("ImportedLeak")
        .expect("incompatible imported base");
    assert!(!imported_leak.errors.is_empty());
    assert!(!imported_leak
        .members
        .iter()
        .any(|prop| prop.key == "leakedValue"));
    let missing_leak = context
        .props_type_declarations
        .get("MissingLeak")
        .expect("missing imported base");
    assert!(!missing_leak.errors.is_empty());
    assert!(!missing_leak
        .members
        .iter()
        .any(|prop| prop.key == "missingLeakValue"));
}

#[test]
fn vue3_global_generic_alias_captures_module_lexical_environment() {
    let dir = tempfile::tempdir().expect("temp dir");
    let leaf = dir.path().join("leaf.ts");
    std::fs::write(
        &leaf,
        "export interface Leaf { leafValue: string }",
    )
    .expect("write generic global dependency");
    let global = dir.path().join("global.d.ts");
    std::fs::write(
        &global,
        r#"
import type { Leaf } from './leaf'
export {}
type LocalBox<T> = T & { localValue: number }
declare global {
  type GenericGlobalProps<T> = LocalBox<T> & Leaf
}
"#,
    )
    .expect("write generic global augmentation");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<GenericGlobalProps<{ componentValue: boolean }>>()
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
        .contains("componentValue: { type: Boolean, required: true }"));
    assert!(script
        .content
        .contains("localValue: { type: Number, required: true }"));
    assert!(script
        .content
        .contains("leafValue: { type: String, required: true }"));
    assert_eq!(
        script.deps.iter().cloned().collect::<BTreeSet<_>>(),
        [normalize_path_string(&global), normalize_path_string(&leaf)]
            .into_iter()
            .collect()
    );
}

#[test]
fn vue3_global_augmentation_groups_union_interface_dependencies() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first = dir.path().join("first.ts");
    let second = dir.path().join("second.ts");
    std::fs::write(
        &first,
        "export interface First { firstValue: string }",
    )
    .expect("write first global dependency");
    std::fs::write(
        &second,
        "export interface Second { secondValue: number }",
    )
    .expect("write second global dependency");
    let global = dir.path().join("global.d.ts");
    std::fs::write(
        &global,
        r#"
import type { First } from './first'
import type { Second } from './second'
export {}
declare global {
  namespace Shared { interface Props extends First {} }
}
declare global {
  namespace Shared { interface Props extends Second {} }
}
"#,
    )
    .expect("write global dependency groups");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<Shared.Props>()
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
        .contains("firstValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("secondValue: { type: Number, required: true }"));
    assert_eq!(
        script.deps.iter().cloned().collect::<BTreeSet<_>>(),
        [
            normalize_path_string(&global),
            normalize_path_string(&first),
            normalize_path_string(&second),
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn vue3_ambient_global_forward_aliases_preserve_import_type_dependencies() {
    let dir = tempfile::tempdir().expect("temp dir");
    let leaf = dir.path().join("leaf.ts");
    std::fs::write(
        &leaf,
        "export interface Leaf { leafValue: string }",
    )
    .expect("write ambient global dependency");
    let global = dir.path().join("global.d.ts");
    std::fs::write(
        &global,
        "interface GlobalProps extends Alias {}\ntype Alias = import('./leaf').Leaf",
    )
    .expect("write ambient global forward alias");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<GlobalProps>()
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
        .contains("leafValue: { type: String, required: true }"));
    assert_eq!(
        script.deps.iter().cloned().collect::<BTreeSet<_>>(),
        [normalize_path_string(&global), normalize_path_string(&leaf)]
            .into_iter()
            .collect()
    );
}

#[test]
fn vue3_definition_file_namespaces_are_implicitly_ambient() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.d.ts");
    std::fs::write(
        &types,
        r#"
export namespace Types {
  interface Props { implicitValue: boolean }
}
"#,
    )
    .expect("write implicit ambient namespace");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Module from './types'
defineProps<Module.Types.Props>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("implicitValue: { type: Boolean, required: true }"));
}

#[test]
fn vue3_definition_file_namespace_specifiers_preserve_members_and_dependencies() {
    let dir = tempfile::tempdir().expect("temp dir");
    let leaf = dir.path().join("leaf.ts");
    std::fs::write(
        &leaf,
        r#"
export interface DirectLeaf { directValue: string }
export interface RenamedLeaf { renamedValue: number }
"#,
    )
    .expect("write namespace specifier dependency");
    let types = dir.path().join("types.d.ts");
    std::fs::write(
        &types,
        r#"
declare namespace Direct {
  type Props = import('./leaf').DirectLeaf
}
declare namespace Local {
  namespace Nested {
    type Props = import('./leaf').RenamedLeaf
  }
}
export { Direct, Local as Public }
"#,
    )
    .expect("write ambient namespace specifier exports");
    let facade = dir.path().join("facade.ts");
    std::fs::write(&facade, "export { Public as Facade } from './types'")
        .expect("write namespace specifier facade");

    let context = vue3_external_type_context_from_path(
        &types,
        &mut BTreeSet::new(),
        &Vue3TypeResolverContext::default(),
    )
    .expect("load ambient namespace specifier context");
    assert!(context.props_type_declarations.contains_key("Direct.Props"));
    assert!(context
        .props_type_declarations
        .contains_key("Public.Nested.Props"));
    assert!(!context
        .props_type_declarations
        .contains_key("Local.Nested.Props"));

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type { Direct } from './types'
import type { Facade } from './facade'
defineProps<Direct.Props & Facade.Nested.Props>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("directValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("renamedValue: { type: Number, required: true }"));
    assert_eq!(
        script.deps.iter().cloned().collect::<BTreeSet<_>>(),
        [
            normalize_path_string(&types),
            normalize_path_string(&facade),
            normalize_path_string(&leaf),
        ]
            .into_iter()
            .collect()
    );
}

#[test]
fn vue3_regular_namespace_specifier_exports_keep_private_members_hidden() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
namespace Local {
  export interface PublicProps { publicValue: string }
  interface PrivateProps { privateValue: boolean }
}
declare namespace Declared {
  interface AmbientProps { ambientValue: boolean }
}
export { Local as Public, Declared }
"#,
    )
    .expect("write regular namespace specifier export");

    let context = vue3_external_type_context_from_path(
        &types,
        &mut BTreeSet::new(),
        &Vue3TypeResolverContext::default(),
    )
    .expect("load regular namespace specifier context");
    assert!(context
        .props_type_declarations
        .contains_key("Public.PublicProps"));
    assert!(!context
        .props_type_declarations
        .contains_key("Public.PrivateProps"));
    assert!(!context
        .props_type_declarations
        .contains_key("Local.PublicProps"));
    assert!(context
        .props_type_declarations
        .contains_key("Declared.AmbientProps"));

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type { Declared, Public } from './types'
defineProps<Public.PublicProps & Declared.AmbientProps>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("publicValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("ambientValue: { type: Boolean, required: true }"));
    assert!(!script.content.contains("privateValue:"));
}

#[test]
fn vue3_namespace_specifier_alias_chains_use_unmodified_sources() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
namespace A {
  export interface Props { aValue: string }
}
namespace B {
  export interface Props { bValue: number }
}
export { A as B, B as C }
"#,
    )
    .expect("write chained namespace specifier aliases");

    let context = vue3_external_type_context_from_path(
        &types,
        &mut BTreeSet::new(),
        &Vue3TypeResolverContext::default(),
    )
    .expect("load chained namespace specifier context");
    let b = context
        .props_type_declarations
        .get("B.Props")
        .expect("project A through exported B");
    assert!(b.members.iter().any(|member| member.key == "aValue"));
    assert!(!b.members.iter().any(|member| member.key == "bValue"));
    let c = context
        .props_type_declarations
        .get("C.Props")
        .expect("project original B through exported C");
    assert!(c.members.iter().any(|member| member.key == "bValue"));
    assert!(!c.members.iter().any(|member| member.key == "aValue"));

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type { B, C } from './types'
defineProps<B.Props & C.Props>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("aValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("bValue: { type: Number, required: true }"));
}

#[test]
fn vue3_type_specifier_alias_chains_use_unmodified_sources() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
interface A { aValue: string }
interface B { bValue: number }
export { A as B, B as C }
"#,
    )
    .expect("write chained type specifier aliases");

    let context = vue3_external_type_context_from_path(
        &types,
        &mut BTreeSet::new(),
        &Vue3TypeResolverContext::default(),
    )
    .expect("load chained type specifier context");
    let b = context
        .props_type_declarations
        .get("B")
        .expect("project A through exported B");
    assert!(b.members.iter().any(|member| member.key == "aValue"));
    assert!(!b.members.iter().any(|member| member.key == "bValue"));
    let c = context
        .props_type_declarations
        .get("C")
        .expect("project original B through exported C");
    assert!(c.members.iter().any(|member| member.key == "bValue"));
    assert!(!c.members.iter().any(|member| member.key == "aValue"));

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type { B, C } from './types'
defineProps<B & C>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("aValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("bValue: { type: Number, required: true }"));
}

#[test]
fn vue3_nested_namespaces_resolve_intermediate_ancestor_members() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    let leaf = dir.path().join("leaf.ts");
    std::fs::write(&leaf, "export interface Leaf { leafValue: string }")
        .expect("write ancestor namespace dependency");
    std::fs::write(
        &types,
        r#"
import type { Leaf } from './leaf'
export namespace Root {
  export namespace A {
    export interface Base extends Leaf { ancestorValue: number }
  }
}
export namespace Root {
  export namespace A {
    export namespace B {
      export interface Props extends Base { nestedValue: boolean }
    }
  }
}
"#,
    )
    .expect("write intermediate ancestor namespaces");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Types from './types'
defineProps<Types.Root.A.B.Props>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("leafValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("ancestorValue: { type: Number, required: true }"));
    assert!(script
        .content
        .contains("nestedValue: { type: Boolean, required: true }"));
    assert_eq!(
        script.deps.iter().cloned().collect::<BTreeSet<_>>(),
        [normalize_path_string(&types), normalize_path_string(&leaf)]
            .into_iter()
            .collect()
    );
}

#[test]
fn vue3_namespace_members_resolve_merged_interfaces_in_their_block() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
export namespace Merged {
  export interface Base { firstValue: string }
}
export namespace Merged {
  export interface Base { secondValue: number }
  export type Props = Base
}
"#,
    )
    .expect("write namespace-local merged interface reference");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Types from './types'
defineProps<Types.Merged.Props>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("firstValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("secondValue: { type: Number, required: true }"));
}

#[test]
fn vue3_namespaces_preserve_transitive_generic_aliases() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
type Later<T> = { value: T }
type Box<T> = Later<T>
export namespace Nested {
  export type Props = Box<string>
}
"#,
    )
    .expect("write transitive generic namespace aliases");

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
        .contains("value: { type: String, required: true }"));
}

#[test]
fn vue3_namespace_generics_keep_their_outer_lexical_aliases() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
type Later<T> = { outerValue: T }
type Box<T> = Later<T>
export namespace Scoped {
  type Later<T> = { innerValue: T }
  export type Props = Box<string>
}
"#,
    )
    .expect("write shadowed generic aliases");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Types from './types'
defineProps<Types.Scoped.Props>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("outerValue: { type: String, required: true }"));
    assert!(!script.content.contains("innerValue:"));
}

#[test]
fn vue3_namespaces_export_lazy_transitive_generic_aliases() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
type Later<T> = { value: T }
export namespace Scoped {
  export type Box<T> = Later<T>
}
"#,
    )
    .expect("write lazy transitive generic alias");

    let context = vue3_external_type_context_from_path(
        &types,
        &mut BTreeSet::new(),
        &Vue3TypeResolverContext::default(),
    )
    .expect("load lazy transitive generic alias context");
    assert!(context.generic_type_aliases.contains_key("Scoped.Box"));

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Types from './types'
defineProps<Types.Scoped.Box<string>>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("value: { type: String, required: true }"));
}

#[test]
fn vue3_namespaces_forward_merged_generic_interfaces_through_aliases() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types.ts");
    std::fs::write(
        &types,
        r#"
export namespace Scoped {
  interface Base<T> { firstValue: T }
  interface Base<T> { secondValue: T }
  export type Box<T> = Base<T>
}
"#,
    )
    .expect("write forwarded merged generic interface");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
import type * as Types from './types'
defineProps<Types.Scoped.Box<string>>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("firstValue: { type: String, required: true }"));
    assert!(script
        .content
        .contains("secondValue: { type: String, required: true }"));
}

#[test]
fn vue3_global_files_merge_interfaces_and_refresh_earlier_dependents() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first_leaf = dir.path().join("first.ts");
    let second_leaf = dir.path().join("second.ts");
    std::fs::write(
        &first_leaf,
        "export interface First { firstValue: string }",
    )
    .expect("write first global interface dependency");
    std::fs::write(
        &second_leaf,
        "export interface Second { secondValue: number }",
    )
    .expect("write second global interface dependency");
    let first = dir.path().join("first-global.d.ts");
    let second = dir.path().join("second-global.d.ts");
    std::fs::write(
        &first,
        r#"
import type { First } from './first'
export {}
declare global {
  interface Shared extends First {}
  interface Consumer extends Shared {}
}
"#,
    )
    .expect("write first global interface fragment");
    std::fs::write(
        &second,
        r#"
import type { Second } from './second'
export {}
declare global {
  interface Shared extends Second {}
}
"#,
    )
    .expect("write second global interface fragment");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<Consumer>()
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

        assert!(
            script.errors.is_empty(),
            "{:?} for {files:?}",
            script.errors
        );
        assert!(script
            .content
            .contains("firstValue: { type: String, required: true }"), "{}", script.content);
        assert!(script
            .content
            .contains("secondValue: { type: Number, required: true }"), "{}", script.content);
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
fn vue3_global_files_merge_qualified_namespace_interfaces_in_any_order() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first = dir.path().join("first-global.d.ts");
    let second = dir.path().join("second-global.d.ts");
    std::fs::write(
        &first,
        "declare namespace Shared { interface Props { firstValue: string } }",
    )
    .expect("write first global namespace fragment");
    std::fs::write(
        &second,
        "declare namespace Shared { interface Props { secondValue: number } }",
    )
    .expect("write second global namespace fragment");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<Shared.Props>()
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
        assert!(script
            .content
            .contains("firstValue: { type: String, required: true }"));
        assert!(script
            .content
            .contains("secondValue: { type: Number, required: true }"));
    }
}

#[test]
fn vue3_global_files_merge_classes_and_interfaces_in_any_order() {
    let dir = tempfile::tempdir().expect("temp dir");
    let class = dir.path().join("class-global.d.ts");
    let interface = dir.path().join("interface-global.d.ts");
    std::fs::write(
        &class,
        r#"
declare class SharedClass {
  sharedValue: string
  method(value: string): string
  method(value: number): number
}
interface Consumer extends SharedClass { consumerValue: boolean }
"#,
    )
    .expect("write global class declaration and consumer");
    std::fs::write(
        &interface,
        r#"
interface SharedClass {
  sharedValue: string
  method(value: boolean): boolean
  mergedValue: string
}
"#,
    )
    .expect("write global interface declaration");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<Consumer>()
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
                .contains("mergedValue: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(
            script
                .content
                .contains("sharedValue: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(
            script
                .content
                .contains("consumerValue: { type: Boolean, required: true }"),
            "{}",
            script.content
        );
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [normalize_path_string(&class), normalize_path_string(&interface)]
                .into_iter()
                .collect()
        );
    }
}

#[test]
fn vue3_global_class_accessors_merge_as_properties_in_any_order() {
    let dir = tempfile::tempdir().expect("temp dir");
    let class = dir.path().join("accessor-class.d.ts");
    let interface = dir.path().join("accessor-interface.d.ts");
    std::fs::write(
        &class,
        r#"
declare class AccessorClass {
  get getterValue(): string
  set setterValue(value: number)
  get divergentValue(): boolean
  set divergentValue(value: boolean | null)
  set reverseValue(value: string | number)
  get reverseValue(): string
  accessor automaticValue: Date
}
interface AccessorConsumer extends AccessorClass { consumerValue: boolean }
"#,
    )
    .expect("write accessor class");
    std::fs::write(
        &interface,
        r#"
interface AccessorClass {
  getterValue: string
  setterValue: number
  divergentValue: boolean
  reverseValue: string
  automaticValue: Date
  mergedValue: string
}
"#,
    )
    .expect("write matching accessor interface");
    let filename = dir.path().join("Comp.vue");

    for files in [
        vec![class.clone(), interface.clone()],
        vec![interface.clone(), class.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in ["AccessorClass", "AccessorConsumer"] {
            assert!(!context.silent_unresolved_type_names.contains(name));
            assert!(vue3_type_context_has_name(&context, name));
        }
    }
}

#[test]
fn vue3_global_class_accessors_reject_incompatible_interface_members() {
    let dir = tempfile::tempdir().expect("temp dir");
    let class = dir.path().join("incompatible-accessor-classes.d.ts");
    let interface = dir.path().join("incompatible-accessor-interfaces.d.ts");
    std::fs::write(
        &class,
        r#"
declare class GetterConflict { get value(): string }
declare class SetterConflict { set value(value: string) }
declare class AutomaticConflict { accessor value: string }
declare class AccessorMethodConflict { get value(): string }
declare class GetterReadonlyConflict { get value(): string }
declare class OptionalMethodConflict { value?(): void; value(): void }
"#,
    )
    .expect("write incompatible accessor classes");
    std::fs::write(
        &interface,
        r#"
interface GetterConflict { value: number }
interface SetterConflict { value: number }
interface AutomaticConflict { value: number }
interface AccessorMethodConflict { value(): string }
interface GetterReadonlyConflict { readonly value: string }
type GetterConflictConsumer = GetterConflict
type SetterConflictConsumer = SetterConflict
type AutomaticConflictConsumer = AutomaticConflict
type AccessorMethodConflictConsumer = AccessorMethodConflict
type GetterReadonlyConflictConsumer = GetterReadonlyConflict
type OptionalMethodConflictConsumer = OptionalMethodConflict
"#,
    )
    .expect("write incompatible accessor interfaces");
    let filename = dir.path().join("Comp.vue");

    for files in [
        vec![class.clone(), interface.clone()],
        vec![interface.clone(), class.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in [
            "GetterConflict",
            "SetterConflict",
            "AutomaticConflict",
            "AccessorMethodConflict",
            "GetterReadonlyConflict",
            "OptionalMethodConflict",
            "GetterConflictConsumer",
            "SetterConflictConsumer",
            "AutomaticConflictConsumer",
            "AccessorMethodConflictConsumer",
            "GetterReadonlyConflictConsumer",
            "OptionalMethodConflictConsumer",
        ] {
            assert!(context.silent_unresolved_type_names.contains(name));
            assert!(!vue3_type_context_has_name(&context, name));
        }
    }
}

#[test]
fn vue3_duplicate_global_index_signatures_fail_closed_in_any_order() {
    let dir = tempfile::tempdir().expect("temp dir");
    let same_file = dir.path().join("same-file-index.d.ts");
    std::fs::write(
        &same_file,
        r#"
interface SameFileIndex { [key: string]: string }
interface SameFileIndex { readonly [other: string]: string }
type SameFileIndexConsumer = SameFileIndex
"#,
    )
    .expect("write same-file duplicate index signatures");
    let filename = dir.path().join("Comp.vue");
    let context = vue3_global_type_context(
        &filename.to_string_lossy(),
        &[same_file.to_string_lossy().to_string()],
        &Vue3TypeResolverContext::default(),
    );
    for name in ["SameFileIndex", "SameFileIndexConsumer"] {
        assert!(context.silent_unresolved_type_names.contains(name));
        assert!(!vue3_type_context_has_name(&context, name));
    }

    let first = dir.path().join("first-index.d.ts");
    let second = dir.path().join("second-index.d.ts");
    let class = dir.path().join("class-index.d.ts");
    let interface = dir.path().join("class-index-interface.d.ts");
    let consumer = dir.path().join("index-consumer.d.ts");
    std::fs::write(&first, "interface CrossFileIndex { [key: string]: string }")
        .expect("write first cross-file index signature");
    std::fs::write(
        &second,
        "interface CrossFileIndex { [other: string]: string }",
    )
    .expect("write second cross-file index signature");
    std::fs::write(&class, "declare class ClassIndex { [key: string]: string }")
        .expect("write class index signature");
    std::fs::write(
        &interface,
        "interface ClassIndex { [other: string]: string }",
    )
    .expect("write interface index signature");
    std::fs::write(
        &consumer,
        "type CrossFileIndexConsumer = CrossFileIndex\ntype ClassIndexConsumer = ClassIndex",
    )
    .expect("write index signature consumers");
    for files in [
        vec![
            first.clone(),
            second.clone(),
            class.clone(),
            interface.clone(),
            consumer.clone(),
        ],
        vec![
            consumer.clone(),
            interface.clone(),
            class.clone(),
            second.clone(),
            first.clone(),
        ],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in [
            "CrossFileIndex",
            "CrossFileIndexConsumer",
            "ClassIndex",
            "ClassIndexConsumer",
        ] {
            assert!(context.silent_unresolved_type_names.contains(name));
            assert!(!vue3_type_context_has_name(&context, name));
        }
    }
}

#[test]
fn vue3_distinct_global_index_domains_remain_mergeable() {
    let dir = tempfile::tempdir().expect("temp dir");
    let string = dir.path().join("string-index.d.ts");
    let number = dir.path().join("number-index.d.ts");
    let symbol = dir.path().join("symbol-index.d.ts");
    let consumer = dir.path().join("distinct-index-consumer.d.ts");
    std::fs::write(
        &string,
        "interface DistinctIndexDomains { [key: string]: string | number }",
    )
    .expect("write string index signature");
    std::fs::write(
        &number,
        "interface DistinctIndexDomains { [key: number]: number }",
    )
    .expect("write number index signature");
    std::fs::write(
        &symbol,
        "interface DistinctIndexDomains { [key: symbol]: Date; visible: boolean }",
    )
    .expect("write symbol index signature");
    std::fs::write(
        &consumer,
        "type DistinctIndexDomainsConsumer = DistinctIndexDomains",
    )
    .expect("write distinct index domain consumer");
    let filename = dir.path().join("Comp.vue");
    for files in [
        vec![
            string.clone(),
            number.clone(),
            symbol.clone(),
            consumer.clone(),
        ],
        vec![consumer.clone(), symbol.clone(), number.clone(), string.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in ["DistinctIndexDomains", "DistinctIndexDomainsConsumer"] {
            assert!(!context.silent_unresolved_type_names.contains(name));
            assert!(vue3_type_context_has_name(&context, name));
        }
    }
}

#[test]
fn vue3_global_files_merge_enums_and_refresh_runtime_types_in_any_order() {
    let dir = tempfile::tempdir().expect("temp dir");
    let text = dir.path().join("text-enum-global.d.ts");
    let numeric = dir.path().join("numeric-enum-global.d.ts");
    std::fs::write(&text, "declare enum SharedKind { Text = 'text' }")
        .expect("write string enum fragment");
    std::fs::write(
        &numeric,
        r#"
declare enum SharedKind { Numeric = 1 }
interface EnumProps { kind: SharedKind }
"#,
    )
    .expect("write numeric enum fragment and consumer");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<EnumProps>()
</script>"#;
    for files in [
        vec![text.clone(), numeric.clone()],
        vec![numeric.clone(), text.clone()],
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
                .contains("kind: { type: [String, Number], required: true }")
                || script
                    .content
                    .contains("kind: { type: [Number, String], required: true }"),
            "{}",
            script.content
        );
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [normalize_path_string(&text), normalize_path_string(&numeric)]
                .into_iter()
                .collect()
        );
    }
}

#[test]
fn vue3_later_unique_global_declarations_refresh_earlier_consumers_in_any_order() {
    let dir = tempfile::tempdir().expect("temp dir");
    let consumer = dir.path().join("consumer-global.d.ts");
    let later = dir.path().join("later-global.d.ts");
    std::fs::write(
        &consumer,
        "interface Consumer extends Later { consumerValue: boolean }",
    )
    .expect("write forward global consumer");
    std::fs::write(&later, "interface Later { laterValue: string }")
        .expect("write later unique global declaration");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<Consumer>()
</script>"#;
    for files in [
        vec![consumer.clone(), later.clone()],
        vec![later.clone(), consumer.clone()],
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
        assert!(script
            .content
            .contains("consumerValue: { type: Boolean, required: true }"));
        assert!(
            script
                .content
                .contains("laterValue: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [normalize_path_string(&consumer), normalize_path_string(&later)]
                .into_iter()
                .collect()
        );
    }
}

#[test]
fn vue3_global_type_and_value_declarations_coexist_with_complete_dependencies() {
    let dir = tempfile::tempdir().expect("temp dir");
    let interface = dir.path().join("interface-global.d.ts");
    let function = dir.path().join("function-global.d.ts");
    std::fs::write(
        &interface,
        "interface Shared { interfaceValue: string }",
    )
    .expect("write interface declaration");
    std::fs::write(
        &function,
        "declare function Shared(): { functionValue: number }",
    )
    .expect("write value declaration");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<Shared & { result: ReturnType<typeof Shared> }>()
</script>"#;
    for files in [
        vec![interface.clone(), function.clone()],
        vec![function.clone(), interface.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        assert!(context
            .return_type_runtime_type_declarations
            .contains_key("Shared"));
        assert_eq!(
            context.type_deps.get("Shared"),
            Some(
                &[normalize_path_string(&interface), normalize_path_string(&function)]
                    .into_iter()
                    .collect()
            )
        );
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
            "{}",
            script.content
        );
        assert!(
            script
                .content
                .contains("result: { type: Object, required: true }"),
            "{}",
            script.content
        );
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [
                normalize_path_string(&interface),
                normalize_path_string(&function),
            ]
            .into_iter()
            .collect()
        );
    }
}

#[test]
fn vue3_incompatible_global_type_declarations_fail_closed_in_any_order() {
    let dir = tempfile::tempdir().expect("temp dir");
    let alias = dir.path().join("alias-global.d.ts");
    let interface = dir.path().join("interface-global.d.ts");
    std::fs::write(&alias, "type Shared = { aliasValue: string }")
        .expect("write type alias");
    std::fs::write(&interface, "interface Shared { interfaceValue: number }")
        .expect("write incompatible interface");

    let filename = dir.path().join("Comp.vue");
    let expected_deps = [normalize_path_string(&alias), normalize_path_string(&interface)]
        .into_iter()
        .collect::<BTreeSet<_>>();
    for files in [
        vec![alias.clone(), interface.clone()],
        vec![interface.clone(), alias.clone()],
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
        assert!(context.silent_unresolved_type_names.contains("Shared"));
        assert_eq!(context.type_deps.get("Shared"), Some(&expected_deps));
    }
}

#[test]
fn vue3_global_type_conflicts_preserve_the_independent_value_space() {
    let dir = tempfile::tempdir().expect("temp dir");
    let alias = dir.path().join("alias-global.d.ts");
    let interface = dir.path().join("interface-global.d.ts");
    let function = dir.path().join("function-global.d.ts");
    std::fs::write(&alias, "type Shared = { aliasValue: string }")
        .expect("write conflicting type alias");
    std::fs::write(&interface, "interface Shared { interfaceValue: number }")
        .expect("write conflicting interface");
    std::fs::write(&function, "declare function Shared(): string")
        .expect("write independent value declaration");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<{ result: ReturnType<typeof Shared> }>()
</script>"#;
    let expected_deps = [
        normalize_path_string(&alias),
        normalize_path_string(&interface),
        normalize_path_string(&function),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    for files in [
        vec![function.clone(), alias.clone(), interface.clone()],
        vec![alias.clone(), interface.clone(), function.clone()],
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
        assert!(context.silent_unresolved_type_names.contains("Shared"));
        assert!(!context.props_type_declarations.contains_key("Shared"));
        assert!(!context
            .props_options_type_declarations
            .contains_key("Shared"));
        assert_eq!(
            context.return_type_runtime_type_declarations.get("Shared"),
            Some(&vec!["String".to_string()])
        );
        assert_eq!(context.type_deps.get("Shared"), Some(&expected_deps));

        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: global_files,
                ..SfcScriptCompileOptions::default()
            },
        );
        assert!(
            script.errors.is_empty(),
            "{:?} for {files:?}",
            script.errors
        );
        assert!(
            script
                .content
                .contains("result: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            expected_deps
        );
    }
}

#[test]
fn vue3_global_value_projection_provenance_is_order_independent() {
    let dir = tempfile::tempdir().expect("temp dir");
    let function = dir.path().join("function-global.d.ts");
    std::fs::write(&function, "declare function Shared(): string")
        .expect("write value declaration");
    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<{ result: ReturnType<typeof Shared> }>()
</script>"#;

    for (case, alias_source) in [
        ("object", "type Shared = { leaked: string }"),
        ("callable", "type Shared = () => number"),
    ] {
        let alias = dir.path().join(format!("{case}-alias-global.d.ts"));
        std::fs::write(&alias, alias_source).expect("write type declaration");
        let expected_deps = [normalize_path_string(&alias), normalize_path_string(&function)]
            .into_iter()
            .collect::<BTreeSet<_>>();
        for files in [
            vec![function.clone(), alias.clone()],
            vec![alias.clone(), function.clone()],
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
            assert!(context.silent_unresolved_type_names.contains("Shared"));
            assert_eq!(
                context.return_type_runtime_type_declarations.get("Shared"),
                Some(&vec!["String".to_string()]),
                "case {case}, files {files:?}"
            );
            assert!(!context
                .props_options_type_declarations
                .contains_key("Shared"));
            assert_eq!(context.type_deps.get("Shared"), Some(&expected_deps));

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
            assert!(
                script
                    .content
                    .contains("result: { type: String, required: true }"),
                "{}",
                script.content
            );
        }
    }
}

#[test]
fn vue3_same_file_callable_type_and_value_conflicts_keep_the_value_projection() {
    let dir = tempfile::tempdir().expect("temp dir");
    let filename = dir.path().join("Comp.vue");
    for (index, declarations) in [
        "declare function Shared(): string\ntype Shared = () => number",
        "type Shared = () => number\ndeclare function Shared(): string",
    ]
    .into_iter()
    .enumerate()
    {
        let types = dir.path().join(format!("same-file-{index}.d.ts"));
        std::fs::write(&types, declarations).expect("write conflicting declarations");
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &[types.to_string_lossy().to_string()],
            &Vue3TypeResolverContext::default(),
        );
        assert!(context.silent_unresolved_type_names.contains("Shared"));
        assert_eq!(
            context.return_type_runtime_type_declarations.get("Shared"),
            Some(&vec!["String".to_string()])
        );
        assert!(!context
            .props_options_type_declarations
            .contains_key("Shared"));
        assert_eq!(
            context.type_deps.get("Shared"),
            Some(&BTreeSet::from([normalize_path_string(&types)]))
        );
    }
}

#[test]
fn vue3_global_type_conflicts_preserve_declared_const_keyof_projection() {
    let dir = tempfile::tempdir().expect("temp dir");
    let alias = dir.path().join("alias-global.d.ts");
    let interface = dir.path().join("interface-global.d.ts");
    let value = dir.path().join("value-global.d.ts");
    std::fs::write(&alias, "type Shared = { aliasValue: string }")
        .expect("write conflicting alias");
    std::fs::write(&interface, "interface Shared { interfaceValue: number }")
        .expect("write conflicting interface");
    std::fs::write(&value, "declare const Shared: { key: string }")
        .expect("write independent value");
    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<{ key: keyof typeof Shared }>()
</script>"#;
    let expected_deps = [
        normalize_path_string(&alias),
        normalize_path_string(&interface),
        normalize_path_string(&value),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();

    for files in [
        vec![value.clone(), alias.clone(), interface.clone()],
        vec![alias.clone(), interface.clone(), value.clone()],
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
        assert!(context.silent_unresolved_type_names.contains("Shared"));
        assert!(!context.props_type_declarations.contains_key("Shared"));
        assert_eq!(
            context.keyof_type_query_declared_types.get("Shared"),
            Some(&vec!["String".to_string()])
        );
        assert_eq!(context.type_deps.get("Shared"), Some(&expected_deps));

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
        assert!(
            script
                .content
                .contains("key: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            expected_deps
        );
    }
}

#[test]
fn vue3_kind_only_global_values_still_participate_in_conflicts_and_deps() {
    let dir = tempfile::tempdir().expect("temp dir");
    let alias = dir.path().join("alias-global.d.ts");
    let function = dir.path().join("function-global.d.ts");
    std::fs::write(&alias, "type Shared = () => number").expect("write callable alias");
    std::fs::write(&function, "declare function Shared();")
    .expect("write value without a runtime return projection");
    let filename = dir.path().join("Comp.vue");
    let expected_deps = [normalize_path_string(&alias), normalize_path_string(&function)]
        .into_iter()
        .collect::<BTreeSet<_>>();

    for files in [
        vec![alias.clone(), function.clone()],
        vec![function.clone(), alias.clone()],
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
        assert!(!context
            .return_type_runtime_type_declarations
            .contains_key("Shared"));
        assert_eq!(context.type_deps.get("Shared"), Some(&expected_deps));
    }

    let interface = dir.path().join("interface-global.d.ts");
    std::fs::write(&interface, "interface Shared { key: string }")
        .expect("write compatible type declaration");
    let expected_deps = [
        normalize_path_string(&interface),
        normalize_path_string(&function),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    for files in [
        vec![interface.clone(), function.clone()],
        vec![function.clone(), interface.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        assert!(!context.silent_unresolved_type_names.contains("Shared"));
        assert!(context.props_type_declarations.contains_key("Shared"));
        assert_eq!(context.type_deps.get("Shared"), Some(&expected_deps));
    }
}

#[test]
fn vue3_global_conflict_propagation_distinguishes_referenced_spaces() {
    let dir = tempfile::tempdir().expect("temp dir");
    let alias = dir.path().join("alias.d.ts");
    let interface = dir.path().join("interface.d.ts");
    let function = dir.path().join("function.d.ts");
    let consumer = dir.path().join("consumer.d.ts");
    std::fs::write(&alias, "type Shared = { aliasValue: number }").expect("write alias");
    std::fs::write(&interface, "interface Shared { interfaceValue: boolean }")
        .expect("write interface");
    std::fs::write(&function, "declare function Shared(): string")
        .expect("write function");
    std::fs::write(
        &consumer,
        r#"
type TypeDependent = Shared
type ValueDependent = ReturnType<typeof Shared>
"#,
    )
    .expect("write consumers");
    let filename = dir.path().join("Comp.vue");

    for files in [
        vec![alias.clone(), interface.clone(), function.clone(), consumer.clone()],
        vec![consumer.clone(), function.clone(), interface.clone(), alias.clone()],
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
        assert!(context.silent_unresolved_type_names.contains("Shared"));
        assert!(context
            .silent_unresolved_type_names
            .contains("TypeDependent"));
        assert!(!context
            .silent_unresolved_type_names
            .contains("ValueDependent"));
        assert!(vue3_type_context_has_name(&context, "ValueDependent"));

        let source = r#"<script setup lang="ts">
defineProps<{ result: ValueDependent }>()
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
        assert!(
            script
                .content
                .contains("result: { type: String, required: true }"),
            "{}",
            script.content
        );
    }
}

#[test]
fn vue3_global_value_conflicts_preserve_independent_type_dependents() {
    let dir = tempfile::tempdir().expect("temp dir");
    let interface = dir.path().join("interface.d.ts");
    let string_value = dir.path().join("string-value.d.ts");
    let number_value = dir.path().join("number-value.d.ts");
    let consumer = dir.path().join("consumer.d.ts");
    std::fs::write(&interface, "interface Shared { kept: string }")
        .expect("write interface");
    std::fs::write(&string_value, "declare const Shared: string")
        .expect("write string value");
    std::fs::write(&number_value, "declare const Shared: number")
        .expect("write number value");
    std::fs::write(
        &consumer,
        "type TypeDependent = Shared\ntype ValueDependent = typeof Shared",
    )
    .expect("write consumers");
    let filename = dir.path().join("Comp.vue");

    for files in [
        vec![
            interface.clone(),
            string_value.clone(),
            number_value.clone(),
            consumer.clone(),
        ],
        vec![
            consumer.clone(),
            number_value.clone(),
            string_value.clone(),
            interface.clone(),
        ],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        assert!(context.props_type_declarations.contains_key("Shared"));
        assert!(vue3_type_context_has_name(&context, "TypeDependent"));
        assert!(!context
            .silent_unresolved_type_names
            .contains("TypeDependent"));
        assert!(context
            .silent_unresolved_type_names
            .contains("ValueDependent"));
        assert!(!context.type_query_declared_types.contains_key("Shared"));
    }
}

#[test]
fn vue3_provably_distinct_global_interface_heritage_blocks_dependents() {
    let dir = tempfile::tempdir().expect("temp dir");
    let same_file = dir.path().join("same-file.d.ts");
    let string_base = dir.path().join("string-base.d.ts");
    let number_base = dir.path().join("number-base.d.ts");
    let string_fragment = dir.path().join("string-fragment.d.ts");
    let number_fragment = dir.path().join("number-fragment.d.ts");
    let compatible = dir.path().join("compatible.d.ts");
    let reconciled_fragment = dir.path().join("reconciled-fragment.d.ts");
    let consumer = dir.path().join("heritage-consumer.d.ts");
    std::fs::write(
        &same_file,
        r#"
interface SameFileStringBase { value: string }
interface SameFileNumberBase { value: number }
interface SameFileConflict extends SameFileStringBase, SameFileNumberBase {}
type SameFileConsumer = SameFileConflict
interface InvalidOwn extends SameFileStringBase, SameFileNumberBase { value: boolean }
type InvalidOwnConsumer = InvalidOwn
interface InvalidOptionalOwn extends SameFileStringBase { value?: string }
type InvalidOptionalOwnConsumer = InvalidOptionalOwn
interface Box<T> { boxed: T }
interface GenericCombination extends Box<string>, Box<number> {}
"#,
    )
    .expect("write same-file heritage conflict");
    std::fs::write(&string_base, "interface StringBase { value: string }")
        .expect("write string base");
    std::fs::write(&number_base, "interface NumberBase { value: number }")
        .expect("write number base");
    std::fs::write(
        &string_fragment,
        "interface SplitConflict extends StringBase {}",
    )
    .expect("write string heritage fragment");
    std::fs::write(
        &number_fragment,
        "interface SplitConflict extends NumberBase {}",
    )
    .expect("write number heritage fragment");
    std::fs::write(
        &compatible,
        r#"
interface CompatibleLeft { left: string; shared: boolean }
interface CompatibleRight { right: number; shared: boolean }
interface Compatible extends CompatibleLeft, CompatibleRight {}
interface Reconciled extends StringBase {}
interface Reconciled extends NumberBase {}
"#,
    )
    .expect("write compatible heritage");
    std::fs::write(
        &reconciled_fragment,
        "interface Reconciled { value: never }",
    )
    .expect("write reconciling own member");
    std::fs::write(
        &consumer,
        r#"
type SplitConsumer = SplitConflict
type CompatibleConsumer = Compatible
type ReconciledConsumer = Reconciled
type GenericConsumer = GenericCombination
"#,
    )
    .expect("write heritage consumers");
    let filename = dir.path().join("Comp.vue");

    for files in [
        vec![
            same_file.clone(),
            string_base.clone(),
            number_base.clone(),
            string_fragment.clone(),
            number_fragment.clone(),
            compatible.clone(),
            reconciled_fragment.clone(),
            consumer.clone(),
        ],
        vec![
            consumer.clone(),
            reconciled_fragment.clone(),
            compatible.clone(),
            number_fragment.clone(),
            string_fragment.clone(),
            number_base.clone(),
            string_base.clone(),
            same_file.clone(),
        ],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in [
            "SameFileConflict",
            "SameFileConsumer",
            "InvalidOwn",
            "InvalidOwnConsumer",
            "InvalidOptionalOwn",
            "InvalidOptionalOwnConsumer",
            "SplitConflict",
            "SplitConsumer",
        ] {
            assert!(
                context.silent_unresolved_type_names.contains(name),
                "missing blocked name {name} for {files:?}; available={:?}; unresolved={:?}",
                vue3_type_context_names(&context),
                context.silent_unresolved_type_names,
            );
            assert!(!vue3_type_context_has_name(&context, name));
        }
        for name in [
            "Compatible",
            "CompatibleConsumer",
            "Reconciled",
            "ReconciledConsumer",
            "Box",
            "GenericCombination",
            "GenericConsumer",
        ] {
            assert!(!context.silent_unresolved_type_names.contains(name));
            assert!(vue3_type_context_has_name(&context, name));
        }
        let props = context
            .props_type_declarations
            .get("Compatible")
            .expect("compatible inherited props");
        assert_eq!(
            props
                .members
                .iter()
                .map(|prop| prop.key.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["left", "right", "shared"])
        );
    }
}

#[test]
fn vue3_unproven_interface_heritage_differences_are_not_falsely_blocked() {
    let dir = tempfile::tempdir().expect("temp dir");
    let declarations = dir.path().join("structural-heritage.d.ts");
    std::fs::write(
        &declarations,
        r#"
interface Shape extends Date {}
interface DateBase { value: Date }
interface ShapeBase { value: Shape }
interface StructuralCompatible extends DateBase, ShapeBase {}

interface ExcludedBase { value: Exclude<string | number, number> }
interface StringBase { value: string }
interface UtilityCompatible extends ExcludedBase, StringBase {}

interface OverloadedFn { (): string; (): number }
interface ReturnTypeBase { value: ReturnType<OverloadedFn> }
interface NumberBase { value: number }
interface ReturnTypeCompatible extends ReturnTypeBase, NumberBase {}

type Intersection = { value: string } & { value: number }
interface IntersectionBase extends Intersection {}
interface NeverBase { value: never }
interface IntersectionCompatible extends IntersectionBase, NeverBase {}

interface WideBase { value: string | number }
interface NarrowBase { value: string }
interface Narrowed extends WideBase, NarrowBase { value: string }

interface OrderedUnionBase { value: string | number }
interface ReorderedUnionBase { value: number | never | string }
interface NormalizedUnionCompatible extends OrderedUnionBase, ReorderedUnionBase {}

interface NonNullableBase { value: string }
interface NullableBase { value: string | null }
interface ConfigurationSensitive extends NonNullableBase, NullableBase {}

class ClassFirstReconciled { value!: never }
interface ClassFirstReconciled extends DateBase, NumberBase {}
interface InterfaceFirstReconciled extends DateBase, NumberBase {}
class InterfaceFirstReconciled { value!: never }
"#,
    )
    .expect("write structurally compatible heritage");
    let filename = dir.path().join("Comp.vue");
    let context = vue3_global_type_context(
        &filename.to_string_lossy(),
        &[declarations.to_string_lossy().to_string()],
        &Vue3TypeResolverContext::default(),
    );

    for name in [
        "StructuralCompatible",
        "UtilityCompatible",
        "ReturnTypeCompatible",
        "IntersectionCompatible",
        "Narrowed",
        "NormalizedUnionCompatible",
        "ConfigurationSensitive",
        "ClassFirstReconciled",
        "InterfaceFirstReconciled",
    ] {
        assert!(!context.silent_unresolved_type_names.contains(name));
        assert!(vue3_type_context_has_name(&context, name));
    }
}

#[test]
fn vue3_provably_distinct_ambient_namespace_interface_heritage_is_blocked() {
    let dir = tempfile::tempdir().expect("temp dir");
    let namespace = dir.path().join("namespace-heritage.d.ts");
    std::fs::write(
        &namespace,
        r#"
declare namespace HeritageNs {
  interface Left { nested: string }
  interface Right { nested: number }
  interface Conflict extends Left, Right {}
}
type NamespaceConsumer = HeritageNs.Conflict
"#,
    )
    .expect("write namespace heritage conflict");
    let filename = dir.path().join("Comp.vue");
    let context = vue3_global_type_context(
        &filename.to_string_lossy(),
        &[namespace.to_string_lossy().to_string()],
        &Vue3TypeResolverContext::default(),
    );

    for name in ["HeritageNs.Conflict", "NamespaceConsumer"] {
        assert!(context.silent_unresolved_type_names.contains(name));
        assert!(!vue3_type_context_has_name(&context, name));
    }
    for name in ["HeritageNs.Left", "HeritageNs.Right"] {
        assert!(!context.silent_unresolved_type_names.contains(name));
        assert!(vue3_type_context_has_name(&context, name));
    }
}

#[test]
fn vue3_imported_global_augmentation_reconciles_interface_heritage_before_blocking() {
    let dir = tempfile::tempdir().expect("temp dir");
    let string_fragment = dir.path().join("string-fragment.d.ts");
    let number_fragment = dir.path().join("number-fragment.d.ts");
    let augmentation_leaf = dir.path().join("augmentation-leaf.ts");
    let augmentation = dir.path().join("augmentation.ts");
    let transitive = dir.path().join("transitive.ts");
    let barrel = dir.path().join("barrel.ts");
    let import_type = dir.path().join("import-type.ts");
    let cycle_a = dir.path().join("cycle-a.ts");
    let cycle_b = dir.path().join("cycle-b.ts");
    std::fs::write(
        &string_fragment,
        "interface StringBase { value: string }\ninterface ImportedReconciled extends StringBase {}",
    )
    .expect("write string heritage fragment");
    std::fs::write(
        &number_fragment,
        "interface NumberBase { value: number }\ninterface ImportedReconciled extends NumberBase {}",
    )
    .expect("write number heritage fragment");
    std::fs::write(
        &augmentation_leaf,
        "export interface AugmentationLeaf { nested: boolean }",
    )
    .expect("write augmentation leaf type");
    std::fs::write(
        &augmentation,
        r#"
import type { AugmentationLeaf as Leaf } from './augmentation-leaf'
interface ModulePrivate { leaked: boolean }
export type Marker = true
declare global { interface ImportedReconciled { value: never; leaf: Leaf } }
"#,
    )
    .expect("write global augmentation module");
    std::fs::write(
        &transitive,
        "import './augmentation'\nexport type Marker = true",
    )
    .expect("write transitive augmentation import");
    std::fs::write(&barrel, "export { Marker } from './augmentation'")
        .expect("write augmentation barrel");
    std::fs::write(
        &import_type,
        "export type ImportedMarker = import('./augmentation').Marker",
    )
    .expect("write import type augmentation edge");
    std::fs::write(&cycle_a, "import './cycle-b'\nexport type CycleA = true")
        .expect("write first cyclic module");
    std::fs::write(
        &cycle_b,
        "import './cycle-a'\nexport { Marker } from './augmentation'",
    )
    .expect("write second cyclic module");
    let filename = dir.path().join("Comp.vue");
    let cases = [
        (
            "setup-side-effect",
            r#"<script setup lang="ts">
import './augmentation'
defineProps<ImportedReconciled>()
</script>"#,
        ),
        (
            "normal-side-effect",
            r#"<script lang="ts">
import './augmentation'
export default {}
</script>
<script setup lang="ts">defineProps<ImportedReconciled>()</script>"#,
        ),
        (
            "normal-named",
            r#"<script lang="ts">
import type { Marker } from './augmentation'
export default {}
</script>
<script setup lang="ts">defineProps<ImportedReconciled>()</script>"#,
        ),
        (
            "transitive-side-effect",
            r#"<script setup lang="ts">
import './transitive'
defineProps<ImportedReconciled>()
</script>"#,
        ),
        (
            "named-re-export",
            r#"<script setup lang="ts">
import type { Marker } from './barrel'
defineProps<ImportedReconciled>()
</script>"#,
        ),
        (
            "import-type",
            r#"<script setup lang="ts">
import type { ImportedMarker } from './import-type'
defineProps<ImportedReconciled>()
</script>"#,
        ),
        (
            "cyclic-module-graph",
            r#"<script setup lang="ts">
import './cycle-a'
defineProps<ImportedReconciled>()
</script>"#,
        ),
    ];
    let expected_heritage_deps = [
        normalize_path_string(&string_fragment),
        normalize_path_string(&number_fragment),
        normalize_path_string(&augmentation),
        normalize_path_string(&augmentation_leaf),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();

    for (case, source) in cases {
        for files in [
            vec![string_fragment.clone(), number_fragment.clone()],
            vec![number_fragment.clone(), string_fragment.clone()],
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

            assert!(script.errors.is_empty(), "{case}: {:?}", script.errors);
            assert!(
                script
                    .content
                    .contains("value: { type: null, required: true }"),
                "missing reconciled prop for {case} and {files:?}: {}",
                script.content,
            );
            assert!(
                script
                    .content
                    .contains("leaf: { type: Object, required: true }"),
                "missing imported augmentation prop for {case}: {}",
                script.content,
            );
            assert!(
                expected_heritage_deps
                    .is_subset(&script.deps.iter().cloned().collect()),
                "missing augmentation deps for {case}: {:?}",
                script.deps,
            );
            assert!(!script.content.contains("leaked:"), "{case}");
        }
    }

    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(
        filename.to_string_lossy(),
        r#"<script setup lang="ts">
import './augmentation'
defineProps<ModulePrivate>()
</script>"#,
    );
    let script = compiler.compile_script(
        &descriptor,
        SfcScriptCompileOptions {
            global_type_files: vec![
                string_fragment.to_string_lossy().to_string(),
                number_fragment.to_string_lossy().to_string(),
            ],
            ..SfcScriptCompileOptions::default()
        },
    );
    assert!(!script.errors.is_empty());
    assert!(!script.content.contains("leaked:"));
}

#[test]
fn vue3_triple_slash_references_add_global_program_files() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path_globals = dir.path().join("path-globals.d.ts");
    let script_globals = dir.path().join("script-globals.ts");
    let augmentation = dir.path().join("augmentation.ts");
    let transitive = dir.path().join("transitive.ts");
    let forced_module = dir.path().join("forced-module.mts");
    let import_equals_module = dir.path().join("import-equals-module.ts");
    let cycle_a = dir.path().join("cycle-a.d.ts");
    let cycle_b = dir.path().join("cycle-b.d.ts");
    let type_package = dir
        .path()
        .join("node_modules")
        .join("@types")
        .join("reference-package");
    let type_index = type_package.join("index.d.ts");
    let modern_type_package = dir
        .path()
        .join("node_modules")
        .join("@types")
        .join("modern-reference-package");
    let modern_type_index = modern_type_package.join("index.d.mts");
    let script_declaration_package = dir
        .path()
        .join("node_modules")
        .join("@types")
        .join("script-declaration-package");
    let script_declaration_index = script_declaration_package.join("index.d.mts");
    std::fs::write(
        dir.path().join("tsconfig.json"),
        r#"{"compilerOptions":{"types":[]}}"#,
    )
    .expect("write tsconfig with disabled automatic types");
    std::fs::write(
        &path_globals,
        "interface PathProps { pathValue: string }",
    )
    .expect("write path reference globals");
    std::fs::write(
        &script_globals,
        r#"interface ScriptProps { scriptValue: string }
namespace ScriptNamespace {
  interface Private { privateValue: boolean }
  export interface Public { publicValue: number }
}"#,
    )
    .expect("write script reference globals");
    std::fs::write(
        &augmentation,
        "export {}; declare global { interface AugmentedProps { augmentedValue: number } }",
    )
    .expect("write reference augmentation module");
    std::fs::write(
        &transitive,
        "/// <reference path='./path-globals.d.ts' />\nexport {}",
    )
    .expect("write transitive reference module");
    std::fs::write(
        &forced_module,
        "declare interface ForcedModulePrivate { forcedModuleValue: string }",
    )
    .expect("write forced module reference");
    std::fs::write(
        &import_equals_module,
        r#"import Dependency = require('./missing')
declare interface ImportEqualsPrivate { importEqualsValue: string }
type Marker = Dependency"#,
    )
    .expect("write import-equals module reference");
    std::fs::write(
        &cycle_a,
        "/// <reference path='./cycle-b.d.ts' />\ninterface CycleA { first: boolean }",
    )
    .expect("write first reference cycle file");
    std::fs::write(
        &cycle_b,
        "/// <reference path='./cycle-a.d.ts' />\ninterface CycleB { second: bigint }",
    )
    .expect("write second reference cycle file");
    std::fs::create_dir_all(&type_package).expect("create reference type package");
    std::fs::write(
        type_package.join("package.json"),
        r#"{"types":"index.d.ts"}"#,
    )
    .expect("write reference type package metadata");
    std::fs::write(
        &type_index,
        "interface TypeReferenceProps { typeValue: symbol }",
    )
    .expect("write reference type package entry");
    std::fs::create_dir_all(&modern_type_package).expect("create modern reference type package");
    std::fs::write(
        modern_type_package.join("package.json"),
        r#"{"types":"index.d.mts"}"#,
    )
    .expect("write modern reference type package metadata");
    std::fs::write(
        &modern_type_index,
        "export {}; declare global { interface ModernReferenceProps { modernValue: Date } }",
    )
    .expect("write modern reference type package entry");
    std::fs::create_dir_all(&script_declaration_package)
        .expect("create script declaration type package");
    std::fs::write(
        script_declaration_package.join("package.json"),
        r#"{"types":"index.d.mts"}"#,
    )
    .expect("write script declaration package metadata");
    std::fs::write(
        &script_declaration_index,
        "interface ScriptDeclarationProps { declarationValue: string }",
    )
    .expect("write script declaration package entry");

    let filename = dir.path().join("Comp.vue");
    let cases = [
        (
            "root-path",
            r#"<script setup lang="ts">
/// <reference path="./path-globals" />
defineProps<PathProps>()
</script>"#,
            "pathValue: { type: String, required: true }",
            path_globals.clone(),
        ),
        (
            "transitive-path",
            r#"<script setup lang="ts">
import './transitive'
defineProps<PathProps>()
</script>"#,
            "pathValue: { type: String, required: true }",
            path_globals.clone(),
        ),
        (
            "script-path",
            r#"<script setup lang="ts">
/// <reference path="./script-globals.ts" />
defineProps<ScriptProps & ScriptNamespace.Public>()
</script>"#,
            "scriptValue: { type: String, required: true }",
            script_globals.clone(),
        ),
        (
            "script-namespace-path",
            r#"<script setup lang="ts">
/// <reference path="./script-globals.ts" />
defineProps<ScriptNamespace.Public>()
</script>"#,
            "publicValue: { type: Number, required: true }",
            script_globals.clone(),
        ),
        (
            "external-module-path",
            r#"<script setup lang="ts">
/// <reference path="./augmentation.ts" />
defineProps<AugmentedProps>()
</script>"#,
            "augmentedValue: { type: Number, required: true }",
            augmentation.clone(),
        ),
        (
            "explicit-types",
            r#"<script setup lang="ts">
/// <reference types="reference-package" />
defineProps<TypeReferenceProps>()
</script>"#,
            "typeValue: { type: Symbol, required: true }",
            type_index.clone(),
        ),
        (
            "modern-explicit-types",
            r#"<script setup lang="ts">
/// <reference types="modern-reference-package" />
defineProps<ModernReferenceProps>()
</script>"#,
            "modernValue: { type: Date, required: true }",
            modern_type_index.clone(),
        ),
        (
            "script-declaration-types",
            r#"<script setup lang="ts">
/// <reference types="script-declaration-package" />
defineProps<ScriptDeclarationProps>()
</script>"#,
            "declarationValue: { type: String, required: true }",
            script_declaration_index.clone(),
        ),
    ];

    for (case, source, expected_prop, expected_dep) in cases {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty(), "{case}: {:?}", script.errors);
        assert!(
            script.content.contains(expected_prop),
            "missing {case} prop: {}",
            script.content,
        );
        assert!(
            script
                .deps
                .contains(&normalize_path_string(&expected_dep)),
            "missing {case} dependency: {:?}",
            script.deps,
        );
    }

    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(
        filename.to_string_lossy(),
        r#"<script setup lang="ts">
/// <reference path="./cycle-a.d.ts" />
defineProps<CycleA & CycleB>()
</script>"#,
    );
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());
    assert!(script.errors.is_empty(), "{:?}", script.errors);
    assert!(script
        .content
        .contains("first: { type: Boolean, required: true }"), "{}", script.content);
    assert!(script
        .content
        .contains("second: { type: null, required: true }"), "{}", script.content);
    let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
    assert!(deps.contains(&normalize_path_string(&cycle_a)));
    assert!(deps.contains(&normalize_path_string(&cycle_b)));

    for (case, source, private_member) in [
        (
            "namespace-private",
            r#"<script setup lang="ts">
/// <reference path="./script-globals.ts" />
defineProps<ScriptNamespace.Private>()
</script>"#,
            "privateValue:",
        ),
        (
            "forced-module-private",
            r#"<script setup lang="ts">
/// <reference path="./forced-module.mts" />
defineProps<ForcedModulePrivate>()
</script>"#,
            "forcedModuleValue:",
        ),
        (
            "import-equals-private",
            r#"<script setup lang="ts">
/// <reference path="./import-equals-module.ts" />
defineProps<ImportEqualsPrivate>()
</script>"#,
            "importEqualsValue:",
        ),
    ] {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());
        assert!(
            !script.content.contains(private_member),
            "leaked {case} member: {}",
            script.content,
        );
    }
}

#[test]
fn vue3_reference_types_do_not_promote_implementation_files() {
    let dir = tempfile::tempdir().expect("temp dir");
    let runtime = dir.path().join("runtime.ts");
    std::fs::write(
        dir.path().join("tsconfig.json"),
        r#"{"compilerOptions":{"types":[]}}"#,
    )
    .expect("write tsconfig with disabled automatic types");
    std::fs::write(
        &runtime,
        "interface RuntimeOnlyProps { leaked: string }",
    )
    .expect("write implementation file");

    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
/// <reference types="./runtime" />
defineProps<RuntimeOnlyProps>()
</script>"#;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.to_string_lossy(), source);
    let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

    assert!(!script.content.contains("leaked:"), "{}", script.content);
    assert!(!script.deps.contains(&normalize_path_string(&runtime)));
}

#[test]
fn vue3_incompatible_global_interface_and_enum_members_block_dependents() {
    let dir = tempfile::tempdir().expect("temp dir");
    let string_interface = dir.path().join("string-interface.d.ts");
    let number_interface = dir.path().join("number-interface.d.ts");
    let consumer = dir.path().join("consumer.d.ts");
    std::fs::write(&string_interface, "interface Shared { value: string }")
        .expect("write string interface");
    std::fs::write(&number_interface, "interface Shared { value: number }")
        .expect("write number interface");
    std::fs::write(
        &consumer,
        r#"
type Uses = Shared
interface NestedConsumer {
  actual: Shared
  method<Shared>(value: Shared): Shared
}
"#,
    )
    .expect("write consumers");
    let filename = dir.path().join("Comp.vue");

    for files in [
        vec![string_interface.clone(), number_interface.clone(), consumer.clone()],
        vec![consumer.clone(), number_interface.clone(), string_interface.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in ["Shared", "Uses", "NestedConsumer"] {
            assert!(
                context.silent_unresolved_type_names.contains(name),
                "missing blocked name {name} for {files:?}"
            );
            assert!(!vue3_type_context_has_name(&context, name));
        }
    }

    let string_index = dir.path().join("string-index.d.ts");
    let number_index = dir.path().join("number-index.d.ts");
    let index_consumer = dir.path().join("index-consumer.d.ts");
    std::fs::write(
        &string_index,
        "interface Indexed { [key: string]: string }",
    )
    .expect("write string index signature");
    std::fs::write(
        &number_index,
        "interface Indexed { [key: string]: number }",
    )
    .expect("write number index signature");
    std::fs::write(&index_consumer, "type IndexedConsumer = Indexed")
        .expect("write index consumer");
    for files in [
        vec![
            string_index.clone(),
            number_index.clone(),
            index_consumer.clone(),
        ],
        vec![index_consumer.clone(), number_index.clone(), string_index.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in ["Indexed", "IndexedConsumer"] {
            assert!(context.silent_unresolved_type_names.contains(name));
            assert!(!vue3_type_context_has_name(&context, name));
        }
    }

    let enum_first = dir.path().join("enum-first.d.ts");
    let enum_second = dir.path().join("enum-second.d.ts");
    let enum_consumer = dir.path().join("enum-consumer.d.ts");
    std::fs::write(&enum_first, "declare enum Duplicate { Value = 1 }")
        .expect("write first duplicate enum member");
    std::fs::write(&enum_second, "declare enum Duplicate { Value = 2 }")
        .expect("write second duplicate enum member");
    std::fs::write(
        &enum_consumer,
        "type DuplicateConsumer = typeof Duplicate.Value",
    )
    .expect("write duplicate enum consumer");
    for files in [
        vec![
            enum_first.clone(),
            enum_second.clone(),
            enum_consumer.clone(),
        ],
        vec![enum_consumer.clone(), enum_second.clone(), enum_first.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in ["Duplicate", "DuplicateConsumer"] {
            assert!(context.silent_unresolved_type_names.contains(name));
            assert!(!vue3_type_context_has_name(&context, name));
        }
    }

    let const_enum = dir.path().join("const-enum.d.ts");
    let regular_enum = dir.path().join("regular-enum.d.ts");
    let constness_consumer = dir.path().join("constness-consumer.d.ts");
    std::fs::write(&const_enum, "declare const enum Constness { Text = 'text' }")
        .expect("write const enum fragment");
    std::fs::write(&regular_enum, "declare enum Constness { Numeric = 1 }")
        .expect("write regular enum fragment");
    std::fs::write(&constness_consumer, "type ConstnessConsumer = Constness")
        .expect("write enum constness consumer");
    for files in [
        vec![
            const_enum.clone(),
            regular_enum.clone(),
            constness_consumer.clone(),
        ],
        vec![constness_consumer.clone(), regular_enum.clone(), const_enum.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in ["Constness", "ConstnessConsumer"] {
            assert!(context.silent_unresolved_type_names.contains(name));
            assert!(!vue3_type_context_has_name(&context, name));
        }
    }

    let first_implicit = dir.path().join("first-implicit-enum.d.ts");
    let second_implicit = dir.path().join("second-implicit-enum.d.ts");
    let implicit_consumer = dir.path().join("implicit-enum-consumer.d.ts");
    std::fs::write(&first_implicit, "declare enum Implicit { First }")
        .expect("write first implicit enum fragment");
    std::fs::write(&second_implicit, "declare enum Implicit { Second }")
        .expect("write second implicit enum fragment");
    std::fs::write(&implicit_consumer, "type ImplicitConsumer = Implicit")
        .expect("write implicit enum consumer");
    for files in [
        vec![
            first_implicit.clone(),
            second_implicit.clone(),
            implicit_consumer.clone(),
        ],
        vec![
            implicit_consumer.clone(),
            second_implicit.clone(),
            first_implicit.clone(),
        ],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in ["Implicit", "ImplicitConsumer"] {
            assert!(context.silent_unresolved_type_names.contains(name));
            assert!(!vue3_type_context_has_name(&context, name));
        }
    }

    let initialized = dir.path().join("initialized-enum.d.ts");
    std::fs::write(&initialized, "declare enum Continuation { Second = 2 }")
        .expect("write initialized enum continuation");
    std::fs::write(&first_implicit, "declare enum Continuation { First }")
        .expect("write one implicit enum fragment");
    std::fs::write(
        &implicit_consumer,
        "type ContinuationConsumer = Continuation",
    )
    .expect("write valid enum continuation consumer");
    for files in [
        vec![
            first_implicit.clone(),
            initialized.clone(),
            implicit_consumer.clone(),
        ],
        vec![implicit_consumer.clone(), initialized.clone(), first_implicit.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in ["Continuation", "ContinuationConsumer"] {
            assert!(!context.silent_unresolved_type_names.contains(name));
            assert!(vue3_type_context_has_name(&context, name));
        }
    }
}

#[test]
fn vue3_computed_global_members_merge_by_symbol_identity() {
    let dir = tempfile::tempdir().expect("temp dir");
    let filename = dir.path().join("Comp.vue");
    let key = dir.path().join("shared-key.d.ts");
    let string_member = dir.path().join("computed-string.d.ts");
    let number_member = dir.path().join("computed-number.d.ts");
    let consumer = dir.path().join("computed-consumer.d.ts");
    std::fs::write(&key, "declare const sharedKey: unique symbol")
        .expect("write shared unique symbol");
    std::fs::write(
        &string_member,
        "interface SharedComputed { [sharedKey]: string }",
    )
    .expect("write string computed member");
    std::fs::write(
        &number_member,
        "interface SharedComputed { [sharedKey]: number }",
    )
    .expect("write number computed member");
    std::fs::write(&consumer, "type ComputedConsumer = SharedComputed")
        .expect("write computed member consumer");
    for files in [
        vec![
            key.clone(),
            string_member.clone(),
            number_member.clone(),
            consumer.clone(),
        ],
        vec![
            consumer.clone(),
            number_member.clone(),
            string_member.clone(),
            key.clone(),
        ],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in ["SharedComputed", "ComputedConsumer"] {
            assert!(context.silent_unresolved_type_names.contains(name));
            assert!(!vue3_type_context_has_name(&context, name));
        }
    }

    let local_string = dir.path().join("local-computed-string.ts");
    let local_number = dir.path().join("local-computed-number.ts");
    let local_consumer = dir.path().join("local-computed-consumer.d.ts");
    std::fs::write(
        &local_string,
        r#"
export {}
declare const key: unique symbol
declare global { interface DistinctComputed { [key]: string } }
"#,
    )
    .expect("write module-local string symbol");
    std::fs::write(
        &local_number,
        r#"
export {}
declare const key: unique symbol
declare global { interface DistinctComputed { [key]: number } }
"#,
    )
    .expect("write module-local number symbol");
    std::fs::write(
        &local_consumer,
        "type DistinctComputedConsumer = DistinctComputed",
    )
    .expect("write distinct computed consumer");
    for files in [
        vec![
            local_string.clone(),
            local_number.clone(),
            local_consumer.clone(),
        ],
        vec![
            local_consumer.clone(),
            local_number.clone(),
            local_string.clone(),
        ],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in ["DistinctComputed", "DistinctComputedConsumer"] {
            assert!(!context.silent_unresolved_type_names.contains(name));
            assert!(vue3_type_context_has_name(&context, name));
        }
    }

    let symbols = dir.path().join("symbols.d.ts");
    let imported_string = dir.path().join("imported-computed-string.ts");
    let imported_number = dir.path().join("imported-computed-number.ts");
    let imported_consumer = dir.path().join("imported-computed-consumer.d.ts");
    std::fs::write(&symbols, "export declare const key: unique symbol")
        .expect("write exported unique symbol");
    std::fs::write(
        &imported_string,
        r#"
import { key as stringKey } from './symbols'
declare global { interface ImportedComputed { [stringKey]: string } }
"#,
    )
    .expect("write imported string symbol");
    std::fs::write(
        &imported_number,
        r#"
import * as Symbols from './symbols'
declare global { interface ImportedComputed { [Symbols.key]: number } }
"#,
    )
    .expect("write namespace-imported number symbol");
    std::fs::write(
        &imported_consumer,
        "type ImportedComputedConsumer = ImportedComputed",
    )
    .expect("write imported computed consumer");
    for files in [
        vec![
            imported_string.clone(),
            imported_number.clone(),
            imported_consumer.clone(),
        ],
        vec![
            imported_consumer.clone(),
            imported_number.clone(),
            imported_string.clone(),
        ],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in ["ImportedComputed", "ImportedComputedConsumer"] {
            assert!(context.silent_unresolved_type_names.contains(name));
            assert!(!vue3_type_context_has_name(&context, name));
        }
    }

    let literal = dir.path().join("literal-computed.d.ts");
    let named = dir.path().join("named-computed.d.ts");
    std::fs::write(&literal, "interface LiteralComputed { ['value']: string }")
        .expect("write literal computed member");
    std::fs::write(&named, "interface LiteralComputed { value: number }")
        .expect("write named member");
    for files in [vec![literal.clone(), named.clone()], vec![named, literal]] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        assert!(context
            .silent_unresolved_type_names
            .contains("LiteralComputed"));
        assert!(!vue3_type_context_has_name(&context, "LiteralComputed"));
    }

    let computed_class = dir.path().join("computed-class.d.ts");
    let computed_interface = dir.path().join("computed-interface.d.ts");
    std::fs::write(
        &computed_class,
        "declare class ComputedClass { [sharedKey]: string }",
    )
    .expect("write computed class member");
    std::fs::write(
        &computed_interface,
        "interface ComputedClass { [sharedKey]: string; visible: boolean }\ninterface ComputedClassConsumer extends ComputedClass { consumer: boolean }",
    )
    .expect("write compatible computed interface member");
    for files in [
        vec![key.clone(), computed_class.clone(), computed_interface.clone()],
        vec![
            computed_interface.clone(),
            computed_class.clone(),
            key.clone(),
        ],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in ["ComputedClass", "ComputedClassConsumer"] {
            assert!(!context.silent_unresolved_type_names.contains(name));
        }
    }
    std::fs::write(
        &computed_interface,
        "interface ComputedClass { [sharedKey]: number; visible: boolean }\ninterface ComputedClassConsumer extends ComputedClass { consumer: boolean }",
    )
    .expect("write computed interface member");
    for files in [
        vec![key.clone(), computed_class.clone(), computed_interface.clone()],
        vec![computed_interface, computed_class, key],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in ["ComputedClass", "ComputedClassConsumer"] {
            assert!(context.silent_unresolved_type_names.contains(name));
            assert!(!vue3_type_context_has_name(&context, name));
        }
    }

    let distinct_local_keys = dir.path().join("distinct-local-keys.ts");
    let distinct_local_consumer = dir.path().join("distinct-local-consumer.d.ts");
    std::fs::write(
        &distinct_local_keys,
        r#"
export {}
declare const firstKey: unique symbol
declare const secondKey: unique symbol
declare namespace First { const key: unique symbol }
declare namespace Second { const key: unique symbol }
declare global {
  interface DistinctLocalKeys {
    [firstKey]: string
    [secondKey]: number
    [First.key]: boolean
    [Second.key]: Date
  }
}
"#,
    )
    .expect("write distinct module-local computed keys");
    std::fs::write(
        &distinct_local_consumer,
        "type DistinctLocalKeysConsumer = DistinctLocalKeys",
    )
    .expect("write distinct module-local computed consumer");
    for files in [
        vec![distinct_local_keys.clone(), distinct_local_consumer.clone()],
        vec![distinct_local_consumer.clone(), distinct_local_keys.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in ["DistinctLocalKeys", "DistinctLocalKeysConsumer"] {
            assert!(!context.silent_unresolved_type_names.contains(name));
            assert!(vue3_type_context_has_name(&context, name));
        }
    }

    let exported_key = dir.path().join("exported-computed-key.ts");
    let imported_key = dir.path().join("imported-exported-computed-key.ts");
    let exported_consumer = dir.path().join("exported-computed-consumer.d.ts");
    std::fs::write(
        &exported_key,
        r#"
export declare const exportedKey: unique symbol
declare global { interface ExportedComputedKey { [exportedKey]: string } }
"#,
    )
    .expect("write directly exported computed key");
    std::fs::write(
        &imported_key,
        r#"
import { exportedKey } from './exported-computed-key'
declare global { interface ExportedComputedKey { [exportedKey]: number } }
"#,
    )
    .expect("write imported directly exported computed key");
    std::fs::write(
        &exported_consumer,
        "type ExportedComputedKeyConsumer = ExportedComputedKey",
    )
    .expect("write exported computed key consumer");
    for files in [
        vec![
            exported_key.clone(),
            imported_key.clone(),
            exported_consumer.clone(),
        ],
        vec![
            exported_consumer.clone(),
            imported_key.clone(),
            exported_key.clone(),
        ],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in ["ExportedComputedKey", "ExportedComputedKeyConsumer"] {
            assert!(context.silent_unresolved_type_names.contains(name));
            assert!(!vue3_type_context_has_name(&context, name));
        }
    }
}

#[test]
fn vue3_namespace_conflicts_block_qualified_dependents() {
    let dir = tempfile::tempdir().expect("temp dir");
    let alias = dir.path().join("alias.d.ts");
    let interface = dir.path().join("interface.d.ts");
    let consumer = dir.path().join("consumer.d.ts");
    std::fs::write(&alias, "declare namespace Box { type Bad = { alias: string } }")
        .expect("write namespace alias");
    std::fs::write(
        &interface,
        "declare namespace Box { interface Bad { interfaceValue: number } }",
    )
    .expect("write namespace interface");
    std::fs::write(&consumer, "declare namespace Box { type Dep = Bad }")
        .expect("write namespace consumer");
    let filename = dir.path().join("Comp.vue");

    for files in [
        vec![alias.clone(), interface.clone(), consumer.clone()],
        vec![consumer.clone(), interface.clone(), alias.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        assert!(context.silent_unresolved_type_names.contains("Box.Bad"));
        assert!(context.silent_unresolved_type_names.contains("Box.Dep"));
        assert!(!vue3_type_context_has_name(&context, "Box.Dep"));
    }

    let enum_file = dir.path().join("namespace-enum.d.ts");
    let member_alias = dir.path().join("namespace-member-alias.d.ts");
    let member_consumer = dir.path().join("namespace-member-consumer.d.ts");
    std::fs::write(
        &enum_file,
        "declare namespace Box { enum Member { Value = 1 } }",
    )
    .expect("write namespace enum");
    std::fs::write(
        &member_alias,
        "declare namespace Box { type Member = { alias: string } }",
    )
    .expect("write conflicting namespace member alias");
    std::fs::write(
        &member_consumer,
        "declare namespace Box { type MemberConsumer = typeof Member.Value }",
    )
    .expect("write namespace enum member consumer");
    for files in [
        vec![
            enum_file.clone(),
            member_alias.clone(),
            member_consumer.clone(),
        ],
        vec![member_consumer.clone(), member_alias.clone(), enum_file.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in ["Box.Member", "Box.MemberConsumer"] {
            assert!(context.silent_unresolved_type_names.contains(name));
            assert!(!vue3_type_context_has_name(&context, name));
        }
    }
}

#[test]
fn vue3_callable_interface_value_ambiguity_blocks_only_the_type_space() {
    let dir = tempfile::tempdir().expect("temp dir");
    let interface = dir.path().join("interface.d.ts");
    let function = dir.path().join("function.d.ts");
    let consumer = dir.path().join("consumer.d.ts");
    std::fs::write(&interface, "interface Shared { (): number }")
        .expect("write callable interface");
    std::fs::write(&function, "declare function Shared(): string")
        .expect("write function");
    std::fs::write(
        &consumer,
        r#"
type TypeDependent = ReturnType<Shared>
type ValueDependent = ReturnType<typeof Shared>
"#,
    )
    .expect("write consumers");
    let filename = dir.path().join("Comp.vue");

    for files in [
        vec![interface.clone(), function.clone(), consumer.clone()],
        vec![consumer.clone(), function.clone(), interface.clone()],
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
        assert!(context
            .silent_unresolved_type_names
            .contains("TypeDependent"));
        assert!(!context
            .silent_unresolved_type_names
            .contains("ValueDependent"));
        assert_eq!(
            context.declared_types.get("ValueDependent"),
            Some(&vec!["String".to_string()])
        );
    }
}

#[test]
fn vue3_global_interface_and_self_returning_function_coexist_in_any_order() {
    let dir = tempfile::tempdir().expect("temp dir");
    let interface = dir.path().join("interface.d.ts");
    let function = dir.path().join("function.d.ts");
    std::fs::write(&interface, "interface Shared { value: string }")
        .expect("write interface");
    std::fs::write(&function, "declare function Shared(): Shared")
        .expect("write self-returning function");
    let filename = dir.path().join("Comp.vue");
    let source = r#"<script setup lang="ts">
defineProps<Shared & { result: ReturnType<typeof Shared> }>()
</script>"#;

    for files in [
        vec![interface.clone(), function.clone()],
        vec![function.clone(), interface.clone()],
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
        assert!(context.props_type_declarations.contains_key("Shared"));
        assert!(context
            .return_type_props_options_declarations
            .contains_key("Shared"));

        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: global_files,
                ..SfcScriptCompileOptions::default()
            },
        );
        assert!(
            script.errors.is_empty(),
            "{:?} for {files:?}",
            script.errors
        );
        assert!(
            script
                .content
                .contains("value: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(
            script
                .content
                .contains("result: { type: Object, required: true }"),
            "{}",
            script.content
        );
    }
}

#[test]
fn vue3_conflicting_return_types_block_dependent_function_values() {
    let dir = tempfile::tempdir().expect("temp dir");
    let alias = dir.path().join("alias.d.ts");
    let interface = dir.path().join("interface.d.ts");
    let function = dir.path().join("function.d.ts");
    let consumer = dir.path().join("consumer.d.ts");
    std::fs::write(&alias, "type Shared = { aliasValue: string }").expect("write alias");
    std::fs::write(&interface, "interface Shared { interfaceValue: number }")
        .expect("write interface");
    std::fs::write(&function, "declare function Shared(): Shared")
        .expect("write dependent function");
    std::fs::write(&consumer, "type UsesValue = ReturnType<typeof Shared>")
        .expect("write value consumer");
    let filename = dir.path().join("Comp.vue");

    for files in [
        vec![alias.clone(), interface.clone(), function.clone(), consumer.clone()],
        vec![consumer.clone(), function.clone(), interface.clone(), alias.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in ["Shared", "UsesValue"] {
            assert!(
                context.silent_unresolved_type_names.contains(name),
                "missing blocked name {name} for {files:?}"
            );
        }
        assert!(!context
            .return_type_runtime_type_declarations
            .contains_key("Shared"));
    }
}

#[test]
fn vue3_generic_bindings_do_not_inherit_unrelated_global_conflicts() {
    let dir = tempfile::tempdir().expect("temp dir");
    let alias = dir.path().join("alias.d.ts");
    let interface = dir.path().join("interface.d.ts");
    let consumer = dir.path().join("consumer.d.ts");
    std::fs::write(&alias, "type T = string").expect("write alias");
    std::fs::write(&interface, "interface T { value: number }").expect("write interface");
    std::fs::write(&consumer, "interface Box<T> { value: T }").expect("write consumer");
    let filename = dir.path().join("Comp.vue");

    for files in [
        vec![alias.clone(), interface.clone(), consumer.clone()],
        vec![consumer.clone(), interface.clone(), alias.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        assert!(context.silent_unresolved_type_names.contains("T"));
        assert!(!context.silent_unresolved_type_names.contains("Box"));
        assert!(vue3_type_context_has_name(&context, "Box"));
    }
}

#[test]
fn vue3_global_function_overloads_are_order_independent_or_fail_closed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let string_function = dir.path().join("string-function.d.ts");
    let number_function = dir.path().join("number-function.d.ts");
    let duplicate_function = dir.path().join("duplicate-function.d.ts");
    let consumer = dir.path().join("consumer.d.ts");
    std::fs::write(&string_function, "declare function Shared(): string")
        .expect("write string overload");
    std::fs::write(&number_function, "declare function Shared(): number")
        .expect("write number overload");
    std::fs::write(&duplicate_function, "declare function Shared(): string")
        .expect("write compatible overload");
    std::fs::write(&consumer, "type Uses = ReturnType<typeof Shared>")
        .expect("write overload consumer");
    let filename = dir.path().join("Comp.vue");

    for files in [
        vec![string_function.clone(), number_function.clone(), consumer.clone()],
        vec![consumer.clone(), number_function.clone(), string_function.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        assert!(!context
            .return_type_runtime_type_declarations
            .contains_key("Shared"));
        assert!(context.silent_unresolved_type_names.contains("Uses"));
    }

    for files in [
        vec![string_function.clone(), duplicate_function.clone()],
        vec![duplicate_function.clone(), string_function.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        assert_eq!(
            context.return_type_runtime_type_declarations.get("Shared"),
            Some(&vec!["String".to_string()])
        );
    }
}

#[test]
fn vue3_global_variable_conflicts_do_not_block_sibling_declarators() {
    let dir = tempfile::tempdir().expect("temp dir");
    let string_value = dir.path().join("string-value.d.ts");
    let number_value = dir.path().join("number-value.d.ts");
    let consumers = dir.path().join("consumers.d.ts");
    std::fs::write(&string_value, "declare const Bad: string")
        .expect("write string value");
    std::fs::write(&number_value, "declare const Bad: number")
        .expect("write number value");
    std::fs::write(
        &consumers,
        "declare const Dependent: typeof Bad, Independent: string",
    )
    .expect("write sibling declarators");
    let filename = dir.path().join("Comp.vue");

    for files in [
        vec![string_value.clone(), number_value.clone(), consumers.clone()],
        vec![consumers.clone(), number_value.clone(), string_value.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        assert!(!context.type_query_declared_types.contains_key("Bad"));
        assert!(!context
            .type_query_declared_types
            .contains_key("Dependent"));
        assert_eq!(
            context.type_query_declared_types.get("Independent"),
            Some(&vec!["String".to_string()])
        );
    }

    let same_file = dir.path().join("same-file.d.ts");
    std::fs::write(
        &same_file,
        "declare const Shared: string\ndeclare const Shared: number",
    )
    .expect("write same-file conflict");
    let context = vue3_global_type_context(
        &filename.to_string_lossy(),
        &[same_file.to_string_lossy().to_string()],
        &Vue3TypeResolverContext::default(),
    );
    assert!(!context.type_query_declared_types.contains_key("Shared"));
}

#[test]
fn vue3_namespace_references_bind_to_the_nearest_declared_member() {
    let dir = tempfile::tempdir().expect("temp dir");
    let alias = dir.path().join("root-alias.d.ts");
    let interface = dir.path().join("root-interface.d.ts");
    std::fs::write(&alias, "type Bad = { aliasValue: string }").expect("write root alias");
    std::fs::write(&interface, "interface Bad { interfaceValue: number }")
        .expect("write root interface");
    let filename = dir.path().join("Comp.vue");

    for (index, declarations) in [
        r#"
declare namespace Box {
  interface Bad { localValue: boolean }
  type Dep = Bad
}
"#,
        r#"
declare namespace Box {
  type Dep = Bad
  interface Bad { localValue: boolean }
}
"#,
    ]
    .into_iter()
    .enumerate()
    {
        let consumer = dir.path().join(format!("consumer-{index}.d.ts"));
        std::fs::write(&consumer, declarations).expect("write namespace consumer");
        for files in [
            vec![alias.clone(), interface.clone(), consumer.clone()],
            vec![consumer.clone(), interface.clone(), alias.clone()],
        ] {
            let context = vue3_global_type_context(
                &filename.to_string_lossy(),
                &files
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect::<Vec<_>>(),
                &Vue3TypeResolverContext::default(),
            );
            assert!(context.silent_unresolved_type_names.contains("Bad"));
            assert!(!context.silent_unresolved_type_names.contains("Box.Bad"));
            assert!(!context.silent_unresolved_type_names.contains("Box.Dep"));
            assert!(vue3_type_context_has_name(&context, "Box.Dep"));
        }
    }
}

#[test]
fn vue3_class_and_enum_conflicts_invalidate_value_dependents() {
    let dir = tempfile::tempdir().expect("temp dir");
    let alias = dir.path().join("alias.d.ts");
    let enum_file = dir.path().join("enum.d.ts");
    let consumer = dir.path().join("consumer.d.ts");
    std::fs::write(&alias, "type Shared = { aliasValue: string }").expect("write alias");
    std::fs::write(&enum_file, "declare enum Shared { Value = 1 }").expect("write enum");
    std::fs::write(
        &consumer,
        "type UsesValue = typeof Shared\ntype UsesKeys = keyof typeof Shared\ntype UsesMember = typeof Shared.Value",
    )
    .expect("write value consumers");
    let filename = dir.path().join("Comp.vue");

    for files in [
        vec![alias.clone(), enum_file.clone(), consumer.clone()],
        vec![consumer.clone(), enum_file.clone(), alias.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in ["Shared", "UsesValue", "UsesKeys", "UsesMember"] {
            assert!(context.silent_unresolved_type_names.contains(name));
            assert!(!vue3_type_context_has_name(&context, name));
        }
    }

    let class = dir.path().join("class.d.ts");
    let function = dir.path().join("function.d.ts");
    std::fs::write(&class, "declare class Collision {}")
        .expect("write class collision");
    std::fs::write(&function, "declare function Collision(): string")
        .expect("write function collision");
    for files in [
        vec![class.clone(), function.clone()],
        vec![function.clone(), class.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        assert!(context.silent_unresolved_type_names.contains("Collision"));
        assert!(!context
            .return_type_runtime_type_declarations
            .contains_key("Collision"));
    }

    let bad_alias = dir.path().join("bad-alias.d.ts");
    let bad_interface = dir.path().join("bad-interface.d.ts");
    let dependent_class = dir.path().join("dependent-class.d.ts");
    std::fs::write(&bad_alias, "type Bad = { aliasValue: string }")
        .expect("write bad alias");
    std::fs::write(&bad_interface, "interface Bad { interfaceValue: number }")
        .expect("write bad interface");
    std::fs::write(
        &dependent_class,
        "declare class DependentClass { value: Bad }\ntype ClassValueConsumer = typeof DependentClass",
    )
    .expect("write dependent class");
    for files in [
        vec![
            bad_alias.clone(),
            bad_interface.clone(),
            dependent_class.clone(),
        ],
        vec![dependent_class.clone(), bad_interface.clone(), bad_alias.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        for name in ["Bad", "DependentClass", "ClassValueConsumer"] {
            assert!(context.silent_unresolved_type_names.contains(name));
            assert!(!vue3_type_context_has_name(&context, name));
        }
    }
}

#[test]
fn vue3_module_local_aliases_preserve_their_actual_global_dependencies() {
    let dir = tempfile::tempdir().expect("temp dir");
    let alias = dir.path().join("alias.d.ts");
    let interface = dir.path().join("interface.d.ts");
    let local = dir.path().join("local-global.ts");
    let leaf = dir.path().join("leaf.d.ts");
    let imported = dir.path().join("imported-global.d.ts");
    std::fs::write(&alias, "type Bad = { aliasValue: string }").expect("write alias");
    std::fs::write(&interface, "interface Bad { interfaceValue: number }")
        .expect("write interface");
    std::fs::write(
        &local,
        r#"
export {}
type Local = Bad
class LocalClass { static value: Bad }
namespace LocalTypes {
  type Private = Bad
  export namespace Nested {
    export type Exported = Private
  }
}
namespace Split {
  type Bad = { cleanValue: string }
  export type Clean = Bad
}
namespace Split {
  export type Tainted = Bad
}
namespace Shadow {
  export type Bad = { shadowValue: string }
  export type Relay = Bad
}
declare global {
  type LocalConsumer = Local
  type LocalClassConsumer = typeof LocalClass
  type LocalClassMemberConsumer = typeof LocalClass.value
  type NamespaceConsumer = LocalTypes.Nested.Exported
  type CleanConsumer = Split.Clean
  type TaintedConsumer = Split.Tainted
  type ShadowConsumer = Shadow.Relay
}
"#,
    )
    .expect("write local consumer");
    std::fs::write(&leaf, "export interface Bad { importedValue: boolean }")
        .expect("write imported leaf");
    std::fs::write(
        &imported,
        "import type { Bad } from './leaf'\ndeclare global { type ImportedConsumer = Bad }\nexport {}",
    )
    .expect("write imported consumer");
    let filename = dir.path().join("Comp.vue");

    for files in [
        vec![
            alias.clone(),
            interface.clone(),
            local.clone(),
            imported.clone(),
        ],
        vec![
            imported.clone(),
            local.clone(),
            interface.clone(),
            alias.clone(),
        ],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        assert!(context.silent_unresolved_type_names.contains("Bad"));
        assert!(context
            .silent_unresolved_type_names
            .contains("LocalConsumer"));
        assert!(!vue3_type_context_has_name(&context, "LocalConsumer"));
        for name in ["LocalClassConsumer", "LocalClassMemberConsumer"] {
            assert!(context.silent_unresolved_type_names.contains(name));
            assert!(!vue3_type_context_has_name(&context, name));
        }
        assert!(context
            .silent_unresolved_type_names
            .contains("NamespaceConsumer"));
        assert!(!vue3_type_context_has_name(&context, "NamespaceConsumer"));
        assert!(!context
            .silent_unresolved_type_names
            .contains("CleanConsumer"));
        assert!(vue3_type_context_has_name(&context, "CleanConsumer"));
        assert!(context
            .silent_unresolved_type_names
            .contains("TaintedConsumer"));
        assert!(!vue3_type_context_has_name(&context, "TaintedConsumer"));
        assert!(!context
            .silent_unresolved_type_names
            .contains("ShadowConsumer"));
        assert!(vue3_type_context_has_name(&context, "ShadowConsumer"));
        assert!(!context
            .silent_unresolved_type_names
            .contains("ImportedConsumer"));
        assert!(vue3_type_context_has_name(&context, "ImportedConsumer"));
        assert!(context
            .type_deps
            .get("ImportedConsumer")
            .is_some_and(|deps| deps.contains(&normalize_path_string(&leaf))));
        assert!(vue3_type_context_names(&context)
            .iter()
            .chain(&context.silent_unresolved_type_names)
            .chain(context.type_deps.keys())
            .all(|name| !name.starts_with("local:")));
    }
}

#[test]
fn vue3_current_global_namespaces_shadow_module_imports_in_dependency_graphs() {
    let dir = tempfile::tempdir().expect("temp dir");
    let alias = dir.path().join("namespace-alias.d.ts");
    let interface = dir.path().join("namespace-interface.d.ts");
    let leaf = dir.path().join("leaf.ts");
    let consumer = dir.path().join("consumer.ts");
    std::fs::write(
        &alias,
        "declare namespace Types { type Bad = { aliasValue: string } }",
    )
    .expect("write namespace alias");
    std::fs::write(
        &interface,
        "declare namespace Types { interface Bad { interfaceValue: number } }",
    )
    .expect("write namespace interface");
    std::fs::write(&leaf, "export interface Bad { importedValue: boolean }")
        .expect("write imported namespace leaf");
    std::fs::write(
        &consumer,
        r#"
import type * as Types from './leaf'
import type * as Imported from './leaf'
export {}
declare global {
  namespace Types { interface Marker {} }
  type NamespaceConsumer = Types.Bad
  type ImportedConsumer = Imported.Bad
}
"#,
    )
    .expect("write namespace import shadow consumer");
    let filename = dir.path().join("Comp.vue");

    for files in [
        vec![
            alias.clone(),
            interface.clone(),
            consumer.clone(),
        ],
        vec![consumer.clone(), interface.clone(), alias.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        assert!(context.silent_unresolved_type_names.contains("Types.Bad"));
        assert!(context
            .silent_unresolved_type_names
            .contains("NamespaceConsumer"));
        assert!(!vue3_type_context_has_name(&context, "NamespaceConsumer"));
        assert!(!context
            .silent_unresolved_type_names
            .contains("ImportedConsumer"));
        assert!(vue3_type_context_has_name(&context, "ImportedConsumer"));
        assert!(context
            .type_deps
            .get("ImportedConsumer")
            .is_some_and(|deps| deps.contains(&normalize_path_string(&leaf))));
    }
}

#[test]
fn vue3_global_member_signatures_ignore_shadowed_module_local_identities() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first = dir.path().join("first.ts");
    let second = dir.path().join("second.ts");
    std::fs::write(
        &first,
        r#"
export {}
interface Base { localFirst: Date }
declare global {
  interface Base { globalFirst: string }
  interface Shared { value: Base }
  interface Consumer extends Shared { first: boolean }
}
"#,
    )
    .expect("write first shadowed signature");
    std::fs::write(
        &second,
        r#"
export {}
interface Base { localSecond: RegExp }
declare global {
  interface Base { globalSecond: number }
  interface Shared { value: Base }
  interface Consumer extends Shared { second: bigint }
}
"#,
    )
    .expect("write second shadowed signature");
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
        for name in ["Base", "Shared", "Consumer"] {
            assert!(!context.silent_unresolved_type_names.contains(name));
            assert!(vue3_type_context_has_name(&context, name));
        }
    }
}

#[test]
fn vue3_illegal_same_file_alias_and_class_redeclarations_fail_closed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let filename = dir.path().join("Comp.vue");

    for (index, declarations) in [
        "type Shared = { first: string }\ntype Shared = { second: number }",
        "type Shared = { second: number }\ntype Shared = { first: string }",
    ]
    .into_iter()
    .enumerate()
    {
        let file = dir.path().join(format!("alias-{index}.d.ts"));
        std::fs::write(
            &file,
            format!("{declarations}\ntype AliasConsumer = Shared"),
        )
        .expect("write duplicate aliases");
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &[file.to_string_lossy().to_string()],
            &Vue3TypeResolverContext::default(),
        );
        for name in ["Shared", "AliasConsumer"] {
            assert!(context.silent_unresolved_type_names.contains(name));
            assert!(!vue3_type_context_has_name(&context, name));
        }
    }

    for (index, declarations) in [
        "declare class Repeated { first: string }\ndeclare class Repeated { second: number }",
        "declare class Repeated { second: number }\ndeclare class Repeated { first: string }",
    ]
    .into_iter()
    .enumerate()
    {
        let file = dir.path().join(format!("class-{index}.d.ts"));
        std::fs::write(
            &file,
            format!("{declarations}\ntype ClassConsumer = Repeated"),
        )
        .expect("write duplicate classes");
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &[file.to_string_lossy().to_string()],
            &Vue3TypeResolverContext::default(),
        );
        for name in ["Repeated", "ClassConsumer"] {
            assert!(context.silent_unresolved_type_names.contains(name));
            assert!(!vue3_type_context_has_name(&context, name));
        }
    }
}

#[test]
fn vue3_missing_nearest_namespace_members_do_not_fall_back_to_outer_namespaces() {
    let dir = tempfile::tempdir().expect("temp dir");
    let alias = dir.path().join("outer-alias.d.ts");
    let interface = dir.path().join("outer-interface.d.ts");
    let consumer = dir.path().join("inner-consumer.d.ts");
    std::fs::write(
        &alias,
        "declare namespace A.Types { type Bad = { aliasValue: string } }",
    )
    .expect("write outer namespace alias");
    std::fs::write(
        &interface,
        "declare namespace A.Types { interface Bad { interfaceValue: number } }",
    )
    .expect("write outer namespace interface");
    std::fs::write(
        &consumer,
        r#"
declare namespace A.B {
  namespace Types {}
  type Consumer = Types.Bad
}
type ClosestConsumer = A.B.Consumer
"#,
    )
    .expect("write nearest namespace consumer");
    let filename = dir.path().join("Comp.vue");

    for files in [
        vec![alias.clone(), interface.clone(), consumer.clone()],
        vec![consumer.clone(), interface.clone(), alias.clone()],
    ] {
        let context = vue3_global_type_context(
            &filename.to_string_lossy(),
            &files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            &Vue3TypeResolverContext::default(),
        );
        assert!(context.silent_unresolved_type_names.contains("A.Types.Bad"));
        for name in ["A.B.Consumer", "ClosestConsumer"] {
            assert!(!context.silent_unresolved_type_names.contains(name));
        }
    }
}
