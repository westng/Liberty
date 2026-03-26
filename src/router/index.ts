import { createRouter, createWebHistory } from "vue-router";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: "/",
      name: "new-job",
      component: () => import("@/views/NewJobView.vue"),
      meta: { title: "新建会议任务", titleKey: "newJob" },
    },
    {
      path: "/jobs",
      name: "jobs",
      component: () => import("@/views/JobsView.vue"),
      meta: { title: "任务列表", titleKey: "jobs" },
    },
    {
      path: "/jobs/:id",
      name: "job-detail",
      component: () => import("@/views/JobDetailView.vue"),
      meta: { title: "任务详情", titleKey: "jobDetail" },
    },
    {
      path: "/jobs/:id/workbench",
      name: "workbench",
      component: () => import("@/views/WorkbenchView.vue"),
      meta: { title: "结果工作台", titleKey: "workbench" },
    },
    {
      path: "/models",
      name: "models",
      component: () => import("@/views/ModelManagementView.vue"),
      meta: { title: "模型管理", titleKey: "models" },
    },
    {
      path: "/templates",
      name: "templates",
      component: () => import("@/views/TemplateManagementView.vue"),
      meta: { title: "模板管理", titleKey: "templates" },
    },
    {
      path: "/ai-summary",
      name: "ai-summary",
      component: () => import("@/views/AiSummaryView.vue"),
      meta: { title: "AI 总结", titleKey: "aiSummary", standalone: true },
    },
    {
      path: "/meeting-notes",
      name: "meeting-notes",
      component: () => import("@/views/MeetingNotesView.vue"),
      meta: { title: "会议纪要", standalone: true },
    },
    {
      path: "/model-editor",
      name: "model-editor",
      component: () => import("@/views/ModelEditorView.vue"),
      meta: { title: "模型编辑", standalone: true },
    },
    {
      path: "/template-editor",
      name: "template-editor",
      component: () => import("@/views/TemplateEditorView.vue"),
      meta: { title: "模板编辑", standalone: true },
    },
    {
      path: "/settings",
      name: "settings",
      component: () => import("@/views/SettingsView.vue"),
      meta: { title: "系统设置", titleKey: "settings" },
    },
  ],
});

export default router;
