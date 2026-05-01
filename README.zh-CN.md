<p align="center">
  <img src="https://avatars.githubusercontent.com/u/277389313?s=200&v=4" width="128" height="128" alt="Liberty">
</p>

<h1 align="center">Liberty</h1>

<p align="center">
  面向桌面端的会议音视频处理工作台。
</p>

<p align="center">
  本地转写 · 说话人分离 · AI 总结 · 结果整理
</p>

<p align="center">
  <a href="src-tauri/tauri.conf.json"><img src="https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white" alt="Tauri 2"></a>
  <a href="package.json"><img src="https://img.shields.io/badge/Vue-3-42B883?logo=vue.js&logoColor=white" alt="Vue 3"></a>
  <a href="package.json"><img src="https://img.shields.io/badge/TypeScript-5-3178C6?logo=typescript&logoColor=white" alt="TypeScript 5"></a>
  <a href="src-tauri/Cargo.toml"><img src="https://img.shields.io/badge/Rust-stable-000000?logo=rust&logoColor=white" alt="Rust stable"></a>
  <a href="scripts/runtime_requirements.txt"><img src="https://img.shields.io/badge/Python-3.9-3776AB?logo=python&logoColor=white" alt="Python 3.9"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-green.svg" alt="License"></a>
</p>

[English](./README.md) | [简体中文](./README.zh-CN.md)

Liberty 是一款面向桌面端的会议音视频处理工作台，围绕“本地转写、AI 总结、结果整理”这一完整链路设计。项目基于 Tauri 2、Vue 3、TypeScript、Rust 与 Python 构建，重点强调本地可运行、桌面端工作流完整以及配置集中管理。

![Liberty 项目截图](docs/images/QQ20260501-112920.png)

## 项目概述

Liberty 用于将原始会议音视频文件转换为结构化、可审阅、可导出的结果：

- 通过桌面原生文件选择器导入本地会议文件
- 使用本地 FunASR 运行环境完成转写与说话人分离
- 在桌面应用内查看处理进度、处理日志与错误信息
- 基于用户配置的在线模型生成 AI 总结
- 在专用结果工作台中整理逐字稿、讲话人、纪要与导出内容

Liberty 不只是一个转写工具，它更像是一套桌面端会议内容处理工作流，用于管理任务、封装本地运行环境，并沉淀可持续复用的结果。

## 核心能力

### 1. 本地会议文件处理

- 支持导入本地音频与视频文件
- 支持格式：`m4a`、`mp3`、`wav`、`aac`、`flac`、`mp4`、`mov`、`mkv`
- 自动通过 `ffmpeg` 对媒体进行预处理，再进入转写链路
- 支持记录任务状态、处理时长、日志与最终结果

### 2. 本地转写与说话人分离

- 使用 Python 3.9 + FunASR 构建本地转写链路
- 支持说话人分离开关
- 支持配置本地运行参数，包括设备、线程数和批处理时长
- 本地运行环境安装在用户应用数据目录下，不依赖系统 Python，也不要求管理员权限

### 3. AI 总结

- AI 总结由用户手动触发，不自动执行
- 内置模型管理与模板管理
- 兼容 OpenAI 标准接口格式
- 支持保存多次总结结果，并切换当前展示结果

### 4. 桌面端工作流

- 新建任务
- 任务列表
- 任务详情
- 结果工作台
- 会议纪要窗口
- AI 总结窗口
- 系统设置、本地运行环境、模型管理、模板管理

## 运行模式

Liberty 当前主要包含两条处理链路：

### 本地运行链路

用于桌面端离线处理会议媒体文件。

包含内容：

- 托管 Python 3.9 运行时
- Python 依赖
- 默认 FunASR 模型
- `ffmpeg`
- Rust 侧任务调度与 SQLite 持久化

首次使用本地转写前，需要在 `系统设置 -> 本地运行环境` 中执行“下载并安装”。

### AI 总结链路

用于在转写结果基础上生成结构化会议内容。

包含内容：

- 在线模型配置
- 模板配置
- Prompt 组装
- 总结记录持久化

AI 总结与本地转写相互独立，由用户在需要时主动触发。

## 技术架构

