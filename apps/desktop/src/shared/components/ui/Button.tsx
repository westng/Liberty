import SemiButton from "@douyinfe/semi-ui/lib/es/button";
import type { ButtonProps as SemiButtonProps } from "@douyinfe/semi-ui/lib/es/button";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "text" | "danger";

export interface ButtonProps extends Omit<SemiButtonProps, "htmlType" | "theme" | "type"> {
  type?: "button" | "reset" | "submit";
  variant?: ButtonVariant;
}

const variantProps: Record<ButtonVariant, Pick<SemiButtonProps, "theme" | "type">> = {
  primary: { theme: "solid", type: "primary" },
  secondary: { theme: "outline", type: "secondary" },
  ghost: { theme: "borderless", type: "secondary" },
  text: { theme: "borderless", type: "tertiary" },
  danger: { theme: "borderless", type: "danger" },
};

export function Button({ className, type = "button", variant = "secondary", ...props }: ButtonProps) {
  const semiProps = variantProps[variant];
  const classes = ["liberty-ui-button", `liberty-ui-button--${variant}`, className].filter(Boolean).join(" ");

  return <SemiButton {...props} {...semiProps} className={classes} htmlType={type} />;
}
