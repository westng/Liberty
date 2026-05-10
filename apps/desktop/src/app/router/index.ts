import { createRouter, createWebHistory } from "vue-router";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: "/",
      name: "new-job",
      component: () => import("@/features/jobs/views/NewJobView.vue"),
      meta: { title: "新建会议任务", titleKey: "newJob" },
    },
    {
      path: "/jobs",
      name: "jobs",
      component: () => import("@/features/jobs/views/JobsView.vue"),
      meta: { title: "任务列表", titleKey: "jobs" },
    },
    {
      path: "/jobs/:id",
      name: "job-detail",
      component: () => import("@/features/jobs/views/JobDetailView.vue"),
      meta: { title: "任务详情", titleKey: "jobDetail" },
    },
    {
      path: "/jobs/:id/workbench",
      name: "workbench",
      component: () => import("@/features/jobs/views/WorkbenchView.vue"),
      meta: { title: "结果工作台", titleKey: "workbench" },
    },
    {
      path: "/models",
      name: "models",
      component: () => import("@/features/models/views/ModelManagementView.vue"),
      meta: { title: "模型管理", titleKey: "models" },
    },
    {
      path: "/templates",
      name: "templates",
      component: () => import("@/features/templates/views/TemplateManagementView.vue"),
      meta: { title: "模板管理", titleKey: "templates" },
    },
    {
      path: "/members",
      name: "members",
      component: () => import("@/features/members/views/MemberManagementView.vue"),
      meta: { title: "人员管理", titleKey: "members" },
    },
    {
      path: "/pet",
      name: "pet",
      component: () => import("@/features/pet/views/PetManagementView.vue"),
      meta: { title: "宠物中心", titleKey: "pet" },
    },
    {
      path: "/ai-summary",
      name: "ai-summary",
      component: () => import("@/features/ai-summary/views/AiSummaryView.vue"),
      meta: { title: "AI 总结", titleKey: "aiSummary", standalone: true },
    },
    {
      path: "/meeting-notes",
      name: "meeting-notes",
      component: () => import("@/features/meeting-notes/views/MeetingNotesView.vue"),
      meta: { title: "会议纪要", standalone: true },
    },
    {
      path: "/model-editor",
      name: "model-editor",
      component: () => import("@/features/models/views/ModelEditorView.vue"),
      meta: { title: "模型编辑", standalone: true },
    },
    {
      path: "/template-editor",
      name: "template-editor",
      component: () => import("@/features/templates/views/TemplateEditorView.vue"),
      meta: { title: "模板编辑", standalone: true },
    },
    {
      path: "/settings",
      name: "settings",
      component: () => import("@/features/settings/views/SettingsView.vue"),
      meta: { title: "系统设置", titleKey: "settings" },
    },
    {
      path: "/member-editor",
      name: "member-editor",
      component: () => import("@/features/members/views/MemberEditorView.vue"),
      meta: { title: "人员编辑", standalone: true },
    },
    {
      path: "/pet-desktop",
      name: "pet-desktop",
      component: () => import("@/features/pet/views/PetDesktopView.vue"),
      meta: { title: "桌面宠物", standalone: true },
    },
  ],
});

export default router;
