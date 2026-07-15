import { message, open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import { useRouter } from "@/app/router/RouterContext";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { formatMessage, getMessages } from "@/shared/i18n";
import type { MeetingSourceFile } from "@/shared/types/meeting";
import { jobDetailPath, jobRef } from "./jobRoutes";

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
  const messages = getMessages(store.settings.locale).newJob;
  const commonMessages = getMessages(store.settings.locale).common;
  const summaryTemplate = store.settings.summaryTemplate.trim() || messages.defaultSummaryTemplateName;
  const shouldWarnModelDownloadRequired = isLocalMode && !store.runtimeStatus.shellReady;
  const remoteCreationUnavailable = !isLocalMode && !store.canRemoteOperation("jobs.create");
  const currentLanguageLabel = lang === "en-US"
    ? messages.langEn
    : lang === "ja-JP"
      ? messages.langJa
      : messages.langZh;

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
    <section className="new-job-native-page">
      <input
        ref={fileInput}
        type="file"
        accept=".m4a,.mp3,.wav,.aac,.flac,.mp4,.mov,.mkv"
        multiple={!isLocalMode}
        hidden
        onChange={onNativeFileChange}
      />

      <div className="new-job-workspace">
        <main className="new-job-composer">
          <header className="new-job-page-head">
            <div>
              <h2>{messages.heroTitle}</h2>
              <p>{messages.heroCopy}</p>
            </div>
          </header>

          <article className="new-job-sheet">
            <div className="field new-job-title-field">
              <label htmlFor="job-title">{messages.jobTitle}</label>
              <input id="job-title" value={title} onChange={(event) => setTitle(event.target.value)} placeholder={messages.titlePlaceholder} />
            </div>

            <section className="new-job-section">
              <div className="new-job-section-head">
                <div>
                  <h3>{messages.inputFiles}</h3>
                  <p>{isLocalMode ? messages.desktopFilePicker : messages.mediaSupported}</p>
                </div>
              </div>

              <div className={`drop-zone new-job-file-box ${files.length ? "has-files" : ""}`}>
                {!files.length ? (
                  <button className="drop-zone-button" type="button" disabled={remoteCreationUnavailable} onClick={pickFiles}>
                    <div className="drop-zone-copy">
                      <strong>{messages.addFiles}</strong>
                      <p>{isLocalMode ? messages.desktopFilePicker : messages.mediaSupported}</p>
                    </div>
                  </button>
                ) : (
                  <>
                    <div className="new-job-file-box-head">
                      <span>{formatMessage(messages.selectedFiles, { count: files.length })}</span>
                      <div className="new-job-file-box-actions">
                        <button className="text-button" type="button" disabled={remoteCreationUnavailable} onClick={pickFiles}>
                          {isLocalMode ? messages.reselect : messages.continueAdding}
                        </button>
                        <button className="text-button danger-text" type="button" onClick={() => setFiles([])}>
                          {messages.clearList}
                        </button>
                      </div>
                    </div>
                    <div className="file-list new-job-file-list">
                      {files.map((file) => (
                        <div key={file.id} className="new-job-file-row">
                          <div>
                            <div className="new-job-file-name">{file.name}</div>
                            <div className="job-meta-line">
                              {file.kind === "audio" ? commonMessages.audio : commonMessages.video} · {file.sizeLabel}
                            </div>
                          </div>
                          <button className="text-button danger-text" type="button" onClick={() => removeFile(file.id)}>
                            {commonMessages.remove}
                          </button>
                        </div>
                      ))}
                    </div>
                  </>
                )}
              </div>
            </section>

            <details className="new-job-options-panel">
              <summary className="new-job-options-summary">
                <div>
                  <h3>{messages.advancedSettings}</h3>
                  <p>{currentLanguageLabel} · {messages.speaker} {enableSpeaker ? commonMessages.enabled : commonMessages.disabled} · {summaryTemplate}</p>
                </div>
              </summary>

              <div className="new-job-options-grid">
                <div className="new-job-option">
                  <div>
                    <strong>{messages.language}</strong>
                    <p>{messages.languageHint}</p>
                  </div>
                  <select id="job-lang" value={lang} onChange={(event) => setLang(event.target.value)}>
                    <option value="zh-CN">{messages.langZh}</option>
                    <option value="en-US">{messages.langEn}</option>
                    <option value="ja-JP">{messages.langJa}</option>
                  </select>
                </div>

                <div className="new-job-option">
                  <div>
                    <strong>{messages.speaker}</strong>
                    <p>{messages.speakerHint}</p>
                  </div>
                  <label>
                    <input checked={enableSpeaker} onChange={(event) => setEnableSpeaker(event.target.checked)} type="checkbox" />
                    <span>{enableSpeaker ? commonMessages.enabled : commonMessages.disabled}</span>
                  </label>
                </div>

                <div className="field new-job-hotwords-field">
                  <label htmlFor="job-hotwords">{messages.hotwords}</label>
                  <textarea id="job-hotwords" value={hotwordsText} onChange={(event) => setHotwordsText(event.target.value)} placeholder={messages.hotwordsPlaceholder} />
                </div>
              </div>
            </details>

            {(submitError || remoteCreationUnavailable) && (
              <div className="error-block new-job-error">
                {submitError || messages.remoteUploadUnavailable}
              </div>
            )}

            <footer className="new-job-submit-bar">
              <button
                className="primary-button new-job-create-button"
                type="button"
                disabled={isSubmitting || remoteCreationUnavailable || !title.trim() || !files.length}
                title={remoteCreationUnavailable ? messages.remoteUploadUnavailable : undefined}
                onClick={submit}
              >
                {isSubmitting ? messages.creating : messages.createJob}
              </button>
            </footer>
          </article>
        </main>
      </div>
    </section>
  );
}
