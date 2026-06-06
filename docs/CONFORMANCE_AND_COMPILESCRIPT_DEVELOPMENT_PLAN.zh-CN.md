# Conformance 与 CompileScript 开发计划

本文是 `docs/CONFORMANCE_AND_COMPILESCRIPT_ARCHITECTURE_PLAN.md` 的落地开发计划。目标是把架构方案拆成可执行阶段，并为每个阶段定义明确验收条件。

## 总原则

- 每个大功能单独提交，提交信息要说明阶段和主要行为变化。
- 不用路径白名单证明 Rust 覆盖。路径只能作为默认元数据，最终分类必须来自 manifest 和运行时 provenance。
- 不把 prepared official tests 写成 unmodified official tests。
- 不把 suite-only bridge command 等同于 public API parity。
- 不为了性能改变 public API result shape。`compileScript` 的 public 输出必须由 output contract 和 conformance 保护。
- 性能优化先建立可复现 profiling，再接受具体耗时数字作为 gate。

## 总体验收

全部阶段完成后，至少满足：

- `cargo fmt --all -- --check` 通过。
- `git diff --check` 通过。
- `cargo test -p xtask` 通过。
- `cargo test -p vuec_sfc --lib` 通过。
- `cargo test -p vuec_node_bridge` 通过。
- `cargo xtask summarize-compat --locked` 通过，并且报告里不会出现 coverage `source` 与 `reason` 自相矛盾。
- `cargo xtask run-conformance --suite vue27-sfc` 通过。
- `cargo xtask run-conformance --suite vue3-core` 通过。
- `cargo xtask run-conformance --suite vue3-sfc` 通过。
- `cargo xtask run-conformance --suite vue3-ssr` 通过。
- `cargo xtask run-output-contract --version-line vue2_7 --package vue --entry vue/compiler-sfc` 通过。
- `cargo xtask run-output-contract --version-line vue3 --package @vue/compiler-sfc` 通过。
- `cargo xtask verify-vue2-project-corpus` 通过，或在与本阶段无关且已知外部网络/checkout 问题时明确记录原因。

## 阶段 0：基线证据冻结

目标：在改报告和性能路径前，固定当前可比较基线。

开发任务：

- 新增或更新一份本轮 baseline 记录，包含当前 commit、compat lock hash、主要 conformance 数字、Vue 2 corpus 数字。
- 保存当前 `summarize-compat --locked` 输出的关键摘要。
- 保存当前 Vue 2.7 / Vue 3 SFC `compileScript` profiling 命令缺失状态，避免后续误用不可复现数字。

验收条件：

- 文档中记录 baseline commit 和执行日期。
- 如果未运行完整 suite，必须写明未运行项和原因。
- 后续阶段的报告变化都能和这份 baseline 对照。
- 本阶段不改变 Rust/JS 行为代码。

建议提交：

- `Document conformance and compileScript baseline`

## 阶段 1：Prepared Test Manifest

目标：所有官方测试 rewrite 都能被机器读出来，报告明确区分 prepared 与 unmodified。

开发任务：

- 为 prepared official tests 增加 manifest 数据结构，例如 `PreparedTestManifest`。
- 每个 rewrite helper 在写入 prepared 文件或 helper 文件时登记：
  - original path
  - prepared path
  - rewrite kind
  - helper path
  - related bridge commands
  - expected provenance dimensions
- 将 manifest 写到 `target/conformance/<lock>/prepared/<suite>/prepared-test-manifest.json` 或 suite report 内。
- 对 Vue 3 core 的 vBind / transformElement / transform suite helper 至少加单元测试。
- 对 Vue 3 SFC / SSR 的 prepared source import rewrite 加测试。

验收条件：

