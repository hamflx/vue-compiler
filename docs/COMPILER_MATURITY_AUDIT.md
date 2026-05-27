# 编译器产品化与架构成熟度审计

审计日期：2026-05-27，Asia/Shanghai。

本文从编译器产品化和编译器架构两个角度审视当前仓库。结论基于当前工作区源码、已有设计文档、兼容性清单、本地测试和 conformance 产物。本文只记录审计结论，不修改实现代码。

## 总体结论

当前项目已经不是早期玩具项目。它具备 Rust workspace、方言 crate、AST/HIR/MIR 设计文档、Vue 2 模板编译器较强兼容性、Vue 3 core 的 Rust-backed 官方测试覆盖、API/alias/output contract 工具链，以及可运行的官方测试 harness。

但它还不能被判断为产品成熟的 Vue 编译器，也不能被判断为架构已经成熟的完整 Rust Vue 编译器。更准确的定位是：高级兼容性原型加局部成熟编译器实现。Vue 2 模板编译器和 Vue 2.7 SFC 的 generated-alias 官方兼容门禁已关闭；Vue 3 DOM、SSR、SFC 的官方通过主要来自 mixed harness，不能等同于纯 Rust 编译器闭环。

距离产品成熟主要差在：纯 Rust conformance 覆盖闭环、SFC/script/style 生态完整性、最终 NAPI/WASM/CLI 包装、稳定诊断与 sourcemap、性能与增量体系、CI/发布流程、模糊测试和安全鲁棒性。

距离合理成熟的编译器架构主要差在：巨型文件和职责集中、`xtask` shim 语义泄漏风险、AST/HIR/MIR 主路径约束仍未彻底制度化、pass pipeline 还只是轻量骨架、JS 语义分析与字符串重写混用、source span/diagnostic/source map 契约还没有贯穿所有阶段、产品 API 边界和测试 harness 边界没有充分分离。

## 审计范围

本次扫描了仓库中非生成、非第三方目录下的所有 80 个文件。排除目录为 `target/`、`node_modules/`、`vendor/`、`.git/`。扫描命令：

```powershell
rg --files --hidden -g '!target/**' -g '!node_modules/**' -g '!vendor/**' -g '!.git/**'
```

被纳入审计的文件类别包括：

| 类别 | 文件 |
| --- | --- |
| 根配置 | `.cargo/config.toml`、`.gitignore`、`.mise.toml`、`Cargo.lock`、`Cargo.toml`、`package.json`、`pnpm-workspace.yaml` |
| 设计与过程文档 | `docs/0.RESEARCH.md`、`docs/1.RUST_VUE_COMPILER_DESIGN.md`、`docs/2.DEVELOPMENT_PLAN.md`、`docs/3.AST_HIR_MIR_DESIGN.md`、`docs/COMPATIBILITY_CONCERNS.md`、`docs/goal.md`、`docs/MEMORY.md`、`docs/PENDING_DECISIONS.md`、`docs/UNRESOLVED_PROBLEMS.md`、`docs/WORK_PART.md` |
| 兼容性基线 | `compat/official-revisions.lock`、`compat/api/**`、`compat/options/**`、`compat/output/**` |
| Rust crate 清单 | `crates/*/Cargo.toml`、`xtask/Cargo.toml` |
| Rust 实现 | `crates/*/src/lib.rs`、`crates/vuec_node_bridge/src/main.rs`、`xtask/src/main.rs`、`xtask/src/compat.rs` |

主要 Rust 源文件规模如下，规模本身不是错误，但它暴露了职责集中和维护风险：

| 文件 | 行数 | 审计含义 |
| --- | ---: | --- |
| `crates/vuec_vue3_core/src/lib.rs` | 31181 | Vue 3 parser、transform、lowering、DOM/SSR codegen、projection、测试高度集中 |
| `xtask/src/compat.rs` | 12419 | API manifest、alias runtime、官方测试准备、option/output/conformance runner、JS shim 高度集中 |
| `crates/vuec_sfc/src/lib.rs` | 7139 | SFC parse、Vue 2.7 parseComponent、compileScript、compileTemplate、compileStyle 包装集中 |
| `crates/vuec_node_bridge/src/main.rs` | 4545 | Node JSON bridge 承载大量产品 API 投影与测试适配 |
| `crates/vuec_vue2/src/lib.rs` | 3894 | Vue 2 模板编译主体集中 |
| `crates/vuec_ast/src/lib.rs` | 1572 | AST/HIR/MIR 基础类型集中 |
| `crates/vuec_vue3_dom/src/lib.rs` | 1445 | Vue 3 DOM facade 与测试 |
| `crates/vuec_style/src/lib.rs` | 1382 | style/scoped/css vars 处理集中 |
| `crates/vuec_js/src/lib.rs` | 643 | JS AST store 与 Oxc 解析包装 |
| `crates/vuec_html/src/lib.rs` | 616 | HTML tokenizer/parser 基础 |
| `crates/vuec_vue3_asset/src/lib.rs` | 597 | asset URL 处理 |
| `crates/vuec_codegen/src/lib.rs` | 264 | writer/source map builder 基础 |
| `crates/vuec_vue3_ssr/src/lib.rs` | 250 | Vue 3 SSR facade |
| `crates/vuec_source/src/lib.rs` | 196 | source map/source frame 基础 |
| `crates/vuec_pass/src/lib.rs` | 148 | pass scheduler 骨架 |
| `crates/vuec_diagnostics/src/lib.rs` | 121 | diagnostic 基础 |
| `xtask/src/main.rs` | 121 | xtask CLI 入口 |

