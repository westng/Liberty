import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect } from "react";
import MeetingNotesPanel from "@/shared/components/MeetingNotesPanel";
import StatusBadge from "@/shared/components/StatusBadge";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { getMessages } from "@/shared/i18n";
import { createEmptyMeetingSummary } from "@/shared/services/ai/storage";

export default function MeetingNotesView() {
  const meetingStore = useMeetingStore();
  const messages = getMessages(meetingStore.settings.locale).notes;
  const commonMessages = getMessages(meetingStore.settings.locale).common;
  const jobId = new URLSearchParams(window.location.search).get("jobId") ?? "";
  const job = meetingStore.getJobById(jobId);

  useEffect(() => {
    if (jobId) {
      void meetingStore.refreshJob(jobId);
    }
  }, [jobId]);

  async function closeWindow() {
    await getCurrentWebviewWindow().close();
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
        <div className="section-heading summary-centered-heading">
          <h3>{messages.sectionTitle}</h3>
          <StatusBadge status={job?.summaryStatus || "idle"} />
        </div>

        <MeetingNotesPanel summary={job?.summary || createEmptyMeetingSummary(job?.title)} />
      </article>
    </section>
  );
}
