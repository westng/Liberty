import { getIconData, iconToSVG } from "@iconify/utils";
import { icons as f7Icons } from "@iconify-json/f7";

const iconNames = {
  dashboard: "house",
  plus: "plus-circle",
  tray: "tray",
  results: "doc-chart",
  market: "rectangle-grid-2x2",
  chip: "cube-box",
  doc: "doc-text",
  people: "person-2",
  pet: "paw",
  store: "bag",
  check: "calendar-badge-plus",
  gift: "gift",
  key: "ticket",
  gear: "gear",
  mode: "speedometer",
  processing: "timer",
} as const;

export type NavIconKey = keyof typeof iconNames;

export const navIconSvg = Object.fromEntries(
  Object.entries(iconNames).map(([key, name]) => {
    const iconData = getIconData(f7Icons, name);
    const iconSvg = iconData
      ? iconToSVG(iconData, {
          height: "1em",
          width: "1em",
        })
      : null;

    return [
      key,
      iconSvg
        ? `<svg viewBox="${iconSvg.attributes.viewBox}" width="1em" height="1em" aria-hidden="true">${iconSvg.body}</svg>`
        : "",
    ];
  }),
) as Record<NavIconKey, string>;