## 本地验证结果

以下命令已在本地执行。通过项只能说明当前静态构建、代表性 public contract 和官方 generated-alias conformance 健康；mixed 覆盖项不能替代纯 Rust conformance。

| 命令 | 结果 | 解释 |
| --- | --- | --- |
| `cargo fmt --all --check` | 通过 | 格式化一致 |
| `cargo test --workspace` | 通过 | Rust 单元测试和 doc tests 通过 |
| `cargo xtask verify-official-lock` | 通过 | 官方版本锁有效 |
| `cargo xtask diff-api --all` | 通过 | 7 个 API manifest 与官方基线字段匹配，`compat/api/allowed-diff.json` 当前为空 |
| `cargo xtask verify-npm-alias --all` | 通过 | 7 个 alias smoke 通过 |
| `cargo xtask run-output-contract --all` | 通过 | 7 个 target 的代表性输出契约通过，每个 target 5 项检查 |
| `cargo xtask run-option-matrix --all` | 通过 | 7/7 target 通过，所有当前 option rows 无 fail/pending |
| `cargo xtask run-conformance --all` | 通过 | 7 个官方 suite generated-alias 运行通过 |
| `cargo xtask summarize-compat --locked` | 通过 | 7/7 target 聚合为 pass |

上一版审计记录的 SFC option matrix 和 Vue 2.7 SFC 官方失败已经关闭。关闭方式包括 Rust style scanner 保留 selector 到 `{` 的原始空白、Vue 2.7 script setup test-build marker/empty-return parity、以及 generated alias API shape 修复。当前仍不能把这些结果扩展解释为“所有 Vue 3 编译器语义已纯 Rust 化”。

当前 conformance 产物位于：

```text
target/conformance/ee33465b421a58b83fac04aa850a6d250ee09ec169fa80ed8820794a0c9a2769/
```

当前官方测试覆盖摘要：

| Suite | 总体通过 | 总体失败 | 总计 | Rust-backed | Mixed | 结论 |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| `vue2-compiler` | 188 | 0 | 188 | 188/188 | 0/0 | Vue 2.6 模板编译器兼容度强 |
| `vue27-compiler` | 190 | 0 | 190 | 190/190 | 0/0 | Vue 2.7 模板编译器兼容度强 |
| `vue27-sfc` | 144 | 0 | 144 | 134/134 | 10/10 | 官方 suite 关闭；`compileStyle.spec.ts` 因 PostCSS callback 边界为 mixed |
| `vue3-core` | 652 | 0 | 652 | 199/199 | 453/453 | core 有 Rust-backed 子集，但大部分仍是 mixed |
| `vue3-dom` | 133 | 0 | 133 | 34/34 | 99/99 | 不能作为纯 Rust DOM compiler parity 证明 |
| `vue3-sfc` | 461 | 0 | 461 | 0/0 | 461/461 | 不能作为纯 Rust SFC parity 证明 |
| `vue3-ssr` | 129 | 0 | 129 | 0/0 | 129/129 | 不能作为纯 Rust SSR compiler parity 证明 |

## 成熟度分级

| 模块 | 当前成熟度 | 主要证据 | 产品化结论 |
| --- | --- | --- | --- |
| Vue 2 模板编译 | 高，接近可产品化 | Vue 2.6/2.7 rust-backed 官方测试全部通过；option/output/API 检查通过 | 仍需补完整发布包、性能、fuzz、诊断契约后才能产品化 |
| Vue 2.7 SFC | 中到高 | generated-alias 官方测试 144/144；coverage 为 rust-backed 134/134、mixed 10/10 | public conformance 已关闭；PostCSS callback 边界仍不是纯 Rust |
| Vue 3 compiler-core | 中 | 199/199 rust-backed 通过，另有 453 mixed 通过；源码已有 AST/HIR/MIR lowering 入口 | core 子集进展明显，但完整 transform/codegen parity 仍需纯 Rust 覆盖 |
| Vue 3 compiler-dom | 低到中 | 官方 suite 133/133；coverage 为 rust-backed 34/34、mixed 99/99 | 不能宣称纯 Rust DOM compiler 成熟 |
| Vue 3 compiler-ssr | 低到中 | 官方 suite 129/129 全为 mixed，rust-backed 0/0 | 不能宣称纯 Rust SSR compiler 成熟 |
| Vue 3 compiler-sfc | 低 | 官方 suite 461/461 全为 mixed，rust-backed 0/0 | 不能宣称纯 Rust SFC compiler 成熟 |
| Style compiler | 中 | scoped/css vars/preprocessor 当前 option 与 Vue 2.7 SFC 官方 suite 通过；PostCSS callbacks 仍在 JS API adapter | 产品风险集中在 CSS 生态完整性和 mixed API 边界 |
| Source/diagnostics/sourcemap | 基础可用 | 有 `vuec_source`、`vuec_diagnostics`、`SourceMapBuilder`，output contract 代表性通过 | 距离完整 source map 和诊断 parity 还有明显距离 |
| Pass pipeline | 早期骨架 | `vuec_pass` 约 148 行，具备 scheduler/context 基础 | 还不是成熟 compiler pipeline |
| Node bridge/API 包装 | 测试桥接可用 | `vuec_node_bridge` 是 JSON CLI bridge，alias runtime 通过 | 还不是最终 NAPI/WASM/CLI 产品边界 |
| Compat tooling | 强但过载 | 官方锁、API diff、alias、output、option、conformance 都集中在 `xtask/src/compat.rs` | 工具价值高，但职责过重，且 shim 语义容易侵蚀编译器边界 |

