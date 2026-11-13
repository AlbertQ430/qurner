# Qurner 项目协作规则

这些规则适用于 Qurner 仓库中的所有 AI coding agent。更靠近文件的
`AGENTS.md` 和用户当前明确指令优先。

## 必读顺序

开始任何实现前，按以下顺序阅读：

1. 用户当前明确指令。
2. `AGENTS.md`
3. `AI_COLLABORATION.md`
4. `SRS.md`
5. `SDS.md`
6. `DEVELOPMENT_PLAN.md`
7. 当前 `docs/tasks/TASK-xxx.md`

## 角色边界

- Codex 是决策与验收方，维护需求、设计、ADR、任务和审查记录。
- DeepSeek harness 是开发方，实现产品代码、自动化测试、测试夹具和返工修复。
- 未经用户明确授权，DeepSeek 不得修改 SRS、SDS、发布范围、公开接口、数据库模式或安全策略。
- 未经用户明确改变角色，Codex 不直接实现产品功能代码。

## 执行规则

- 默认只执行一个状态为 `in_progress` 或 `ready_for_review` 的任务。
- 没有任务文件时，不开始产品代码开发。
- 需求或设计与任务冲突时，停止冲突部分并报告，不自选方向。
- 只修改任务允许的路径，不顺手重构，不扩展 MVP 范围。
- 自动化测试和测试夹具由 DeepSeek 编写和运行；Codex 审核并独立验收。
- 只报告实际执行的命令和真实结果。失败、跳过和未验证内容必须明确说明。
- 不得把任务自行标记为 `accepted`。
- 不得自行安装依赖、访问外部网络、提交、推送、创建 PR 或发布。
- 不得修改、移动、删除用户的原始电子书或个人数据。
- 不得在日志、测试夹具或提交内容中写入私人路径、书籍正文或敏感信息。

## 交接文件

- 任务模板：`docs/tasks/TEMPLATE.md`
- 详细流程：`AI_COLLABORATION.md`
- 重大技术决定：`docs/decisions/ADR-xxx.md`

完成开发后更新当前任务为 `ready_for_review`，填写完整开发报告并等待 Codex 审核。
