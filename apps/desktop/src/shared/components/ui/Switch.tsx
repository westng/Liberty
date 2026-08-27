import SemiSwitch from "@douyinfe/semi-ui/lib/es/switch";
import type { SwitchProps as SemiSwitchProps } from "@douyinfe/semi-ui/lib/es/switch";
import type { ReactNode } from "react";

export interface SwitchProps extends Omit<SemiSwitchProps, "aria-label" | "aria-labelledby" | "id" | "onChange"> {
  checked: boolean;
  id: string;
  label: ReactNode;
  onChange: (checked: boolean) => void;
  wrapperClassName?: string;
}

export function Switch({ className, disabled, id, label, onChange, wrapperClassName, ...props }: SwitchProps) {
  const labelId = `${id}-label`;
  const switchClasses = ["liberty-ui-switch", className].filter(Boolean).join(" ");
  const wrapperClasses = ["liberty-ui-switch-field", disabled && "liberty-ui-switch-field--disabled", wrapperClassName]
    .filter(Boolean)
    .join(" ");

  return (
    <label className={wrapperClasses} htmlFor={id}>
      <SemiSwitch
        {...props}
        aria-labelledby={labelId}
        className={switchClasses}
        disabled={disabled}
        id={id}
        onChange={(checked) => onChange(checked)}
      />
      <span id={labelId}>{label}</span>
    </label>
  );
}
