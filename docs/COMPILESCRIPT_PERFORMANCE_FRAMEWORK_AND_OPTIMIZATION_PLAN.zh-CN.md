# CompileScript 性能框架与优化方案

本文对应外部报告：

`F:\data\repos\vue-compiler-playground\webpack5-vue2.7\artifacts\compiler-perf\performance-profile-report.md`

目标不是只复述报告，而是把报告中的性能问题变成当前仓库可复现、可比较、可验收的工程闭环。

## 当前确认结论

已新增性能框架能力：

- `cargo xtask profile-compile-script --script-ast-mode full|top-level|none`
- `cargo xtask compare-compile-script-profile --full ... --top-level ... --none ...`

当前复现结果（debug build，20 iterations，单 fixture）：

| 版本 | full compileScript median | top-level median | no-AST median | full/no-AST | 结论 |
| --- | ---: | ---: | ---: | ---: | --- |
| Vue 2.7 | 4011 us | 1960 us | 1500 us | 2.674x | AST projection 仍是明确问题 |
| Vue 3 | 31329 us | 28939 us | 28444 us | 1.101x | AST 有影响，但当前主瓶颈更偏 type/macro 路径 |

Vue 2.7 同时显示 serialize median 从 1267 us 降到 116 us，说明 public AST 不只影响 Rust projection，也影响 NAPI JSON 传输体积。

## 已解决与未解决边界

已解决：

- Rust 内部已有 `SfcScriptAstMode::{None, TopLevel, Full}`。
- compileScript context 已复用 template usage 和 setup analysis。
- profiling gate 能显示 `templateUsageScanCount = 1`、`setupAnalysisCount = 1`。
- 比较器能把 full/top-level/none 三份报告变成 pass/fail 证据。

未解决：

- public package alias 的 `compileScript(descriptor)` 默认仍走 full AST。
- NAPI `compileVue27SfcScript(source, options)` 和 `compileSfcScript(source, options)` 仍从 source 重新 parse SFC。
- Vue 2.7 `parseComponent` 仍走较重的 SFC block extraction 路径，未建立 parse-only 细分 gate。

## 架构冲突点

### 1. Public API 兼容性 vs build hot path 性能

`scriptAst` / `scriptSetupAst` 是 public result shape 的一部分。`vuec_node_bridge` 现有测试明确断言默认 `sfc.compileScript` / `sfc.vue27.compileScript` 返回 AST。

因此不能简单把默认改成 no-AST，否则会破坏 public contract 和官方 conformance。

正确边界：

- public/default mode 继续 full AST。
- production-loader/native fast path 显式选择 `__vuecScriptAstMode: "none"` 或 `"top-level"`。
- output contract 和 bridge tests 继续保护默认 full AST。

### 2. Descriptor reuse 不能靠隐式全局缓存硬凑

报告指出 JS alias 已有 descriptor，却把 `descriptor.source` 传回 Rust 重新 parse。这个问题成立。

但直接用全局 source cache 有风险：

- 需要按 source hash、filename、version line、parse options 区分。
- descriptor 可能来自外部构造，不能假设一定由本 compiler 的 `parse()` 产出。
- Vue 2.7 / Vue 3 descriptor shape 不完全一致。

正确方向：

- P0 先落显式 AST fast mode，收益最大且风险低。
- P1 再做 descriptor-token 或 native descriptor payload，避免误复用。

### 3. Vue2.7 parse fast path 是独立问题

外部报告中 parseOnly 5x gap 是真实问题，但它和 compileScript AST projection 是两个不同瓶颈。

不应在 compileScript 修复中混入 SFC parser 大重构。parse fast path 应有单独 gate：

- top-level SFC scanner；
- descriptor shape equality；
- parse-only median；
- malformed tag / source range 回归。

## 优化方案

### P0：显式 fast AST mode 进入 NAPI/alias

新增内部 option：

- `__vuecScriptAstMode: "full" | "top-level" | "none"`

Rust NAPI / node bridge 解析该 option 到 `SfcScriptAstMode`。

Native aliases 增加保守策略：

- 默认不设置，保持 full AST。
- 如果调用方显式传 `__vuecScriptAstMode`，转发给 native。
- 后续 production loader 可统一传 `"none"`。

验收：

- 默认 `compileScript` 仍返回 `scriptAst` / `scriptSetupAst`。
- 显式 `"none"` 不返回 AST。
- 显式 `"top-level"` 返回顶层 AST。
- Vue 2.7 profile comparison full/no-AST ratio >= 1.2。
- output contract 通过。

### P1：descriptor reuse 设计

建议新增显式 descriptor token，而不是隐式全局缓存：

- `parse()` 返回 descriptor 时附加 non-enumerable `__vuecDescriptorToken`。
- native 侧保存 token -> descriptor 的短生命周期缓存。
- `compileScript(descriptor)` 如果 token 有效，直接使用 descriptor。
- token 缺失或失效时回退 source parse。

验收：

- 不改变 public enumerable descriptor shape。
- source / filename / parse options 不匹配时不能复用。
- compileScript reparse profile bucket 下降。

### P2：Vue2.7 parse fast path

单独阶段处理：

- 实现 SFC-only top-level scanner。
- 不 token 化 template body。
- 与现有 parser 逐 fixture 比较 descriptor shape、errors、source ranges。

验收：

- parse-only benchmark 改善。
- Vue 2.7 SFC conformance 和 project corpus 通过。

## 当前下一步

优先实施 P0。理由：

- 已有 Rust `SfcScriptAstMode`，只缺 NAPI/bridge/alias plumbing。
- 不改变 public 默认行为。
- 能直接解决外部报告中最大 Vue 2.7 compileScript 热点。

