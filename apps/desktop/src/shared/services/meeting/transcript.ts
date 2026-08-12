import type { MeetingJob, TranscriptSegment } from "@/shared/types/meeting";

export function getPrimaryTranscriptSegments(job: MeetingJob): TranscriptSegment[] {
  if (job.diarizationStatus === "completed" && job.speakerSegments.length > 0) {
    return job.speakerSegments;
  }

  return job.transcriptSegments;
}

export function hasVerifiedSpeakerSegments(job: MeetingJob): boolean {
  return job.diarizationStatus === "completed" && job.speakerSegments.length > 0;
}
