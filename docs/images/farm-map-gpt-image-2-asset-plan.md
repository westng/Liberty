# 农场地图 gpt-image-2 素材生产清单

## 硬性流程

1. 先用 `gpt-image-2` 生成完整场地图 `farm-full-map.png`。
2. 只以 `farm-full-map.png` 为视觉源，继续用 `gpt-image-2` 编辑/生成分层图。
3. 前端最后按 `farm-map-layers.json` 组装，不允许混入照片素材、临时素材或其它风格素材。

## 首期必须图片

| 文件 | 尺寸 | 背景 | 用途 |
| --- | --- | --- | --- |
| `farm-full-map.png` | `1536x1024` | 不透明 | 完整验收图，所有分层图的唯一视觉源。 |
| `farm-background.png` | `1536x1024` | 不透明 | 去掉三块可交互地块和作物后的场地底图。 |
| `plot-1-soil.png` | 按地块实际包围盒 | 透明 | 左侧空土地图。 |
| `plot-2-soil.png` | 按地块实际包围盒 | 透明 | 中间空土地图。 |
| `plot-3-soil.png` | 按地块实际包围盒 | 透明 | 右侧空土地图。 |
| `plot-1-wheat.png` | 与 `plot-1-soil.png` 一致 | 透明 | 左侧小麦种植层。 |
| `plot-2-carrot.png` | 与 `plot-2-soil.png` 一致 | 透明 | 中间胡萝卜种植层。 |
| `plot-3-tomato.png` | 与 `plot-3-soil.png` 一致 | 透明 | 右侧番茄种植层。 |

## 游戏体验预留图片

| 文件 | 用途 |
| --- | --- |
| `farm-foreground.png` | 树丛、草、石头等前景遮挡层，用于宠物入场和深度遮挡。 |
| `plot-1-watered.png` / `plot-2-watered.png` / `plot-3-watered.png` | 浇水湿润效果。 |
| `plot-1-highlight.png` / `plot-2-highlight.png` / `plot-3-highlight.png` | 地块选中、可播种、可收获高亮。 |
| `plot-1-ready.png` / `plot-2-ready.png` / `plot-3-ready.png` | 可收获提示光效。 |
| `farm-collision-mask.png` | 宠物不可走区域遮罩。 |
| `farm-map-layers.json` | 图片坐标、层级、点击区域、宠物入场点、路径点。 |

## 完整场地图提示词

```text
Use case: stylized-concept
Asset type: 2D game farm map full acceptance image for a desktop pet mini-game
Primary request: Create a complete polished cartoon farm field map that will become the single visual source of truth for later layered game assets.
Scene/backdrop: cozy top-down/isometric farm garden, bright grass field, wooden fence along the upper edge, stone well on the upper-left, small dirt path stones, watering can and harvest basket platform on the upper-right, decorative rocks, flowers, shrubs, and trees around the edges.
Subject: three large rectangular farm plots arranged left to right in the lower-middle area; plot 1 has mature golden wheat, plot 2 has carrots, plot 3 has tomatoes; each plot uses the same cartoon soil and pebble border visual language.
Style/medium: high-quality hand-painted 2D mobile game UI map, cute cozy farming game, consistent cartoon texture, no photorealism.
Composition/framing: 1536x1024 landscape, full map visible, no UI chrome, no text, no labels, no characters, leave enough margin around plots for clicking and pet walking.
Lighting/mood: sunny, friendly, polished casual game mood, soft painted shadows.
Color palette: vivid greens, warm brown soil, golden wheat, orange carrots, red tomatoes, blue watering can accents.
Materials/textures: painterly grass, stylized soil, rounded stones, wooden fence and platform, leafy shrubs.
Constraints: all elements must share one coherent game-art style; the three plots must be cleanly separable into layers later; no realistic photo textures; no watermark; no text; no logo.
Avoid: photorealistic soil, photographic grass, mixed art styles, UI panels, characters, animals, excessive perspective distortion.
```

CLI dry-run 必须通过项目 wrapper 执行。wrapper 会读取 Codex 当前配置里的 provider/base_url，并把它显式注入给 Image API CLI，避免 SDK 回落到默认官方地址。

```bash
node scripts/run-codex-image-gen.mjs --print-effective-config generate \
  --model gpt-image-2 \
  --size 1536x1024 \
  --quality high \
  --out output/imagegen/farm-full-map.png \
  --prompt-file docs/images/farm-map-full-prompt.txt
```

当前环境的调用口径：不手写官方 `OPENAI_BASE_URL`；以 `~/.codex/config.toml` 的 `model_provider` 为准，并从该 provider 读取 `base_url`。

## 分层生成要求

### `farm-background.png`

基于 `farm-full-map.png` 编辑，移除三块地和所有作物，补齐下面的草地/环境。保持井、围栏、工具、树丛、路、石头、花草不变。

### `plot-*-soil.png`

基于 `farm-full-map.png` 对应地块区域生成透明 PNG。只保留空土地图和边缘石头/草，不保留作物。三块地必须同一套卡通土壤质感。

### `plot-*-crop.png`

基于 `farm-full-map.png` 对应地块区域生成透明 PNG。只保留作物和必要贴地阴影/土壤扰动，不包含整块土地图背景。尺寸必须与对应 `plot-*-soil.png` 完全一致。

## 组装验收

前端组装顺序：

1. `farm-background.png`
2. `plot-*-soil.png`
3. `plot-*-crop.png`
4. `plot-*-watered/highlight/ready.png`
5. `farm-foreground.png`
6. 宠物、粒子、UI 热区

验收条件：

- 组装图必须接近 `farm-full-map.png`。
- 页面运行时不能直接使用 `farm-full-map.png` 作为背景。
- 运行时不能使用照片素材。
- 所有土地图、种植图必须来自同一张 `farm-full-map.png` 的视觉体系。
- 地图可移动、缩放，地块点击区域由 `farm-map-layers.json` 驱动。
