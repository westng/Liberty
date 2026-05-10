import { invoke } from "@tauri-apps/api/core";
import type { MeetingMember, MeetingMemberImportResult } from "@/shared/types/meeting";

export function createLocalMembersService() {
  return {
    listMembers: () => invoke<MeetingMember[]>("list_meeting_members"),
    saveMember: (member: MeetingMember) => invoke<void>("save_meeting_member", { member }),
    deleteMember: (id: string) => invoke<void>("delete_meeting_member", { id }),
    importMembersExcel: (filePath: string) =>
      invoke<MeetingMemberImportResult>("import_meeting_members_excel", { filePath }),
    exportMembersExcel: (filePath: string) => invoke<void>("export_meeting_members_excel", { filePath }),
  };
}
