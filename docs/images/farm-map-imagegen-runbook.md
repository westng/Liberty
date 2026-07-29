# 农场地图 gpt-image-2 执行清单

## 前置条件

```bash
export FARM_OUT=output/imagegen/farm-map
mkdir -p "$FARM_OUT"
```

正式执行前必须通过项目 wrapper 确认当前 Codex provider。wrapper 会读取 `~/.codex/config.toml` 的 `model_provider` 和对应 `base_url`，读取 `~/.codex/auth.json` 的 `OPENAI_API_KEY`，再显式注入给 Image API CLI。不要直接依赖 SDK 默认地址，也不要手写 `OPENAI_BASE_URL=https://api.openai.com/v1`。

```bash
node scripts/run-codex-image-gen.mjs --print-effective-config
```

## 1. 生成完整场地图

```bash
node scripts/run-codex-image-gen.mjs generate \
  --model gpt-image-2 \
  --size 1536x1024 \
  --quality high \
  --out "$FARM_OUT/farm-full-map.png" \
  --prompt-file docs/images/farm-map-full-prompt.txt
```

验收 `farm-full-map.png` 后，才能继续拆层。

## 2. 生成场地层

```bash
node scripts/run-codex-image-gen.mjs edit \
  --model gpt-image-2 \
  --quality high \
  --image "$FARM_OUT/farm-full-map.png" \
  --out "$FARM_OUT/farm-background.png" \
  --prompt-file docs/images/prompts/farm-background-prompt.txt

node scripts/run-codex-image-gen.mjs edit \
  --model gpt-image-2 \
  --quality high \
  --image "$FARM_OUT/farm-full-map.png" \
  --out "$FARM_OUT/farm-foreground.png" \
  --prompt-file docs/images/prompts/farm-foreground-prompt.txt
```

## 3. 生成空土地图

```bash
node scripts/run-codex-image-gen.mjs edit \
  --model gpt-image-2 \
  --quality high \
  --image "$FARM_OUT/farm-full-map.png" \
  --out "$FARM_OUT/plot-1-soil.png" \
  --prompt-file docs/images/prompts/plot-1-soil-prompt.txt

node scripts/run-codex-image-gen.mjs edit \
  --model gpt-image-2 \
  --quality high \
  --image "$FARM_OUT/farm-full-map.png" \
  --out "$FARM_OUT/plot-2-soil.png" \
  --prompt-file docs/images/prompts/plot-2-soil-prompt.txt

node scripts/run-codex-image-gen.mjs edit \
  --model gpt-image-2 \
  --quality high \
  --image "$FARM_OUT/farm-full-map.png" \
  --out "$FARM_OUT/plot-3-soil.png" \
  --prompt-file docs/images/prompts/plot-3-soil-prompt.txt
```

## 4. 生成作物种植图

```bash
node scripts/run-codex-image-gen.mjs edit \
  --model gpt-image-2 \
  --quality high \
  --image "$FARM_OUT/farm-full-map.png" \
  --out "$FARM_OUT/plot-1-wheat.png" \
  --prompt-file docs/images/prompts/plot-1-wheat-prompt.txt

node scripts/run-codex-image-gen.mjs edit \
  --model gpt-image-2 \
  --quality high \
  --image "$FARM_OUT/farm-full-map.png" \
  --out "$FARM_OUT/plot-2-carrot.png" \
  --prompt-file docs/images/prompts/plot-2-carrot-prompt.txt

node scripts/run-codex-image-gen.mjs edit \
  --model gpt-image-2 \
  --quality high \
  --image "$FARM_OUT/farm-full-map.png" \
  --out "$FARM_OUT/plot-3-tomato.png" \
  --prompt-file docs/images/prompts/plot-3-tomato-prompt.txt
```

## 5. 复制到运行资产目录

只有通过视觉验收后，才复制到正式运行目录：

```bash
cp "$FARM_OUT/farm-background.png" apps/desktop/src/assets/images/farm-layers/farm-background.png
cp "$FARM_OUT/farm-foreground.png" apps/desktop/src/assets/images/farm-layers/farm-foreground.png
cp "$FARM_OUT/plot-1-soil.png" apps/desktop/src/assets/images/farm-layers/plot-1-soil.png
cp "$FARM_OUT/plot-2-soil.png" apps/desktop/src/assets/images/farm-layers/plot-2-soil.png
cp "$FARM_OUT/plot-3-soil.png" apps/desktop/src/assets/images/farm-layers/plot-3-soil.png
cp "$FARM_OUT/plot-1-wheat.png" apps/desktop/src/assets/images/farm-layers/plot-1-wheat.png
cp "$FARM_OUT/plot-2-carrot.png" apps/desktop/src/assets/images/farm-layers/plot-2-carrot.png
cp "$FARM_OUT/plot-3-tomato.png" apps/desktop/src/assets/images/farm-layers/plot-3-tomato.png
```

## 验收规则

- `farm-full-map.png` 先验收，通过后才能拆层。
- 分层图必须和 `farm-full-map.png` 同风格、同坐标体系。
- 空土地图不能出现照片质感。
- 作物层不能包含整块土地图背景。
- 组装结果必须能接近 `farm-full-map.png`。
- 前端运行时不能直接把 `farm-full-map.png` 当背景图使用。
