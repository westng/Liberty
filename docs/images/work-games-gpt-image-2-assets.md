# 打工小游戏 gpt-image-2 资产说明

本文记录「矿场挖矿」「工厂打螺丝」「便利店值班」三款游戏的图片资产口径。验收标准对齐农场：运行时图片必须来自 `gpt-image-2` 生成或基于 `gpt-image-2` 地图源裁切/转制，不能混入 SVG 占位、照片素材或纯 CSS 假图。

## 运行时资产

| 游戏 | 地图资产 | 岗位内容资产 |
| --- | --- | --- |
| 矿场挖矿 | `apps/desktop/src/assets/images/work-maps/mine-map.webp` | `apps/desktop/src/assets/images/work-maps/content/mine-*-content.png` |
| 工厂打螺丝 | `apps/desktop/src/assets/images/work-maps/factory-map.webp` | `apps/desktop/src/assets/images/work-maps/content/factory-*-content.png` |
| 便利店值班 | `apps/desktop/src/assets/images/work-maps/convenience-store-map.webp` | `apps/desktop/src/assets/images/work-maps/content/store-*-content.png` |

三张旧 SVG 占位图已移除。页面只引用 `.webp` 地图和 `.png` 岗位内容贴图。

## 生成记录

地图 prompt:

- `docs/images/prompts/work-games-mine-map-prompt.txt`
- `docs/images/prompts/work-games-factory-map-prompt.txt`
- `docs/images/prompts/work-games-store-map-prompt.txt`

岗位内容贴图 prompt:

- `docs/images/prompts/work-game-content-icons-jobs.jsonl`

岗位贴图的独立生成命令：

```bash
PYTHON=/usr/local/bin/python3.12 IMAGEGEN_PYTHONPATH=tmp/imagegen-py-x86 \
node scripts/run-codex-image-gen.mjs generate-batch \
  --model gpt-image-2 \
  --size 1024x1024 \
  --quality high \
  --output-format png \
  --input docs/images/prompts/work-game-content-icons-jobs.jsonl \
  --out-dir output/imagegen/work-games/content-key \
  --concurrency 2 \
  --max-attempts 2 \
  --force
```

独立贴图生成后，使用 chroma-key 去底并复制到运行时目录：

```bash
for file in output/imagegen/work-games/content-key/*-key.png; do
  name="$(basename "$file" -key.png).png"
  python "${CODEX_HOME:-$HOME/.codex}/skills/.system/imagegen/scripts/remove_chroma_key.py" \
    --input "$file" \
    --out "apps/desktop/src/assets/images/work-maps/content/$name" \
    --auto-key border \
    --soft-matte \
    --transparent-threshold 12 \
    --opaque-threshold 220 \
    --despill \
    --force
done
```

## 前端使用标准

- `WorkGameView` 使用 1536x1024 世界坐标，与农场一样支持拖拽、滚轮缩放、热区聚焦。
- 每个岗位热区必须显示岗位内容图片、状态气泡、进度条和操作反馈。
- 纯 CSS 圆点只作为缺失资产兜底，不应出现在最终验收路径。
- 牛马市场卡片使用同一套地图资产作为封面。
