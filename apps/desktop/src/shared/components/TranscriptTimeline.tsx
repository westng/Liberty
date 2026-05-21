import { useMemo, useState } from "react";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { getMessages } from "@/shared/i18n";
import type { TranscriptSegment } from "@/shared/types/meeting";

type TranscriptTimelineProps = {
  segments: TranscriptSegment[];
  query: string;
  busy?: boolean;
  onRenameSpeaker: (fromSpeaker: string, toSpeaker: string) => void;
};

export default function TranscriptTimeline({
  segments,
  query,
  busy = false,
  onRenameSpeaker,
}: TranscriptTimelineProps) {
  const store = useMeetingStore();
  const [editingSegmentId, setEditingSegmentId] = useState<string | null>(null);
  const [originalSpeaker, setOriginalSpeaker] = useState("");
  const [draftSpeaker, setDraftSpeaker] = useState("");
  const commonMessages = getMessages(store.settings.locale).common;
  const workbenchMessages = getMessages(store.settings.locale).workbench;
  const filteredSegments = useMemo(() => {
    const keyword = query.trim().toLowerCase();

    return segments.filter((segment) => {
      const body = `${segment.speaker ?? ""} ${segment.text}`.toLowerCase();
      return !keyword || body.includes(keyword);
    });
  }, [query, segments]);

  function formatClock(ms: number) {
    const date = new Date(ms);
    return date.toISOString().slice(14, 19);
  }

  function getSpeakerLabel(segment: TranscriptSegment) {
    return segment.speaker?.trim() || commonMessages.unknownSpeaker;
  }

  function startEdit(segment: TranscriptSegment) {
    setEditingSegmentId(segment.id);
    setOriginalSpeaker(segment.speaker?.trim() || "");
    setDraftSpeaker(segment.speaker?.trim() || "");
  }

  function cancelEdit() {
    setEditingSegmentId(null);
    setOriginalSpeaker("");
    setDraftSpeaker("");
  }

  function submitEdit() {
    const nextSpeaker = draftSpeaker.trim();

    if (!editingSegmentId || !nextSpeaker || busy) {
      return;
    }

    onRenameSpeaker(originalSpeaker, nextSpeaker);
    cancelEdit();
  }

  return (
    <div className="timeline">
      {filteredSegments.map((segment) => (
        <div key={segment.id} className="timeline-item">
          <div className="timeline-head">
            <div className="timeline-speaker-tools">
              {editingSegmentId === segment.id ? (
                <>
                  <input
                    value={draftSpeaker}
                    onChange={(event) => setDraftSpeaker(event.target.value)}
                    className="speaker-edit-input"
                    type="text"
                    placeholder={workbenchMessages.speakerInputPlaceholder}
                    disabled={busy}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") {
                        event.preventDefault();
                        submitEdit();
                      }
                      if (event.key === "Escape") {
                        event.preventDefault();
                        cancelEdit();
                      }
                    }}
                  />
                  <button className="text-button" type="button" disabled={busy} onClick={submitEdit}>
                    {commonMessages.save}
                  </button>
                  <button className="text-button" type="button" disabled={busy} onClick={cancelEdit}>
                    {commonMessages.cancel}
                  </button>
                </>
              ) : (
                <>
                  <span className="speaker-tag">{getSpeakerLabel(segment)}</span>
                  <button
                    className="text-button speaker-edit-button"
                    type="button"
                    disabled={busy}
                    onClick={() => startEdit(segment)}
                  >
                    {commonMessages.edit}
                  </button>
                </>
              )}
            </div>
            <span className="job-meta-line">
              {formatClock(segment.startMs)} - {formatClock(segment.endMs)}
            </span>
          </div>
          <div className="timeline-text">{segment.text}</div>
        </div>
      ))}

      {!filteredSegments.length && (
        <div className="empty-state">{workbenchMessages.emptyFilteredTranscript}</div>
      )}
    </div>
  );
}
