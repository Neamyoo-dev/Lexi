# Lexi 输入法 - Code Wiki

> **版本**: 0.1.0 | **语言**: Rust | **平台**: Windows (优先) → macOS → 移动端  

---

## 一、项目概述

### 1.1 项目定位

Lexi 是一款面向中文用户的拼音输入法，核心差异化策略为「极致视觉美感 + 极简轻量」。它基于 RIME（中州韵）输入引擎做壳，使用 **Tauri v2 + Rust** 架构实现 Windows TSF 框架集成，候选栏通过 **Skia** 进行自绘渲染，呈现毛玻璃 + 纯色极简融合风格。

### 1.2 技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| 应用框架 | Tauri v2 | Rust 驱动的跨平台桌面应用框架 |
| UI 渲染 | Tauri WebView (wry) | 设置面板使用原生 WebView；候选栏使用 Skia 自绘 |
| 图形引擎 | Skia (skia-safe 0.93) | 候选栏像素级渲染（毛玻璃、渐变、阴影） |
| 输入引擎 | RIME (librime.dll) | 通过 C FFI 绑定，负责拼音→汉字转换 |
| Windows 集成 | TSF (Text Services Framework) | 独立 COM DLL (`lexi_tsf.dll`)，注册为系统输入法 |
| 进程通信 | Windows Named Pipe | TSF DLL ↔ Tauri 主进程之间的双向消息通道 |
| 前端 | Vanilla HTML/CSS/JS | 设置面板，无框架依赖 |
| 序列化 | serde / serde_json | Rust 结构体与 JSON 互转 |

### 1.3 设计决策摘要

- **平台路线**: Windows 先发 → macOS → 移动端（分阶段）
- **输入方案**: v1 仅拼音，基于 RIME 做引擎
- **候选栏**: 传统横排跟随光标；默认 6 个候选词，数字键选词 + Tab/[] 翻页
- **主题**: 内置浅色/深色两套主题，支持主色调自定义
- **词库**: 预装高质量词库（如朙月拼音），开箱即用
- **同步**: 本地文件手动导入导出，零后端

---

## 二、项目目录结构

```
Lexi/
├── docs/                            # 文档
│   ├── CODE_WIKI.md                 # Code Wiki（本文档）
│   └── ISSUE_REPORT.md              # 问题审查报告
├── scripts/                         # 部署/注册脚本
│   ├── register-tsf.ps1             # 以管理员权限注册 TSF 输入法到系统
│   └── unregister-tsf.ps1           # 从系统注销 TSF 输入法
├── src-tauri/                       # Tauri 应用主体（Rust 工作区）
│   ├── Cargo.toml                   # 工作区根配置 + 主 crate 依赖
│   ├── build.rs                     # Tauri 构建脚本
│   ├── tauri.conf.json              # Tauri 应用配置
│   ├── capabilities/
│   │   └── default.json             # 窗口权限声明
│   ├── icons/                       # 应用图标（多尺寸）
│   ├── crates/
│   │   └── tsf-service/             # TSF 输入法 COM DLL 子 crate
│   │       ├── Cargo.toml           # lexi-tsf crate 配置
│   │       └── src/
│   │           ├── lib.rs           # DLL 入口 & COM ClassFactory
│   │           ├── text_service.rs  # TSF 文本服务实现
│   │           └── pipe_client.rs   # Named Pipe 客户端（连接主进程）
│   └── src/                         # 主 crate (lexi_lib) 源码
│       ├── main.rs                  # 程序入口（Windows 子系统）
│       ├── lib.rs                   # 库入口：状态管理、Tauri 命令、应用启动
│       ├── pipe_server.rs           # Named Pipe 服务端（接收 TSF DLL 消息）
│       ├── candidate_bar.rs         # 候选栏：Skia 渲染 + Win32 分层窗口
│       └── ime/
│           ├── mod.rs               # IME 模块入口
│           └── rime/
│               ├── mod.rs           # RIME 引擎封装（RimeEngine）
│               └── ffi.rs           # RIME C API 的 Rust FFI 绑定
├── src/                             # Tauri 前端资源目录（frontendDist）
│   ├── index.html                   # 设置面板 HTML
│   ├── styles.css                   # 全局样式
│   ├── main.js                      # 入口 JS
│   ├── css/
│   │   └── settings.css             # 设置面板样式
│   └── js/
│       └── settings.js              # 设置面板逻辑
├── LICENSE
├── README.md
└── .gitignore
```

---

## 三、架构总览

### 3.1 整体架构图