- 每个 generated-alias official suite 都输出 manifest。
- manifest 为空时才允许 report 写 `unmodified-official`。
- 有 manifest 的 suite report 必须写 `prepared-official`。
- `cargo test -p xtask prepared` 或等价 focused xtask tests 覆盖 manifest 生成。
- `cargo xtask run-conformance --suite vue3-core` 产物中能看到 manifest 或 report manifest 引用。

建议提交：

- `Add prepared official test manifest`

## 阶段 2：Coverage Provenance Schema

目标：把 `rust-backed` / `mixed` / `shim-backed` 从手写标签升级为从 provenance 维度推导出来的结果。

开发任务：

- 在 conformance report 中新增维度：
  - `test_origin`
  - `execution_path`
  - `api_surface`
  - `adapter_role`
  - `bridge_commands`
- 保留 legacy `rust-backed` / `mixed` / `shim-backed`，但只作为派生字段。
- 删除或降级“路径白名单直接升级为 rust-backed”的逻辑。路径可以给默认 expectation，不能作为最终证明。
- 增加 report invariant：
  - `source = rust-backed` 时，reason 不能描述 mixed official source / JS semantic adapter / callback boundary。
  - `suite-only-bridge-command` 不能派生成 `pure-rust-public-api`。
  - callback boundary 必须派生成 `mixed`。

验收条件：

- `cargo test -p xtask coverage` 或等价 focused tests 覆盖 provenance 派生逻辑。
- 用 fixture report 验证：同一文件即使路径在白名单，只要有 mixed marker，最终仍为 `mixed`。
- `cargo xtask run-conformance --suite vue3-ssr` 的 report 不再出现 `source = rust-backed` 但 reason 描述 mixed SSR/DOM source 的情况。
- `cargo xtask summarize-compat --locked` 输出兼容旧三类统计，同时能引用新 provenance summary。

建议提交：

- `Add conformance provenance coverage schema`

## 阶段 3：运行时 Provenance 采集

目标：报告分类来自实际执行路径，而不是只靠文件名或 suite 名。

开发任务：

- 在 alias runtime 的 `callBridge()` 包装层记录 command marker。
- 在 JS semantic adapter 关键点记录 marker，例如：
  - transform context mutation
  - JS transformElement props/directive/patch flag 处理
  - PostCSS plugin callback
  - caller-provided NodeTransform / directiveTransform
- Vitest/Jasmine runner 在每个 assertion 或 test file 结束时 flush markers。
- conformance report 聚合 marker 到文件级和 suite 级 provenance。

验收条件：

- 至少有一个测试 fixture 能证明 `callBridge('vue3.core.transformElementSuite')` 被记录。
- 至少有一个测试 fixture 能证明 JS callback boundary 会把 coverage 降级为 `mixed`。
- `cargo test -p xtask` 通过。
- `cargo xtask run-conformance --suite vue27-sfc` 的 PostCSS callback case 仍标记为 mixed。
- `cargo xtask run-conformance --suite vue3-core` 中 caller-provided transform/context extension cases 仍不计入 pure Rust completion。

建议提交：

- `Record runtime provenance in conformance reports`

## 阶段 4：Alias Runtime 职责拆分

目标：把 `ALIAS_RUNTIME_JS` 中不同职责分层，避免语义 shim 混入薄 adapter。

开发任务：

- 将 runtime JS 逻辑按职责拆成片段或生成模块：
  - package/API adapter
  - bridge shape adapter
  - callback boundary
  - semantic JS shim
  - suite helper
- 每个片段在 report manifest 中有 role 标识。
- 新增 semantic JS shim 必须附带 Rust migration note。
- 保留现有 public package API 行为。

验收条件：

- `cargo test -p xtask alias_runtime` 或等价 focused tests 覆盖各片段存在和拼接顺序。
- API manifest 不退化：相关 `verify-napi-api` / alias API gate 按当前项目命令通过，或至少 focused package alias smoke 通过。
- Vue 2.7 SFC PostCSS callback 仍可执行。
- Vue 3 core prepared suite helper 仍可调用相关 bridge command。
- report 中 adapter role 不再只有一段不可区分的 `ALIAS_RUNTIME_JS`。

