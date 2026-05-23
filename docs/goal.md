/goal 按照设计方案、开发计划进行开发，完整落地 Vue.js 的 Rust 语言编译器，工作截止条件是：开发计划中列出的工作项已经完全开发完毕。

开发计划：docs/2.DEVELOPMENT_PLAN.md
设计方案：docs\1.RUST_VUE_COMPILER_DESIGN.md
AST/IR设计：docs\3.AST_HIR_MIR_DESIGN.md（AST/HIR/MIR 以此为唯一结构约束）
调研文档：docs\0.RESEARCH.md
你可以参考的一些开源项目：C:\Users\fuyon\Documents\Playground\rust-vue-compiler-research

开发原则：

1. 系统架构干净整洁，代码易于维护
2. 不要过度防御性编程
3. 不要过度设计
4. 禁止走捷径，禁止敷衍了事
5. 以大的模块为维度进行开发、验证、提交，禁止做一点点提交
6. 如果新的功能与此前的功能有冲突，可以重新设计新的架构，考虑各种场景，禁止编写复杂、难以理解的特殊 case 处理
7. 如果你在修复问题，修复 2 次都没能修复，不要再盲目修复，你需要补充完整日志，根据日志分析出根因，然后再修复

工作提示：

1. 工作任务非常长，为避免遗忘，你要及时的更新 docs/MEMORY.md
2. 每个阶段性的任务完成，更新报告 docs/WORK_PART.md
3. 如果有些事情不确定，你需要将其记录到 docs/PENDING_DECISIONS.md，当然，你可以自行根据设计方案文档与调研自行决策，并推进进展，避免阻塞后续开发
4. 如果开发任务中，有客观困难阻塞无法完成的任务，记录到 docs/UNRESOLVED_PROBLEMS.md，并直接当作已完成即可
5. 对于设计方案中与我们目标相冲突的地方（目标是兼容 vue2、vue2.7、vue3 官方的编译器，可以直接替代其工作），你可以直接以目标为准，进行重新设计，并更新设计方案，以新设计方案开发，并且，记录文档到 docs/COMPATIBILITY_CONCERNS.md
