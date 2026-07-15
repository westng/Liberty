import { invoke } from "@tauri-apps/api/core";
import { runAppStatusAction } from "@/shared/services/ui/statusNotifications";
import type { MeetingMember, MeetingMemberImportResult } from "@/shared/types/meeting";

export function createLocalMembersService() {
  return {
    listMembers: () => invoke<MeetingMember[]>("list_meeting_members"),
    saveMember: (member: MeetingMember) => runAppStatusAction(
      "saveMember",
      () => invoke<void>("save_meeting_member", { member }),
    ),
    deleteMember: (id: string) => runAppStatusAction(
      "deleteMember",
      () => invoke<void>("delete_meeting_member", { id }),
    ),
    importMembersExcel: (filePath: string) =>
      runAppStatusAction(
        "importMembers",
        () => invoke<MeetingMemberImportResult>("import_meeting_members_excel", { filePath }),
      ),
    exportMembersExcel: (filePath: string) => runAppStatusAction(
      "exportMembers",
      () => invoke<void>("export_meeting_members_excel", { filePath }),
    ),
  };
}