## 已具备的优势

1. Workspace 分层已经有雏形，crate 名称覆盖 AST、HTML、JS、pass、codegen、source、diagnostics、style、Vue2、Vue3 core/dom/ssr、SFC、Node bridge。
2. `docs/3.AST_HIR_MIR_DESIGN.md` 明确了 AST/HIR/MIR、public projection、lowering、target-split、source span、JS AST store 的设计约束。
3. Vue 2.6 和 Vue 2.7 模板编译器 rust-backed 官方 conformance 当前全部通过，这是项目最强的可见资产。
4. API manifest、npm alias smoke、output contract 已经形成自动化基线，且当前均通过。
5. Vue 3 core 已经存在结构化 AST 到 HIR/MIR lowering 入口，例如 `lower_vue3_ast_to_dom_mir` 和 `lower_vue3_ast_to_ssr_mir` 会记录 AST 到 HIR、HIR 到 MIR 的映射，并登记 JS AST store。
6. 现有文档已经明确禁止把 `xtask/src/compat.rs` 中的 JS shim 语义当作 Rust 编译器完成度，这是正确的工程边界。

## 产品成熟度缺口

### 1. 纯 Rust conformance 没有闭环

最重要的问题不是测试数量，而是测试来源。`xtask/src/compat.rs` 明确把 `vue3-core`、`vue3-dom`、`vue3-sfc`、`vue3-ssr` 标为 `mixed`。其说明中也写明 Vue 3 DOM/SFC/SSR official tests 会执行官方 TypeScript source、generated alias、compat adapter 和 Rust bridge 的混合路径。

因此：

1. Vue 3 DOM 133/133、SFC 461/461、SSR 129/129 的通过，不能整体计入纯 Rust 编译器完成度。
2. Vue 3 core 的 652/652 总通过中，只有 199/199 是 rust-backed，453/453 是 mixed。
3. Vue 3 DOM 的 133/133 总通过中，只有 34/34 是 rust-backed，99/99 是 mixed。
4. 产品成熟的验收必须把 mixed 视为 harness 健康度，而不是编译器 parity。

### 2. Vue 2.7 SFC public conformance 已关闭，但仍有 mixed API 边界

`vue27-sfc` 当前 generated-alias 官方 suite 为 144/144。覆盖分类为 `rust-backed 134/134`、`mixed 10/10`、`shim-backed 0/0`。mixed 部分集中在 `compileStyle.spec.ts`，原因是调用者提供的 PostCSS plugin callbacks/options 和 LazyResult/Promise 行为无法通过 JSON bridge 直接进入 Rust，需要由 generated JavaScript API adapter 执行。

因此 Vue 2.7 SFC 的 public conformance 已经关闭，但不能把 PostCSS callback API 边界描述为纯 Rust style compiler 完成度。

### 3. SFC 能力还未产品化

SFC 产品级能力至少应覆盖：

1. `parse` 的 descriptor、block loc、padding、source map、errors/warnings parity。
2. `compileScript` 的 `<script setup>`、宏、泛型、type resolve、import usage、defineProps/defineEmits/defineExpose/defineModel/withDefaults、binding metadata、reactivity transform 兼容边界。
3. `compileTemplate` 与 SFC descriptor、scoped、slotted、SSR、asset URL、binding metadata 的组合。
4. `compileStyle`、`compileStyleAsync`、scoped、CSS vars、modules、preprocessors、PostCSS plugin、source map、dependencies、errors。
5. Vue 2.7 与 Vue 3 的行为差异隔离。

当前 `vuec_sfc` 具备大量实现，Vue 2.7 public conformance 已关闭；但文件职责仍高度集中，Vue 3 SFC conformance 仍为 mixed，产品级还需要把 SFC script/template/style 的核心路径继续迁移为 Rust-backed 覆盖并拆分模块边界。

### 4. Style compiler 生态不完整

`vuec_style` 当前更接近 scanner/string rewrite 型实现。它可以支撑 scoped selector、CSS vars 等核心路径，但产品级 style compiler 需要处理真实 CSS parser、PostCSS AST、插件链、preprocessor 异步依赖、source map、warning/error 形态、CSS modules 和复杂 selector 语义。

上一轮 option matrix 暴露的 selector 空格差异已经通过保留原始 brace 前空白修复。剩余风险不在该 fixture，而在真实 CSS parser、PostCSS AST、插件链、异步依赖和复杂 source map 生态闭环。

### 5. 诊断和 source map 尚未达到稳定契约

当前有 `vuec_source`、`vuec_diagnostics`、`vuec_codegen::SourceMapBuilder`，并且 output contract 的 representative source-map parity 通过。但产品级编译器需要的是全路径稳定契约：

