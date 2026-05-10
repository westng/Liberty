<script setup lang="ts">
import { confirm, message, open, save } from "@tauri-apps/plugin-dialog";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { formatMessage, getMessages } from "@/shared/i18n";
import { createLocalMembersService } from "@/shared/services/tauri/members";
import { openMemberEditorWindow } from "@/shared/services/ui/windows";
import type { MeetingMember } from "@/shared/types/meeting";

const meetingStore = useMeetingStore();
const membersService = createLocalMembersService();
const messages = computed(() => getMessages(meetingStore.settings.value.locale).members);
const commonMessages = computed(() => getMessages(meetingStore.settings.value.locale).common);

const members = ref<MeetingMember[]>([]);
const loading = ref(false);
const errorMessage = ref("");

const recorderMember = computed(() => members.value.find((item) => item.isRecorder) ?? null);
const sortedMembers = computed(() =>
  [...members.value].sort((left, right) => {
    if (left.sortOrder !== right.sortOrder) {
      return left.sortOrder - right.sortOrder;
    }

    const updatedAtDiff = right.updatedAt.localeCompare(left.updatedAt);
    if (updatedAtDiff !== 0) {
      return updatedAtDiff;
    }

    return left.name.localeCompare(right.name, meetingStore.settings.value.locale);
  }),
);

onMounted(() => {
  void loadMembers();
  window.addEventListener("focus", handleWindowFocus);
});

onBeforeUnmount(() => {
  window.removeEventListener("focus", handleWindowFocus);
});

function handleWindowFocus() {
  void loadMembers();
}

async function loadMembers() {
  loading.value = true;

  try {
    members.value = await membersService.listMembers();
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally {
    loading.value = false;
  }
}

async function startCreate() {
  await openMemberEditorWindow();
}

async function startEdit(member: MeetingMember) {
  await openMemberEditorWindow(member.id);
}

async function removeMember(member: MeetingMember) {
  const confirmed = await confirm(formatMessage(messages.value.deleteConfirm, { name: member.name }), {
    title: messages.value.deleteTitle,
    kind: "warning",
    okLabel: commonMessages.value.delete,
    cancelLabel: commonMessages.value.cancel,
  });

  if (!confirmed) {
    return;
  }

  try {
    await membersService.deleteMember(member.id);
    await loadMembers();
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  }
}

async function importMembers() {
  const selected = await open({
    multiple: false,
    filters: [{ name: "Excel", extensions: ["xlsx"] }],
  });

  if (!selected || Array.isArray(selected)) {
    return;
  }

  try {
    const result = await membersService.importMembersExcel(selected);
    await loadMembers();
    await message(
      formatMessage(messages.value.importSuccess, {
        created: result.created,
        updated: result.updated,
      }),
      { title: messages.value.title, kind: "info" },
    );
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  }
}

async function exportMembers() {
  const filePath = await save({
    defaultPath: "人员信息.xlsx",
    filters: [{ name: "Excel", extensions: ["xlsx"] }],
  });

  if (!filePath) {
    return;
  }

  try {
    await membersService.exportMembersExcel(filePath);
    await message(formatMessage(messages.value.exportSuccess, { path: filePath }), {
      title: messages.value.title,
      kind: "info",
    });
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  }
}
</script>

<template>
  <section class="view-stack model-page-stack" style="margin-top: 20px;">
    <article class="surface">
      <div class="section-heading">
        <div>
          <h3>{{ messages.title }}</h3>
          <p class="section-copy">{{ messages.copy }}</p>
        </div>
        <div class="button-row">
          <button class="secondary-button" type="button" @click="importMembers">
            {{ commonMessages.import }}
          </button>
          <button class="secondary-button" type="button" @click="exportMembers">
            {{ commonMessages.export }}
          </button>
          <button class="primary-button" type="button" @click="startCreate">
            {{ messages.add }}
          </button>
        </div>
      </div>
      <div class="summary-inline">
        <span>{{ messages.total }} {{ members.length }}</span>
        <span>{{ messages.recorder }} {{ recorderMember?.name ?? commonMessages.notSet }}</span>
      </div>
      <p class="section-copy">{{ messages.importHint }}</p>
    </article>

    <div class="section-heading model-management-header">
      <h3>{{ messages.listTitle }}</h3>
    </div>

    <article class="surface model-list-card">
      <div v-if="sortedMembers.length" class="model-list-rows">
        <article
          v-for="member in sortedMembers"
          :key="member.id"
          class="model-list-row"
          @click="startEdit(member)"
        >
          <div class="model-row-main member-row-main">
            <strong>{{ member.name }}</strong>
            <span>{{ member.department || messages.emptyDepartment }}</span>
          </div>

          <div class="model-row-side">
            <span class="record-meta">#{{ member.sortOrder }}</span>
            <span class="record-tag" :class="{ active: member.isRecorder }">
              {{ member.isRecorder ? messages.recorderTag : messages.normalTag }}
            </span>
            <button class="text-button" type="button" @click.stop="startEdit(member)">
              {{ commonMessages.edit }}
            </button>
            <button class="text-button danger-text" type="button" @click.stop="removeMember(member)">
              {{ commonMessages.delete }}
            </button>
          </div>
        </article>
      </div>

      <div v-else class="empty-state">
        {{ loading ? commonMessages.noData : messages.empty }}
      </div>
    </article>
  </section>
</template>
