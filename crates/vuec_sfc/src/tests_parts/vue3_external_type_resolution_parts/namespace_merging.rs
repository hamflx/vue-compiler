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
