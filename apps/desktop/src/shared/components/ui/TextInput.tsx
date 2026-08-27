import SemiInput from "@douyinfe/semi-ui/lib/es/input";
import type { InputProps as SemiInputProps } from "@douyinfe/semi-ui/lib/es/input";
import type { ChangeEvent } from "react";

export interface TextInputProps extends Omit<SemiInputProps, "onChange"> {
  onChange?: (value: string, event: ChangeEvent<HTMLInputElement>) => void;
}

export function TextInput({ className, onChange, ...props }: TextInputProps) {
  const classes = ["liberty-ui-input", className].filter(Boolean).join(" ");

  return <SemiInput {...props} className={classes} onChange={onChange} />;
}