```
┌──────────────────────────────────────────────────────────────┐
│                    Windows 系统层                             │
│  ┌─────────────┐     ┌───────────────────────────────────┐   │
│  │  应用程序    │     │         TSF 框架                  │   │
│  │ (任意文本域) │     │  ┌─────────────────────────────┐ │   │
│  │             │     │  │  lexi_tsf.dll (COM DLL)     │ │   │
│  └──────┬──────┘     │  │  - LexiTextService          │ │   │
│         │ 按键事件   │  │  - LexiKeyEventSink         │ │   │
│         ▼            │  │  - PipeClient               │ │   │
│  ┌─────────────┐     │  └───────────┬─────────────────┘ │   │
│  │ 候选栏窗口   │     └──────────────┼───────────────────┘   │
│  │ (Skia 自绘) │                    │                        │
│  │ 毛玻璃+极简  │           Named Pipe                       │
│  └──────┬──────┘           "\\.\pipe\LexiInputMethod"       │
│         │                              │                     │
│         │   共享状态                    ▼                     │
│         │  (Arc<Mutex>)   ┌───────────────────────────────┐ │
│         └────────────────►│    Tauri 主进程 (lexi.exe)    │ │
│                           │  ┌─────────────────────────┐  │ │
│                           │  │ AppState                 │  │ │
│                           │  │  - RimeEngine            │  │ │
│                           │  │  - BarData (共享状态)    │  │ │
│                           │  ├─────────────────────────┤  │ │
│                           │  │ PipeServer               │  │ │
│                           │  │ (tokio named pipe)       │  │ │
│                           │  ├─────────────────────────┤  │ │
│                           │  │ 候选栏线程               │  │ │
│                           │  │ (Win32 message loop)     │  │ │
│                           │  │ + Skia 渲染              │  │ │
│                           │  ├─────────────────────────┤  │ │
│                           │  │ 系统托盘                 │  │ │
│                           │  │ (偏好设置 / 退出)        │  │ │
│                           │  ├─────────────────────────┤  │ │
│                           │  │ 设置面板                 │  │ │
│                           │  │ (Tauri WebView 窗口)     │  │ │
│                           │  └─────────────────────────┘  │ │
│                           └───────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

### 3.2 数据流

```
用户按键
  │
  ▼
Windows TSF 框架
  │
  ▼
lexi_tsf.dll (LexiKeyEventSink::OnKeyDown)
  │  序列化为 JSON: {"type":"keydown","keycode":...,"modifiers":...}
  ▼
Named Pipe Client ──────────────────────► Named Pipe Server (Tauri 主进程)
                                               │
                                               ▼
                                         pipe_handler → RimeEngine::process_key()
                                               │
                                               ▼
                                         返回 ContextData (候选词列表)
                                               │
                                               ▼
                                         更新 BarData (Arc<Mutex<BarData>>)
                                               │ 返回 handled + 候选词给 TSF DLL
                                               ▼
                                         signal_update() → PostMessageW(RENDER_MSG)
                                               │
                                               ▼
                                         候选栏窗口 bar_wndproc → Skia 渲染
