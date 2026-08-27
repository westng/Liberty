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

export function hasDisplayableLegacySpeakerSegments(job: MeetingJob): boolean {
  return job.diarizationStatus === "legacy_unverified"
    && job.speakerSegments.length > 0
    && job.speakerSegments.length === job.transcriptSegments.length
    && job.speakerSegments.every((segment, index) => {
      const transcriptSegment = job.transcriptSegments[index];

      return Boolean(segment.speaker?.trim())
        && transcriptSegment?.startMs === segment.startMs
        && transcriptSegment.endMs === segment.endMs
        && transcriptSegment.text === segment.text;
    });
}

export function getDisplayTranscriptSegments(job: MeetingJob): TranscriptSegment[] {
  if (hasVerifiedSpeakerSegments(job) || hasDisplayableLegacySpeakerSegments(job)) {
    return job.speakerSegments;
  }

  return job.transcriptSegments;
}
