# 农场地图分层图 gpt-image-2 提示词

这些提示词必须在 `farm-full-map.png` 已生成并确认后使用。所有分层图都以 `farm-full-map.png` 作为参考图/编辑输入，不能单独发散生成。

## 通用约束

- 输入图：`farm-full-map.png`
- 模型：`gpt-image-2`
- 风格：保持输入图完全一致的 2D 手绘卡通农场游戏风格
- 禁止：照片质感、写实土壤、风格漂移、文字、水印、UI 面板
- 输出：PNG
- 分层输出必须方便前端在相同坐标系中重新组装

## `farm-background.png`

```text
Edit the provided farm-full-map image to create the base background layer for a 2D farming game map.

Remove only the three interactive farm plots and all crops from the lower-middle area. Fill the removed areas with coherent grass, small plants, dirt patches, and environmental details that match the original hand-painted cartoon game style.

Keep the stone well, wooden fences, path stones, watering-can platform, harvest basket, shrubs, trees, rocks, flowers, lighting, camera angle, and canvas size exactly consistent with the input image.

Output a complete 1536x1024 opaque PNG background layer. No UI, no text, no labels, no characters, no watermark. Do not introduce photorealistic textures.
```

## `farm-foreground.png`

```text
Create a foreground occlusion layer from the provided farm-full-map image.

Keep only foreground objects that should visually appear in front of a small pet walking through the farm map, such as front-edge shrubs, tree leaves, grass clumps, large rocks, and decorative flowers near the lower and side edges. Remove the flat background, plots, crops, sky-free ground areas, and distant objects.

Output a PNG layer aligned to the original 1536x1024 canvas. The layer should be suitable for alpha compositing over characters and effects. Preserve the exact hand-painted cartoon game style of the input image. No text, no UI, no watermark.
```

## `plot-1-soil.png`

```text
From the provided farm-full-map image, isolate the left farm plot as an empty soil layer.

Use the exact same plot shape, pebble border, grass edge, perspective, lighting, and cartoon soil texture from the source image. Remove all wheat crops while keeping a natural empty tilled soil surface underneath.

Output only the left plot layer on a clean removable flat chroma-key background, with generous padding matching the plot bounding box. No text, no UI, no watermark, no photo texture.
```

## `plot-2-soil.png`

```text
From the provided farm-full-map image, isolate the middle farm plot as an empty soil layer.

Use the exact same plot shape, pebble border, grass edge, perspective, lighting, and cartoon soil texture from the source image. Remove all carrot crops while keeping a natural empty tilled soil surface underneath.

Output only the middle plot layer on a clean removable flat chroma-key background, with generous padding matching the plot bounding box. No text, no UI, no watermark, no photo texture.
```

## `plot-3-soil.png`

```text
From the provided farm-full-map image, isolate the right farm plot as an empty soil layer.

Use the exact same plot shape, pebble border, grass edge, perspective, lighting, and cartoon soil texture from the source image. Remove all tomato crops while keeping a natural empty tilled soil surface underneath.

Output only the right plot layer on a clean removable flat chroma-key background, with generous padding matching the plot bounding box. No text, no UI, no watermark, no photo texture.
```

## `plot-1-wheat.png`

```text
From the provided farm-full-map image, isolate only the wheat crop elements from the left farm plot.

Keep the exact crop positions, scale, painted shadows, color, and style from the source image. Remove the base soil rectangle, grass background, fences, decorations, and all non-wheat elements. Preserve only wheat plants and the minimum necessary contact shadows or soil disturbances directly under the plants.

Output the crop layer on a clean removable flat chroma-key background, with the same bounding box and padding as plot-1-soil.png. No text, no UI, no watermark, no photo texture.
```

## `plot-2-carrot.png`

```text
From the provided farm-full-map image, isolate only the carrot crop elements from the middle farm plot.

Keep the exact crop positions, scale, painted shadows, color, and style from the source image. Remove the base soil rectangle, grass background, fences, decorations, and all non-carrot elements. Preserve only carrot plants and the minimum necessary contact shadows or soil disturbances directly under the plants.

Output the crop layer on a clean removable flat chroma-key background, with the same bounding box and padding as plot-2-soil.png. No text, no UI, no watermark, no photo texture.
```

## `plot-3-tomato.png`

```text
From the provided farm-full-map image, isolate only the tomato crop elements from the right farm plot.

Keep the exact crop positions, scale, painted shadows, color, and style from the source image. Remove the base soil rectangle, grass background, fences, decorations, and all non-tomato elements. Preserve only tomato plants and the minimum necessary contact shadows or soil disturbances directly under the plants.

Output the crop layer on a clean removable flat chroma-key background, with the same bounding box and padding as plot-3-soil.png. No text, no UI, no watermark, no photo texture.
```

## 状态层提示词

### `plot-*-watered.png`

```text
Create a watered overlay for the specified farm plot from the provided farm-full-map image.

Keep only subtle cartoon wet-soil shine, tiny puddle highlights, and darker damp patches that align with the plot perspective. The overlay must be composited above the soil and below crops. Do not include the entire soil plot, crops, grass background, text, UI, or watermark.
```

### `plot-*-highlight.png`

```text
Create a soft selectable highlight overlay for the specified farm plot from the provided farm-full-map image.

The highlight should match a polished cozy farming game UI: warm golden outline, subtle glow, and readable hover/selected state. It must align exactly to the plot border and remain lightweight enough not to cover crop art. No text, no icons, no UI panel.
```

### `plot-*-ready.png`

```text
Create a harvest-ready sparkle overlay for the specified farm plot from the provided farm-full-map image.

Use small hand-painted golden sparkles and soft glow accents around mature crops. Keep it subtle and game-like. Do not include text, icons, UI panels, or unrelated objects.
```