1. parse、transform、lowering、codegen、SFC script/style 的所有错误都要有稳定 code、severity、message、span、frame。
2. SFC block offset 必须映射回原始 `.vue` 文件，而不是局部 template/style/script substring。
3. generated span、missing span、source span 的策略要被测试覆盖并在 public API 中一致投影。
4. source map 需要覆盖 template、script transform、style transform、SSR、hoist、cache、helper import、asset URL rewrite 等输出。

目前这些能力有基础，但没有完整证据证明已经全路径成熟。

### 6. JS 语义分析仍未系统化

`vuec_js::JsAstStore` 是正确方向，它使用 Oxc 登记表达式、语句、pattern、program 并按需解析。但当前大文件中仍存在大量字符串级表达式处理和输出重写模式。产品级 Vue compiler 对模板表达式、event statement、`v-for` alias、slot params、TS/JSX 边界、scope analysis、identifier prefix、constant analysis 的要求很高。

如果 JS 语义分析没有统一到 AST/store/scope 模型，后续会在以下场景持续出现特殊 case：

1. prefixIdentifiers 和 binding metadata 组合。
2. event handler statement 与 expression 的判定。
3. 可缓存 handler、constant expression、静态提升。
4. `<script setup>` binding 与 template expression 的联动。
5. TypeScript syntax、optional chaining、destructuring、import type/export type 等边界。

### 7. 产品发布形态缺失

当前 `vuec_node_bridge` 是 JSON CLI bridge，主要服务 alias runtime 和官方测试。它不是最终面向用户的 NAPI 包、WASM 包、Node ESM/CJS 包或 CLI 产品。

产品成熟还缺：

1. `@vue/compiler-*` 兼容包的正式构建与发布流程。
2. NAPI 或 WASM 的稳定 ABI/API 设计。
3. Node ESM/CJS 双入口、types、exports、平台包策略。
4. CLI 行为、错误码、stdin/stdout、watch/incremental 体验。
5. 与 Vite、Rollup、Webpack、Vue loader、language tools 的集成验证。

### 8. 性能、增量和内存模型没有产品证明

编译器产品化不能只看正确性。当前没有看到完整 benchmark、内存 profile、incremental cache、large SFC stress、并发编译、arena 生命周期策略、避免重复 JS parse 的性能验收基线。

风险点包括：

1. 巨型单文件实现导致局部优化和 profile 困难。
2. JSON bridge 可能掩盖 Rust 核心性能，也可能成为后续产品瓶颈。
3. SFC compileScript/template/style 之间的数据复用还没有明确的 incremental contract。
4. AST/HIR/MIR 和 public projection 的重复结构可能带来内存膨胀。

### 9. 鲁棒性、安全和 fuzz 体系缺失

模板、HTML、JS、CSS、SFC 都是高风险输入面。产品级编译器需要：

1. HTML/template fuzz。
2. JS expression fuzz。
3. SFC block parser fuzz。
4. CSS/scoped selector fuzz。
5. Panic-free public API contract。
6. 超大输入、深层嵌套、恶意 unicode、异常换行、无效 UTF-8 边界策略。

当前源码中主路径存在少量 `unwrap`/`expect`/`panic` 风格代码，例如 `vuec_html`、`vuec_js`、`vuec_vue3_core`、`vuec_node_bridge` 中有非测试路径的断言式处理。部分上下文可能逻辑上安全，但产品 API 仍需要明确 panic-free 策略和 fuzz 证明。

### 10. CI、发布和兼容矩阵还不完整

`package.json` 只有基础脚本，当前没有看到完整 CI 配置、跨平台发布矩阵、nightly conformance、版本升级流程、官方锁更新流程、性能回归阈值、最小支持 Rust/Node 版本策略。

产品成熟至少需要：

1. Windows/macOS/Linux CI。
2. Rust stable/MSRV 策略。
3. Node LTS 矩阵。
4. locked official revisions 的自动验证和人工升级流程。
5. 发布前 gate：fmt、clippy、unit、api diff、alias、option matrix、output contract、rust-backed conformance、bench smoke。
6. 产物签名、changelog、semver 兼容策略。

## 架构成熟度缺口

### 1. 巨型文件破坏编译器阶段边界

`crates/vuec_vue3_core/src/lib.rs` 超过 31000 行，是当前最大的架构风险。它混合了 parser、transform、projection、HIR/MIR lowering、DOM/SSR codegen、source map、helper ordering、表达式处理和大量测试。对于成熟编译器，这种结构会带来以下问题：

1. 阶段边界不清，AST transform、lowering、codegen 的不变量难以单独验证。
2. 代码审查粒度过大，局部改动容易影响远端行为。
3. 测试夹在实现同一文件中，难以形成 fixture 化 conformance。
4. 新贡献者难以定位语义归属。
5. 很容易用输出字符串修补绕过 AST/HIR/MIR 主路径。

建议拆分为至少以下子模块：

| 建议模块 | 职责 |
| --- | --- |
| `parse/` | Vue 3 template tokenizer/parser 和错误恢复 |
| `ast/` | Vue 3 internal AST 和 public projection |
| `transform/` | compiler-core transform pass |
| `lowering/dom.rs` | AST 到 HIR 到 DOM MIR |
| `lowering/ssr.rs` | AST 到 HIR 到 SSR MIR |
| `codegen/dom.rs` | DOM render codegen |
| `codegen/ssr.rs` | SSR render codegen |
| `diagnostics.rs` | Vue 3 compiler diagnostics |
| `sourcemap.rs` | Vue 3 source map |
| `tests/fixtures` | fixture 驱动测试 |

