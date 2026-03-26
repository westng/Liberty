import type { LocaleCode } from "@/types/meeting";

type MessageTree = {
  nav: {
    newJob: string;
    jobs: string;
    models: string;
    templates: string;
    settings: string;
  };
  routeTitles: Record<string, string>;
  shell: {
    slogan: string;
    modeLabel: string;
    processingLabel: string;
    back: string;
    forward: string;
    localReady: string;
    remoteReady: string;
    mockMode: string;
    localMode: string;
    remoteMode: string;
    mockModeShort: string;
    github: string;
  };
  settings: {
    pageTitle: string;
    appearance: string;
    appearanceHint: string;
    themeSection: string;
    themeMode: string;
    glassStyle: string;
    locale: string;
    accentColor: string;
    auto: string;
    light: string;
    dark: string;
    transparent: string;
    tinted: string;
    localeZh: string;
    localeEn: string;
    transparentHint: string;
    tintedHint: string;
    runtimeOverview: string;
    runtimeMode: string;
    runtimeModeHint: string;
    localDatabaseReady: string;
    waitingLocalConfig: string;
    localRuntime: string;
    pythonPath: string;
    pythonPathHint: string;
    localAsrDevice: string;
    localAsrDeviceHint: string;
    localAsrDeviceAuto: string;
    localAsrDeviceCpu: string;
    localAsrDeviceMps: string;
    localAsrDeviceCuda: string;
    processingDefaults: string;
    defaultHotwords: string;
    defaultHotwordsHint: string;
    defaultSummaryTemplate: string;
    defaultSummaryTemplateHint: string;
    concurrency: string;
    concurrencyHint: string;
    localAsrThreads: string;
    localAsrThreadsHint: string;
    localAsrBatchSizeSeconds: string;
    localAsrBatchSizeSecondsHint: string;
    remoteCompatibility: string;
    backendUrl: string;
    backendUrlHint: string;
    apiToken: string;
    apiTokenHint: string;
    save: string;
    copyright: string;
    authorGithub: string;
    colorLabels: Record<string, string>;
  };
};

