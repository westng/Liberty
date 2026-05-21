import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { getMessages } from "@/shared/i18n";
import type { JobStage } from "@/shared/types/meeting";

type StatusBadgeProps = {
  status: JobStage;
  text?: string;
};

export default function StatusBadge({ status, text }: StatusBadgeProps) {
  const store = useMeetingStore();
  const labels = getMessages(store.settings.locale).status;
  const label = text ?? labels[status];

  return <span className={`status-badge status-${status}`}>{label}</span>;
}