### 2. `xtask/src/compat.rs` 同时承担工具、runner 和语义 shim

`xtask/src/compat.rs` 超过 12000 行，包含官方版本锁、npm install、API manifest、alias package、JS runtime、output contract、option probes、conformance suite 准备、Vitest/Jasmine runner、coverage 分类和大量测试。

工具集中本身可以接受，但现在的问题是它还包含 JS alias/shim 行为。文档已经明确这些 shim 只能作为测试 import、包入口、AST hydration/dehydration 和临时适配层，不能作为 Rust 编译器完成度。这个原则必须继续强化到代码结构上。

建议拆分：

| 建议模块 | 职责 |
| --- | --- |
| `compat/lock.rs` | 官方版本锁读取与校验 |
| `compat/api.rs` | API manifest export/diff |
| `compat/alias.rs` | alias package 生成 |
| `compat/output_contract.rs` | 输出契约 probe |
| `compat/options.rs` | option matrix |
| `compat/conformance.rs` | suite orchestration |
| `compat/runners/vue2.rs` | Vue 2 runner 准备 |
| `compat/runners/vue3.rs` | Vue 3 runner 准备 |
| `compat/js_runtime/*.js` | JS runtime 作为显式资源文件，而不是 Rust raw string 巨块 |

并且所有 shim 函数必须标注用途：import adapter、public API bridge、hydration/dehydration、temporary semantic shim。temporary semantic shim 必须有迁移 issue 或文档条目。

### 3. AST/HIR/MIR 设计还未成为硬性主路径

`docs/3.AST_HIR_MIR_DESIGN.md` 的方向是正确的：AST 表示方言 parse/transform 视图，HIR 表示跨方言语义，MIR 表示目标运行时代码生成，DOM/SSR/Vapor MIR 分裂，JS 表达式进入 `JsAstStore`。

当前 Vue 3 core 已有 `lower_vue3_ast_to_dom_mir` 和 `lower_vue3_ast_to_ssr_mir`，这说明结构不是空文档。但从产品成熟角度，仍缺硬性保证：

1. DOM codegen 是否必须从 DOM MIR 读取，还是仍可直接从 AST/codegen projection 读取。
2. SSR codegen 是否必须从 SSR MIR 读取，还是仍可复用 DOM 或 AST 字符串逻辑。
3. public projection 是否只是 API 输出层，不能反向驱动 compiler semantics。
4. HIR 中是否禁止 helper、patch flag、SSR push 等目标细节。
5. 是否有测试防止新代码绕过 lowering。

成熟架构需要把这些变成代码层约束，而不是只靠文档约束。

### 4. Pass pipeline 仍是骨架

`vuec_pass` 目前很小，说明 pass system 仍处于基础阶段。成熟编译器 pipeline 需要：

1. pass dependency graph。
2. pass 输入输出类型约束。
3. invalidation 和 incremental reuse。
4. diagnostic emission contract。
5. phase ordering audit。
6. feature flag 和方言差异隔离。
7. transform trace/debug dump。

否则复杂的 Vue transform 会继续堆在方言大文件中，以函数调用顺序而不是 pipeline contract 维持正确性。

### 5. 方言边界和目标边界还不够硬

Vue 2、Vue 2.7、Vue 3 DOM、Vue 3 SSR、Vue 3 SFC 之间有大量相似概念，但语义差异非常关键。成熟架构应避免共享过度，也应避免重复无约束。

当前风险：

1. `vuec_sfc` 同时依赖 `vuec_vue3_dom`、`vuec_vue3_ssr`、`vuec_vue3_core`、`vuec_style`、`vuec_js` 等，容易成为集成大球。
2. `vuec_node_bridge` 依赖几乎所有 crate，容易把测试 API shape 和产品 API shape 混在一起。
3. Vue 3 DOM/SSR facade crate 较薄，真实复杂度仍在 core 单文件。
4. Vue 2.7 SFC 和 Vue 3 SFC 的差异如果没有独立 contract，会不断产生特殊 case。

### 6. Public API 投影与内部结构耦合风险

为了兼容官方包，public AST/API shape 必须精确。但是成熟架构应明确：

1. internal AST/HIR/MIR 是编译器语义源。
2. public projection 是输出适配层。
3. bridge serialization 是产品边界或测试边界，不是内部模型。
4. official AST hydration/dehydration 只用于兼容测试或 public API，不应参与主编译逻辑。

当前 `xtask` alias runtime、`vuec_node_bridge` JSON 投影、core public projection 共同存在，边界需要继续硬化。

### 7. 测试形态仍偏局部断言和巨型单元测试

`cargo test --workspace` 通过是好信号，但成熟编译器测试结构应更分层：

1. parser fixture。
2. transform fixture。
3. lowering fixture。
4. codegen snapshot。
5. diagnostic snapshot。
6. source map snapshot。
7. SFC integration fixture。
8. official conformance。
9. fuzz/property/stress。
10. performance regression。

当前大量测试仍在大源码文件内部，且许多断言是字符串包含或局部顺序检查。它们适合快速迭代，但不能替代 fixture 化、快照化和官方 pure Rust conformance。

## 模块级问题清单

### `vuec_ast`

