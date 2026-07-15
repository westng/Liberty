import { useEffect, useState } from "react";
import MeetingNotesPanel from "@/shared/components/MeetingNotesPanel";
import StatusBadge from "@/shared/components/StatusBadge";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { getMessages } from "@/shared/i18n";
import { createEmptyMeetingSummary } from "@/shared/services/ai/storage";
import { closeCurrentWindow } from "@/shared/services/tauri/window";
import type { ProcessingMode } from "@/shared/types/meeting";

export default function MeetingNotesView() {
  const meetingStore = useMeetingStore();
  const messages = getMessages(meetingStore.settings.locale).notes;
  const commonMessages = getMessages(meetingStore.settings.locale).common;
  const query = new URLSearchParams(window.location.search);
  const jobId = query.get("jobId")?.trim() ?? "";
  const windowScopeToken = query.get("scopeToken")?.trim() ?? "";
  const source = parseJobSource(query.get("source"));
  const job = meetingStore.getJobById(jobId, source ?? undefined);
  const [loadError, setLoadError] = useState("");

  useEffect(() => {
    if (!jobId || !source || !windowScopeToken) {
      setLoadError("capability_unavailable: 独立窗口需要有效的任务作用域。");
      return;
    }
    void meetingStore
      .refreshJobResult({ jobId, source, windowScopeToken })
      .then(() => setLoadError(""))
      .catch((error) => setLoadError(error instanceof Error ? error.message : String(error)));
  }, [jobId, source, windowScopeToken]);

  async function closeWindow() {
    await closeCurrentWindow();
  }

  return (
    <section className="summary-window-shell native-summary-window meeting-notes-window">
      <article className="surface native-window-hero summary-window-hero">
        <div className="job-title-line">
          <div>
            <h3>{job?.title || messages.windowTitle}</h3>
            <p className="section-copy">{messages.windowCopy}</p>
          </div>
          <div className="button-row">
            <StatusBadge status={job?.summaryStatus || "idle"} />
            <button className="secondary-button" type="button" onClick={closeWindow}>
              {commonMessages.closeWindow}
            </button>
          </div>
        </div>
      </article>

      <article className="surface summary-window-result meeting-notes-result">
        {loadError && <div className="note-block error-block">{loadError}</div>}
        <div className="section-heading summary-centered-heading">
          <h3>{messages.sectionTitle}</h3>
          <StatusBadge status={job?.summaryStatus || "idle"} />
        </div>

        <MeetingNotesPanel summary={job?.summary || createEmptyMeetingSummary(job?.title)} />
      </article>
    </section>
  );
}

function parseJobSource(value: string | null): ProcessingMode | null {
  return value === "local" || value === "remote" ? value : null;
}