```

---

## 四、核心模块详解

### 4.1 输入引擎模块 (`src/ime/`)

#### 4.1.1 `ime/rime/ffi.rs` — RIME C API FFI 绑定

该文件定义了与 `librime.dll` 交互所需的全部 C 结构体和函数指针类型。

**关键 C 结构体**:

| 结构体 | 用途 |
|--------|------|
| `RimeTraits` | 引擎初始化参数：共享数据目录、用户数据目录、应用名等 |
| `RimeCommit` | 上屏文本结果 |
| `RimeCandidate` | 单个候选词：文本 + 注释 |
| `RimeMenu` | 候选菜单：每页数量、当前页码、高亮索引、候选词数组 |
| `RimeComposition` | 编码串（preedit）：长度、光标位置、选区 |
| `RimeContext` | 完整上下文：编码串 + 菜单 + 上屏预览 |
| `RimeStatus` | 输入状态：中/英文模式、繁/简体、全角/半角 |
| `RimeApi` | 函数表：包含所有 RIME API 函数指针 |

**核心类**: `RimeLibrary`
- `load()` — 通过 Win32 `LoadLibraryA` + `GetProcAddress` 动态加载 `librime.dll`
- 使用 RAII Guard 确保所有提前返回路径释放 DLL 句柄
- `Drop` — 自动调用 `FreeLibrary` 释放 DLL

#### 4.1.2 `ime/rime/mod.rs` — RIME 引擎封装

**核心类**: `RimeEngine` — 使用单锁 `Mutex<EngineInner>` 封装所有字段

**核心方法**:

| 方法 | 说明 |
|------|------|
| `new()` | 构造未初始化的引擎实例 |
| `initialize()` | 加载 librime.dll，设置数据目录，调用 RIME setup/initialize |
| `ensure_session()` | 确保存在 RIME 会话 |
| `process_key()` | 处理按键事件 → 返回上下文和按键处理标志 |
| `select_candidate()` | 选择候选词 → 获取上屏文本 |

---

### 4.2 TSF 服务模块 (`crates/tsf-service/`)

独立 `cdylib` crate，编译为 `lexi_tsf.dll`，注册为 Windows COM 服务器。

| 文件 | 核心内容 |
|------|----------|
| `lib.rs` | DLL 入口 (`DllMain`, `DllGetClassObject`), COM ClassFactory |
| `text_service.rs` | `LexiTextService`(ITfTextInputProcessor) + `LexiKeyEventSink`(ITfKeyEventSink) |
| `pipe_client.rs` | Named Pipe 客户端，使用 OVERLAPPED I/O 与主进程通信 |

---

### 4.3 主程序模块 (`src/`)

| 文件 | 核心内容 |
|------|----------|
| `main.rs` | 程序入口，调用 `lexi_lib::run()` |
| `lib.rs` | `AppState` 状态管理 + Tauri 命令 + 应用启动流程 + Pipe 消息处理 |
| `pipe_server.rs` | Named Pipe 服务端，异步处理 TSF DLL 连接请求 |
| `candidate_bar.rs` | Win32 + Skia 候选栏窗口渲染 |

---

## 五、Cargo 工作区结构

| Crate 名 | 类型 | 产出物 | 说明 |
|----------|------|--------|------|
| `lexi` | bin + lib | `lexi.exe` + `lexi_lib.dll` | 主应用 |
| `lexi-tsf` | cdylib | `lexi_tsf.dll` | TSF COM 输入法 DLL |

---

## 六、项目运行方式

### 前置条件
- Rust 工具链 (edition 2021)
- Windows 10/11
- `librime.dll` 放置在 `src-tauri/target/debug/` 或系统 PATH 中

### 构建与注册

```powershell
cd src-tauri
cargo build -p lexi-tsf        # 构建 TSF DLL
cargo build                     # 构建主应用
cd ../scripts
.\register-tsf.ps1             # 注册输入法（管理员权限）
```

### 开发运行

```powershell
cd src-tauri
cargo tauri dev                 # 开发模式
cargo build                     # 仅构建后端
```

---

## 七、IPC 协议

| 属性 | 值 |
|------|-----|
| 管道路径 | `\\.\pipe\LexiInputMethod` |
| 通信模式 | 请求-响应（同步） |
| 序列化格式 | JSON (UTF-8) |
| 超时 | 500ms |

**请求**:
```json
{"type":"keydown","keycode":65,"modifiers":0}
```
**响应**:
```json
{"handled":true,"commit":"","candidates":["我","你","他"],"preedit":"wo"}
```

---

## 八、线程模型

| 线程 | 职责 | 通信机制 |
|------|------|----------|
| **主线程** | Tauri 事件循环、系统托盘、WebView | — |
| **候选栏线程** | Win32 消息循环 + Skia 渲染 | `Arc<Mutex<BarData>>` + `PostMessageW(RENDER_MSG)` |
| **Tokio 异步运行时** | Named Pipe 服务端 | tokio task per connection |

---

## 九、文件索引

| 文件 | 行数 | 核心内容 |
|------|------|----------|
| `src-tauri/src/main.rs` | 4 | 程序入口 |
| `src-tauri/src/lib.rs` | ~200 | 应用核心：状态管理、Tauri 命令、启动流程 |
| `src-tauri/src/pipe_server.rs` | ~105 | Named Pipe 服务端 |
| `src-tauri/src/candidate_bar.rs` | ~350 | Skia + Win32 候选栏渲染 |
| `src-tauri/src/ime/rime/mod.rs` | ~350 | RIME 引擎封装 |
| `src-tauri/src/ime/rime/ffi.rs` | ~230 | RIME C FFI 绑定 |
| `src-tauri/crates/tsf-service/src/lib.rs` | ~135 | DLL 入口、COM ClassFactory |
| `src-tauri/crates/tsf-service/src/text_service.rs` | ~210 | TSF 文本服务 |
| `src-tauri/crates/tsf-service/src/pipe_client.rs` | ~180 | Named Pipe 客户端 |
