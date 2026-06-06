# Conformance 与 CompileScript 阶段 0 基线

本文是 `docs/CONFORMANCE_AND_COMPILESCRIPT_DEVELOPMENT_PLAN.zh-CN.md` 阶段 0 的基线记录。它只冻结当前证据，不改变 Rust/JS 行为代码。

## 基线身份

- 记录时间：2026-06-06 21:41:31 +08:00。
- 基线提交：`673d016c1958504b6a1ef8a3db4c1a3106d34a80`。
- 基线提交说明：`673d016 Add conformance and compileScript development plan`。
- lock 文件：`compat/official-revisions.lock`。
- lock SHA256：`a9b1d2dee8c939951eabda27ef594a9d7189603985cff8d4e1338494ec317b77`。
- OS：Windows。
- Rust：`rustc 1.95.0 (59807616e 2026-04-14)`。
- Node：`v24.4.1`。

官方基线提交：

| 版本线 | 官方提交 |
| --- | --- |
| Vue 2.6 | `612fb89547711cacb030a3893a0065b785802860` |
| Vue 2.7 | `13f4e7dc03e2caed900ac70ff8b8fe58dda45663` |
| Vue 3 | `57545e958ae28ed17aa9e0ed321abcd8dc99f752` |

## summarize-compat 基线

执行命令：

```text
cargo xtask summarize-compat --locked
```

结果：

- status：`pass`
- total：`7`
- pass：`7`
- pending：`0`
- fail：`0`
- 报告 lock hash：`a9b1d2dee8c939951eabda27ef594a9d7189603985cff8d4e1338494ec317b77`
- 报告 Rust commit：`673d016c1958504b6a1ef8a3db4c1a3106d34a80`

summary targets：

| target | status | detail |
| --- | --- | --- |
| `vue2_6::vue-template-compiler/index` | pass | `api=pass, options=pass, output=pass, conformance=pass, lock=pass` |
| `vue2_7::vue-template-compiler/index` | pass | `api=pass, options=pass, output=pass, conformance=pass, lock=pass` |
| `vue2_7::vue/vue/compiler-sfc` | pass | `api=pass, options=pass, output=pass, conformance=pass, lock=pass` |
| `vue3::@vue/compiler-core/index` | pass | `api=pass, options=pass, output=pass, conformance=pass, lock=pass` |
| `vue3::@vue/compiler-dom/index` | pass | `api=pass, options=pass, output=pass, conformance=pass, lock=pass` |
| `vue3::@vue/compiler-ssr/index` | pass | `api=pass, options=pass, output=pass, conformance=pass, lock=pass` |
| `vue3::@vue/compiler-sfc/index` | pass | `api=pass, options=pass, output=pass, conformance=pass, lock=pass` |

## Conformance Coverage 基线

以下数据读取自 `target/conformance/a9b1d2dee8c939951eabda27ef594a9d7189603985cff8d4e1338494ec317b77/*.json`。

| suite | counts | coverage source | rust-backed | mixed | shim-backed |
| --- | ---: | --- | ---: | ---: | ---: |
| `vue2-compiler` | `188/188` | `rust-backed` | `188/188` | `0/0` | `0/0` |
| `vue27-compiler` | `190/190` | `rust-backed` | `190/190` | `0/0` | `0/0` |
| `vue27-sfc` | `144/144` | `mixed` | `134/134` | `10/10` | `0/0` |
| `vue3-core` | `599/599` | `mixed` | `587/587` | `12/12` | `0/0` |
| `vue3-dom` | `133/133` | `rust-backed` | `133/133` | `0/0` | `0/0` |
| `vue3-ssr` | `129/129` | `rust-backed` | `129/129` | `0/0` | `0/0` |
| `vue3-sfc` | `460 pass / 461 total / 1 skip` | `rust-backed` | `460/461` | `0/0` | `0/0` |

当前需要后续阶段重点治理的基线现象：

- `vue3-ssr` 当前 report 聚合为 `rust-backed 129/129`。这正是后续阶段要用 provenance schema 和 runtime markers 重新校验的对象，因为历史报告和文档中曾出现过 `mixed 129/129` 与 `rust-backed 129/129` 的口径漂移。
- `vue3-sfc` 当前 report 为 `rust-backed 460/461`，其中 `resolveType.spec.ts` 有 `1 skip`。后续阶段不能把 skip 误读成 pass。
- 当前报告仍主要依赖已有 coverage source 分类；阶段 1-3 要把 prepared manifest、provenance schema、runtime markers 加进去后再重新生成基线。

