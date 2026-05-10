import type { MeetingJob, TranscriptSegment } from "@/shared/types/meeting";

export function getPrimaryTranscriptSegments(job: MeetingJob): TranscriptSegment[] {
  if (job.enableSpeaker && job.speakerSegments.length > 0) {
    return job.speakerSegments;
  }

  return job.transcriptSegments;
}
