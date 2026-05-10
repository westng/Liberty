<script setup lang="ts">
import { confirm } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { computed, onMounted, reactive, ref } from "vue";
import { useRoute } from "vue-router";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { formatMessage, getMessages } from "@/shared/i18n";
import { createLocalMembersService } from "@/shared/services/tauri/members";
import type { MeetingMember } from "@/shared/types/meeting";

const route = useRoute();
const meetingStore = useMeetingStore();
const membersService = createLocalMembersService();

const selectedId = ref<string | null>(null);
const members = ref<MeetingMember[]>([]);
const errorMessage = ref("");
const draft = reactive(createDraft());

const messages = computed(() => getMessages(meetingStore.settings.value.locale).members);
const commonMessages = computed(() => getMessages(meetingStore.settings.value.locale).common);

onMounted(async () => {
  await meetingStore.ensureSettingsLoaded();
  await loadMembers();
  const memberId = typeof route.query.id === "string" ? route.query.id : null;

  if (memberId) {
    const member = members.value.find((item) => item.id === memberId);
    if (member) {
      selectedId.value = member.id;
      Object.assign(draft, createDraft(member));
      await syncWindowTitle(true);
      return;
    }
  }

  await syncWindowTitle(false);
});

function createDraft(member?: MeetingMember) {
  return {
    id: member?.id ?? crypto.randomUUID(),
    name: member?.name ?? "",
    department: member?.department ?? "",
    sortOrder: member?.sortOrder ?? 0,
    isRecorder: member?.isRecorder ?? false,
    createdAt: member?.createdAt ?? new Date().toISOString(),
    updatedAt: member?.updatedAt ?? new Date().toISOString(),
  };
}

async function loadMembers() {
  members.value = await membersService.listMembers();
}

function validateDraft() {
  if (!draft.name.trim()) {
    return messages.value.validationName;
  }

  if (!Number.isInteger(Number(draft.sortOrder))) {
    return messages.value.validationSortOrder;
  }

  return "";
}

async function syncWindowTitle(isEdit: boolean) {
  try {
    await getCurrentWindow().setTitle(isEdit ? messages.value.editorEditTitle : messages.value.editorNewTitle);
  } catch {
    // ignore
  }
}

async function saveMember() {
  const validation = validateDraft();
  if (validation) {
    errorMessage.value = validation;
    return;
  }

  const now = new Date().toISOString();
  const nextMember: MeetingMember = {
    id: draft.id,
    name: draft.name.trim(),
    department: draft.department.trim(),
    sortOrder: Number(draft.sortOrder),
    isRecorder: draft.isRecorder,
    createdAt: draft.createdAt,
    updatedAt: now,
  };

  await membersService.saveMember(nextMember);
  selectedId.value = nextMember.id;
  Object.assign(draft, { ...nextMember });
  errorMessage.value = "";
  await syncWindowTitle(true);
}

async function removeMember() {
  if (!selectedId.value) {
    return;
  }

  const confirmed = await confirm(formatMessage(messages.value.deleteConfirm, { name: draft.name.trim() }), {
    title: messages.value.deleteTitle,
    kind: "warning",
    okLabel: commonMessages.value.delete,
    cancelLabel: commonMessages.value.cancel,
  });
  if (!confirmed) {
    return;
  }

  await membersService.deleteMember(selectedId.value);
  selectedId.value = null;
  Object.assign(draft, createDraft());
  errorMessage.value = "";
  await syncWindowTitle(false);
}

function resetDraft() {
  selectedId.value = null;
  Object.assign(draft, createDraft());
  errorMessage.value = "";
  void syncWindowTitle(false);
}
</script>

<template>
  <section class="editor-window-shell">
    <article class="surface editor-window-card">
      <div class="section-heading">
        <h3>{{ selectedId ? messages.editorEditTitle : messages.editorNewTitle }}</h3>
      </div>

      <div class="field-grid">
        <div class="field">
          <label for="member-name">{{ messages.name }}</label>
          <input id="member-name" v-model="draft.name" :placeholder="messages.namePlaceholder" />
        </div>

        <div class="field">
          <label for="member-department">{{ messages.department }}</label>
          <input id="member-department" v-model="draft.department" :placeholder="messages.departmentPlaceholder" />
        </div>

        <div class="field">
          <label for="member-sort-order">{{ messages.sortOrder }}</label>
          <input
            id="member-sort-order"
            v-model="draft.sortOrder"
            type="number"
            step="1"
            :placeholder="messages.sortOrderPlaceholder"
          />
        </div>

        <div class="field-grid two-col">
          <label class="toggle-field">
            <input v-model="draft.isRecorder" type="checkbox" />
            <span>{{ messages.recorderSwitch }}</span>
          </label>
        </div>
      </div>

      <div v-if="errorMessage" class="note-block error-block">
        {{ errorMessage }}
      </div>

      <div class="button-row">
        <button class="primary-button" type="button" @click="saveMember">
          {{ messages.save }}
        </button>
        <button class="secondary-button" type="button" @click="resetDraft">
          {{ messages.reset }}
        </button>
        <button v-if="selectedId" class="text-button danger-text" type="button" @click="removeMember">
          {{ commonMessages.delete }}
        </button>
      </div>
    </article>
  </section>
</template>
