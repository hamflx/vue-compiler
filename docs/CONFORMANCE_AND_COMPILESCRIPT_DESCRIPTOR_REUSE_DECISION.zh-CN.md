# CompileScript Descriptor Reuse 决策

本文件对应 `docs/CONFORMANCE_AND_COMPILESCRIPT_DEVELOPMENT_PLAN.zh-CN.md` 的阶段 14。结论：当前不实现跨 public API 调用的 descriptor reuse / descriptor cache。

## 输入数据

依据阶段 13 的 profiling 报告：

- `docs/CONFORMANCE_AND_COMPILESCRIPT_PERFORMANCE_REPORT.zh-CN.md`
- current commit：`98966da Use line index for script AST locations`
- iterations：60
- build profile：debug

当前 profile 摘要：

| 版本线 | fixture | parse median | compileScript median | total median | parse / total | parse / compileScript |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Vue 2.7 | `compat/perf/vue27-sfc/base-layout-header.vue` | 1155 us | 1454 us | 2727 us | 42.4% | 79.4% |
| Vue 3 | `compat/perf/vue3-sfc/base-layout-header.vue` | 1137 us | 21697 us | 23234 us | 4.9% | 5.2% |

需要注意：这里的 parse 是 profiling 命令中每轮从 SFC source 构造 descriptor 的成本。它不等于 public package 调用中一定能被安全复用的全部成本，因为 public `parse()` 结果、descriptor 对象、options、filename、source map、block 引用和用户可能的 descriptor mutation 都会影响后续 `compileScript()` 语义。

## 决策

当前不做 descriptor reuse。

原因：

- Vue 3 是更重的 compileScript 路径，parse 只占 total median 约 4.9%，不是当前主要瓶颈。
- Vue 2.7 parse 占比高，但绝对值约 1.1 ms；阶段 8-13 已把 compileScript median 从 baseline 的 11170 us 降到 1454 us，下一步更应该扩大真实项目 corpus 和 release profile，而不是马上引入跨调用缓存。
- descriptor reuse 不是局部纯函数优化。它会把 public `parse()` / `compileScript()` 调用之间的对象身份、source identity、options identity、descriptor mutation、跨版本 descriptor shape 和 bridge 序列化策略耦合起来。
- 当前 public alias / native / wasm 边界允许调用者传入自行构造或修改过的 descriptor。缓存如果只按 source/filename 命中，可能错误忽略用户对 descriptor block、attrs、content、loc 或 errors 的修改。
- Rust 侧 `SfcCompiler` 已有单次 compile 内部上下文复用。继续做 descriptor cache 属于跨 public API 调用的有状态缓存，复杂度和语义风险高于当前 profile 证明的收益。

## 如果未来要做，需要满足的触发条件

只有同时满足以下条件，才重新设计 descriptor reuse：

- release profile 也显示 parse 是明确瓶颈，而不是 debug-only 比例偏高。
- 多文件真实项目 corpus 中，parse median 或 p95 对端到端构建时间有显著影响。
- profile 能证明缓存收益高于上下文维护、序列化、hash、失效判断和内存占用成本。
- output contract 和 conformance 中存在大量 `parse(source)` 后立即 `compileScript(descriptor)` 的真实调用形态，并且这些调用没有用户 mutation 介入。

建议触发线：

- Vue 3 或真实项目 corpus 中 parse / total median 超过 20%，并且 parse p95 对总 p95 有可见贡献。
- 同一 source 在同一进程内被重复 parse 至少 2 次以上，且能证明 descriptor 没有用户可观察 mutation 语义。

## 未来设计约束

如果未来重新打开 descriptor reuse，只允许按以下边界设计：

- 不改变 public `parse()` / `compileScript()` 的调用语义。
- 不跨 source、filename、version line、parse options、compiler options 错误复用。
- 不复用用户传回并可能已修改的 descriptor，除非能证明该 descriptor 是不可变快照或带有明确 internal identity。
- cache key 必须包含 source hash、filename、version line、parse options、pad/source map 相关选项。
- cache value 不应暴露给 JS 用户直接 mutation；需要区分 public descriptor projection 与 Rust internal parse artifact。
- cache 生命周期应局限在单进程、单 compiler instance 或明确的 request scope，不能变成全局不可控状态。
- 必须有 output contract、conformance 和 project corpus 验证，覆盖 parse 后 compileScript、手写 descriptor、descriptor mutation、不同 options 连续调用、跨 Vue 2.7 / Vue 3 混用。

## 验收结论

阶段 14 通过，当前决策为“不做 descriptor reuse”。这不是放弃性能优化，而是基于 profile 数据把优化边界收在已经证明收益明显的 Rust compileScript 内部热点上，避免为了约 1 ms 级别的 parse 成本引入跨 public API 调用的高风险缓存。

下一步应回到真实项目 corpus 和 release profile：如果未来数据证明 parse 成为主要瓶颈，再按本文件的触发条件和安全约束重新设计。