| 层级 | 技术 |
| --- | --- |
| 桌面壳 | Tauri 2 |
| 前端界面 | Vue 3 + Vue Router + TypeScript + Vite |
| 本地能力 | Rust |
| 本地转写 | Python 3.9 + FunASR |
| 本地存储 | SQLite |
| AI 接口 | OpenAI 兼容接口 |

### 主要职责划分

- `src/`：前端页面、状态管理、样式、多语言与前端服务封装
- `src-tauri/src/`：本地数据库、任务执行、运行时安装、系统调用等桌面能力
- `scripts/`：FunASR Runner、运行时预热、依赖校验与 Python 侧辅助脚本

## 支持平台

- macOS Intel
- macOS Apple Silicon
- Windows x64

## 开发环境要求

- Node.js 20+
- `pnpm`
- Rust stable
- Tauri CLI

## 本地开发

安装依赖：

```bash
pnpm install
```

启动前端开发服务：

```bash
pnpm dev
```

启动桌面端开发：

```bash
pnpm tauri dev
```

说明：

- 前端页面与样式改动通常可以热更新
- Rust 代码、Tauri 配置、内置脚本资源改动后，通常需要重新启动 `pnpm tauri dev`

## 构建

构建前端：

```bash
pnpm build
```

构建桌面应用：

```bash
pnpm tauri build
```

## 本地运行环境

本地运行环境由应用内部安装并维护，目标是让终端用户在未配置开发环境的机器上也能完成本地转写。

安装内容包括：

- Python 3.9 运行时
- Python 依赖
- 默认 FunASR 模型
- `ffmpeg`

安装路径：

- macOS：`~/Library/Application Support/com.westng.liberty/runtime/`
- Windows：`%LOCALAPPDATA%\\com.westng.liberty\\runtime\\`

设计原则：

- 不依赖系统 Python
- 不要求管理员权限
- 支持在应用内完成下载、重装与日志排查

## 项目结构

```text
.
├─ src/
│  ├─ assets/                静态资源
│  ├─ composables/           前端状态与业务逻辑
│  ├─ services/              本地服务、AI、导出、多语言、外观
│  ├─ views/                 页面与窗口
│  └─ style.css              全局样式
├─ src-tauri/
│  ├─ resources/             运行时清单与内置资源
│  ├─ src/
│  │  ├─ local_db.rs         SQLite 与持久化数据
│  │  ├─ local_jobs.rs       本地任务执行链路
│  │  ├─ local_runtime.rs    本地运行环境安装与日志
│  │  ├─ local_ai.rs         AI 模型与总结数据
│  │  └─ local_settings.rs   系统设置
│  └─ tauri.conf.json        Tauri 配置
├─ scripts/
│  ├─ funasr_runner.py       本地转写 Runner
│  ├─ runtime_warmup.py      默认模型预热
│  ├─ runtime_validate.py    Python 运行时校验
│  └─ runtime_requirements.txt
└─ README.zh-CN.md
```

## 主要页面与模块

- `新建任务`：导入媒体文件并配置处理任务
- `任务列表`：查看任务状态、处理时间与可执行操作
- `任务详情`：查看输入文件、任务设置、处理进度与处理日志
- `结果工作台`：查看逐字稿、讲话人、AI 总结与导出入口
- `模型管理`：维护可复用的在线模型配置
- `模板管理`：维护 AI 总结模板
- `系统设置`：主题、多语言、本地运行参数与本地运行环境安装

## 持久化与结果

Liberty 使用 SQLite 进行本地持久化，当前主要保存：

- 任务基本信息
- 输入文件记录
- 转写分段与讲话人分段
- 任务处理日志
- AI 总结记录
- 模型配置与模板配置

这样可以保证任务结果在应用重启后依然可查看、可追踪。

## 注意事项

- 当前本地 FunASR 链路按“单文件任务”运行
- 部分日志直接来自底层依赖，例如 FunASR、ModelScope 或 jieba
- 如果媒体文件存在坏帧或头信息异常，`ffmpeg` 可能打印警告，但不一定影响最终处理成功
- AI 总结依赖用户自行配置的在线模型接口

## 许可证

本项目采用 MIT License，详见 [LICENSE](./LICENSE)。
