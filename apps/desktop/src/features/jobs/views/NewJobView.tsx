import { message, open } from "@tauri-apps/plugin-dialog";
import {
  IconChevronDownStroked,
  IconDeleteStroked,
  IconFolderOpenStroked,
  IconGlobeStroked,
  IconLightningStroked,
  IconMicrophoneStroked,
  IconMusicNoteStroked,
  IconPlusStroked,
  IconServerStroked,
  IconSettingStroked,
  IconTextStroked,
  IconTickCircle,
  IconVideoStroked,
} from "@douyinfe/semi-icons";
import SemiSelect from "@douyinfe/semi-ui/lib/es/select";
import SemiTag from "@douyinfe/semi-ui/lib/es/tag";
import SemiTextArea from "@douyinfe/semi-ui/lib/es/input/textarea";
import { useEffect, useRef, useState } from "react";
import { useRouter } from "@/app/router/RouterContext";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { Button, Switch, TextInput } from "@/shared/components/ui";
import { formatMessage, getMessages } from "@/shared/i18n";
import type { MeetingSourceFile } from "@/shared/types/meeting";
import { jobDetailPath, jobRef } from "./jobRoutes";
import "./NewJobView.css";

export default function NewJobView() {
  const router = useRouter();
  const store = useMeetingStore();
  const [title, setTitle] = useState("");
  const [hotwordsText, setHotwordsText] = useState(store.settings.defaultHotwords);
  const [previousDefaultHotwords, setPreviousDefaultHotwords] = useState(store.settings.defaultHotwords);
  const [lang, setLang] = useState("zh-CN");
  const [enableSpeaker, setEnableSpeaker] = useState(true);
  const [files, setFiles] = useState<MeetingSourceFile[]>([]);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState("");
  const fileInput = useRef<HTMLInputElement | null>(null);
  const isLocalMode = store.localMode;
  const allMessages = getMessages(store.settings.locale);
  const messages = allMessages.newJob;
  const commonMessages = allMessages.common;
  const settingsMessages = allMessages.settings;
  const summaryTemplate = store.settings.summaryTemplate.trim() || messages.defaultSummaryTemplateName;
  const shouldWarnModelDownloadRequired = isLocalMode && !store.runtimeStatus.shellReady;
  const remoteCreationUnavailable = !isLocalMode && !store.canRemoteOperation("jobs.create");
  const currentLanguageLabel = lang === "en-US"
    ? messages.langEn
    : lang === "ja-JP"
      ? messages.langJa
      : messages.langZh;
  const modeLabel = isLocalMode ? messages.localPython : messages.remoteService;
  const creationHint = remoteCreationUnavailable
    ? messages.remoteUploadUnavailable
    : !files.length
      ? messages.selectMediaFirst
      : !title.trim()
        ? messages.enterTitle
        : shouldWarnModelDownloadRequired
          ? messages.pendingEnvHint
          : messages.readyToCreate;
  const creationState = remoteCreationUnavailable
    ? "error"
    : !files.length || !title.trim()
      ? "pending"
      : shouldWarnModelDownloadRequired
        ? "warning"
        : "ready";

  useEffect(() => {
    if (!hotwordsText.trim() || hotwordsText === previousDefaultHotwords) {
      setHotwordsText(store.settings.defaultHotwords);
    }
    setPreviousDefaultHotwords(store.settings.defaultHotwords);
  }, [store.settings.defaultHotwords]);

  function inferKind(fileName: string): "audio" | "video" {
    return /\.(mp4|mov|mkv)$/i.test(fileName) ? "video" : "audio";
  }

  function humanSize(size?: number) {
    if (!size) {
      return commonMessages.unknownSize;
    }

    const mb = size / 1024 / 1024;
    return `${mb.toFixed(1)} MB`;
  }

  function fileNameToTitle(fileName: string) {
    return fileName.replace(/\.[^.]+$/, "").trim();
  }

  function fileToSource(file: File): MeetingSourceFile {
    return {
      id: crypto.randomUUID(),
      name: file.name,
      sizeLabel: humanSize(file.size),
      kind: inferKind(file.name),
    };
  }

  function addFiles(next: MeetingSourceFile[]) {
    const lastFile = next.at(-1);

    if (isLocalMode) {
      setFiles((current) => (lastFile ? [lastFile] : current));
      if (!title.trim() && lastFile) {
        setTitle(fileNameToTitle(lastFile.name));
      }
      return;
    }

    setFiles((current) => [...current, ...next]);

    if (!title.trim() && lastFile) {
      setTitle(fileNameToTitle(lastFile.name));
    }
  }

  async function pickFiles() {
    if (remoteCreationUnavailable) {
      return;
    }
    try {
      const selected = await open({
        multiple: !isLocalMode,
        directory: false,
        filters: [
          {
            name: "Meeting Media",
            extensions: ["m4a", "mp3", "wav", "aac", "flac", "mp4", "mov", "mkv"],
          },
        ],
      });

      if (!selected) {
        return;
      }

      const normalized = Array.isArray(selected) ? selected : [selected];

      addFiles(
        normalized.map((path) => {
          const name = path.split("/").pop() ?? path;

          return {
            id: crypto.randomUUID(),
            name,
            path,
            sizeLabel: commonMessages.localPath,
            kind: inferKind(name),
          } satisfies MeetingSourceFile;
        }),
      );
    } catch {
      fileInput.current?.click();
    }
  }

  function onNativeFileChange(event: React.ChangeEvent<HTMLInputElement>) {
    const selected = Array.from(event.target.files ?? []).map(fileToSource);
    addFiles(selected);
    event.target.value = "";
  }

  function removeFile(id: string) {
    setFiles((current) => current.filter((file) => file.id !== id));
  }

  async function submit() {
    setSubmitError("");

    if (!files.length || !title.trim()) {
      return;
    }

    if (remoteCreationUnavailable) {
      setSubmitError(messages.remoteUploadUnavailable);
      return;
    }

    if (shouldWarnModelDownloadRequired) {
      await message(commonMessages.modelUnavailableMessage, {
        title: commonMessages.modelUnavailableTitle,
        kind: "warning",
      });
      return;
    }

    if (isLocalMode && files.some((file) => !file.path)) {
      setSubmitError(messages.localPathRequired);
      return;
    }

    setIsSubmitting(true);

    try {
      const job = await store.createJob({
        title: title.trim(),
        files,
        hotwords: hotwordsText
          .split(",")
          .map((item) => item.trim())
          .filter(Boolean),
        lang,
        enableSpeaker,
        summaryTemplate,
      });

      await router.push(jobDetailPath(jobRef(job)));
    } catch (error) {
      setSubmitError(error instanceof Error ? error.message : messages.createFailed);
    } finally {
      setIsSubmitting(false);
    }
  }

  return (
    <section className="new-job-native-page new-job-builder">
      <input
        ref={fileInput}
        type="file"
        accept=".m4a,.mp3,.wav,.aac,.flac,.mp4,.mov,.mkv"
        multiple={!isLocalMode}
        hidden
        onChange={onNativeFileChange}
      />

      <header className="new-job-builder-header">
        <div className="new-job-builder-title">
          <h2>{messages.heroTitle}</h2>
          <p>{messages.heroCopy}</p>
        </div>
        <div className="new-job-builder-mode" aria-label={`${messages.currentStatus}: ${modeLabel}`}>
          <IconServerStroked aria-hidden="true" />
          <span>{modeLabel}</span>
        </div>
      </header>

      <form
        className="new-job-builder-grid"
        noValidate
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
      >
        <section className="new-job-builder-source" aria-labelledby="new-job-source-title">
          <header className="new-job-builder-section-head">
            <span className="new-job-builder-step" aria-hidden="true">01</span>
            <div>
              <h3 id="new-job-source-title">{messages.inputFiles}</h3>
              <p id="new-job-file-rule">{isLocalMode ? messages.localFileRule : messages.remoteFileRule}</p>
            </div>
          </header>

          {!files.length ? (
            <button
              className="new-job-builder-picker"
              type="button"
              disabled={remoteCreationUnavailable}
              aria-describedby="new-job-file-rule"
              onClick={() => void pickFiles()}
            >
              <span className="new-job-builder-picker-icon" aria-hidden="true">
                <IconFolderOpenStroked />
              </span>
              <span className="new-job-builder-picker-copy">
                <strong>{messages.addFiles}</strong>
                <span>{isLocalMode ? messages.desktopFilePicker : messages.mediaSupported}</span>
              </span>
              <span className="new-job-builder-formats">{messages.supportedFormats}</span>
            </button>
          ) : (
            <section className="new-job-builder-selection" aria-label={formatMessage(messages.selectedFiles, { count: files.length })}>
              <header className="new-job-builder-selection-head">
                <div>
                  <strong>{formatMessage(messages.selectedFiles, { count: files.length })}</strong>
                  <span>{isLocalMode ? messages.localFileRule : messages.remoteFileRule}</span>
                </div>
                <div className="new-job-builder-selection-actions">
                  <Button
                    disabled={remoteCreationUnavailable}
                    icon={<IconPlusStroked />}
                    onClick={() => void pickFiles()}
                    variant="text"
                  >
                    {isLocalMode ? messages.reselect : messages.continueAdding}
                  </Button>
                  <Button icon={<IconDeleteStroked />} onClick={() => setFiles([])} variant="danger">
                    {messages.clearList}
                  </Button>
                </div>
              </header>

              <div className="new-job-builder-file-list">
                {files.map((file) => {
                  const fileTypeLabel = file.kind === "audio" ? commonMessages.audio : commonMessages.video;
                  const removeLabel = formatMessage(messages.removeFile, { name: file.name });

                  return (
                    <div key={file.id} className="new-job-builder-file-row" data-kind={file.kind}>
                      <span className="new-job-builder-file-icon" aria-hidden="true">
                        {file.kind === "audio" ? <IconMusicNoteStroked /> : <IconVideoStroked />}
                      </span>
                      <div className="new-job-builder-file-copy">
                        <strong title={file.name}>{file.name}</strong>
                        <div>
                          <SemiTag color={file.kind === "audio" ? "blue" : "purple"} size="small" type="light">
                            {fileTypeLabel}
                          </SemiTag>
                          <span>{file.sizeLabel}</span>
                        </div>
                      </div>
                      <Button
                        aria-label={removeLabel}
                        className="new-job-builder-remove-file"
                        icon={<IconDeleteStroked />}
                        onClick={() => removeFile(file.id)}
                        title={removeLabel}
                        variant="danger"
                      />
                    </div>
                  );
                })}
              </div>
            </section>
          )}

          <footer className="new-job-builder-source-note">
            <IconLightningStroked aria-hidden="true" />
            <span>{messages.mediaSupported}</span>
          </footer>
        </section>

        <aside className="new-job-builder-config" aria-labelledby="new-job-config-title">
          <div className="new-job-builder-config-body">
            <header className="new-job-builder-section-head new-job-builder-config-head">
              <span className="new-job-builder-step" aria-hidden="true">02</span>
              <div>
                <h3 id="new-job-config-title">{messages.basicInfo}</h3>
                <p>{currentLanguageLabel} · {messages.speaker} {enableSpeaker ? commonMessages.enabled : commonMessages.disabled}</p>
              </div>
            </header>

            <div className="new-job-builder-field">
              <label htmlFor="job-title">
                {messages.jobTitle}
                <span aria-hidden="true">*</span>
              </label>
              <TextInput
                id="job-title"
                aria-describedby="new-job-readiness"
                aria-required="true"
                className="new-job-builder-title-input"
                composition
                placeholder={messages.titlePlaceholder}
                required
                value={title}
                onChange={(value) => setTitle(value)}
              />
            </div>

            <section className="new-job-builder-settings" aria-labelledby="new-job-settings-title">
              <header>
                <IconSettingStroked aria-hidden="true" />
                <h4 id="new-job-settings-title">{messages.advancedSettings}</h4>
              </header>

              <div className="new-job-builder-option">
                <div className="new-job-builder-option-copy">
                  <IconGlobeStroked aria-hidden="true" />
                  <div>
                    <strong id="new-job-language-label">{messages.language}</strong>
                    <span id="new-job-language-hint">{messages.languageHint}</span>
                  </div>
                </div>
                <SemiSelect<string>
                  id="job-lang"
                  aria-describedby="new-job-language-hint"
                  aria-labelledby="new-job-language-label"
                  className="new-job-builder-language"
                  optionList={[
                    { label: messages.langZh, value: "zh-CN" },
                    { label: messages.langEn, value: "en-US" },
                    { label: messages.langJa, value: "ja-JP" },
                  ]}
                  value={lang}
                  onChange={(value) => {
                    if (typeof value === "string") {
                      setLang(value);
                    }
                  }}
                />
              </div>

              <div className="new-job-builder-option">
                <div className="new-job-builder-option-copy">
                  <IconMicrophoneStroked aria-hidden="true" />
                  <div>
                    <strong>{messages.speaker}</strong>
                    <span id="new-job-speaker-hint">{messages.speakerHint}</span>
                  </div>
                </div>
                <Switch
                  id="job-speaker"
                  aria-describedby="new-job-speaker-hint"
                  checked={enableSpeaker}
                  label={(
                    <>
                      <span className="new-job-builder-sr-only">{messages.speaker}: </span>
                      {enableSpeaker ? commonMessages.enabled : commonMessages.disabled}
                    </>
                  )}
                  onChange={setEnableSpeaker}
                  wrapperClassName="new-job-builder-speaker-switch"
                />
              </div>

              <div className="new-job-builder-option new-job-builder-option-readonly">
                <div className="new-job-builder-option-copy">
                  <IconTextStroked aria-hidden="true" />
                  <div>
                    <strong>{settingsMessages.defaultSummaryTemplate}</strong>
                    <span title={summaryTemplate}>{summaryTemplate}</span>
                  </div>
                </div>
              </div>

              <details className="new-job-builder-advanced">
                <summary>
                  <div className="new-job-builder-option-copy">
                    <IconSettingStroked aria-hidden="true" />
                    <div>
                      <strong>{messages.hotwords}</strong>
                      <span>{messages.hotwordsHint}</span>
                    </div>
                  </div>
                  <IconChevronDownStroked className="new-job-builder-advanced-chevron" aria-hidden="true" />
                </summary>
                <div className="new-job-builder-advanced-content">
                  <label htmlFor="job-hotwords">
                    {messages.hotwords}
                    <span>{commonMessages.optional}</span>
                  </label>
                  <SemiTextArea
                    id="job-hotwords"
                    aria-describedby="new-job-hotwords-hint"
                    autosize={{ minRows: 3, maxRows: 6 }}
                    className="new-job-builder-hotwords"
                    composition
                    placeholder={messages.hotwordsPlaceholder}
                    resize="none"
                    value={hotwordsText}
                    onChange={(value) => setHotwordsText(value)}
                  />
                  <span id="new-job-hotwords-hint">{messages.hotwordsHint}</span>
                </div>
              </details>
            </section>
          </div>

          <footer className="new-job-builder-submit">
            {(submitError || remoteCreationUnavailable) && (
              <div className="new-job-builder-error" role="alert" aria-live="assertive">
                {submitError || messages.remoteUploadUnavailable}
              </div>
            )}
            <div className="new-job-builder-readiness" id="new-job-readiness" data-state={creationState} role="status" aria-live="polite">
              {creationState === "ready" ? <IconTickCircle aria-hidden="true" /> : <IconLightningStroked aria-hidden="true" />}
              <span>{creationHint}</span>
            </div>
            <Button
              block
              className="new-job-builder-create"
              disabled={isSubmitting || remoteCreationUnavailable || !title.trim() || !files.length}
              icon={<IconLightningStroked />}
              loading={isSubmitting}
              title={remoteCreationUnavailable ? messages.remoteUploadUnavailable : undefined}
              type="submit"
              variant="primary"
            >
              {isSubmitting ? messages.creating : messages.createJob}
            </Button>
          </footer>
        </aside>
      </form>
    </section>
  );
}
