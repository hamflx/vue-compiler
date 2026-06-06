# Conformance 与 CompileScript 架构方案

这份文档是中文版本，用来解释评审报告里的问题哪些成立、根因在哪里、以及后续应该怎么重构。本文只写方案，不改实现代码。

核对状态：

- 当前提交基线：`9f470a6 Document Vue 2 corpus completion`。
- 写方案前工作树是干净的。
- 评审报告里提到的 profiling 文件没有在当前仓库或相邻的 `..\webpack5-vue2.7\artifacts\compiler-perf` 路径下找到，所以报告中的具体微秒数这次没有独立复验。下面的性能判断主要来自当前代码结构。

## 总结

官方测试本身是有价值的，问题不在于“官方测试没用”。真正的问题是当前证据模型太粗，把 prepared 官方测试、JavaScript 包适配层、suite 专用 bridge、JS callback 边界、Rust public API 执行混在 `rust-backed` / `mixed` / `shim-backed` 这几个标签下面。这样报告看起来容易比真实调用路径更乐观。

当前最需要治理的是覆盖来源的可信度。`xtask/src/compat.rs` 里 suite 级别默认把 Vue 3 generated conformance 标成 `mixed`，但文件级别又通过路径白名单把不少 Vue 3 SFC / SSR / core 文件重新标成 `rust-backed`。只要 Vitest report 存在，文件级统计就可能把整个 suite 聚合回 `rust-backed`。缓存报告和历史文档里已经出现这种漂移：同一个 Vue 3 SSR suite，有的报告是 `mixed 129/129`，另一个报告是 `rust-backed 129/129`，而 reason 文字仍然在描述 prepared SSR/DOM TypeScript source 和 alias runtime 参与。

`compileScript` 性能报告的方向基本可信。即使没有 profiling artifact，当前 `vuec_sfc` 代码也确实存在三个结构性热点：急切生成 public JSON AST 投影、Vue 2.7 return bindings 对每个 import 重复扫描 template、script setup analysis 在 errors/content/bindings 多条路径重复执行。

## 逐条核对

### Conformance 与报告口径

| 评审观点 | 当前核对结果 | 建议处理 |
| --- | --- | --- |
| `rust-backed` 分类偏乐观。 | 成立，是架构问题。`conformance_coverage_kind()` 默认把 generated Vue 3 suite 标为 `mixed`，但 `conformance_coverage_file_kind()` 会按文件路径把很多文件重新标成 `rust-backed`，包括 Vue 3 SSR 文件。只要有执行结果，所有文件就可能聚合成 `rust-backed`，即使 suite reason 仍说明 prepared SSR/DOM TypeScript source 和 alias runtime 参与了执行。 | 不再用文件路径白名单证明 Rust 执行。路径只能是元数据，真实分类要来自实际调用路径和 bridge/runtime provenance。 |
| `ALIAS_RUNTIME_JS` 不是薄适配。 | 成立。它定义 runtime enum/helper、transform context、helper tracking、AST hydrate/materialize，还在 JS 里实现了 `runtime.transformElement()` 的 props、directives、patch flags、block、dynamic props、callback 处理，再调用 Rust projection 做子决策。 | 把 JS 适配层拆成薄 package/API adapter、Rust projection hydrate、callback boundary、semantic JS shim。只有前两类能比较干净地算 Rust 证据。 |
| 官方测试不是原封不动执行，而是 prepared suite。 | 成立。`write_vue3_core_conformance_shims()` 和多个 `rewrite_vue3_*_public_api_spec()` 会改写 import、加入 `.rust-api.ts` helper。这个策略合理，但报告必须写成 `prepared official tests`。 | 生成机器可读的 prepared-test manifest，记录每个文件做了什么 rewrite。 |
| suite bridge 命令有过拟合风险。 | 风险成立，但不是说这些命令一定错。`vuec_node_bridge` 暴露了 `vue3.core.transformBindSuite`、`transformElementSuite`、`cacheStaticSuite` 等 suite 命令。它们有迁移价值，但不是公开 Vue API，有些逻辑会围绕官方 spec helper 形状组装 public JSON AST。 | 把这些命令标成 `suite-projection` 或 `suite-command`，不能直接等同于 `pure-rust public api`。 |
| SFC/style 的 JS callback 边界合理，但报告要更清楚。 | 成立。Vue 2.7 style alias 会在 Rust style 编译后，在 JS 里执行 caller-provided PostCSS plugin；bridge 返回 placeholder `rawResult`。这是合理的 JS API 边界，但必须是 `mixed`。 | 保留 JS callback 执行，但报告写清楚：Rust style compiler + JS callback boundary。 |
| 外部项目 corpus 还不够中心。 | 部分已经改善。Vue 2 project corpus 现在已经很强，最近 15/15 通过，但它仍然是独立证据，没有成为 `summarize-compat` 这类主报告的一部分。 | 把 corpus 报告提升到 release / CI summary，作为生产项目语料证据，和官方 conformance 分开呈现。 |