建议提交：

- `Split alias runtime provenance roles`

## 阶段 5：Bridge Command Registry

目标：所有 bridge command 都有明确类别，suite-only command 不再伪装成 public API。

开发任务：

- 增加 bridge command registry，至少包含：
  - command name
  - category: `public-command` / `projection-command` / `suite-command`
  - owning Rust crate/module
  - public API equivalent, if any
  - migration note
- command dispatch 使用或校验 registry。
- coverage provenance 从 registry 读取 `api_surface` 默认值。

验收条件：

- registry 覆盖当前 `vuec_node_bridge` dispatch 中所有 command。
- 如果新增 dispatch command 但 registry 缺失，`cargo test -p vuec_node_bridge` 或 xtask test 失败。
- `vue3.core.transformBindSuite` / `vue3.core.transformElementSuite` 等 suite command 在 report 中标为 `suite-only-bridge-command`。
- public commands 如 `sfc.compileScript`、`vue3.dom.compile`、`vue3.ssr.compile` 标为 public command。

建议提交：

- `Classify node bridge commands`

## 阶段 6：文档与 Summary 口径修正

目标：修掉历史文档和 summary 中互相矛盾的 coverage 口径。

开发任务：

- 更新 `docs/2.DEVELOPMENT_PLAN.md`、`docs/WORK_PART.md` 中当前仍矛盾的 Vue 3 SSR / SFC coverage 表述。
- `summarize-compat --locked` 输出中增加 production corpus evidence 的引用或状态。
- Vue 2 corpus report 与 official conformance 分开展示，不互相覆盖。

验收条件：

- `rg "vue3-ssr.*rust-backed 129/129|mixed 129/129" docs` 的结果没有互相冲突的“当前状态”表述；历史记录必须明确是历史切片。
- `cargo xtask summarize-compat --locked` 能显示 official conformance 和 production corpus 是两类证据。
- `cargo xtask verify-release-docs` 如该 gate 覆盖相关文档，则必须通过。

建议提交：

- `Align conformance evidence documentation`

## 阶段 7：CompileScript Profiling Gate

目标：先建立可复现 profiling，再做性能重构。

开发任务：

- 增加 profiling 命令，例如：

```text
cargo xtask profile-compile-script --version-line vue2_7 --fixture-corpus compat/perf/vue27-sfc --iterations 60
cargo xtask profile-compile-script --version-line vue3 --fixture-corpus compat/perf/vue3-sfc --iterations 60
```

- 报告写到 `target/perf/compile-script/<version-line>.json`。
- 报告至少包含：
  - rust commit
  - OS / arch
  - profile/release/debug mode
  - fixture list
  - source byte length
  - script/setup/template byte length
  - phase median / p95
  - AST projection enabled
  - template usage scan count
  - setup analysis count

验收条件：

- profiling 命令在没有外部不可控依赖时可重复运行。
- report JSON schema 有 xtask 单元测试。
- report 能证明当前 baseline 中 AST projection、template usage scan、setup analysis count 的存在。
- 不要求本阶段性能变快，只要求数据可复现。

建议提交：

- `Add compileScript profiling gate`

## 阶段 8：SfcScriptCompileContext 骨架

目标：先建立通用上下文边界，不急着改所有逻辑。

开发任务：

- 新增通用 context 概念或 trait：
  - normal script metadata
  - setup script metadata
  - raw combined content
  - source type
  - AST projection mode
  - template usage cache slot
- 新增 `Vue27ScriptCompileContext` 和 `Vue3ScriptCompileContext` 的最小结构。
- 初始阶段允许 context 只包住现有逻辑，不改变输出。

验收条件：

