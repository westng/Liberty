import { TextInput, type TextInputProps } from "./TextInput";

export type PasswordInputProps = Omit<TextInputProps, "mode" | "type">;

export function PasswordInput(props: PasswordInputProps) {
  return <TextInput {...props} type="password" />;
}
