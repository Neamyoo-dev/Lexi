# Lexi 输入法 - Code Wiki

> **版本**: 0.1.0 | **语言**: Rust | **平台**: Windows (优先) → macOS → 移动端  
> **项目仓库**: [d:\desktop\project\Lexi](file:///d:\desktop\project\Lexi)

---

## 一、项目概述

### 1.1 项目定位

Lexi 是一款面向中文用户的拼音输入法，核心差异化策略为「极致视觉美感 + 极简轻量」。它基于 [RIME（中州韵）](https://rime.im/) 输入引擎做壳，使用 **Tauri v2 + Rust** 架构实现 Windows TSF 框架集成，候选栏通过 **Skia** 进行自绘渲染，呈现毛玻璃 + 纯色极简融合风格。

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
├── .grill/                          # 设计文档
│   └── lexi-ime-design.md           # 输入法方案设计（产品决策文档）
├── .vscode/                         # VS Code 配置
│   └── extensions.json              # 推荐扩展（Tauri + rust-analyzer）
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
├── .gitignore
├── README.md
└── CODE_WIKI.md                     # 本文档
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
  │  序列化为 JSON: {"type":"keydown","keycode":...}
  ▼
Named Pipe Client ──────────────────────► Named Pipe Server (Tauri 主进程)
                                               │
                                               ▼
                                         PipeHandler (create_pipe_handler)
                                               │ emit("tsf_key_event", ...)
                                               ▼
                                         Tauri 前端 (WebView)
                                               │ 调用 process_key 命令
                                               ▼
                                         process_key() @ lib.rs
                                               │
                                               ▼
                                         RimeEngine::process_key()
                                               │ C FFI → librime.dll
                                               ▼
                                         返回 ContextData (候选词列表)
                                               │
                                               ▼
                                         更新 BarData (Arc<Mutex<BarData>>)
                                               │
                                               ▼
                                         signal_update() → PostMessageW(RENDER_MSG)
                                               │
                                               ▼
                                         候选栏窗口 bar_wndproc
                                               │
                                               ▼
                                         Skia 渲染 + UpdateLayeredWindow (毛玻璃效果)
```

---

## 四、核心模块详解

### 4.1 输入引擎模块 (`src/ime/`)

#### 4.1.1 `ime/rime/ffi.rs` — RIME C API FFI 绑定

**文件路径**: [ffi.rs](file:///d:\desktop\project\Lexi\src-tauri\src\ime\rime\ffi.rs)

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
| `RimeApi` | 函数表：包含所有 RIME API 函数指针（setup、process_key、get_context 等） |

**核心类**: `RimeLibrary`

```rust
pub struct RimeLibrary {
    lib: *mut std::ffi::c_void,    // DLL 句柄
    api: Option<&'static RimeApi>, // 函数表引用
}
```

- `load()` — 通过 Win32 `LoadLibraryA` + `GetProcAddress` 动态加载 `librime.dll`，获取 `rime_get_api` 函数地址，得到 `RimeApi` 函数表。
- `api()` — 返回函数表引用。
- `Drop` — 自动调用 `FreeLibrary` 释放 DLL。

**RimeContext 扩展方法**:
- `preedit()` — 将 C 字符串转为 Rust `&str`（编码串）
- `candidates()` — 遍历候选词指针数组，返回 `Vec<(String, String)>`
- `page_no()` / `is_last_page()` / `highlighted_index()` — 分页与高亮信息

---

#### 4.1.2 `ime/rime/mod.rs` — RIME 引擎封装

**文件路径**: [mod.rs](file:///d:\desktop\project\Lexi\src-tauri\src\ime\rime\mod.rs)

**核心类**: `RimeEngine`

```rust
pub struct RimeEngine {
    library: Mutex<Option<RimeLibrary>>,
    session_id: Mutex<Option<RimeSessionId>>,
    initialized: Mutex<bool>,
}
```

三个字段均使用 `Mutex` 保护，确保线程安全。`RimeEngine` 实现 `Send + Sync` 特性。

**关键数据结构**:

| 结构体 | 字段 | 说明 |
|--------|------|------|
| `ContextData` | `preedit`, `candidates`, `page_no`, `is_last_page`, `highlighted_index`, `commit_text` | 每次按键处理后的上下文快照（可序列化） |
| `CandidateData` | `text`, `comment` | 单个候选词信息 |
| `StatusData` | `is_composing`, `is_ascii_mode` | 输入状态快照 |

**核心方法**:

| 方法 | 签名 | 说明 |
|------|------|------|
| `new()` | `-> Self` | 构造未初始化的引擎实例 |
| `initialize()` | `(&self, app_handle: &AppHandle) -> Result<(), String>` | 加载 librime.dll，设置数据目录，调用 setup/initialize API |
| `ensure_session()` | `(&self) -> Result<RimeSessionId, String>` | 确保存在 RIME 会话，不存在则创建 |
| `process_key()` | `(&self, keycode: i32, modifiers: i32) -> Result<Option<ContextData>, String>` | 处理按键事件 → 返回上下文（候选词列表） |
| `select_candidate()` | `(&self, index: i32) -> Result<Option<ContextData>, String>` | 选择候选词 → 获取上屏文本 |
| `clear_composition()` | `(&self) -> Result<(), String>` | 清除当前编码串 |
| `get_status()` | `(&self) -> Result<StatusData, String>` | 查询当前输入状态 |
| `destroy_session()` | `(&self) -> Result<(), String>` | 销毁 RIME 会话 |
| `destroy()` | `(&self) -> Result<(), String>` | 销毁会话 + finalize 引擎 |

**初始化流程**:
1. 动态加载 `librime.dll`
2. 获取 `RimeApi` 函数表
3. 获取应用资源的 `rime/` 目录（共享数据）和用户数据目录 `%LOCALAPPDATA%/Lexi/rime/`
4. 填充 `RimeTraits` → 调用 `setup()` → 调用 `initialize()`

---

### 4.2 TSF 服务模块 (`crates/tsf-service/`)

这是一个独立的 Rust `cdylib` crate，编译为 `lexi_tsf.dll`，注册为 Windows COM 服务器以实现 TSF 输入法接口。

#### 4.2.1 `tsf-service/src/lib.rs` — DLL 入口 & COM ClassFactory

**文件路径**: [lib.rs](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\lib.rs)

**CLSID**: `{12340001-0000-0000-C000-000000000046}`

**导出函数**:

| 函数 | 说明 |
|------|------|
| `DllMain()` | DLL 加载/卸载回调；加载时保存 HINSTANCE；卸载时断开 Pipe |
| `DllGetClassObject()` | COM 入口，验证 CLSID 后创建 `LexiClassFactory` |
| `DllCanUnloadNow()` | 检查是否有活跃文本服务，决定是否允许卸载 |
| `DllRegisterServer()` | 注册 TSF 文本服务（目前为占位实现） |
| `DllUnregisterServer()` | 注销 TSF 文本服务（目前为占位实现） |

**COM ClassFactory**: `LexiClassFactory`
- `CreateInstance()` — 创建 `LexiTextService` 实例，将其作为 `IUnknown` 返回，查询请求的接口

---

#### 4.2.2 `tsf-service/src/text_service.rs` — TSF 文本服务

**文件路径**: [text_service.rs](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\text_service.rs)

**核心类**: `LexiTextService`

实现接口: `ITfTextInputProcessor`, `ITfTextInputProcessorEx`

```rust
pub struct LexiTextService {
    client_id: Mutex<u32>,
    thread_mgr: Mutex<Option<ITfThreadMgr>>,
    active: Mutex<bool>,
    key_sink_installed: Mutex<bool>,
}
```

**生命周期**: `Activate()` → 按键监听 → `Deactivate()`

- `Activate(ptim, tid)` — 保存 ClientId 和 ThreadMgr，尝试连接 Named Pipe，安装键盘事件接收器
- `Deactivate(ptim, tid)` — 卸载键盘事件接收器，断开 Pipe
- `ActivateEx(ptim, tid, flags)` — TSF 扩展版本的激活，直接委托给 `Activate()`

**核心类**: `LexiKeyEventSink`

实现接口: `ITfKeyEventSink`

- `OnKeyDown(pic, wParam, lParam)` — **关键回调**：将按键事件序列化为 JSON `{"type":"keydown","keycode":...,"modifiers":...}`，通过 Named Pipe 发送给 Tauri 主进程

---

#### 4.2.3 `tsf-service/src/pipe_client.rs` — Named Pipe 客户端

**文件路径**: [pipe_client.rs](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\pipe_client.rs)

**管道名称**: `\\.\pipe\LexiInputMethod`  
**超时**: 5000ms

| 函数 | 说明 |
|------|------|
| `connect()` | 使用 `CreateFileW` 打开命名管道（支持 OVERLAPPED I/O），句柄保存在全局 `PIPE_HANDLE` 中 |
| `disconnect()` | 关闭管道句柄 |
| `send_message(data)` | 异步写入请求 → 等待完成 → 异步读取响应 → 返回 `Option<String>` |

通信协议：
- **请求**: UTF-8 JSON 字符串（如 `{"type":"keydown","keycode":65,"modifiers":0}`）
- **响应**: UTF-8 JSON 字符串（如 `{"handled":true,"commit":"文本"}`）
- 缓冲区大小: 4096 字节

---

### 4.3 主程序模块 (`src/`)

#### 4.3.1 `src/main.rs` — 程序入口

**文件路径**: [main.rs](file:///d:\desktop\project\Lexi\src-tauri\src\main.rs)

仅一行代码，调用 `lexi_lib::run()`。Release 模式下隐藏控制台窗口（`windows_subsystem = "windows"`）。

---

#### 4.3.2 `src/lib.rs` — 应用核心

**文件路径**: [lib.rs](file:///d:\desktop\project\Lexi\src-tauri\src\lib.rs)

**核心状态**: `AppState`

```rust
struct AppState {
    engine: RimeEngine,             // RIME 引擎实例
    initialized: AtomicBool,        // 初始化标志
    bar_state: Arc<Mutex<BarData>>, // 候选栏共享状态
    bar_hwnd: Arc<Mutex<isize>>,    // 候选栏窗口句柄
}
```

**Tauri 命令（前端可调用）**:

| 命令 | 说明 |
|------|------|
| `init_ime()` | 初始化 RIME 引擎（加载 librime.dll） |
| `process_key(keycode, modifiers)` | 处理按键：调用引擎 → 更新候选栏状态 → 触发渲染 |
| `select_candidate(index)` | 选择候选词上屏 |
| `clear_composition()` | 清除编码串 |
| `get_ime_status()` | 获取当前输入状态（中英文模式等） |
| `update_bar_theme(theme)` | 切换候选栏主题（light/dark） |
| `update_bar_color(r, g, b)` | 设置候选栏主色调 |
| `update_bar_position(x, y)` | 更新候选栏位置（跟随光标） |

**应用启动流程** (`run()`):

1. 创建 `BarData` 共享状态和窗口句柄容器
2. 启动候选栏线程 (`start_bar()`) — 独立线程运行 Win32 消息循环
3. 构建 Tauri 应用：
   - 注册 `tauri_plugin_opener` 插件
   - **setup 阶段**:
     - 创建系统托盘菜单（偏好设置 / 退出）
     - 启动 Named Pipe 服务器（`PipeServer::start()`）
     - 注册 `pipe_handler` 处理来自 TSF DLL 的消息
   - 注入 `AppState` 到 Tauri 状态管理器
   - 注册所有 Tauri 命令
4. 运行 Tauri 应用

**Pipe Handler** (`create_pipe_handler`):
- 解析 TSF DLL 发来的 JSON 消息
- 若 `type == "keydown"`，通过 Tauri `emit("tsf_key_event", ...)` 转发给前端
- 返回 `{"handled":true}` 或 `{"handled":false}`

---

#### 4.3.3 `src/pipe_server.rs` — Named Pipe 服务端

**文件路径**: [pipe_server.rs](file:///d:\desktop\project\Lexi\src-tauri\src\pipe_server.rs)

**管道名称**: `\\.\pipe\LexiInputMethod`（与 TSF DLL 客户端一致）  
**缓冲区**: 4096 字节

**核心类**: `PipeServer`

```rust
pub struct PipeServer {
    running: Mutex<bool>,
    notify: Notify,        // tokio 通知机制用于优雅关闭
}
```

| 方法 | 说明 |
|------|------|
| `start(handler)` | 创建命名管道 → 循环接受连接 → 每个连接 spawn 独立任务处理 |
| `stop()` | 设置 running=false → 通知等待循环退出 |

**客户端处理** (`handle_client`):
- 循环读取请求 → 调用 handler 处理 → 写回响应
- 读取 0 字节或出错时断开连接

---

#### 4.3.4 `src/candidate_bar.rs` — 候选栏窗口

**文件路径**: [candidate_bar.rs](file:///d:\desktop\project\Lexi\src-tauri\src\candidate_bar.rs)

这是一个原始的 Win32 窗口，使用 Skia 进行像素级渲染，通过 `UpdateLayeredWindow` 实现毛玻璃 Alpha 混合效果。

**核心数据结构**: `BarData`

```rust
pub struct BarData {
    pub preedit: String,          // 编码串（如 "ni'hao"）
    pub candidates: Vec<String>,  // 候选词列表
    pub active_index: usize,      // 当前高亮候选词索引
    pub page_no: usize,           // 当前页码
    pub total_pages: usize,       // 总页数
    pub visible: bool,            // 可见性
    pub pos_x: i32,               // 窗口 X 坐标（跟随光标）
    pub pos_y: i32,               // 窗口 Y 坐标
    pub theme: String,            // "light" | "dark"
    pub primary_color: (u8, u8, u8), // 主色调 RGB
}
```

**窗口属性**:
- 样式: `WS_POPUP | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`
- 特点: 无焦点（不抢输入焦点）、透明穿透（鼠标事件穿透）、分层窗口（支持 Alpha 混合）、置顶

**核心函数**:

| 函数 | 说明 |
|------|------|
| `start_bar(state, hwnd_out)` | 在独立线程中启动候选栏窗口 |
| `run_bar(state, hwnd_out)` | Win32 消息循环：注册窗口类 → 创建窗口 → 初始化 Skia → 循环 `GetMessageW` |
| `bar_wndproc(hwnd, msg, wparam, lparam)` | 窗口过程：处理 `RENDER_MSG`（重新渲染）和 `WM_DESTROY` |
| `render_frame(hwnd, data, typeface)` | 创建 DIB Section → Skia 绘制 → `UpdateLayeredWindow`（Alpha 混合） |
| `draw_skia(w, h, data, typeface, bits)` | Skia 渲染管线：透明背景 → 阴影 + 圆角背景 → 候选词文本 |
| `draw_background(canvas, w, h, is_dark)` | 绘制毛玻璃效果背景（圆角 + 阴影 + 半透明填充） |
| `draw_candidates_skia(...)` | 绘制候选词：序号 + 文本 + 高亮条 + 页码指示器 |
| `signal_update(hwnd)` | 发送 `RENDER_MSG` 触发重绘（由主线程调用） |

**渲染管线**:
1. 根据候选词数量计算窗口尺寸（每个候选词约 68px 宽）
2. 创建内存 DIB Section（32位 BGRA）
3. 使用 Skia 绘制：透明背景 → 阴影蒙版 → 圆角矩形背景 → 候选词文本
4. 通过 `UpdateLayeredWindow` 将内存位图合成到屏幕（支持每像素 Alpha）

**字体选择**:
- 优先 `Microsoft YaHei`（微软雅黑），回退 `Arial`

---

### 4.4 部署脚本 (`scripts/`)

#### `register-tsf.ps1`
- 要求管理员权限运行
- 注册 COM 服务器：`HKLM\SOFTWARE\Classes\CLSID\{CLSID}\InProcServer32`
- 注册 TSF 输入处理器：`HKLM\SOFTWARE\Microsoft\CTF\TIP\{CLSID}\LanguageProfile\0x00000804`
- 设置键盘类别 GUID

#### `unregister-tsf.ps1`
- 要求管理员权限运行
- 移除 TSF 输入处理器注册表项
- 移除 COM 服务器注册表项

---

## 五、Cargo 工作区结构

### 5.1 Workspace 成员

```
[workspace]
members = ["crates/tsf-service"]
```

### 5.2 Crate 清单

| Crate 名 | 类型 | 产出物 | 说明 |
|----------|------|--------|------|
| `lexi` | bin + lib | `lexi.exe` + `lexi_lib.dll` | 主应用：`lib.rs`→库, `main.rs`→exe |
| `lexi-tsf` | cdylib | `lexi_tsf.dll` | TSF COM 输入法 DLL |

### 5.3 关键依赖 (lexi crate)

| 依赖 | 版本 | 用途 |
|------|------|------|
| `tauri` | v2 | 应用框架（含 tray-icon feature） |
| `tauri-plugin-opener` | v2 | URL/文件打开插件 |
| `tokio` | v1 (full) | 异步运行时（Named Pipe 服务端） |
| `serde` / `serde_json` | v1 | 序列化/反序列化 |
| `skia-safe` | v0.93 | Skia 图形引擎（含 textlayout） |
| `windows` | v0.58 | Win32 API 绑定（GDI、Pipes、Threading、UI 等） |
| `dirs` | v5 | 系统目录获取 |
| `image` | v0.25 | 图标 PNG 加载 |
| `raw-window-handle` | v0.6 | 原始窗口句柄抽象 |

### 5.4 关键依赖 (lexi-tsf crate)

| 依赖 | 版本 | 用途 |
|------|------|------|
| `windows` | v0.58 | Win32 + TSF COM API 绑定 |
| `windows-core` | v0.58 | COM 基础设施（implement 宏等） |
| `serde` / `serde_json` | v1 | Pipe 消息序列化 |

---

## 六、配置说明

### 6.1 `tauri.conf.json` 关键配置

```json
{
  "productName": "Lexi 输入法",
  "identifier": "cn.neamyoo.lexi",
  "build": { "frontendDist": "../src" },
  "app": {
    "windows": [{
      "label": "settings",
      "title": "Lexi 设置",
      "width": 480, "height": 560,
      "decorations": false,      // 无边框
      "transparent": true,       // 透明背景
      "visible": false,          // 默认隐藏
      "center": true
    }],
    "security": { "csp": null }  // 无 CSP 限制
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/32x32.png", "icons/128x128.png", ...]
  }
}
```

### 6.2 `capabilities/default.json`

授予主窗口以下权限:
- `core:default` — Tauri 核心 API
- `opener:default` — 打开外部资源
- `core:window:*` — 窗口显示/隐藏/位置/大小控制

---

## 七、项目运行方式

### 7.1 前置条件

- [Rust 工具链](https://rustup.rs/) (edition 2021)
- Windows 10/11 操作系统
- [librime.dll](https://rime.im/) 放置在 `src-tauri/target/debug/` 或系统 PATH 中
- RIME 词库数据（如 朙月拼音）放置在应用资源目录下的 `rime/` 文件夹

### 7.2 构建步骤

```powershell
# 1. 构建 TSF DLL
cd src-tauri
cargo build -p lexi-tsf

# 2. 构建主应用
cargo build

# 3. 注册输入法（需管理员权限）
cd ../scripts
.\register-tsf.ps1
```

### 7.3 开发运行

```powershell
# 开发模式运行（自动重载前端）
cargo tauri dev

# 仅构建 Rust 后端
cargo build
```

### 7.4 注销输入法

```powershell
# 需管理员权限
.\scripts\unregister-tsf.ps1
```

### 7.5 发布打包

```powershell
cargo tauri build
```

---

## 八、进程间通信协议

### 8.1 Named Pipe 规格

| 属性 | 值 |
|------|-----|
| 管道路径 | `\\.\pipe\LexiInputMethod` |
| 通信模式 | 请求-响应（同步） |
| 缓冲区大小 | 4096 字节 |
| 序列化格式 | JSON (UTF-8) |
| 超时 | 5000ms |

### 8.2 消息格式

**请求（TSF DLL → 主进程）**:
```json
{
  "type": "keydown",
  "keycode": 65,
  "modifiers": 0
}
```

**响应（主进程 → TSF DLL）**:
```json
{
  "handled": true,
  "commit": "文本"
}
```

---

## 九、线程模型

| 线程 | 职责 | 通信机制 |
|------|------|----------|
| **主线程** | Tauri 事件循环、系统托盘、WebView | — |
| **候选栏线程** | Win32 消息循环 + Skia 渲染 | `Arc<Mutex<BarData>>` + `PostMessageW(RENDER_MSG)` |
| **Tokio 异步运行时** | Named Pipe 服务端 | tokio task spawn per connection |
| **Pipe 客户端线程** | TSF DLL 中同步 Pipe I/O | Win32 OVERLAPPED I/O |

所有跨线程共享状态通过 `Arc<Mutex<T>>` 保护。

---

## 十、COM 接口注册信息

| 属性 | 值 |
|------|-----|
| CLSID | `{12340001-0000-0000-C000-000000000046}` |
| Profile GUID | `{12340001-0000-0000-C000-000000000047}` |
| 语言配置 | `0x00000804` (中文简体) |
| ThreadingModel | Apartment |
| 实现接口 | `ITfTextInputProcessor`, `ITfTextInputProcessorEx`, `ITfKeyEventSink` |
| 注册表根路径 | `HKLM\SOFTWARE\Microsoft\CTF\TIP\{CLSID}` |

---

## 十一、RIME 数据目录布局

```
应用资源目录/rime/           # 共享数据（只读，随应用分发）
├── default.yaml             # 全局配置
├── luna_pinyin.schema.yaml  # 朙月拼音方案
├── luna_pinyin.dict.yaml    # 朙月拼音词库
└── ...

%LOCALAPPDATA%/Lexi/rime/    # 用户数据（可写）
├── user.yaml                # 用户词典
├── luna_pinyin.userdb/      # 用户调频数据库
└── ...
```

---

## 十二、开发路线图（参考设计文档）

| 阶段 | 内容 | 状态 |
|------|------|------|
| **MVP** | 基础 TSF 集成 + RIME 引擎 + 候选栏渲染 + 拼音输入 | 进行中 |
| **v1** | 设置面板、主题切换、v 模式、中英文混合、表情快捷输入 | 规划中 |
| **v2** | macOS 支持 | 规划中 |
| **v3** | 移动端支持 | 远期 |

---

## 十三、文件索引

| 文件 | 行数 | 核心内容 |
|------|------|----------|
| [main.rs](file:///d:\desktop\project\Lexi\src-tauri\src\main.rs) | 4 | 程序入口 |
| [lib.rs](file:///d:\desktop\project\Lexi\src-tauri\src\lib.rs) | 205 | 应用核心：状态管理、Tauri 命令、启动流程 |
| [pipe_server.rs](file:///d:\desktop\project\Lexi\src-tauri\src\pipe_server.rs) | 104 | Named Pipe 服务端 |
| [candidate_bar.rs](file:///d:\desktop\project\Lexi\src-tauri\src\candidate_bar.rs) | 334 | Skia + Win32 候选栏渲染 |
| [ime/mod.rs](file:///d:\desktop\project\Lexi\src-tauri\src\ime\mod.rs) | 1 | IME 模块入口 |
| [ime/rime/mod.rs](file:///d:\desktop\project\Lexi\src-tauri\src\ime\rime\mod.rs) | 337 | RIME 引擎封装 |
| [ime/rime/ffi.rs](file:///d:\desktop\project\Lexi\src-tauri\src\ime\rime\ffi.rs) | 224 | RIME C FFI 绑定 |
| [tsf-service/lib.rs](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\lib.rs) | 132 | DLL 入口、COM ClassFactory |
| [tsf-service/text_service.rs](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\text_service.rs) | 184 | TSF 文本服务 |
| [tsf-service/pipe_client.rs](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\pipe_client.rs) | 176 | Named Pipe 客户端 |
| [Cargo.toml](file:///d:\desktop\project\Lexi\src-tauri\Cargo.toml) | 37 | 工作区配置与依赖 |
| [tauri.conf.json](file:///d:\desktop\project\Lexi\src-tauri\tauri.conf.json) | 40 | Tauri 应用配置 |
| [register-tsf.ps1](file:///d:\desktop\project\Lexi\scripts\register-tsf.ps1) | 66 | TSF 注册脚本 |
| [unregister-tsf.ps1](file:///d:\desktop\project\Lexi\scripts\unregister-tsf.ps1) | 24 | TSF 注销脚本 |
| [lexi-ime-design.md](file:///d:\desktop\project\Lexi\.grill\lexi-ime-design.md) | 43 | 产品设计文档 |
