import { renderToStaticMarkup } from "react-dom/server";
import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@douyinfe/semi-ui/lib/es/button", () => ({
  default: ({
    htmlType,
    theme,
    type,
    ...props
  }: ButtonHTMLAttributes<HTMLButtonElement> & { htmlType?: string; theme?: string; type?: string }) => (
    <button
      {...props}
      className={`${props.className ?? ""} semi-button-${type}`}
      data-theme={theme}
      type={htmlType as "button" | "reset" | "submit"}
    />
  ),
}));

vi.mock("@douyinfe/semi-ui/lib/es/input", () => ({
  default: ({ onChange, ...props }: Omit<InputHTMLAttributes<HTMLInputElement>, "onChange"> & { onChange?: (value: string) => void }) => (
    <input {...props} onChange={(event) => onChange?.(event.target.value)} />
  ),
}));

vi.mock("@douyinfe/semi-ui/lib/es/switch", () => ({
  default: ({ checked, className, id, onChange, ...props }: InputHTMLAttributes<HTMLInputElement>) => (
    <input
      {...props}
      aria-checked={checked}
      checked={checked}
      className={className}
      id={id}
      onChange={onChange}
      role="switch"
      type="checkbox"
    />
  ),
}));

vi.mock("@douyinfe/semi-ui/lib/es/tabs", () => ({
  default: ({
    activeKey,
    className,
    tabList,
  }: {
    activeKey?: string;
    className?: string;
    tabList?: Array<{ disabled?: boolean; itemKey: string; tab?: ReactNode }>;
  }) => (
    <div className={className} role="tablist">
      {tabList?.map((item) => (
        <button
          aria-selected={item.itemKey === activeKey}
          disabled={item.disabled}
          key={item.itemKey}
          role="tab"
          type="button"
        >
          {item.tab}
        </button>
      ))}
    </div>
  ),
}));

import { Button, PasswordInput, Switch, Tabs, TextInput } from "./index";

describe("Liberty UI adapters", () => {
  it("maps Liberty button intent to Semi and native button semantics", () => {
    const markup = renderToStaticMarkup(
      <Button aria-label="Save model" disabled type="submit" variant="primary">
        Save
      </Button>,
    );

    expect(markup).toContain("liberty-ui-button--primary");
    expect(markup).toContain("semi-button-primary");
    expect(markup).toContain('type="submit"');
    expect(markup).toContain("disabled");
    expect(markup).toContain('aria-label="Save model"');
  });

  it("keeps input labels and descriptions addressable", () => {
    const textMarkup = renderToStaticMarkup(
      <TextInput aria-describedby="model-help" id="model-id" onChange={() => undefined} value="gpt-4.1" />,
    );
    const passwordMarkup = renderToStaticMarkup(
      <PasswordInput autoComplete="new-password" id="model-api-key" onChange={() => undefined} value="secret" />,
    );

    expect(textMarkup).toContain('id="model-id"');
    expect(textMarkup).toContain('aria-describedby="model-help"');
    expect(passwordMarkup).toContain('type="password"');
    expect(passwordMarkup).toContain('autoComplete="new-password"');
  });

  it("associates switch labels with native switch controls", () => {
    const markup = renderToStaticMarkup(
      <Switch checked disabled id="model-enabled" label="Enabled" onChange={() => undefined} />,
    );

    expect(markup).toContain('for="model-enabled"');
    expect(markup).toContain('id="model-enabled"');
    expect(markup).toContain('role="switch"');
    expect(markup).toContain('aria-labelledby="model-enabled-label"');
    expect(markup).toContain('aria-checked="true"');
    expect(markup).toContain("disabled");
  });

  it("maps controlled tab items to Semi tab semantics", () => {
    const markup = renderToStaticMarkup(
      <Tabs
        activeKey="activity"
        ariaLabel="Dashboard sections"
        items={[
          { key: "activity", label: "Activity" },
          { key: "insights", label: "Insights" },
        ]}
        onChange={() => undefined}
      />,
    );

    expect(markup).toContain('aria-label="Dashboard sections"');
    expect(markup).toContain('role="tablist"');
    expect(markup).toContain('aria-selected="true"');
    expect(markup).toContain("Insights");
  });
});
