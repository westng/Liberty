import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { getMessages } from "@/shared/i18n";
import type { MeetingSummary } from "@/shared/types/meeting";

type MeetingNotesPanelProps = {
  summary: MeetingSummary;
};

export default function MeetingNotesPanel({ summary }: MeetingNotesPanelProps) {
  const store = useMeetingStore();
  const messages = getMessages(store.settings.locale).notes;

  return (
    <div className="notes-list">
      <article className="note-block">
        <div className="notes-head">
          <h4>{messages.summary}</h4>
        </div>
        <div className="note-content">{summary.overview || messages.emptySummary}</div>
      </article>

      {summary.topics.length > 0 && (
        <article className="note-block">
          <div className="notes-head">
            <h4>{messages.topics}</h4>
          </div>
          <ul className="note-list">
            {summary.topics.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
        </article>
      )}

      {summary.decisions.length > 0 && (
        <article className="note-block">
          <div className="notes-head">
            <h4>{messages.decisions}</h4>
          </div>
          <ul className="note-list">
            {summary.decisions.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
        </article>
      )}

      {summary.actionItems.length > 0 && (
        <article className="note-block">
          <div className="notes-head">
            <h4>{messages.actionItems}</h4>
          </div>
          <ul className="note-list">
            {summary.actionItems.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
        </article>
      )}

      {summary.risks?.length ? (
        <article className="note-block">
          <div className="notes-head">
            <h4>{messages.risks}</h4>
          </div>
          <ul className="note-list">
            {summary.risks.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
        </article>
      ) : null}

      {summary.followUps?.length ? (
        <article className="note-block">
          <div className="notes-head">
            <h4>{messages.followUps}</h4>
          </div>
          <ul className="note-list">
            {summary.followUps.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
        </article>
      ) : null}
    </div>
  );
}
