import SemiTabs from "@douyinfe/semi-ui/lib/es/tabs";
import type { ReactNode } from "react";

export type TabItem = {
  disabled?: boolean;
  key: string;
  label: ReactNode;
};

export type TabsProps = {
  activeKey: string;
  appearance?: "button" | "line";
  ariaLabel: string;
  className?: string;
  items: TabItem[];
  onChange: (activeKey: string) => void;
};

export function Tabs({ activeKey, appearance = "line", ariaLabel, className, items, onChange }: TabsProps) {
  const classes = ["liberty-ui-tabs", className].filter(Boolean).join(" ");

  return (
    <nav aria-label={ariaLabel} className={classes}>
      <SemiTabs
        activeKey={activeKey}
        onChange={onChange}
        size="small"
        tabList={items.map((item) => ({
          disabled: item.disabled,
          itemKey: item.key,
          tab: item.label,
        }))}
        tabPaneMotion={false}
        type={appearance}
      />
    </nav>
  );
}
