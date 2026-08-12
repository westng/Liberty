# Liberty Desktop Pet Design Record

文档类型：设计
状态：历史快照
创建日期：2026-05-06
最后核实：2026-08-12（仅核实分类与迁移路径，正文技术事实未重新核实）
适用范围：Liberty 桌面宠物初版方案
权威边界：本文保留历史设计背景，不代表当前宠物规则；当前行为以代码、测试和 `docs/pet-system.md` 为准
依据：迁移前同名历史设计（原 Superpowers 过程文档）

This is a historical design record for the first desktop-pet pass.

Current pet behavior, growth, LP economy, store rules, daily blind box, data model, and implementation boundaries are now maintained in:

- [Liberty 宠物系统](../../pet-system.md)

## Status

- Superseded as the source of current rules.
- Kept only to preserve the original product direction and decision history.
- Do not copy numeric rules, route names, schema details, or acceptance criteria from this file into new work.

## Original Decisions Kept

- Liberty should use a real desktop-persistent pet, not only an in-app avatar.
- The pet should reward meaningful Liberty usage instead of idle grinding.
- Pet failures must not block meeting workflows.
- The main app owns management, settings, history, and detailed state.
- The desktop pet window owns lightweight presence and immediate feedback.

For current implementation details, use [docs/pet-system.md](../../pet-system.md).