### CompileScript 性能

| 评审观点 | 当前核对结果 | 建议处理 |
| --- | --- | --- |
| `script_ast` / `script_setup_ast` JSON 投影贵。 | 代码结构上成立。`compile_script()` 和 `compile_vue27_script()` 会对 JS-like script block 调 `sfc_script_ast_body()`；它递归构造 `serde_json::Value`、`loc` 和 `source` slice。`position_at()` 每次从源码开头扫描到 offset，节点多时成本会放大。 | 增加 lazy/optional public AST projection；需要 loc 时用 line index 做 offset 到 line/column 的转换。 |
| Vue 2.7 return bindings 对 template 重复扫描。 | 成立。`vue27_script_setup_return_bindings()` 对每个 import 调 `vue27_script_setup_import_is_returned()`，再调 `vue27_template_uses_identifier()`，后者每次都重建 usage string。 | 一次性构建 `TemplateUsageIndex`，所有 import 复用。 |
| `analyze_vue27_script_setup()` 重复运行。 | 成立。当前 compile 路径在 script errors、generated content、binding metadata 等路径都会分析同一个 setup block。 | 引入 compile context，保存 setup analysis，供 errors/content/bindings/return bindings 共用。 |
| NAPI/SFC parse 是主因。 | 本轮没有验证为主因。bridge 确实会为 `sfc.vue27.compileScript` parse source 得到 descriptor，但当前代码更支持“AST 投影和重复分析才是主要结构性成本”的判断。 | descriptor reuse 放到后面优化，先处理 Rust 内部热点。 |
| 应按 profiling 微秒数定优先级。 | 具体数值本轮不能独立确认，因为 artifact 没找到。 | 先补一个可复现的 profiling 命令，再把具体微秒数当性能 gate。 |

## Conformance 架构方案

### 1. 用 provenance 维度替代单一来源标签

当前 `rust-backed`、`mixed`、`shim-backed` 应该变成从多个维度汇总出来的结果，而不是靠文件路径推断。

建议新增这些维度：

- `test_origin`：`unmodified-official`、`prepared-official`、`project-corpus`、`api-manifest`、`option-matrix`、`output-contract`、`custom-regression`。
- `execution_path`：`pure-rust-public-api`、`rust-bridge-shape-adapter`、`hybrid-js-adapter-rust-projection`、`mixed-js-callback-boundary`、`shim-backed-semantic-js`。
- `api_surface`：`public-package-api`、`public-rust-api`、`suite-only-bridge-command`、`internal-helper-import`。
- `adapter_role`：`import-rewrite`、`runner-support`、`hydration-dehydration`、`callback-materialization`、`semantic-shim`。
- `bridge_commands`：该文件或断言实际观察到的 `callBridge()` 命令。

建议报告结构：

```json
{
  "coverage": {
    "summary": {
      "pure-rust-public-api": { "pass": 0, "total": 0 },
      "rust-bridge-shape-adapter": { "pass": 0, "total": 0 },
      "hybrid-js-adapter-rust-projection": { "pass": 0, "total": 0 },
      "mixed-js-callback-boundary": { "pass": 0, "total": 0 },
      "shim-backed-semantic-js": { "pass": 0, "total": 0 }
    },
    "files": [
      {
        "path": "packages/compiler-core/__tests__/transforms/transformElement.spec.ts",
        "test_origin": "prepared-official",
        "execution_path": "hybrid-js-adapter-rust-projection",
        "api_surface": "suite-only-bridge-command",
        "bridge_commands": ["vue3.core.transformElementSuite"],
        "adapter_role": ["import-rewrite", "hydration-dehydration"],
        "reason": "..."
      }
    ]
  }
}
```

兼容旧报告时，`rust-backed`、`mixed`、`shim-backed` 可以继续保留，但必须从这些维度推导：

- `rust-backed`：只允许 `pure-rust-public-api`，或很窄的 `rust-bridge-shape-adapter` 且没有 JS 语义决策参与。
- `mixed`：只要 JS callback、官方 TypeScript source、suite-only projection 参与被断言的行为，就归 mixed。
- `shim-backed`：JS 实现了 Rust 没有执行的编译器语义。

