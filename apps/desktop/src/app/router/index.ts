import { lazy, type ComponentType, type LazyExoticComponent } from "react";

type RouteComponent = ComponentType | LazyExoticComponent<ComponentType>;

export type RouteConfig = {
  path: string;
  title: string;
  titleKey?: string;
  standalone?: boolean;
  component: RouteComponent;
};

export const routes: RouteConfig[] = [
  {
    path: "/",
    title: "新建会议任务",
    titleKey: "newJob",
    component: lazy(() => import("@/features/jobs/views/NewJobView")),
  },
  {
    path: "/jobs",
    title: "任务列表",
    titleKey: "jobs",
    component: lazy(() => import("@/features/jobs/views/JobsView")),
  },
  {
    path: "/jobs/:id",
    title: "任务详情",
    titleKey: "jobDetail",
    component: lazy(() => import("@/features/jobs/views/JobDetailView")),
  },
  {
    path: "/jobs/:id/workbench",
    title: "结果工作台",
    titleKey: "workbench",
    component: lazy(() => import("@/features/jobs/views/WorkbenchView")),
  },
  {
    path: "/models",
    title: "模型管理",
    titleKey: "models",
    component: lazy(() => import("@/features/models/views/ModelManagementView")),
  },
  {
    path: "/templates",
    title: "模板管理",
    titleKey: "templates",
    component: lazy(() => import("@/features/templates/views/TemplateManagementView")),
  },
  {
    path: "/members",
    title: "人员管理",
    titleKey: "members",
    component: lazy(() => import("@/features/members/views/MemberManagementView")),
  },
  {
    path: "/pet",
    title: "宠物中心",
    titleKey: "pet",
    component: lazy(() => import("@/features/pet/views/PetManagementView")),
  },
  {
    path: "/pet-store",
    title: "宠物商店",
    titleKey: "petStore",
    component: lazy(() => import("@/features/pet-store/views/PetStoreView")),
  },
  {
    path: "/pet-store-item",
    title: "商品详情",
    standalone: true,
    component: lazy(() => import("@/features/pet-store/views/PetStoreItemDetailView")),
  },
  {
    path: "/ai-summary",
    title: "AI 总结",
    standalone: true,
    component: lazy(() => import("@/features/ai-summary/views/AiSummaryView")),
  },
  {
    path: "/meeting-notes",
    title: "会议纪要",
    standalone: true,
    component: lazy(() => import("@/features/meeting-notes/views/MeetingNotesView")),
  },
  {
    path: "/model-editor",
    title: "模型编辑",
    standalone: true,
    component: lazy(() => import("@/features/models/views/ModelEditorView")),
  },
  {
    path: "/template-editor",
    title: "模板编辑",
    standalone: true,
    component: lazy(() => import("@/features/templates/views/TemplateEditorView")),
  },
  {
    path: "/settings",
    title: "系统设置",
    titleKey: "settings",
    component: lazy(() => import("@/features/settings/views/SettingsView")),
  },
  {
    path: "/member-editor",
    title: "人员编辑",
    standalone: true,
    component: lazy(() => import("@/features/members/views/MemberEditorView")),
  },
];

export function matchRoute(pathname: string) {
  for (const route of routes) {
    const params = matchPath(route.path, pathname);
    if (params) {
      return { route, params };
    }
  }

  return { route: routes[0], params: {} };
}

function matchPath(pattern: string, pathname: string) {
  const patternParts = pattern.split("/").filter(Boolean);
  const pathParts = pathname.split("/").filter(Boolean);

  if (patternParts.length !== pathParts.length) {
    return null;
  }

  const params: Record<string, string> = {};
  for (let index = 0; index < patternParts.length; index += 1) {
    const patternPart = patternParts[index];
    const pathPart = pathParts[index];
    if (patternPart.startsWith(":")) {
      params[patternPart.slice(1)] = decodeURIComponent(pathPart);
      continue;
    }
    if (patternPart !== pathPart) {
      return null;
    }
  }

  return params;
}