const messages: Record<LocaleCode, MessageTree> = {
  "zh-CN": {
    nav: {
      newJob: "新建任务",
      jobs: "任务列表",
      models: "模型管理",
      templates: "模板管理",
      settings: "系统设置",
    },
    routeTitles: {
      newJob: "新建会议任务",
      jobs: "任务列表",
      jobDetail: "任务详情",
      workbench: "结果工作台",
      models: "模型管理",
      templates: "模板管理",
      aiSummary: "AI 总结",
      settings: "系统设置",
    },
    shell: {
      slogan: "一半烟火以谋生，一半诗意以谋爱。",
      modeLabel: "当前模式",
      processingLabel: "处理中",
      back: "返回上一页",
      forward: "前进到下一页",
      localReady: "本地 Python 已配置",
      remoteReady: "远端服务已配置",
      mockMode: "当前为本地 Mock 模式",
      localMode: "本地 FunASR",
      remoteMode: "远端服务",
      mockModeShort: "Mock",
      github: "项目 GitHub",
    },
    settings: {
      pageTitle: "系统设置",
      appearance: "外观",
      appearanceHint: "主题、玻璃效果、语言和强调色会直接作用到整个桌面应用。",
      themeSection: "主题",
      themeMode: "主题模式",
      glassStyle: "Liquid Glass",
      glassStyleHint: "选取喜欢的 Liquid Glass外观",
      locale: "界面语言",
      accentColor: "颜色",
      auto: "跟随系统",
      light: "浅色",
      dark: "深色",
      transparent: "透明",
      tinted: "色调",
      localeZh: "简体中文",
      localeEn: "English",
      transparentHint: "保留更强的透明感和背景层次。",
      tintedHint: "提高面板对比度，信息会更稳定。",
      runtimeOverview: "运行概览",
      runtimeMode: "当前模式",
      runtimeModeHint: "Liberty 会按本地运行配置优先级决定桌面端实际走哪条处理链路。",
      localDatabaseReady: "本地数据库已启用",
      waitingLocalConfig: "等待本地配置",
      localRuntime: "本地运行",
      pythonPath: "Python 可执行文件路径",
      pythonPathHint: "桌面端本地转写和重试任务都会直接调用这里的 Python 环境，Runner 脚本使用项目内置入口。",
      localAsrDevice: "推理设备",
      localAsrDeviceHint: "自动模式会优先尝试 CUDA、Apple Silicon，再回退到 CPU。",
      localAsrDeviceAuto: "自动",
      localAsrDeviceCpu: "CPU",
      localAsrDeviceMps: "Apple Silicon",
      localAsrDeviceCuda: "CUDA",
      processingDefaults: "处理默认值",
      defaultHotwords: "默认热词",
      defaultHotwordsHint: "建议只保留项目名、专有名词和行业术语，降低普通词带来的噪声。",
      defaultSummaryTemplate: "默认纪要模板",
      defaultSummaryTemplateHint: "新建会议任务时的默认模板名称，只影响初始填充值。",
      concurrency: "并发上传数",
      concurrencyHint: "用于控制桌面端任务处理的默认并发度，建议保持在 1 到 8 之间。",
      localAsrThreads: "本地线程数",
      localAsrThreadsHint: "0 表示自动，建议先保持自动；若 CPU 占用过低再手动提高。",
      localAsrBatchSizeSeconds: "批处理时长",
      localAsrBatchSizeSecondsHint: "单位为秒，数值越大通常越快，但会占用更多内存，默认 300 秒。",
      remoteCompatibility: "远端兼容",
      backendUrl: "服务地址",
      backendUrlHint: "保留给远端接口兼容模式使用，本地链路已完整配置时不会优先走这里。",
      apiToken: "API Token",
      apiTokenHint: "仅在远端服务需要鉴权时填写，不影响本地 Python 与本地数据库流程。",
      save: "保存设置",
      copyright: "版权所有 © 2026 Liberty. 保留所有权利。",
      authorGithub: "作者 west · GitHub @westng",
      colorLabels: {
        "#8f96a3": "雾灰",
        "#2f6dff": "海蓝",
        "#a65dd9": "莓紫",
        "#f062a8": "桃粉",
        "#ff6a57": "珊瑚",
        "#ffb020": "琥珀",
        "#f5dd00": "亮黄",
        "#33c96f": "青绿",
      },
    },
  },
  "en-US": {
    nav: {
      newJob: "New Job",
      jobs: "Jobs",
      models: "Models",
      templates: "Templates",
      settings: "Settings",
    },
    routeTitles: {
      newJob: "New Meeting Job",
      jobs: "Jobs",
      jobDetail: "Job Detail",
      workbench: "Workbench",
      models: "Model Management",
      templates: "Template Management",
      aiSummary: "AI Summary",
      settings: "Settings",
    },
    shell: {
      slogan: "Half for making a living, half for keeping poetry alive.",
      modeLabel: "Mode",
      processingLabel: "Processing",
      back: "Go Back",
      forward: "Go Forward",
      localReady: "Local Python configured",
      remoteReady: "Remote service configured",
      mockMode: "Running in local mock mode",
      localMode: "Local FunASR",
      remoteMode: "Remote Service",
      mockModeShort: "Mock",
      github: "Project GitHub",
    },
    settings: {
      pageTitle: "Settings",
      appearance: "Appearance",
      appearanceHint: "Theme, glass style, language, and accent color apply across the desktop app.",
      themeSection: "Theme",
      themeMode: "Theme Mode",
      glassStyle: "Liquid Glass",
      glassStyleHint: "Choose your preferred Liquid Glass style",
      locale: "Language",
      accentColor: "Color",
      auto: "Auto",
      light: "Light",
      dark: "Dark",
      transparent: "Transparent",
      tinted: "Tinted",
      localeZh: "简体中文",
      localeEn: "English",
      transparentHint: "Keeps stronger transparency and more visible background layers.",
      tintedHint: "Improves panel contrast for a steadier reading surface.",
      runtimeOverview: "Runtime Overview",
      runtimeMode: "Current Mode",
      runtimeModeHint: "Liberty chooses the active desktop processing path based on local runtime priority.",
      localDatabaseReady: "Local database enabled",
      waitingLocalConfig: "Waiting for local runtime setup",
      localRuntime: "Local Runtime",
      pythonPath: "Python Executable Path",
      pythonPathHint: "Local transcription and retry both call this Python runtime directly. The runner script uses the built-in project entry.",
      localAsrDevice: "Inference Device",
      localAsrDeviceHint: "Auto prefers CUDA first, then Apple Silicon, and falls back to CPU when needed.",
      localAsrDeviceAuto: "Auto",
      localAsrDeviceCpu: "CPU",
      localAsrDeviceMps: "Apple Silicon",
      localAsrDeviceCuda: "CUDA",
      processingDefaults: "Processing Defaults",
      defaultHotwords: "Default Hotwords",
      defaultHotwordsHint: "Keep project names, proper nouns, and domain terms only to reduce noise.",
      defaultSummaryTemplate: "Default Summary Template",
      defaultSummaryTemplateHint: "Used as the initial template name when creating a new meeting job.",
      concurrency: "Concurrent Uploads",
      concurrencyHint: "Controls the default desktop processing concurrency. Keep it between 1 and 8.",
      localAsrThreads: "Local Threads",
      localAsrThreadsHint: "Use 0 for automatic selection. Increase manually only if CPU usage stays too low.",
      localAsrBatchSizeSeconds: "Batch Window",
      localAsrBatchSizeSecondsHint: "Measured in seconds. Larger values are usually faster but use more memory. Default is 300.",
      remoteCompatibility: "Remote Compatibility",
      backendUrl: "Service URL",
      backendUrlHint: "Reserved for remote compatibility mode. It is ignored when local runtime is fully configured.",
      apiToken: "API Token",
      apiTokenHint: "Only needed for authenticated remote services. It does not affect local Python or SQLite flows.",
      save: "Save Settings",
      copyright: "Copyright © 2026 Liberty. All rights reserved.",
      authorGithub: "Author west · GitHub @westng",
      colorLabels: {
        "#8f96a3": "Mist Gray",
        "#2f6dff": "Ocean Blue",
        "#a65dd9": "Berry Violet",
        "#f062a8": "Peach Pink",
        "#ff6a57": "Coral",
        "#ffb020": "Amber",
        "#f5dd00": "Bright Yellow",
        "#33c96f": "Mint Green",
      },
    },
  },
};

export function getMessages(locale: LocaleCode): MessageTree {
  return messages[locale] ?? messages["zh-CN"];
}