- public `compile_script()` 和 `compile_vue27_script()` 输出与修改前一致。
- `cargo test -p vuec_sfc --lib compile_script` 通过。
- `cargo test -p vuec_node_bridge compile_script` 通过。
- `cargo xtask run-output-contract --version-line vue2_7 --package vue --entry vue/compiler-sfc` 通过。
- `cargo xtask run-output-contract --version-line vue3 --package @vue/compiler-sfc` 通过。

建议提交：

- `Introduce SFC script compile contexts`

## 阶段 9：TemplateUsageIndex

目标：Vue 2.7 和 Vue 3 都不再为每个 import 重扫 template。

开发任务：

- 新增 `TemplateUsageIndex`，支持 Vue 2.7 和 Vue 3 usage 规则。
- Vue 2.7：
  - `vue27_script_setup_return_bindings()` 复用 context 中的 usage index。
- Vue 3：
  - `vue3_script_setup_return_bindings()` 和 `vue3_script_setup_import_metadata()` 复用同一个 usage index。
- profiling report 增加 `template_usage_scan_count`。

验收条件：

- 对无 `src` / 无 `lang` template 的单次 compile，`template_usage_scan_count <= 1`。
- Vue 2.7 import usage 官方断言仍通过。
- Vue 3 import usage / binding metadata 官方断言仍通过。
- `cargo test -p vuec_sfc --lib template_usage` 或等价 focused tests 覆盖 TS/template expression 边界。
- `cargo xtask run-conformance --suite vue27-sfc` 通过。
- `cargo xtask run-conformance --suite vue3-sfc` 通过。

建议提交：

- `Reuse template usage analysis in compileScript`

## 阶段 10：Vue 2.7 Setup Analysis 去重

目标：`analyze_vue27_script_setup()` 在一次 compile 中不再重复执行。

开发任务：

- `vue27_script_compile_errors()` 从 `Vue27ScriptCompileContext` 读取 setup errors。
- `vue27_script_setup_content()` 从 context 读取 setup analysis。
- `vue27_setup_binding_metadata()` 从 context 读取 setup bindings。
- normal script return bindings/imports 缓存在 context 中。
- profiling report 增加 `setup_analysis_count`。

验收条件：

- 含 `<script setup>` 的 Vue 2.7 单次 compile 中，`setup_analysis_count <= 1`，除非不同 production mode 明确需要独立分析并在 report 标注。
- Vue 2.7 script setup macro error tests 仍通过。
- Vue 2.7 binding metadata tests 仍通过。
- `cargo test -p vuec_sfc --lib vue27_compile_script` 通过。
- `cargo test -p vuec_node_bridge vue27_bridge_compile_script` 通过。
- `cargo xtask run-conformance --suite vue27-sfc` 通过。
- `cargo xtask verify-vue2-project-corpus` 通过或明确记录与本阶段无关的外部失败。

建议提交：

- `Deduplicate Vue 2.7 script setup analysis`

## 阶段 11：Vue 3 CompileScript Context 复用

目标：Vue 3 `compile_script()` 也使用 context 复用 parse、type resolver、setup analysis、template usage。

开发任务：

- `vue3_script_compile_errors()` 避免在 `compile_script()` 和 `script_content()` 中重复计算。
- `analyze_vue3_script_setup()` 结果进入 `Vue3ScriptCompileContext`。
- `vue3_normal_script_type_context()`、`vue3_normal_script_user_imports()`、type resolver context 进入 context。
- inline template 所需 binding metadata 从 context 派生。
- profiling report 记录 Vue 3 setup analysis count、template usage scan count。

验收条件：

- 含 `<script setup>` 的 Vue 3 单次 compile 中，setup analysis 不重复执行。
- Vue 3 type resolve / defineProps / defineEmits / defineModel / props destructure focused tests 通过。
- `cargo test -p vuec_sfc --lib vue3_compile_script` 通过。
- `cargo test -p vuec_node_bridge vue3_sfc_bridge_compile_script` 通过。
- `cargo xtask run-conformance --suite vue3-sfc` 通过。
- `cargo xtask run-output-contract --version-line vue3 --package @vue/compiler-sfc` 通过。