1. AST/HIR/MIR 基础容器已有，但设计文档指出现有 `Vue2NodeKind`、`Vue3NodeKind`、`HirNodeKind`、`MirNodeKind` 曾存在占位或职责不一致问题，需要持续确认实现已完全收敛到文档。
2. Public projection 与 internal arena 的边界需要保持单向，不应让 public AST shape 回流驱动内部语义。
3. `AstDocument` root 使用 `expect` 校验存在，作为内部不变量可以接受，但 public API 不应暴露 panic。
4. 缺少跨 crate 的 schema version 和 snapshot 兼容策略。

### `vuec_source`

1. Source file、span、source frame 基础存在，但产品级还需要多文件 source map、SFC block offset、generated/missing span 策略贯穿。
2. Source frame 渲染需要与官方 codeframe 行列、上下文、unicode 宽度、换行风格对齐。
3. 需要明确 byte offset、UTF-16 column、UTF-8 column 在 JS/Vue API 中的转换策略。

### `vuec_diagnostics`

1. 当前 diagnostic 基础很薄，尚不足以承载完整 Vue compiler error/warn code、severity、loc、frame、dedupe、排序。
2. 需要跨 parser、transform、style、script、bridge 的统一 diagnostic contract。
3. 需要官方 warning/error 顺序和 message parity 的 fixture。

### `vuec_html`

1. HTML tokenizer/parser 是 template CST 输入基础，但产品级 Vue template parser 要覆盖 HTML、SVG、MathML、raw text、RCDATA、CDATA、entity、错误恢复、命名空间、特殊 tag。
2. 搜索显示有少量非测试 `panic`/`unwrap` 风格路径，需要确认 public parse 对恶意输入 panic-free。
3. attr span、quote、实体解码、原始 source slice 与 AST projection 的契约需要全量测试。

### `vuec_js`

1. 使用 Oxc 的 `JsAstStore` 是正确方向，但当前项目整体仍存在字符串表达式重写痕迹。
2. 需要统一处理 expression、statement、pattern、program 的 scope analysis、identifier rewrite、constant analysis。
3. 需要保证 Oxc parse error 能稳定映射成 Vue compiler diagnostic，而不是局部失败或静默 fallback。
4. 需要缓存和生命周期策略，避免同一表达式在 SFC/template/codegen 中重复 parse。

### `vuec_pass`

1. 当前是 pass scheduler/context 的基础，不是成熟 pipeline。
2. 缺 pass dependency、phase ordering、invalidations、trace、typed input/output。
3. 缺方言 pass 与 target pass 的分层，如 parse pass、core transform pass、DOM transform pass、SSR lowering pass。
4. 缺 pass 级 diagnostics 和 source map mutation contract。

### `vuec_codegen`

1. `CodeWriter` 和 `SourceMapBuilder` 是基础设施，但 source map parity 只在代表性 output contract 中通过。
2. 产品级 codegen 需要 helper import、hoist、cache、SSR push、static stringify、newline/indent/minify/source map 的全量契约。
3. 需要避免 codegen 直接读 public projection 或原始 AST 绕过 MIR。

### `vuec_vue2`

1. Vue 2.6/2.7 模板编译 rust-backed 官方 conformance 全通过，是当前最成熟部分。
2. 仍需补产品级发布包、diagnostic/source map、performance、fuzz。
3. 单文件 3894 行仍偏集中，parser、optimizer、codegen、warning、public AST projection 可进一步拆分。
4. 需要确保 Vue 2 特有 filter、static optimizer、platform module、directives、whitespace、comments、delimiters 等均有 fixture 和官方覆盖标记。

### `vuec_vue3_core`

1. 31000 行单文件是最大架构债务。
2. 已有 Vue 3 AST/HIR/MIR lowering 入口，是重要进展，但 codegen 主路径是否完全受 MIR 约束仍需硬化。
3. compiler-core 内部 transform 行为仍有 mixed harness 依赖风险，尤其 processExpression、transformExpression、transformElement、processIf、processFor、transformText、buildProps、generate 等。
4. 字符串级 helper ordering、表达式 rewrite、projection patch 容易累积特殊 case。
5. source map、diagnostics、JS scope、public AST projection、DOM/SSR target split 需要拆分后分别建立不变量测试。

### `vuec_vue3_dom`

1. facade crate 较薄，官方 DOM suite 当前 133/133，其中 rust-backed 34/34、mixed 99/99。
2. 不能把当前 DOM official 通过视为纯 Rust DOM compiler 成熟。
3. DOM transform、directive transform、runtime helper、patch flag、static stringify、asset URL、namespace 需要 Rust-backed suite。
4. 需要确认 DOM codegen 不通过 compiler-core mixed adapter 获得隐性语义。

### `vuec_vue3_ssr`

1. facade crate 很薄，官方 SSR suite 当前 129/129 全是 mixed，rust-backed 0/0。
2. SSR 不是 DOM codegen 的简单变体，需要独立 MIR 和 push/stringify/hydration mismatch/teleport/suspense/component slot contract。
3. 需要 Rust-backed SSR official suite，而不是官方 TS source 加 alias runtime 的 mixed 通过。

### `vuec_vue3_asset`

1. asset URL 处理已有独立 crate，是合理边界。
2. 仍需覆盖 SFC template、style `url()`、base、includeAbsolute、tags/attrs 配置、SSR、Vite/Webpack 行为差异。
3. 搜索显示测试中存在 panic 式断言，不是问题本身，但生产路径需要 panic-free。

