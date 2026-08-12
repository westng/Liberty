# Liberty ASR 验证资产

本目录只提交验证契约，不提交会议媒体、人工标注或设备证据。默认 ASR 保持 FunASR；基准结果只能作为后续引擎决策的输入，不能自动修改产品默认值。

## 本地准备

1. 将脱敏媒体和人工标注放在受控目录。标注 JSON 使用 `schemaVersion=1`、`transcript` 和可选的 `speakerTurns: [{ startMs, endMs, speaker }]`。
2. 按 `manifest.schema.json` 创建被忽略的 `manifest.local.json`。每个文件填写 SHA-256；`engines` 必须各有一个 `baseline` 和 `candidate`，`command` 是 Python 与 Runner 的命令前缀，脚本会追加任务参数。
3. 运行 `pnpm asr:fixtures:check`，再运行 `pnpm asr:benchmark`。脚本不下载、不替换样本，不记录媒体绝对路径，也不读取或输出凭据变量。
4. 在对应物理设备按 `smoke-input.example.json` 创建被忽略的 `smoke.local.json`，逐项填写 `passed`、`failed` 或 `blocked` 和可审计说明，再运行 `pnpm platform:smoke`。
5. 将三类平台同一 commit 的 smoke 与基准 JSON 放进同一证据目录，运行 `pnpm asr:evidence:check -- --evidence-dir <目录>`。

`run-asr-benchmark.mjs` 直接执行两个 Runner，计算 CER、WER、DER、失败率、P95 实时率、峰值 RSS、CPU、冷启动、总耗时和安装目录大小。若仓库有未提交改动，它默认拒绝生成正式证据；`--allow-dirty` 只能生成探索结果，聚合门禁仍会拒绝该结果。

8 GiB 是性能证据档位，不是“至少 8 GiB 即等价”。更高配置设备可以完成兼容 smoke，但不能关闭低配置性能门禁。Windows x86 只保留编译支持声明，不计入三平台本地 ASR 验收。
