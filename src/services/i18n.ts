import type { JobStage, LocaleCode } from "@/types/meeting";

type MessageTree = {
  nav: {
    newJob: string;
    jobs: string;
    models: string;
    templates: string;
    settings: string;
  };
  routeTitles: Record<string, string>;
  common: {
    appName: string;
    all: string;
    none: string;
    notSet: string;
    optional: string;
    unknownSpeaker: string;
    audio: string;
    video: string;
    localPath: string;
    unknownSize: string;
    enabled: string;
    disabled: string;
    edit: string;
    save: string;
    cancel: string;
    delete: string;
    remove: string;
    retry: string;
    open: string;
    closeWindow: string;
    choose: string;
    noData: string;
    dash: string;
  };
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
    jobCompletedTitle: string;
    jobCompletedBody: string;
  };
  settings: {
    pageTitle: string;
    appearance: string;
    appearanceHint: string;
    themeSection: string;
    themeMode: string;
    glassStyle: string;
    glassStyleHint: string;
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
    managedRuntime: string;
    managedRuntimeHint: string;
    runtimeStatus: string;
    runtimeStatusMissing: string;
    runtimeStatusInstalling: string;
    runtimeStatusReady: string;
    runtimeStatusFailed: string;
    runtimeStatusRepair: string;
    runtimeVersion: string;
    runtimePythonVersion: string;
    runtimeInstalledAt: string;
    runtimeInstallAction: string;
    runtimeReinstallAction: string;
    runtimeInstallProgress: string;
    runtimeInstallPreparing: string;
    runtimeInstallCompleted: string;
    runtimeInstallDownload: string;
    runtimeInstallVerify: string;
    runtimeInstallExtract: string;
    runtimeInstallResolvePython: string;
    runtimeInstallBootstrapPip: string;
    runtimeInstallUpgradePip: string;
    runtimeInstallPytorch: string;
    runtimeInstallDependencies: string;
    runtimeInstallValidate: string;
    runtimeInstallModels: string;
    runtimeInstallLog: string;
    runtimeInstallLogEmpty: string;
    runtimeDescriptionReady: string;
    runtimeDescriptionInstalling: string;
    runtimeDescriptionFailed: string;
    runtimeDescriptionMissing: string;
    manualPythonOverride: string;
    manualPythonOverrideHint: string;
    pythonPath: string;
    pythonPathHint: string;
    pythonPathPlaceholder: string;
    localAsrDevice: string;
    localAsrDeviceHint: string;
    localAsrDeviceAuto: string;
    localAsrDeviceCpu: string;
    localAsrDeviceMps: string;
    localAsrDeviceCuda: string;
    processingDefaults: string;
    defaultHotwords: string;
    defaultHotwordsHint: string;
    defaultHotwordsPlaceholder: string;
    defaultSummaryTemplate: string;
    defaultSummaryTemplateHint: string;
    defaultSummaryTemplatePlaceholder: string;
    concurrency: string;
    concurrencyHint: string;
    localAsrThreads: string;
    localAsrThreadsHint: string;
    localAsrBatchSizeSeconds: string;
    localAsrBatchSizeSecondsHint: string;
    remoteCompatibility: string;
    backendUrl: string;
    backendUrlHint: string;
    backendUrlPlaceholder: string;
    apiToken: string;
    apiTokenHint: string;
    apiTokenPlaceholder: string;
    save: string;
    copyright: string;
    authorGithub: string;
    colorLabels: Record<string, string>;
  };
  status: Record<JobStage, string>;
  newJob: {
    defaultSummaryTemplateName: string;
    localPython: string;
    remoteService: string;
    envMissing: string;
    localPathRequired: string;
    createFailed: string;
    heroTitle: string;
    heroCopy: string;
    viewJobs: string;
    allJobs: string;
    processing: string;
    failed: string;
    basicInfo: string;
    jobTitle: string;
    titlePlaceholder: string;
    inputFiles: string;
    addFiles: string;
    desktopFilePicker: string;
    mediaSupported: string;
    selectedFiles: string;
    reselect: string;
    continueAdding: string;
    clearList: string;
    createJob: string;
    creating: string;
    advancedSettings: string;
    language: string;
    langZh: string;
    langEn: string;
    langJa: string;
    languageHint: string;
    speaker: string;
    speakerHint: string;
    hotwords: string;
    hotwordsPlaceholder: string;
    hotwordsHint: string;
    currentStatus: string;
    fileRule: string;
    settings: string;
    jobs: string;
    recentJobs: string;
    viewAll: string;
    noTasks: string;
    localModeHint: string;
    remoteModeHint: string;
    pendingEnvHint: string;
    localFileRule: string;
    remoteFileRule: string;
    fileCount: string;
  };
  jobs: {
    deleteConfirm: string;
    pending: string;
    minutes: string;
    hours: string;
    hoursMinutes: string;
    notCompleted: string;
    processingWithHours: string;
    processingWithMinutes: string;
    pageTitle: string;
    pageCopy: string;
    total: string;
    processing: string;
    completed: string;
    queueTitle: string;
    queueCopy: string;
    colTask: string;
    colFileInfo: string;
    colProcessingTime: string;
    colCreatedAt: string;
    colStatus: string;
    colActions: string;
    filesCount: string;
    fileDuration: string;
    diarizationEnabled: string;
    transcriptOnly: string;
    processCompleted: string;
    processFailed: string;
    processRunning: string;
    details: string;
    workbench: string;
    deleteDisabled: string;
    deleteAction: string;
    deleting: string;
  };
  jobDetail: {
    progressSection: string;
    progressEngine: string;
    stageUploaded: string;
    stageAsr: string;
    stageSummary: string;
    stageOverall: string;
    inputFiles: string;
    hotwords: string;
    speaker: string;
    viewWorkbench: string;
    retryJob: string;
    filesSection: string;
    settingsSection: string;
    language: string;
    speakerDiarization: string;
    logSection: string;
    noLog: string;
    notFound: string;
  };
  workbench: {
    allSpeakers: string;
    renameSpeakerTitle: string;
    renameSpeakerConfirm: string;
    replace: string;
    heroCopy: string;
    aiSummary: string;
    viewNotes: string;
    exportBundle: string;
    exporting: string;
    exportTranscript: string;
    exportNotes: string;
    transcriptCount: string;
    summaryCount: string;
    currentTemplate: string;
    notGenerated: string;
    fileCount: string;
    durationMinutes: string;
    hotwords: string;
    notConfigured: string;
    notesReady: string;
    notesEmpty: string;
    activeResultReady: string;
    summaryEmpty: string;
    transcript: string;
    searchPlaceholder: string;
    speakerInputPlaceholder: string;
    emptyFilteredTranscript: string;
    notFound: string;
  };
  aiSummary: {
    activeLabelEmpty: string;
    activeLabelFailed: string;
    activeLabelRunning: string;
    activeLabelSaved: string;
    staleRunError: string;
    jobNotFound: string;
    modelMissing: string;
    templateMissing: string;
    transcriptMissing: string;
    requestFailed: string;
    emptyResponse: string;
    invalidApiJson: string;
    invalidStructuredJson: string;
    deleteConfirm: string;
    deleteTitle: string;
    heroTitle: string;
    heroCopy: string;
    transcriptCount: string;
    currentStatus: string;
    inputFiles: string;
    jobMissing: string;
    currentConfig: string;
    model: string;
    chooseModel: string;
    template: string;
    chooseTemplate: string;
    includeSpeaker: string;
    includeTimestamp: string;
    extraInstructions: string;
    extraInstructionsPlaceholder: string;
    submit: string;
    submitting: string;
    history: string;
    unknownTemplate: string;
    currentResult: string;
    latest: string;
    unknownModel: string;
    emptyRuns: string;
    preview: string;
    setCurrent: string;
    usingCurrent: string;
    deleteCurrent: string;
  };
  models: {
    title: string;
    copy: string;
    add: string;
    total: string;
    enabled: string;
    defaultLabel: string;
    listTitle: string;
    defaultTag: string;
    enabledTag: string;
    disabledTag: string;
    empty: string;
    deleteConfirm: string;
    deleteTitle: string;
    editorEditTitle: string;
    editorNewTitle: string;
    validationName: string;
    validationBaseUrl: string;
    validationApiKey: string;
    validationModel: string;
    name: string;
    namePlaceholder: string;
    baseUrl: string;
    baseUrlPlaceholder: string;
    apiKey: string;
    apiKeyPlaceholder: string;
    model: string;
    modelPlaceholder: string;
    enabledSwitch: string;
    defaultSwitch: string;
    save: string;
    reset: string;
  };
  templates: {
    title: string;
    copy: string;
    add: string;
    total: string;
    builtin: string;
    custom: string;
    listTitle: string;
    emptyDescription: string;
    empty: string;
    deleteConfirm: string;
    deleteTitle: string;
    editorEditTitle: string;
    editorNewTitle: string;
    validationName: string;
    validationPrompt: string;
    builtinReadonly: string;
    duplicate: string;
    name: string;
    description: string;
    includeSpeakerDefault: string;
    includeTimestampDefault: string;
    prompt: string;
    promptPlaceholder: string;
    save: string;
    reset: string;
  };
  notes: {
    summary: string;
    emptySummary: string;
    topics: string;
    decisions: string;
    actionItems: string;
    risks: string;
    followUps: string;
    windowTitle: string;
    windowCopy: string;
    sectionTitle: string;
  };
  export: {
    transcriptHeading: string;
    summaryHeading: string;
    topicsHeading: string;
    decisionsHeading: string;
    actionItemsHeading: string;
    risksHeading: string;
    followUpsHeading: string;
    emptySummary: string;
  };
  windows: {
    aiSummaryTitle: string;
    meetingNotesTitle: string;
    editModel: string;
    newModel: string;
    editTemplate: string;
    newTemplate: string;
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
    common: {
      appName: "Liberty",
      all: "全部",
      none: "无",
      notSet: "未设置",
      optional: "可选",
      unknownSpeaker: "未知说话人",
      audio: "音频",
      video: "视频",
      localPath: "本地路径",
      unknownSize: "未知大小",
      enabled: "开启",
      disabled: "关闭",
      edit: "编辑",
      save: "保存",
      cancel: "取消",
      delete: "删除",
      remove: "移除",
      retry: "重试",
      open: "打开",
      closeWindow: "关闭窗口",
      choose: "请选择",
      noData: "暂无数据",
      dash: "—",
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
      jobCompletedTitle: "会议任务处理完成",
      jobCompletedBody: "“{title}” 已处理完成，可以前往结果工作台查看。",
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
      localDatabaseReady: "本地运行已就绪",
      waitingLocalConfig: "等待本地配置",
      localRuntime: "本地运行",
      managedRuntime: "本地运行环境",
      managedRuntimeHint: "安装并管理 Liberty 内置的 Python、依赖、ffmpeg 与 FunASR 模型。",
      runtimeStatus: "当前状态",
      runtimeStatusMissing: "未安装",
      runtimeStatusInstalling: "安装中",
      runtimeStatusReady: "已就绪",
      runtimeStatusFailed: "安装失败",
      runtimeStatusRepair: "需要修复",
      runtimeVersion: "环境版本",
      runtimePythonVersion: "Python 版本",
      runtimeInstalledAt: "最后安装时间",
      runtimeInstallAction: "安装内置环境",
      runtimeReinstallAction: "重新安装",
      runtimeInstallProgress: "安装进度",
      runtimeInstallPreparing: "准备展开内置环境",
      runtimeInstallCompleted: "安装完成",
      runtimeInstallDownload: "写入内置 Python 资源",
      runtimeInstallVerify: "校验内置资源",
      runtimeInstallExtract: "展开内置 Python 运行时",
      runtimeInstallResolvePython: "定位 Python",
      runtimeInstallBootstrapPip: "校验 Python 运行时",
      runtimeInstallUpgradePip: "写入内置 FFmpeg 资源",
      runtimeInstallPytorch: "校验 FFmpeg 工具",
      runtimeInstallDependencies: "写入内置 FunASR 模型",
      runtimeInstallValidate: "校验模型资源",
      runtimeInstallModels: "展开内置 FunASR 模型",
      runtimeInstallLog: "安装日志",
      runtimeInstallLogEmpty: "当前还没有安装日志。",
      runtimeDescriptionReady: "本地运行环境已就绪，任务会优先使用托管环境执行。",
      runtimeDescriptionInstalling: "正在展开内置本地运行环境，请等待当前安装流程完成。",
      runtimeDescriptionFailed: "安装未完成，请查看下方日志并重新安装。",
      runtimeDescriptionMissing: "当前设备尚未安装内置本地运行环境，安装后即可直接处理会议文件。",
      manualPythonOverride: "手动 Python 回退",
      manualPythonOverrideHint: "仅在托管运行环境不可用时作为高级兼容方案使用。",
      pythonPath: "Python 可执行文件路径",
      pythonPathHint: "桌面端本地转写和重试任务都会直接调用这里的 Python 环境，Runner 脚本使用项目内置入口。",
      pythonPathPlaceholder: "例如：/opt/homebrew/bin/python3 或 C:\\Python311\\python.exe",
      localAsrDevice: "推理设备",
      localAsrDeviceHint: "自动模式会优先尝试 CUDA、Apple Silicon，再回退到 CPU。",
      localAsrDeviceAuto: "自动",
      localAsrDeviceCpu: "CPU",
      localAsrDeviceMps: "Apple Silicon",
      localAsrDeviceCuda: "CUDA",
      processingDefaults: "处理默认值",
      defaultHotwords: "默认热词",
      defaultHotwordsHint: "建议只保留项目名、专有名词和行业术语，降低普通词带来的噪声。",
      defaultHotwordsPlaceholder: "使用英文逗号分隔，例如：SeACo-Paraformer, FunASR, 会议纪要",
      defaultSummaryTemplate: "默认纪要模板",
      defaultSummaryTemplateHint: "新建会议任务时的默认模板名称，只影响初始填充值。",
      defaultSummaryTemplatePlaceholder: "例如：默认会议纪要模板",
      concurrency: "并发上传数",
      concurrencyHint: "用于控制桌面端任务处理的默认并发度，建议保持在 1 到 8 之间。",
      localAsrThreads: "本地线程数",
      localAsrThreadsHint: "0 表示自动，建议先保持自动；若 CPU 占用过低再手动提高。",
      localAsrBatchSizeSeconds: "批处理时长",
      localAsrBatchSizeSecondsHint: "单位为秒，数值越大通常越快，但会占用更多内存，默认 300 秒。",
      remoteCompatibility: "远端兼容",
      backendUrl: "服务地址",
      backendUrlHint: "保留给远端接口兼容模式使用，本地链路已完整配置时不会优先走这里。",
      backendUrlPlaceholder: "例如：http://127.0.0.1:8000",
      apiToken: "API Token",
      apiTokenHint: "仅在远端服务需要鉴权时填写，不影响本地 Python 与本地数据库流程。",
      apiTokenPlaceholder: "可选",
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
    status: {
      idle: "未生成",
      uploaded: "已上传",
      queued: "排队中",
      transcribing: "转写中",
      speaker_processing: "说话人处理中",
      summarizing: "纪要生成中",
      completed: "已完成",
      failed: "失败",
    },
    newJob: {
      defaultSummaryTemplateName: "默认会议纪要模板",
      localPython: "本地 Python",
      remoteService: "在线服务",
      envMissing: "未配置环境",
      localPathRequired: "本地 Python 模式需要通过桌面原生文件选择器获取可读路径。",
      createFailed: "创建任务失败，请检查本地运行环境配置。",
      heroTitle: "创建会议任务",
      heroCopy: "先完成标题和文件输入，参数放到下面按需调整。",
      viewJobs: "查看任务列表",
      allJobs: "全部任务",
      processing: "处理中",
      failed: "失败",
      basicInfo: "基本信息",
      jobTitle: "任务标题",
      titlePlaceholder: "例如：产品周会 2026-03-25",
      inputFiles: "输入文件",
      addFiles: "添加文件",
      desktopFilePicker: "桌面原生文件选择器",
      mediaSupported: "支持音频与视频文件",
      selectedFiles: "已选择 {count} 个文件",
      reselect: "重新选择",
      continueAdding: "继续添加",
      clearList: "清空列表",
      createJob: "创建任务",
      creating: "正在创建...",
      advancedSettings: "高级设置",
      language: "语言",
      langZh: "中文",
      langEn: "英文",
      langJa: "日文",
      languageHint: "决定转写和后续总结默认使用的语言。",
      speaker: "说话人",
      speakerHint: "开启后会尝试区分讲话人，关闭时只生成普通逐字稿。",
      hotwords: "热词",
      hotwordsPlaceholder: "使用英文逗号分隔，例如：SeACo-Paraformer, FunASR, 招投标",
      hotwordsHint: "建议只保留专有名词、项目名和行业术语。",
      currentStatus: "当前状态",
      fileRule: "文件规则",
      settings: "系统设置",
      jobs: "任务列表",
      recentJobs: "最近任务",
      viewAll: "查看全部",
      noTasks: "还没有任务记录。",
      localModeHint: "使用本机 Python 环境和原生文件路径",
      remoteModeHint: "使用在线接口处理当前任务",
      pendingEnvHint: "请先完成处理环境配置",
      localFileRule: "本地模式每次只处理 1 个文件",
      remoteFileRule: "可一次加入多个文件",
      fileCount: "{count} 个文件",
    },
    jobs: {
      deleteConfirm: "确认删除任务“{title}”吗？删除后无法恢复。",
      pending: "待处理",
      minutes: "{count} 分钟",
      hours: "{count} 小时",
      hoursMinutes: "{hours} 小时 {minutes} 分钟",
      notCompleted: "未完成",
      processingWithHours: "{hours}小时 {minutes}分",
      processingWithMinutes: "{minutes}分 {seconds}秒",
      pageTitle: "所有会议任务",
      pageCopy: "统一查看上传、转写、说话人处理和纪要生成阶段。",
      total: "总任务",
      processing: "处理中",
      completed: "已完成",
      queueTitle: "任务队列",
      queueCopy: "按任务名、时长、状态和操作整理，减少在卡片间来回扫描。",
      colTask: "任务",
      colFileInfo: "文件信息",
      colProcessingTime: "处理时间",
      colCreatedAt: "创建时间",
      colStatus: "状态",
      colActions: "操作",
      filesCount: "{count} 个文件",
      fileDuration: "文件时长 {duration}",
      diarizationEnabled: "含说话人分离",
      transcriptOnly: "仅转写",
      processCompleted: "处理完成",
      processFailed: "处理失败",
      processRunning: "处理中",
      details: "详情",
      workbench: "工作台",
      deleteDisabled: "处理中暂不可删除",
      deleteAction: "删除任务",
      deleting: "删除中...",
    },
    jobDetail: {
      progressSection: "处理进度",
      progressEngine: "转写引擎",
      stageUploaded: "上传完成",
      stageAsr: "转写引擎",
      stageSummary: "AI 总结",
      stageOverall: "总状态",
      inputFiles: "输入文件",
      hotwords: "热词",
      speaker: "说话人",
      viewWorkbench: "查看结果工作台",
      retryJob: "重试任务",
      filesSection: "输入文件",
      settingsSection: "任务设置",
      language: "语言",
      speakerDiarization: "说话人分离",
      logSection: "处理日志",
      noLog: "当前还没有处理日志。",
      notFound: "没有找到这个任务。",
    },
    workbench: {
      allSpeakers: "全部",
      renameSpeakerTitle: "编辑讲话人",
      renameSpeakerConfirm: "确认将当前任务中的“{source}”统一替换为“{target}”吗？",
      replace: "替换",
      heroCopy: "当前聚焦逐字稿与 AI 总结，导出和任务上下文收拢到右侧区域。",
      aiSummary: "AI 总结",
      viewNotes: "查看会议纪要",
      exportBundle: "导出完整结果",
      exporting: "导出中...",
      exportTranscript: "导出逐字稿",
      exportNotes: "导出纪要",
      transcriptCount: "逐字稿 {count} 段",
      summaryCount: "AI 总结 {count} 次",
      currentTemplate: "当前模板 {name}",
      notGenerated: "尚未生成",
      fileCount: "文件 {count} 个",
      durationMinutes: "时长 {count} 分钟",
      hotwords: "热词 {value}",
      notConfigured: "未配置",
      notesReady: "会议纪要已生成",
      notesEmpty: "暂无会议纪要",
      activeResultReady: "当前已选结果可导出",
      summaryEmpty: "还没有生成 AI 总结",
      transcript: "逐字稿",
      searchPlaceholder: "搜索说话人或正文",
      speakerInputPlaceholder: "输入讲话人名称",
      emptyFilteredTranscript: "没有匹配到逐字稿片段。",
      notFound: "没有找到这个任务结果。",
    },
    aiSummary: {
      activeLabelEmpty: "尚未生成",
      activeLabelFailed: "最近一次失败",
      activeLabelRunning: "生成中",
      activeLabelSaved: "已保存结果",
      staleRunError: "上一次 AI 总结未完成，可能因网络、代理或窗口中断导致失败。",
      jobNotFound: "没有找到当前任务。",
      modelMissing: "请先在主窗口中配置可用模型。",
      templateMissing: "请先在主窗口中配置可用模板。",
      transcriptMissing: "当前任务还没有可用于总结的逐字稿内容。",
      requestFailed: "AI 总结失败。",
      emptyResponse: "模型返回内容为空。",
      invalidApiJson: "AI 接口返回的不是合法 JSON。",
      invalidStructuredJson: "模型返回内容无法解析为结构化 JSON。",
      deleteConfirm: "删除后无法恢复，确认删除这条 AI 总结记录吗？",
      deleteTitle: "删除总结记录",
      heroTitle: "AI 总结",
      heroCopy: "当前逐字稿段落 {count} 条，模型与模板来自主窗口中的资源管理页。",
      transcriptCount: "逐字稿 {count} 条",
      currentStatus: "当前状态 {status}",
      inputFiles: "输入文件 {count} 个",
      jobMissing: "未找到任务",
      currentConfig: "本次配置",
      model: "模型",
      chooseModel: "请选择模型",
      template: "模板",
      chooseTemplate: "请选择模板",
      includeSpeaker: "包含说话人",
      includeTimestamp: "包含时间戳",
      extraInstructions: "补充要求",
      extraInstructionsPlaceholder: "例如：重点关注风险项和负责人，输出适合直接发飞书。",
      submit: "生成总结",
      submitting: "生成中...",
      history: "生成记录",
      unknownTemplate: "导入/未知模板",
      currentResult: "当前结果",
      latest: "最新",
      unknownModel: "未知模型",
      emptyRuns: "还没有生成过 AI 总结。",
      preview: "结果预览",
      setCurrent: "设为当前结果",
      usingCurrent: "当前采用中",
      deleteCurrent: "删除本次",
    },
    models: {
      title: "模型管理",
      copy: "新增与编辑都在独立窗口中完成。",
      add: "新增模型",
      total: "全部模型",
      enabled: "可用",
      defaultLabel: "默认",
      listTitle: "模型列表",
      defaultTag: "默认",
      enabledTag: "启用",
      disabledTag: "停用",
      empty: "还没有配置模型。",
      deleteConfirm: "确认删除模型“{name}”吗？",
      deleteTitle: "删除模型",
      editorEditTitle: "编辑模型",
      editorNewTitle: "新增模型",
      validationName: "模型名称不能为空。",
      validationBaseUrl: "Base URL 不能为空。",
      validationApiKey: "API Key 不能为空。",
      validationModel: "Model 不能为空。",
      name: "模型名称",
      namePlaceholder: "例如：OpenAI GPT-4.1",
      baseUrl: "Base URL",
      baseUrlPlaceholder: "例如：https://api.openai.com/v1",
      apiKey: "API Key",
      apiKeyPlaceholder: "sk-...",
      model: "Model",
      modelPlaceholder: "例如：gpt-4.1",
      enabledSwitch: "启用该模型",
      defaultSwitch: "设为默认模型",
      save: "保存模型",
      reset: "清空表单",
    },
    templates: {
      title: "总结模板",
      copy: "内置 Prompt 负责结构稳定，自定义模板统一在独立窗口中编辑。",
      add: "新增模板",
      total: "全部模板",
      builtin: "内置",
      custom: "自定义",
      listTitle: "模板列表",
      emptyDescription: "未填写说明",
      empty: "还没有可用模板。",
      deleteConfirm: "确认删除模板“{name}”吗？",
      deleteTitle: "删除模板",
      editorEditTitle: "编辑模板",
      editorNewTitle: "新增模板",
      validationName: "模板名称不能为空。",
      validationPrompt: "Prompt 不能为空。",
      builtinReadonly: "内置模板不可直接修改，请先复制。",
      duplicate: "复制模板",
      name: "模板名称",
      description: "模板说明",
      includeSpeakerDefault: "默认包含说话人",
      includeTimestampDefault: "默认包含时间戳",
      prompt: "Prompt",
      promptPlaceholder: "输入结构化 Prompt",
      save: "保存模板",
      reset: "新建空模板",
    },
    notes: {
      summary: "摘要",
      emptySummary: "当前还没有可用的 AI 总结内容。",
      topics: "议题",
      decisions: "结论",
      actionItems: "行动项",
      risks: "风险",
      followUps: "跟进事项",
      windowTitle: "会议纪要",
      windowCopy: "当前窗口用于阅读完整会议纪要，不和结果工作台正文混排。",
      sectionTitle: "会议纪要",
    },
    export: {
      transcriptHeading: "## 逐字稿",
      summaryHeading: "## 摘要",
      topicsHeading: "## 议题",
      decisionsHeading: "## 结论",
      actionItemsHeading: "## 行动项",
      risksHeading: "## 风险",
      followUpsHeading: "## 跟进事项",
      emptySummary: "当前还没有可导出的 AI 总结内容。",
    },
    windows: {
      aiSummaryTitle: "AI 总结 - {title}",
      meetingNotesTitle: "会议纪要 - {title}",
      editModel: "编辑模型",
      newModel: "新增模型",
      editTemplate: "编辑模板",
      newTemplate: "新增模板",
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
    common: {
      appName: "Liberty",
      all: "All",
      none: "None",
      notSet: "Not Set",
      optional: "Optional",
      unknownSpeaker: "Unknown Speaker",
      audio: "Audio",
      video: "Video",
      localPath: "Local Path",
      unknownSize: "Unknown Size",
      enabled: "On",
      disabled: "Off",
      edit: "Edit",
      save: "Save",
      cancel: "Cancel",
      delete: "Delete",
      remove: "Remove",
      retry: "Retry",
      open: "Open",
      closeWindow: "Close Window",
      choose: "Please choose",
      noData: "No data",
      dash: "—",
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
      jobCompletedTitle: "Meeting job completed",
      jobCompletedBody: "\"{title}\" has finished processing. You can review it in the workbench.",
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
      localDatabaseReady: "Local runtime ready",
      waitingLocalConfig: "Waiting for local runtime setup",
      localRuntime: "Local Runtime",
      managedRuntime: "Managed Runtime",
      managedRuntimeHint:
        "Install and manage Liberty bundled Python, dependencies, ffmpeg, and default FunASR models.",
      runtimeStatus: "Status",
      runtimeStatusMissing: "Missing",
      runtimeStatusInstalling: "Installing",
      runtimeStatusReady: "Ready",
      runtimeStatusFailed: "Failed",
      runtimeStatusRepair: "Repair Required",
      runtimeVersion: "Runtime Version",
      runtimePythonVersion: "Python Version",
      runtimeInstalledAt: "Installed At",
      runtimeInstallAction: "Install Bundled Runtime",
      runtimeReinstallAction: "Reinstall",
      runtimeInstallProgress: "Install Progress",
      runtimeInstallPreparing: "Preparing Bundled Runtime",
      runtimeInstallCompleted: "Completed",
      runtimeInstallDownload: "Staging Bundled Python",
      runtimeInstallVerify: "Verifying Bundled Asset",
      runtimeInstallExtract: "Extracting Bundled Python",
      runtimeInstallResolvePython: "Resolving Python",
      runtimeInstallBootstrapPip: "Validating Python Runtime",
      runtimeInstallUpgradePip: "Staging Bundled FFmpeg",
      runtimeInstallPytorch: "Validating FFmpeg",
      runtimeInstallDependencies: "Staging Bundled FunASR Models",
      runtimeInstallValidate: "Validating Model Assets",
      runtimeInstallModels: "Extracting Bundled FunASR Models",
      runtimeInstallLog: "Install Log",
      runtimeInstallLogEmpty: "No install log yet.",
      runtimeDescriptionReady: "The local runtime is ready and new jobs will prefer the managed environment.",
      runtimeDescriptionInstalling: "Expanding the bundled local runtime. Please wait for the current install to finish.",
      runtimeDescriptionFailed: "Installation did not finish. Check the log below and reinstall.",
      runtimeDescriptionMissing: "This device does not have the bundled local runtime yet. Install it to process meeting files directly.",
      manualPythonOverride: "Manual Python Fallback",
      manualPythonOverrideHint: "Advanced compatibility fallback used only when the managed runtime is unavailable.",
      pythonPath: "Python Executable Path",
      pythonPathHint:
        "Local transcription and retry both call this Python runtime directly. The runner script uses the built-in project entry.",
      pythonPathPlaceholder: "Example: /opt/homebrew/bin/python3 or C:\\Python311\\python.exe",
      localAsrDevice: "Inference Device",
      localAsrDeviceHint: "Auto prefers CUDA first, then Apple Silicon, and falls back to CPU when needed.",
      localAsrDeviceAuto: "Auto",
      localAsrDeviceCpu: "CPU",
      localAsrDeviceMps: "Apple Silicon",
      localAsrDeviceCuda: "CUDA",
      processingDefaults: "Processing Defaults",
      defaultHotwords: "Default Hotwords",
      defaultHotwordsHint: "Keep project names, proper nouns, and domain terms only to reduce noise.",
      defaultHotwordsPlaceholder: "Use commas, for example: SeACo-Paraformer, FunASR, meeting notes",
      defaultSummaryTemplate: "Default Summary Template",
      defaultSummaryTemplateHint: "Used as the initial template name when creating a new meeting job.",
      defaultSummaryTemplatePlaceholder: "Example: Default Meeting Notes Template",
      concurrency: "Concurrent Uploads",
      concurrencyHint: "Controls the default desktop processing concurrency. Keep it between 1 and 8.",
      localAsrThreads: "Local Threads",
      localAsrThreadsHint: "Use 0 for automatic selection. Increase manually only if CPU usage stays too low.",
      localAsrBatchSizeSeconds: "Batch Window",
      localAsrBatchSizeSecondsHint: "Measured in seconds. Larger values are usually faster but use more memory. Default is 300.",
      remoteCompatibility: "Remote Compatibility",
      backendUrl: "Service URL",
      backendUrlHint: "Reserved for remote compatibility mode. It is ignored when local runtime is fully configured.",
      backendUrlPlaceholder: "Example: http://127.0.0.1:8000",
      apiToken: "API Token",
      apiTokenHint: "Only needed for authenticated remote services. It does not affect local Python or SQLite flows.",
      apiTokenPlaceholder: "Optional",
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
    status: {
      idle: "Not Started",
      uploaded: "Uploaded",
      queued: "Queued",
      transcribing: "Transcribing",
      speaker_processing: "Speaker Processing",
      summarizing: "Summarizing",
      completed: "Completed",
      failed: "Failed",
    },
    newJob: {
      defaultSummaryTemplateName: "Default Meeting Notes Template",
      localPython: "Local Python",
      remoteService: "Online Service",
      envMissing: "Environment Not Configured",
      localPathRequired: "Local Python mode requires readable file paths from the native desktop file picker.",
      createFailed: "Failed to create the job. Check the local runtime configuration.",
      heroTitle: "Create Meeting Job",
      heroCopy: "Start with the title and input file, then adjust advanced options below if needed.",
      viewJobs: "View Jobs",
      allJobs: "All Jobs",
      processing: "Processing",
      failed: "Failed",
      basicInfo: "Basic Info",
      jobTitle: "Job Title",
      titlePlaceholder: "Example: Product Weekly Sync 2026-03-25",
      inputFiles: "Input Files",
      addFiles: "Add Files",
      desktopFilePicker: "Native desktop file picker",
      mediaSupported: "Audio and video files supported",
      selectedFiles: "{count} file(s) selected",
      reselect: "Reselect",
      continueAdding: "Add More",
      clearList: "Clear List",
      createJob: "Create Job",
      creating: "Creating...",
      advancedSettings: "Advanced Settings",
      language: "Language",
      langZh: "Chinese",
      langEn: "English",
      langJa: "Japanese",
      languageHint: "Sets the default language for transcription and later summaries.",
      speaker: "Speaker Diarization",
      speakerHint: "When enabled, Liberty tries to separate speakers. Otherwise it only creates plain transcripts.",
      hotwords: "Hotwords",
      hotwordsPlaceholder: "Use commas, for example: SeACo-Paraformer, FunASR, procurement",
      hotwordsHint: "Keep proper nouns, project names, and domain terms only.",
      currentStatus: "Current Status",
      fileRule: "File Rules",
      settings: "Settings",
      jobs: "Jobs",
      recentJobs: "Recent Jobs",
      viewAll: "View All",
      noTasks: "No jobs yet.",
      localModeHint: "Uses the local Python runtime and native file paths",
      remoteModeHint: "Uses the online API for this job",
      pendingEnvHint: "Finish environment setup first",
      localFileRule: "Local mode processes one file at a time",
      remoteFileRule: "You can add multiple files in one batch",
      fileCount: "{count} file(s)",
    },
    jobs: {
      deleteConfirm: "Delete job \"{title}\"? This cannot be undone.",
      pending: "Pending",
      minutes: "{count} min",
      hours: "{count} hr",
      hoursMinutes: "{hours} hr {minutes} min",
      notCompleted: "Not Completed",
      processingWithHours: "{hours}h {minutes}m",
      processingWithMinutes: "{minutes}m {seconds}s",
      pageTitle: "All Meeting Jobs",
      pageCopy: "Review upload, transcription, speaker processing, and summary generation in one place.",
      total: "Total",
      processing: "Processing",
      completed: "Completed",
      queueTitle: "Job Queue",
      queueCopy: "Organized by job name, duration, status, and actions to reduce scanning between cards.",
      colTask: "Task",
      colFileInfo: "File Info",
      colProcessingTime: "Processing Time",
      colCreatedAt: "Created At",
      colStatus: "Status",
      colActions: "Actions",
      filesCount: "{count} file(s)",
      fileDuration: "Duration {duration}",
      diarizationEnabled: "With speaker diarization",
      transcriptOnly: "Transcript only",
      processCompleted: "Completed",
      processFailed: "Failed",
      processRunning: "Processing",
      details: "Details",
      workbench: "Workbench",
      deleteDisabled: "Cannot delete while processing",
      deleteAction: "Delete job",
      deleting: "Deleting...",
    },
    jobDetail: {
      progressSection: "Processing Progress",
      progressEngine: "Transcription Engine",
      stageUploaded: "Upload",
      stageAsr: "Transcription",
      stageSummary: "AI Summary",
      stageOverall: "Overall",
      inputFiles: "Input Files",
      hotwords: "Hotwords",
      speaker: "Speaker",
      viewWorkbench: "Open Workbench",
      retryJob: "Retry Job",
      filesSection: "Input Files",
      settingsSection: "Job Settings",
      language: "Language",
      speakerDiarization: "Speaker Diarization",
      logSection: "Processing Log",
      noLog: "No processing logs yet.",
      notFound: "This job could not be found.",
    },
    workbench: {
      allSpeakers: "All",
      renameSpeakerTitle: "Edit Speaker",
      renameSpeakerConfirm: "Replace \"{source}\" with \"{target}\" across the current job?",
      replace: "Replace",
      heroCopy: "The main area focuses on transcript and AI summary, while export and context stay on the side.",
      aiSummary: "AI Summary",
      viewNotes: "View Notes",
      exportBundle: "Export Full Result",
      exporting: "Exporting...",
      exportTranscript: "Export Transcript",
      exportNotes: "Export Notes",
      transcriptCount: "Transcript {count}",
      summaryCount: "AI Summaries {count}",
      currentTemplate: "Template {name}",
      notGenerated: "Not generated",
      fileCount: "Files {count}",
      durationMinutes: "Duration {count} min",
      hotwords: "Hotwords {value}",
      notConfigured: "Not configured",
      notesReady: "Notes ready",
      notesEmpty: "No notes yet",
      activeResultReady: "Selected result can be exported",
      summaryEmpty: "No AI summary has been generated yet",
      transcript: "Transcript",
      searchPlaceholder: "Search speaker or transcript",
      speakerInputPlaceholder: "Enter speaker name",
      emptyFilteredTranscript: "No transcript segments matched the current filter.",
      notFound: "This job result could not be found.",
    },
    aiSummary: {
      activeLabelEmpty: "Not generated",
      activeLabelFailed: "Last run failed",
      activeLabelRunning: "Generating",
      activeLabelSaved: "Saved",
      staleRunError: "The previous AI summary did not finish. It may have failed because of network, proxy, or window interruption.",
      jobNotFound: "The current job could not be found.",
      modelMissing: "Configure an available model in the main window first.",
      templateMissing: "Configure an available template in the main window first.",
      transcriptMissing: "The current job has no transcript content available for summarization.",
      requestFailed: "AI summary failed.",
      emptyResponse: "The model returned an empty response.",
      invalidApiJson: "The AI API returned invalid JSON.",
      invalidStructuredJson: "The model output could not be parsed into structured JSON.",
      deleteConfirm: "Delete this AI summary record? This action cannot be undone.",
      deleteTitle: "Delete Summary Record",
      heroTitle: "AI Summary",
      heroCopy: "There are {count} transcript segments. Models and templates come from the resource pages in the main window.",
      transcriptCount: "Transcript {count}",
      currentStatus: "Status {status}",
      inputFiles: "Input Files {count}",
      jobMissing: "Job not found",
      currentConfig: "Current Config",
      model: "Model",
      chooseModel: "Choose a model",
      template: "Template",
      chooseTemplate: "Choose a template",
      includeSpeaker: "Include speaker",
      includeTimestamp: "Include timestamp",
      extraInstructions: "Extra Instructions",
      extraInstructionsPlaceholder: "Example: Focus on risks and owners, and make the output suitable for posting to Feishu.",
      submit: "Generate Summary",
      submitting: "Generating...",
      history: "History",
      unknownTemplate: "Imported / Unknown Template",
      currentResult: "Current Result",
      latest: "Latest",
      unknownModel: "Unknown Model",
      emptyRuns: "No AI summaries have been generated yet.",
      preview: "Preview",
      setCurrent: "Use as Current",
      usingCurrent: "Currently Applied",
      deleteCurrent: "Delete This Run",
    },
    models: {
      title: "Model Management",
      copy: "Add and edit actions are handled in a separate window.",
      add: "Add Model",
      total: "Total Models",
      enabled: "Available",
      defaultLabel: "Default",
      listTitle: "Model List",
      defaultTag: "Default",
      enabledTag: "Enabled",
      disabledTag: "Disabled",
      empty: "No models configured yet.",
      deleteConfirm: "Delete model \"{name}\"?",
      deleteTitle: "Delete Model",
      editorEditTitle: "Edit Model",
      editorNewTitle: "Add Model",
      validationName: "Model name is required.",
      validationBaseUrl: "Base URL is required.",
      validationApiKey: "API Key is required.",
      validationModel: "Model is required.",
      name: "Model Name",
      namePlaceholder: "Example: OpenAI GPT-4.1",
      baseUrl: "Base URL",
      baseUrlPlaceholder: "Example: https://api.openai.com/v1",
      apiKey: "API Key",
      apiKeyPlaceholder: "sk-...",
      model: "Model",
      modelPlaceholder: "Example: gpt-4.1",
      enabledSwitch: "Enable this model",
      defaultSwitch: "Use as default model",
      save: "Save Model",
      reset: "Reset Form",
    },
    templates: {
      title: "Summary Templates",
      copy: "Built-in prompts keep the structure stable. Custom templates are edited in a separate window.",
      add: "Add Template",
      total: "Total Templates",
      builtin: "Built-in",
      custom: "Custom",
      listTitle: "Template List",
      emptyDescription: "No description",
      empty: "No templates available yet.",
      deleteConfirm: "Delete template \"{name}\"?",
      deleteTitle: "Delete Template",
      editorEditTitle: "Edit Template",
      editorNewTitle: "Add Template",
      validationName: "Template name is required.",
      validationPrompt: "Prompt is required.",
      builtinReadonly: "Built-in templates cannot be edited directly. Duplicate one first.",
      duplicate: "Duplicate Template",
      name: "Template Name",
      description: "Description",
      includeSpeakerDefault: "Include speaker by default",
      includeTimestampDefault: "Include timestamp by default",
      prompt: "Prompt",
      promptPlaceholder: "Enter a structured prompt",
      save: "Save Template",
      reset: "New Empty Template",
    },
    notes: {
      summary: "Overview",
      emptySummary: "No AI summary content is available yet.",
      topics: "Topics",
      decisions: "Decisions",
      actionItems: "Action Items",
      risks: "Risks",
      followUps: "Follow-ups",
      windowTitle: "Meeting Notes",
      windowCopy: "This window is dedicated to reading the full meeting notes instead of mixing them into the workbench body.",
      sectionTitle: "Meeting Notes",
    },
    export: {
      transcriptHeading: "## Transcript",
      summaryHeading: "## Overview",
      topicsHeading: "## Topics",
      decisionsHeading: "## Decisions",
      actionItemsHeading: "## Action Items",
      risksHeading: "## Risks",
      followUpsHeading: "## Follow-ups",
      emptySummary: "No AI summary content is available for export yet.",
    },
    windows: {
      aiSummaryTitle: "AI Summary - {title}",
      meetingNotesTitle: "Meeting Notes - {title}",
      editModel: "Edit Model",
      newModel: "Add Model",
      editTemplate: "Edit Template",
      newTemplate: "Add Template",
    },
  },
};

export function resolveLocale(locale?: string | null): LocaleCode {
  return locale === "en-US" ? "en-US" : "zh-CN";
}

export function getCurrentLocale(): LocaleCode {
  if (typeof document === "undefined") {
    return "zh-CN";
  }

  return resolveLocale(document.documentElement.lang);
}

export function formatMessage(template: string, values: Record<string, string | number>) {
  return template.replace(/\{(\w+)\}/g, (_, key: string) => String(values[key] ?? ""));
}

export function getMessages(locale: LocaleCode): MessageTree {
  return messages[locale] ?? messages["zh-CN"];
}

export function getCurrentMessages(): MessageTree {
  return getMessages(getCurrentLocale());
}