### `vuec_style`

1. 当前 style 实现覆盖 scoped/css vars/preprocessor 的代表性 public contract，上一版 option matrix 暴露的 selector brace 空白差异已经关闭。
2. 缺成熟 CSS parser/PostCSS AST/plugin/preprocessor 体系。
3. 需要支持 async dependency、CSS modules、source map、errors/warnings、trim/format parity。
4. 需要明确 Vue 2.7 和 Vue 3 style API 差异。

### `vuec_sfc`

1. 7139 行单文件职责过重，混合 parse、compileScript、compileTemplate、compileStyle、Vue 2.7 compatibility。
2. `vue27-sfc` generated-alias 官方 conformance 已关闭为 144/144，但 `compileStyle.spec.ts` 仍按 mixed API 边界统计。
3. Vue 3 SFC 官方 suite 461/461 仍全部是 mixed，不能作为纯 Rust SFC parity 证明。
4. compileScript、compileTemplate、compileStyle 与 descriptor、binding metadata、asset、SSR、source map 的组合仍需要更多 Rust-backed conformance。
5. PostCSS callback、async style dependency 与 complex source map 仍需要更清晰的产品 API 边界。
6. 建议拆分为 `parse.rs`、`descriptor.rs`、`script/`、`template.rs`、`style.rs`、`css_vars.rs`、`rewrite_default.rs`、`errors.rs`。

### `vuec_node_bridge`

1. 当前是 JSON CLI bridge，适合测试和 alias runtime，不是最终产品 API。
2. 它依赖几乎所有 compiler crate，容易成为隐性集成层和 public API 投影层。
3. 需要拆分测试 bridge 与产品 binding，避免测试兼容逻辑进入发布包。
4. 需要定义 NAPI/WASM ABI、serialization schema、error schema、panic boundary。

### `xtask`

1. `xtask/src/main.rs` 是合理 CLI 入口。
2. `xtask/src/compat.rs` 过大，并且含 JS runtime raw string，长期维护风险高。
3. compat 工具链是项目优势，但应该模块化，并把 temporary semantic shim 显式标红。
4. conformance coverage 分类已经做对了，后续必须以 rust-backed coverage 作为完成度口径。

### `compat/`

1. API/option/output fixture 已形成基线。
2. `compat/api/allowed-diff.json` 当前为空，说明 API shape 当前没有登记的允许差异，这是好信号。
3. option/output fixture 数量仍偏代表性，不足以覆盖产品 API 全组合。
4. official revisions lock 需要有升级流程和回归说明。

### `docs/`

1. 已有研究、设计、开发计划、AST/HIR/MIR、memory、pending、unresolved、compat concerns、work part。
2. 这些文档提供方向，但缺少当前状态的产品成熟度审计，所以本文补上。
3. 后续每次重要架构转向都应更新对应设计文档，而不是只改代码。

## 优先级路线

### P0：已关闭的真实失败和口径问题

1. `cargo xtask run-option-matrix --all` 的 SFC option row 已恢复为 7/7 target 通过。
2. `vue27-sfc` generated-alias 官方 suite 已恢复为 144/144。
3. 所有 conformance 进度报告继续区分 rust-backed、mixed、shim-backed。
4. `cargo xtask summarize-compat --locked` 已恢复为通过。

完成标准：

1. option matrix 7/7 target 通过。
2. Vue 2.7 SFC generated-alias 官方测试 144/144 通过，并明确记录 mixed PostCSS 边界。
3. summarize compat 通过。

### P1：把 Vue 3 mixed 覆盖迁移为 Rust-backed 覆盖

1. 为 Vue 3 DOM 建立 Rust-backed official slices。
2. 为 Vue 3 SSR 建立 Rust-backed official slices。
3. 为 Vue 3 SFC 建立 Rust-backed official slices。
4. 将 compiler-core mixed 中的 transform/codegen 语义逐步迁移到 Rust。
5. 每次迁移都记录 coverage 数字变化，mixed 下降，rust-backed 上升。

完成标准：

1. DOM/SSR/SFC 至少核心 public compile API 有 rust-backed official suite。
2. mixed 只能作为 harness 辅助，不再作为完成度证据。

### P2：拆分巨型模块并硬化阶段边界

1. 拆分 `vuec_vue3_core/src/lib.rs`。
2. 拆分 `vuec_sfc/src/lib.rs`。
3. 拆分 `xtask/src/compat.rs`。
4. 为 AST/HIR/MIR 主路径加结构测试，防止 codegen 绕过 MIR。
5. 将 JS runtime raw string 移到资源文件或模板，并分类标注 shim 用途。

完成标准：

1. 单文件职责可审查。
2. compiler 阶段通过模块边界表达。
3. temporary semantic shim 均有迁移计划。

### P3：补齐产品 API 和发布形态

1. 定义最终 NAPI/WASM/CLI 产品边界。
2. 建立 ESM/CJS/types/exports/package 发布策略。
3. 与 alias bridge 分离，测试 bridge 不进入产品 runtime。
4. 加入 panic boundary、error schema、diagnostic schema。

完成标准：

1. 用户可安装并替换官方 compiler 包进行 smoke。
2. bridge/API schema 有版本和兼容策略。

