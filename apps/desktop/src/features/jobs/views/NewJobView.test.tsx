import { renderToStaticMarkup } from "react-dom/server";
import type {
  ButtonHTMLAttributes,
  InputHTMLAttributes,
  ReactNode,
  SelectHTMLAttributes,
  TextareaHTMLAttributes,
} from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const testState = vi.hoisted(() => ({
  localMode: true,
  locale: "zh-CN" as "zh-CN" | "en-US",
  remoteCreationAvailable: true,
  runtimeReady: true,
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  message: vi.fn(),
  open: vi.fn(),
}));

vi.mock("@douyinfe/semi-icons", () => {
  function Icon() {
    return <span aria-hidden="true" />;
  }

  return {
    IconChevronDownStroked: Icon,
    IconDeleteStroked: Icon,
    IconFolderOpenStroked: Icon,
    IconGlobeStroked: Icon,
    IconLightningStroked: Icon,
    IconMicrophoneStroked: Icon,
    IconMusicNoteStroked: Icon,
    IconPlusStroked: Icon,
    IconServerStroked: Icon,
    IconSettingStroked: Icon,
    IconTextStroked: Icon,
    IconTickCircle: Icon,
    IconVideoStroked: Icon,
  };
});

vi.mock("@douyinfe/semi-ui/lib/es/select", () => ({
  default: ({
    optionList,
    ...props
  }: SelectHTMLAttributes<HTMLSelectElement> & {
    optionList?: Array<{ label: ReactNode; value: string }>;
  }) => (
    <select {...props}>
      {optionList?.map((option) => (
        <option key={option.value} value={option.value}>{option.label}</option>
      ))}
    </select>
  ),
}));

vi.mock("@douyinfe/semi-ui/lib/es/tag", () => ({
  default: ({ children }: { children?: ReactNode }) => <span>{children}</span>,
}));

vi.mock("@douyinfe/semi-ui/lib/es/input/textarea", () => ({
  default: ({
    autosize: _autosize,
    composition: _composition,
    resize: _resize,
    onChange: _onChange,
    ...props
  }: TextareaHTMLAttributes<HTMLTextAreaElement> & {
    autosize?: unknown;
    composition?: boolean;
    resize?: string;
  }) => <textarea {...props} readOnly />,
}));

vi.mock("@/shared/components/ui", () => ({
  Button: ({
    block: _block,
    icon,
    loading: _loading,
    variant: _variant,
    ...props
  }: ButtonHTMLAttributes<HTMLButtonElement> & {
    block?: boolean;
    icon?: ReactNode;
    loading?: boolean;
    variant?: string;
  }) => <button {...props}>{icon}{props.children}</button>,
  Switch: ({
    checked,
    id,
    label,
    onChange: _onChange,
    wrapperClassName,
    ...props
  }: InputHTMLAttributes<HTMLInputElement> & {
    label: ReactNode;
    onChange: (checked: boolean) => void;
    wrapperClassName?: string;
  }) => (
    <label className={wrapperClassName} htmlFor={id}>
      <input {...props} checked={checked} id={id} readOnly role="switch" type="checkbox" />
      <span>{label}</span>
    </label>
  ),
  TextInput: ({
    composition: _composition,
    onChange: _onChange,
    ...props
  }: InputHTMLAttributes<HTMLInputElement> & {
    composition?: boolean;
    onChange?: (value: string) => void;
  }) => <input {...props} readOnly />,
}));

vi.mock("@/app/router/RouterContext", () => ({
  useRouter: () => ({ push: vi.fn() }),
}));

vi.mock("@/features/meeting/stores/useMeetingStore", () => ({
  useMeetingStore: () => ({
    canRemoteOperation: () => testState.remoteCreationAvailable,
    createJob: vi.fn(),
    localMode: testState.localMode,
    runtimeStatus: { shellReady: testState.runtimeReady },
    settings: {
      defaultHotwords: "Liberty, FunASR",
      locale: testState.locale,
      summaryTemplate: "Weekly Meeting",
    },
  }),
}));

import NewJobView from "./NewJobView";

function renderView() {
  return renderToStaticMarkup(<NewJobView />);
}

describe("NewJobView", () => {
  beforeEach(() => {
    testState.localMode = true;
    testState.locale = "zh-CN";
    testState.remoteCreationAvailable = true;
    testState.runtimeReady = true;
  });

  it("renders the local creation flow with labelled controls and readiness guidance", () => {
    const markup = renderView();

    expect(markup).toContain("创建会议任务");
    expect(markup).toContain("本地模式每次只处理 1 个文件");
    expect(markup).toContain("M4A · MP3 · WAV");
    expect(markup).toContain('id="job-title"');
    expect(markup).toContain('required=""');
    expect(markup).toContain('id="job-lang"');
    expect(markup).toContain('aria-labelledby="new-job-language-label"');
    expect(markup).toContain('id="job-speaker"');
    expect(markup).toContain('role="switch"');
    expect(markup).toContain("Weekly Meeting");
    expect(markup).toContain('role="status"');
    expect(markup).toContain("请先选择一个会议音频或视频文件。");
  });

  it("renders matching English copy", () => {
    testState.locale = "en-US";

    const markup = renderView();

    expect(markup).toContain("Create Meeting Job");
    expect(markup).toContain("Local mode processes one file at a time");
    expect(markup).toContain("Choose a meeting audio or video file first.");
    expect(markup).toContain("Create Job");
  });

  it("announces and disables unavailable remote creation", () => {
    testState.localMode = false;
    testState.remoteCreationAvailable = false;

    const markup = renderView();

    expect(markup).toContain('role="alert"');
    expect(markup).toContain("安全的远端分块上传协议尚未接入");
    expect(markup).toContain('class="new-job-builder-picker" type="button" disabled=""');
    expect(markup).toContain('class="new-job-builder-create" disabled=""');
    expect(markup).toContain('type="submit"');
  });
});