## Vue 2 Project Corpus 基线

执行命令：

```text
cargo xtask verify-vue2-project-corpus
```

结果：

- status：`fail`
- total：`15`
- pass：`14`
- pending：`0`
- fail：`1`
- 报告：`target/external/vue2-project-corpus/verify_vue2_project_corpus.json`
- 报告 Rust commit：`673d016c1958504b6a1ef8a3db4c1a3106d34a80`

失败项：

| project | status | reason |
| --- | --- | --- |
| `view-design-ViewUI` | fail | `git fetch --tags --force origin` 访问 `https://github.com/view-design/ViewUI.git/` 时返回 `Empty reply from server` |

已通过项目摘要：

| project | result |
| --- | --- |
| `ElemeFE-element` | `421 template modes passed across 135 template files` |
| `PanJiaChen-vue-element-admin` | `387 template modes passed across 128 template files` |
| `PanJiaChen-vue-admin-template` | `73 template modes passed across 24 template files` |
| `iview-iview` | `521 template modes passed across 173 template files` |
| `bootstrap-vue-bootstrap-vue` | `66 template modes passed across 22 template files` |
| `buefy-buefy-v0` | `1133 template modes passed across 375 template files` |
| `vuejs-vue-cli-ui` | `218 template modes passed across 71 template files` |
| `sendya-ant-design-pro-vue` | `277 template modes passed across 91 template files` |
| `iczer-vue-antd-admin` | `249 template modes passed across 83 template files` |
| `d2-projects-d2-admin-v1` | `351 template modes passed across 117 template files` |
| `coreui-free-vue-admin-template-v2` | `151 template modes passed across 46 template files` |
| `statping-statping` | `183 template modes passed across 61 template files` |
| `xaksis-vue-good-table` | `90 template modes passed across 30 template files` |
| `Armour-vue-typescript-admin-template` | `378 template modes passed across 125 template files` |

解释：

- 本次 corpus 失败是外部 GitHub fetch 失败，没有生成该项目的 compiler output comparison。
- 已完成 checkout 的 14 个项目没有出现 Rust/official output mismatch。
- 后续阶段若再次运行 corpus，应优先确认 `view-design-ViewUI` 是否能成功 checkout；如果成功，应回到 15/15 作为生产 corpus 基线。

## CompileScript Profiling 基线

检查命令：

```text
rg -n "profile-compile-script|compile-script.*profile|ProfileCompile|profile_compile" xtask/src crates packages docs
```

结果：

- 只在方案文档中发现 `profile-compile-script` 字样。
- 当前代码中没有实现 `cargo xtask profile-compile-script`。
- 当前没有可复现的 checked-in compileScript profiling gate。

阶段 7 前不能把外部性能报告里的具体微秒数作为验收 gate。可以把已有评审数据当作线索，但必须先实现 profiling 命令并生成本仓库可复现报告。

## 未运行项与原因

- 本阶段没有逐个重新运行所有 `run-conformance --suite ...`，因为阶段 0 的目标是冻结当前 `summarize-compat --locked` 和现有 conformance artifacts 的基线。后续改变 report schema 或 runtime provenance 后，必须按开发计划重新运行相关 suite。
- 本阶段没有运行 Vue 2.7 / Vue 3 SFC compileScript profiling，因为 profiling 命令尚未实现。
- Vue 2 project corpus 已实际运行，但 `view-design-ViewUI` 受外部 GitHub fetch 失败影响，未完成该项目比较。

## 阶段 0 验收状态

| 验收项 | 状态 | 证据 |
| --- | --- | --- |
| 文档记录 baseline commit 和执行日期 | 完成 | 本文“基线身份” |
| 未运行项写明原因 | 完成 | 本文“未运行项与原因” |
| 后续阶段能与 baseline 对照 | 完成 | summarize、coverage、corpus、profiling 四类基线均已记录 |
| 不改变 Rust/JS 行为代码 | 完成 | 本阶段仅新增本文档 |