### P4：建立性能、鲁棒性和 CI gate

1. benchmark：parse、transform、SFC、style、SSR、大文件。
2. fuzz：HTML、SFC、JS expression、CSS。
3. stress：深层嵌套、超大模板、异常 unicode、错误恢复。
4. CI：fmt、clippy、test、api diff、alias、output、option、conformance、bench smoke。
5. 发布前 checklist 和 official lock 升级流程。

完成标准：

1. 性能回归可见。
2. panic-free public API 有测试证明。
3. 发布不依赖人工记忆。

## 成熟编译器架构的目标状态

目标架构应满足以下硬性条件：

1. 所有 public compile API 都有明确入口 crate 和产品 binding。
2. Parser 只负责 CST/AST 和错误恢复，不做 codegen 语义。
3. Transform pass 只修改 AST 或产生语义标记，不输出字符串。
4. Lowering 是 AST 到 HIR 到目标 MIR 的唯一语义桥。
5. DOM、SSR、Vapor 等目标拥有独立 MIR。
6. Codegen 只消费 MIR、helper registry、source map builder，不反向查询 public projection。
7. JS 表达式统一进入 `JsAstStore` 和 scope analysis。
8. Diagnostics 从每个阶段结构化汇总，顺序稳定。
9. Source map 从 source span/generation span 系统生成，覆盖 SFC block offset。
10. Official conformance 的完成度只按 rust-backed 统计。
11. `xtask` 只做工具和 harness，不承载编译器语义。
12. 产品 package 与测试 bridge 分离。

## 不应误判为完成的事项

1. `cargo test --workspace` 通过不等于产品成熟。
2. Vue 3 DOM/SFC/SSR official suite mixed 通过不等于纯 Rust compiler parity。
3. API manifest 匹配不等于行为匹配。
4. output contract 代表性通过不等于所有 source map、diagnostic、runtime 行为通过。
5. JS alias runtime 中补齐的行为不等于 Rust 编译器实现完成。
6. 单个 fixture 的字符串输出匹配不等于 AST/HIR/MIR 架构正确。

## 验收清单

以下清单可作为后续产品化 gate：

| Gate | 必须状态 |
| --- | --- |
| 格式与单元测试 | `cargo fmt --all --check`、`cargo test --workspace` 通过 |
| API 兼容 | `cargo xtask diff-api --all` 通过，允许差异有记录和理由 |
| Alias smoke | `cargo xtask verify-npm-alias --all` 通过 |
| Output contract | `cargo xtask run-output-contract --all` 通过 |
| Option matrix | `cargo xtask run-option-matrix --all` 通过 |
| Rust-backed conformance | Vue 2、Vue 2.7、Vue 3 core/dom/sfc/ssr 的 public compiler API 均有 rust-backed 覆盖 |
| Mixed coverage | 只作为 harness 健康度，不计入完成度 |
| SFC | Vue 2.7 和 Vue 3 SFC parse/script/template/style 均有官方兼容覆盖 |
| Style | scoped、vars、modules、preprocessor、PostCSS、source map 均有契约 |
| Diagnostics | error/warn code、message、loc、frame、顺序稳定 |
| Source map | template/script/style/SSR 全路径覆盖 |
| API 产品化 | NAPI/WASM/CLI/package exports 完整 |
| 性能 | benchmark 有阈值和回归 gate |
| 鲁棒性 | fuzz/stress/panic-free public API |
| 架构 | 巨型文件拆分，AST/HIR/MIR 主路径硬约束 |

## 当前最关键的风险排序

1. Vue 3 DOM/SFC/SSR 通过率主要是 mixed，容易被误读成 Rust compiler 已完成。
2. Vue 2.7 SFC public conformance 已关闭，但 PostCSS callback 等 mixed API 边界不能算作纯 Rust style compiler 完成度。
3. `vuec_vue3_core`、`vuec_sfc`、`xtask/compat.rs` 的单文件体量过大，阻碍阶段边界和长期维护。
4. Style compiler 缺完整 PostCSS/preprocessor 生态，当前通过仍依赖部分 JS API adapter 边界。
5. AST/HIR/MIR 设计正确但尚未完全制度化为不可绕过的代码路径。
6. 产品 binding/package/CI/release 缺失，当前更多是测试桥接和本地工具链。
7. 诊断/source map/fuzz/performance 没有完整产品级证据。

## 审计边界与残余风险

本文是基于当前仓库文件、已有文档、本地测试命令和当前 conformance 产物的工程审计，不是形式化证明，也不是对 Vue 官方全部行为的逐项等价证明。本文已经覆盖当前可见模块和文件，并把架构与产品化风险按证据暴露出来；后续仍需要通过更多 rust-backed official slices、fuzz、benchmark、CI gate 和真实生态集成来发现更深层的行为差异。

## 结语

当前项目有坚实基础，尤其是 Vue 2 模板编译器和兼容性工具链。但从专业编译器开发架构角度，成熟度判断必须以纯 Rust 编译路径、阶段边界、可观察行为 parity、产品发布能力和长期维护性为准。

下一阶段不应继续用 mixed harness 的通过率包装完成度，而应把 Vue 3 DOM/SSR/SFC 的 mixed 覆盖逐步迁移成 Rust-backed 覆盖，并同步拆分巨型模块，让 AST/HIR/MIR 设计从文档约束变成代码结构约束。