### 2. 增加 prepared-test manifest

每个 rewrite helper 在准备官方测试时都应该写 manifest。

manifest 字段：

- 原始文件路径
- prepared 文件路径
- rewrite 类型：import rewrite、helper injection、test utility replacement、callback adapter
- helper 文件路径
- bridge command 列表
- 预期 coverage 维度

这样就不会把“prepared 官方测试通过”误读成“未修改官方测试通过”。只要 manifest 不为空，报告就应该写 `prepared official tests`。

### 3. 运行时记录实际 provenance

alias runtime 应该在每个测试文件或断言执行期间记录真实 marker：

- `callBridge(command)` 命令名。
- JS semantic adapter marker，例如 `js.transformElement.props`、`js.transformContext.replaceNode`、`js.postcss.plugin`。
- callback boundary marker，例如 `callback.nodeTransform`、`callback.directiveTransform`、`callback.postcssPlugin`。

runner 在每个测试结束后把 marker 写进 Vitest/Jasmine report。文件路径分类可以作为默认值，但只要实际观察到 JS 语义或 callback marker，就必须能把文件从 `rust-backed` 降级为 `mixed` 或 `shim-backed`。

### 4. 拆分 alias runtime 职责

`ALIAS_RUNTIME_JS` 当前职责太多，建议至少在概念上拆成这些层：

- `package-api-adapter`：导出名、arity、public export、参数归一化。
- `bridge-shape-adapter`：JSON 序列化、Symbol hydrate、`undefined` 恢复、public result shape。
- `callback-boundary`：PostCSS plugin、NodeTransform、directiveTransform 等无法序列化的 caller callback。
- `semantic-js-shim`：任何会做编译器语义决策的 JS 代码。
- `suite-helper`：prepared 官方 spec helper 和 suite-only bridge glue。

规则：

- 新增 semantic JS shim 必须带 Rust 迁移计划。
- suite helper 不能算 public API parity。
- callback boundary 可以长期存在，但永远是 mixed。
- 薄 adapter 可以长期留在 JS，但不能拥有编译器语义。

### 5. 重新分类 suite bridge 命令

`vue3.core.transformBindSuite`、`vue3.core.transformElementSuite` 这类命令应该标成兼容性 suite 命令。它们可以作为迁移中间证据，但报告不能把它们等同为公开 compiler API。

建议分类：

- `public-command`：例如 `sfc.compileScript`、`vue3.dom.compile`、`vue3.ssr.compile`。
- `projection-command`：Rust 内部 transform 的 projection，可作为 focused evidence。
- `suite-command`：围绕官方 spec helper 形状设计的命令。

验收规则：suite command 可以证明“Rust projection 覆盖”，但不能单独证明“pure Rust public API 覆盖”。

### 6. 提升真实项目 corpus 证据地位

Vue 2 project corpus 应作为单独 release gate 报告：

- CI / release summary 中加入 `verify-vue2-project-corpus`。
- 报告 project count、file count、mode count、official compiler baseline、Rust commit。
- corpus 失败和官方 conformance 失败分开看。
- Vue 3 project corpus 可以后续补，不阻塞当前 reporting 修正。

## CompileScript 架构方案

### 1. 引入 Script Compile Context

为 `compile_vue27_script()` 建一个 `Vue27ScriptCompileContext`，或者更通用的 `SfcScriptCompileContext`，每次 compile 只构建一次。

它应该保存或引用：

- normal script parse result 或 registered program id
- setup script parse result 或 registered program id
- Vue 2.7 setup analysis
- normal script option/default-export analysis
- normal script return bindings/imports
- binding metadata
- template usage index
- CSS vars
- public AST projection state

之后 errors、generated content、bindings、imports、AST projection 都从这个 context 派生，不再重复 parse 或重复 analyze。

### 2. 让 public AST projection 支持 lazy/optional

当前 public result shape 包含 `scriptAst` 和 `scriptSetupAst`，不能直接删除。建议分阶段：

1. 增加内部 `SfcScriptAstMode`：`None`、`TopLevel`、`Full`。
2. official public API alias 先保持当前 full mode，直到 output contract 证明更窄默认值安全。
3. benchmark/corpus/internal 路径如果不检查 AST，就走 `None`。
4. compile context 保存足够的 parse identity，在 caller 或 serializer 真需要时再 materialize AST projection。

loc 投影要用预计算 line index：

