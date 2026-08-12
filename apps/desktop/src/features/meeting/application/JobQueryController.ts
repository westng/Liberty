import type { MeetingJobRef, ProcessingMode } from "@/shared/types/meeting";

type JobIdentity = Pick<MeetingJobRef, "jobId" | "source">;

export type JobRequestFence = JobIdentity & {
  key: string;
  sequence: number;
  sourceGeneration: number;
  writeGeneration: number;
};

export type JobMutationFence = JobIdentity & {
  key: string;
  sequence: number;
  sourceGeneration: number;
};

export function createJobQueryController() {
  let listSequence = 0;
  const requestSequences = new Map<string, number>();
  const mutationSequences = new Map<string, number>();
  const requestGenerations: Record<ProcessingMode, number> = { local: 0, remote: 0 };
  const writeGenerations: Record<ProcessingMode, number> = { local: 0, remote: 0 };

  const keyFor = (reference: JobIdentity) => JSON.stringify([reference.source, reference.jobId]);

  return {
    invalidateSource(source: ProcessingMode) {
      requestGenerations[source] += 1;
      writeGenerations[source] += 1;
      listSequence += 1;
    },
    beginList(source: ProcessingMode) {
      const sequence = ++listSequence;
      const requestGeneration = requestGenerations[source];
      const writeGeneration = writeGenerations[source];
      return () => sequence === listSequence
        && requestGeneration === requestGenerations[source]
        && writeGeneration === writeGenerations[source];
    },
    beginRequest(reference: JobIdentity): JobRequestFence {
      const key = keyFor(reference);
      const sequence = (requestSequences.get(key) ?? 0) + 1;
      requestSequences.set(key, sequence);
      return {
        ...reference,
        key,
        sequence,
        sourceGeneration: requestGenerations[reference.source],
        writeGeneration: writeGenerations[reference.source],
      };
    },
    isRequestCurrent(fence: JobRequestFence) {
      return requestSequences.get(fence.key) === fence.sequence
        && requestGenerations[fence.source] === fence.sourceGeneration
        && writeGenerations[fence.source] === fence.writeGeneration;
    },
    beginMutation(reference: JobIdentity): JobMutationFence {
      const key = keyFor(reference);
      const sequence = (mutationSequences.get(key) ?? 0) + 1;
      mutationSequences.set(key, sequence);
      writeGenerations[reference.source] += 1;
      return {
        ...reference,
        key,
        sequence,
        sourceGeneration: requestGenerations[reference.source],
      };
    },
    isMutationCurrent(fence: JobMutationFence) {
      return mutationSequences.get(fence.key) === fence.sequence
        && requestGenerations[fence.source] === fence.sourceGeneration;
    },
    commitMutation(fence: JobMutationFence) {
      if (
        mutationSequences.get(fence.key) !== fence.sequence
        || requestGenerations[fence.source] !== fence.sourceGeneration
      ) {
        return false;
      }
      writeGenerations[fence.source] += 1;
      return true;
    },
    sourceGeneration(source: ProcessingMode) {
      return requestGenerations[source];
    },
  };
}
