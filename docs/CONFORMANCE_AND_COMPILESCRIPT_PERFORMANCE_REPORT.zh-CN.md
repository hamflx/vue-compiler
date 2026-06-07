# Conformance 与 CompileScript 性能验收报告

本报告对应 `docs/CONFORMANCE_AND_COMPILESCRIPT_DEVELOPMENT_PLAN.zh-CN.md` 的阶段 13：确认前面几阶段的 compileScript 架构优化真实有效，并且没有改变 public 默认输出行为。

## 测试口径

本轮使用同一套 profiling gate 和同一套 fixture 对比优化前后：

- baseline commit：`4768d0a Add compileScript profiling gate`
- current commit：`98966da Use line index for script AST locations`
- build profile：debug
- OS：Windows x86_64
- rustc：`rustc 1.95.0 (59807616e 2026-04-14)`
- Node：`v24.4.1`
- official lock hash：`a9b1d2dee8c939951eabda27ef594a9d7189603985cff8d4e1338494ec317b77`
- iterations：60

运行命令：

```text
cargo xtask profile-compile-script --version-line vue2_7 --fixture-corpus compat/perf/vue27-sfc --iterations 60
cargo xtask profile-compile-script --version-line vue3 --fixture-corpus compat/perf/vue3-sfc --iterations 60
```

baseline 通过临时 worktree 运行：

```text
git worktree add --detach ..\vue-compiler-profile-baseline-4768d0a 4768d0a
```

当前 profile 产物：

- `target/perf/compile-script/vue2_7.json`
- `target/perf/compile-script/vue3.json`

baseline profile 产物：

- `..\vue-compiler-profile-baseline-4768d0a\target\perf\compile-script\vue2_7.json`
- `..\vue-compiler-profile-baseline-4768d0a\target\perf\compile-script\vue3.json`

这些 `target/` 产物不提交；本文件记录可复现命令、commit 和关键结果。

## Vue 2.7 结果

fixture：`compat/perf/vue27-sfc/base-layout-header.vue`

- source bytes：3926
- template bytes：2029
- script setup bytes：1666
- sha256：`d3c9da91c6b12be07a9d3adef3f63f432ed49f256daa4d064df772fdddbb744f`

| 指标 | baseline `4768d0a` | current `98966da` | 变化 |
| --- | ---: | ---: | ---: |
| parse median | 1239 us | 1155 us | -6.8% |
| parse p95 | 1919 us | 1627 us | -15.2% |
| compileScript median | 11170 us | 1454 us | -87.0% |
| compileScript p95 | 15987 us | 2184 us | -86.3% |
| serialize median | 1239 us | 117 us | -90.6% |
| serialize p95 | 2355 us | 202 us | -91.4% |
| total median | 13841 us | 2727 us | -80.3% |
| total p95 | 20353 us | 3914 us | -80.8% |
| output bytes | 32579 | 4000 | -87.7% |

结构化计数：

| 指标 | baseline `4768d0a` | current `98966da` |
| --- | ---: | ---: |
| AST projection enabled | true | false |
| AST projection mode | 未记录 | none |
| AST projection loc strategy | 未记录 | not-run |
| AST projection statement count | 22 | 0 |
| template usage scan count | 10 | 1 |
| setup analysis count | 4 | 1 |
| script compile error analysis count | 1 | 0 |

结论：Vue 2.7 的主要收益来自三个点同时成立：internal profiling 路径关闭 public AST projection、template usage 不再按 import 重复扫描、script setup analysis 合并为单次上下文复用。`compileScript` median 和 p95 均显著下降。

## Vue 3 结果

fixture：`compat/perf/vue3-sfc/base-layout-header.vue`

- source bytes：3956
- template bytes：2051
- script setup bytes：1674
- sha256：`0e5a86f9953478aa41bc6c82aa25e778ac53c07cbc8705d26c88cf8f98bb5ccb`

| 指标 | baseline `4768d0a` | current `98966da` | 变化 |
| --- | ---: | ---: | ---: |
| parse median | 1176 us | 1137 us | -3.3% |
| parse p95 | 1684 us | 1271 us | -24.5% |
| compileScript median | 41420 us | 21697 us | -47.6% |
| compileScript p95 | 49895 us | 23783 us | -52.3% |
| serialize median | 1643 us | 426 us | -74.1% |
| serialize p95 | 2938 us | 469 us | -84.0% |
| total median | 44506 us | 23234 us | -47.8% |
| total p95 | 52608 us | 25422 us | -51.7% |
| output bytes | 46537 | 16628 | -64.3% |

结构化计数：

| 指标 | baseline `4768d0a` | current `98966da` |
| --- | ---: | ---: |
| AST projection enabled | true | false |
| AST projection mode | 未记录 | none |
| AST projection loc strategy | 未记录 | not-run |
| AST projection statement count | 23 | 0 |
| template usage scan count | 18 | 1 |
| setup analysis count | 1 | 1 |
| script compile error analysis count | 2 | 1 |

结论：Vue 3 的收益主要来自 internal no-AST mode、template usage 预计算复用，以及 compile error/setup analysis 路径收敛。Vue 3 本身还有更重的 type resolve / macro 处理成本，所以绝对耗时仍高于 Vue 2.7，但 median 和 p95 都有明显下降。

## 阶段 13 验收

| 验收条件 | 当前证据 | 结论 |
| --- | --- | --- |
| Vue 2.7 template usage scan count 对普通 inline template 不超过 1 | current Vue 2.7 profile：`templateUsageScanCount = 1` | 通过 |
| Vue 2.7 setup analysis count 对单 mode compile 不超过 1 | current Vue 2.7 profile：`setupAnalysisCount = 1` | 通过 |
| Vue 3 template usage scan count 不重复按 import 增长 | current Vue 3 profile：`templateUsageScanCount = 1`，baseline 为 18 | 通过 |
| Vue 3 setup analysis / compile error analysis 不重复执行 | current Vue 3 profile：`setupAnalysisCount = 1`、`scriptCompileErrorAnalysisCount = 1` | 通过 |
| public default mode 无行为回退 | 阶段 12 已通过 Vue 2.7 / Vue 3 SFC output contracts；public 默认仍保留 full AST projection | 通过 |
| internal no-AST mode 有可测性能收益 | Vue 2.7 compileScript median -87.0%；Vue 3 compileScript median -47.6%；AST statement count 均为 0 | 通过 |
| 如果 median 没有改善，必须解释原因 | 两条版本线 median 均改善 | 不适用 |

## 解释与限制

- 本轮 profile 是 debug build，用于回归和相对趋势判断，不代表 release 绝对性能。
- baseline 的结构化字段在 `4768d0a` 还没有 `astProjectionMode` / `astProjectionLocStrategy`，因此表格中标为“未记录”。这不影响 AST projection enabled / statement count / scan count / analysis count 的对比。
- 当前 profiling 命令刻意使用 internal no-AST mode，目的是衡量 Rust compileScript 内部路径，不代表 public package 默认返回会删除 `scriptAst` / `scriptSetupAst`。
- public 默认行为仍由 output contract 和 focused AST tests 保护；性能优化不改变公开 API 结果形状。

## 结论

阶段 13 通过。前面几阶段的架构改动没有只是“让测试变绿”，而是移除了三个可测的结构性成本：

- public AST projection 不再无条件出现在 internal profiling 路径。
- template usage 从按 import 重复扫描变成每次 compile 复用一次索引。
- script setup / compile error analysis 被上下文化复用，避免同一块 setup 源码在单次 compile 中重复分析。

下一步进入阶段 14：基于本报告的 parse 占比决定是否做 descriptor reuse。