- 每个 source 只构建一次 line start offsets。
- offset 转 line/column 用二分。
- UTF-16 column 只对当前行 slice 计算。
- 避免 `position_at()` 对每个节点都从源码开头扫。

这样可以保留 public API 兼容，同时移除热路径上不必要的 JSON 分配。

### 3. template usage 只预计算一次

用可复用的 `TemplateUsageIndex` 替代每个 import 重新扫 template。

最低风险版本：

- 对 template 只调用一次 `vue27_template_usage_check_string(template, is_ts)`。
- 把 usage 传进 `vue27_script_setup_return_bindings()`。
- 所有 import 检查复用这个 usage。

后续更好的版本：

- 把 usage string 解析成 identifier set。
- identifier lookup 变成 O(1)。
- 保留当前 TS/template expression 处理边界，不改变语义。

这项风险低，因为目标是删除重复工作，不改变输出。

### 4. 去重 Vue 2.7 setup analysis

这些消费者应该共享同一份 analysis：

- `vue27_script_compile_errors()`
- `vue27_script_setup_content()`
- `vue27_setup_binding_metadata()`

context 暴露：

- `setup_analysis(is_prod)`
- `setup_errors`
- `setup_bindings`
- `return_bindings`

如果 production mode 会影响 analysis 输出，可以按少量 mode flag 缓存，而不是无条件重复跑。

### 5. descriptor reuse 放到第二阶段

bridge 当前会为 `sfc.vue27.compileScript` parse source 得到 descriptor。跨 `parse()` 和 `compileScript()` 复用 descriptor 可能有收益，但优先级低于：

1. AST projection mode
2. template usage index
3. shared setup analysis

这三项都是 Rust 内部结构性成本，不需要先改 public package call contract。

### 6. 增加可复现性能 gate

实现性能优化前，应先补一个能稳定产出报告的命令，例如：

```text
cargo xtask profile-compile-script --fixture-corpus compat/perf/vue27-sfc --iterations 60
```

报告字段至少包括：

- Rust commit
- official compiler package version
- release/profile/debug mode
- fixture list 和文件大小
- 每个 phase 的 median/p95
- public AST projection 是否开启
- template usage index 是否开启

优化后的验收标准：

- Vue 2.7 SFC conformance 仍通过。
- 受影响路径的 Vue 2 project corpus 仍通过。
- `vue/compiler-sfc` output contract 仍通过。
- public API 需要时仍保留 `scriptAst` / `scriptSetupAst` 形状。
- 同一 corpus 上 median `compileScript` 时间下降，大 template 的 p95 不出现明显回退。

## 建议里程碑

1. 报告真实性修正：
   - 增加 prepared-test manifest。
   - 报告加入 provenance 维度。
   - 停止只靠路径白名单升级为 `rust-backed`。
   - 修正当前互相矛盾的文档和报告口径。

2. 适配层边界拆分：
   - 拆分 `ALIAS_RUNTIME_JS` 职责。
   - 标记 semantic JS shim 和 callback boundary。
   - suite-only bridge command 与 public API 分开。

3. CompileScript 低风险性能优化：
   - 增加共享 Vue 2.7 compile context。
   - template usage 只预计算一次。
   - setup analysis 去重。

4. AST projection 性能优化：
   - 增加 AST projection mode。
   - 增加 loc line index。
   - internal/corpus/benchmark 安全路径走 no-AST mode。

5. 证据加固：
   - 增加可复现 compileScript profiling 命令。
   - 把 Vue 2 project corpus 提升到 summary/CI release evidence。
   - Vue 3 project corpus 另起计划，不和当前修正混在一起。

## 暂时不要改的东西

- 不要移除 PostCSS plugin 或 caller-provided transform 的 JS callback 支持，这些是真实 JS API 边界。
- 不要把 prepared official tests 写成 unmodified official tests。
- 不要在没有等价 public Rust API 或 focused Rust tests 前删除 suite bridge commands。
- 不要在 output contract 和 API consumer 没证明安全前改变 public `compileScript` result shape。
- 不要把新的性能数字作为 gate，除非 profiling artifact 能由 checked-in command 复现。

## 需要你 review 的问题

- `rust-backed` 这个公开标签还要不要保留，还是彻底改成 provenance 分类？
- suite-only bridge command 能不能算 Rust 完成度，还是只能算迁移进度？
- public `compileScript` 是否永远默认 full AST projection，只允许 internal caller 关闭？
- Vue 2 project corpus 应该进入 `summarize-compat`，还是单独作为 production corpus gate 展示？