建议提交：

- `Reuse Vue 3 compileScript analysis context`

## 阶段 12：AST Projection Mode 与 Line Index

目标：public AST projection 不再无条件成为所有 compileScript 路径的热成本。

开发任务：

- 增加内部 `SfcScriptAstMode`：
  - `None`
  - `TopLevel`
  - `Full`
- public package API 默认保持当前行为，除非 output contract 证明可以改变。
- internal benchmark/corpus 路径可选择 `None`。
- AST loc 计算使用预计算 line index，替代每个节点从源码开头扫描。
- projection report 记录 AST mode 和 loc conversion strategy。

验收条件：

- 默认 public mode 下，`scriptAst` / `scriptSetupAst` shape 与修改前一致。
- focused tests 验证 loc start/end offset、line、column 不回退。
- no-AST internal mode 不生成 `serde_json::Value` AST projection。
- `cargo test -p vuec_sfc --lib script_ast` 通过。
- `cargo test -p vuec_node_bridge compile_script_preserves_script_ast` 通过。
- Vue 2.7 / Vue 3 SFC output contracts 通过。
- profiling 显示 no-AST mode 下 AST projection phase 接近 0 或明确不执行。

建议提交：

- `Add compileScript AST projection modes`
- `Use line index for script AST locations`

## 阶段 13：性能回归验收

目标：确认优化真实有效，并且行为没有回退。

开发任务：

- 用阶段 7 的 profiling 命令重跑 Vue 2.7 和 Vue 3。
- 对比 baseline：
  - median
  - p95
  - AST projection phase
  - template usage scan count
  - setup analysis count
- 将结果写入 perf report 或 docs summary。

验收条件：

- Vue 2.7：template usage scan count 对普通 inline template 不超过 1。
- Vue 2.7：setup analysis count 对单 mode compile 不超过 1。
- Vue 3：template usage scan count 不重复按 import 增长。
- Vue 3：setup analysis / compile error analysis 不重复执行。
- public default mode 无行为回退。
- internal no-AST mode 有可测性能收益。
- 如果 median 没有改善，必须在报告中解释原因并确认没有引入复杂度过高的无收益重构。

建议提交：

- `Record compileScript performance improvements`

## 阶段 14：Descriptor Reuse 决策

目标：只在 profiling 证明有必要时，才做 descriptor reuse。

开发任务：

- 分析阶段 13 的报告中 SFC parse 占比。
- 如果占比低，明确记录“不做 descriptor reuse”。
- 如果占比高，再设计 public package call 之间的 descriptor cache 或 bridge payload reuse。

验收条件：

- 有 profiling 数据支持做或不做。
- 如果不做，文档记录理由。
- 如果做，必须满足：
  - 不改变 public `parse()` / `compileScript()` 调用语义。
  - descriptor cache 不跨 source/version/options 错误复用。
  - output contracts 通过。
  - conformance 通过。

建议提交：

- `Document compileScript descriptor reuse decision`

## 开发顺序建议

优先顺序：

1. 阶段 0：基线证据冻结。
2. 阶段 1-3：先让 conformance 报告可信。
3. 阶段 4-6：清理 adapter/bridge/documentation 边界。
4. 阶段 7：建立 profiling gate。
5. 阶段 8-11：上下文和重复分析优化。
6. 阶段 12-13：AST projection 和性能验收。
7. 阶段 14：基于数据决定 descriptor reuse。

原因：

- 如果报告口径不可信，后续任何“通过率”和“Rust-backed 数字”都不能作为验收依据。
- 如果没有 profiling gate，compileScript 优化只能靠感觉判断。
- 如果没有 context，template usage / setup analysis / AST projection 优化会继续散落在函数里，后续难维护。
