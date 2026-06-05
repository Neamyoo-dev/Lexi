# Lexi 输入法 — 代码问题全面审查报告

> 审查范围：全量 Rust 源码（8 个文件，~1300 行）+ 2 个部署脚本  
> 审查维度：🔴 逻辑问题 | 🟠 使用问题 | ⚪ 完整性问题 | 🔵 可重构问题 | 🟣 可重写问题 | 🟢 性能问题  
> 严重程度：🔴 Critical → 必须立即修复 | 🟠 High → 优先级高 | 🟡 Medium → 应尽快修复 | ⚪ Low → 改进项

---

## 一、🔴 逻辑问题（Logic Bugs）

### L1 [🔴 Critical] `OnKeyDown` 吞噬所有按键 — 系统级阻塞

**文件**: [text_service.rs:L138-L157](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\text_service.rs#L138-L157)

```rust
fn OnKeyDown(&self, _pic: &ITfContext, wParam: WPARAM, _lParam: LPARAM) -> HRESULT {
    ...
    HRESULT(0) // ← S_OK = "我吃掉了这个键"
}
```

在 TSF 协议中，`ITfKeyEventSink::OnKeyDown` 返回 `S_OK` (HRESULT(0)) 表示 **"我已处理此按键，不要传递给其他处理器"**。当前代码无论 RIME 是否实际处理了按键，都返回 `S_OK`。这意味着 **Lexi 会吞噬系统中所有应用程序的全部键盘输入**，包括在其他应用中按 Enter、方向键、功能键等。

同样，`OnTestKeyDown` 也固定返回 `S_OK`，表示对所有按键都表达了处理兴趣。

**影响**：安装 Lexi 后，整个系统的键盘输入被劫持。必须修复才能进行任何实际测试。

**修复方向**：
- `OnTestKeyDown` 应根据按键范围选择性返回 `S_OK`/`S_FALSE`
- `OnKeyDown` 应仅在 RIME 实际消费按键后返回 `S_OK`，否则返回 `S_FALSE`
- 需要 Pipe 通信返回"是否已处理"标志

---

### L2 [🔴 Critical] `process_key` 返回值语义混淆 — 候选栏闪烁/错误隐藏

**文件**: [lib.rs:L33-L53](file:///d:\desktop\project\Lexi\src-tauri\src\lib.rs#L33-L53) / [mod.rs:L154-L166](file:///d:\desktop\project\Lexi\src-tauri\src\ime\rime\mod.rs#L154-L166)

```rust
// engine: process_key returns None when:
//   1. RIME processed=FALSE (key not relevant at all)
//   2. RIME context has no candidates (e.g., after commit)
pub fn process_key(&self, keycode: i32, modifiers: i32) -> Result<Option<ContextData>, String> {
    ...
    if processed == FALSE {
        return Ok(None);   // Case 1
    }
    self.read_context(sid, api)  // May return Ok(None) — Case 2
}

// lib.rs: both cases treated as "clear bar"
} else {
    let mut bar = state.bar_state.lock().unwrap();
    bar.visible = false;         // ← 隐藏候选栏
    bar.candidates.clear();
    ...
}
```

当用户正在输入拼音（候选栏可见），按一下非输入键（如 Ctrl、Shift、光标键），RIME 返回 `FALSE` → 返回 `None` → 候选栏被隐藏！用户看到候选栏闪烁消失又出现。

**影响**：严重影响输入体验，候选栏不断闪烁。

**修复方向**：
- 区分 `KeyNotHandled` 和 `NoCandidates` 两种状态
- 非处理键不应改变候选栏可见性
- 或改用 `Result<Option<KeyResult>, ...>` 枚举

---

### L3 [🔴 Critical] `RimeLibrary::load()` 内存泄漏 — DLL 句柄泄漏

**文件**: [ffi.rs:L163-L204](file:///d:\desktop\project\Lexi\src-tauri\src\ime\rime\ffi.rs#L163-L204)

```rust
pub fn load() -> Option<Self> {
    let lib = unsafe { LoadLibraryA(lib_name.as_ptr() as *const u8) };
    if lib.is_null() { return None; }

    let func_ptr = unsafe { GetProcAddress(lib, ...) }?;  // ← ? 提前返回
    // lib 已加载但不会释放!

    let api_ptr = unsafe { (get_api_fn)() };
    if api_ptr.is_null() {
        return None;  // ← 同样泄漏 lib 句柄
    }
    ...
}
```

`LoadLibraryA` 成功后，如果后续 `GetProcAddress` 失败或 `get_api_fn` 返回空，函数通过 `?` 或 `return None` 提前退出，已加载的 DLL 句柄 **永远不会被 `FreeLibrary` 释放**。

**影响**：反复初始化失败会累积 DLL 引用计数，直到进程退出。

**修复方向**：添加 guard/RAII 结构，或确保所有提前返回路径都调用 `FreeLibrary`。

---

### L4 [🔴 Critical] `total_pages` 计算错误 — 单页结果显示错误页码

**文件**: [lib.rs:L42](file:///d:\desktop\project\Lexi\src-tauri\src\lib.rs#L42)

```rust
bar.total_pages = if ctx.is_last_page { ctx.page_no + 1 } else { ctx.page_no + 2 } as usize;
```

当只有一页候选词时（`page_no=0, is_last_page=true`），`total_pages = 0 + 1 = 1` ✓。  
但当 `is_last_page=false, page_no=0`（至少还有一页），`total_pages = 0 + 2 = 2` ✓。  
问题：`page_no=0, is_last_page=true` 应该 `total_pages=1`，但当候选词刚好一页但 `is_last_page` 可能由于 RIME 内部逻辑偶尔返回 `false` 时会显示 `2/2`。更关键的是，RIME 不提供总页数 API，这个计算只是近似值。

**影响**：底部页码指示器不准确。

**修复方向**：RIME 本身不提供总页数，建议移除总页数显示，或改为 `> 箭头` 指示有下一页。

---

### L5 [🟠 High] 修饰键 keycode 硬编码为 0 — 中英文切换失效

**文件**: [text_service.rs:L146](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\text_service.rs#L146)

```rust
let msg = format!(r#"{{"type":"keydown","keycode":{},"modifiers":0}}"#, keycode);
//                                                             ^^^ 永远为 0
```

`lParam` 包含了 [完整的按键状态信息](https://learn.microsoft.com/en-us/windows/win32/inputdev/about-keyboard-input#keystroke-message-flags)（重复计数、扫描码、前一个按键状态等），但代码完全忽略了这些信息。Shift/Ctrl 状态无法传递给 RIME，导致：
- 无法通过 Shift 切换中英文模式
- 无法使用 Ctrl+字母的快捷操作
- RIME 无法正确处理大写输入

**影响**：核心功能缺失。

**修复方向**：从 `lParam` 提取修饰键位，正确构造 modifier mask 传给 RIME。

---

### L6 [🟠 High] `PipeServer::start()` 每次循环泄漏旧管道

**文件**: [pipe_server.rs:L36-L61](file:///d:\desktop\project\Lexi\src-tauri\src\pipe_server.rs#L36-L61)

```rust
loop {
    let server = ServerOptions::new().create(PIPE_NAME)?;  // CreateNamedPipeW
    let connect_result = tokio::select! {
        r = server.connect() => r,   // ConnectNamedPipe
        _ = self.notify.notified() => { ... return Ok(()); }
    };
    match connect_result {
        Ok(()) => {
            tokio::spawn(async move { handle_client(server, handler).await; });
            // ← 问题：server 被 move 进 task，但 task 结束时
            // 没有调用 DisconnectNamedPipe，旧的 pipe handle 被 abandon
            // 下一次循环 CreateNamedPipeW 创建新的实例
        }
        ...
    }
}
```

Windows Named Pipe 的正确用法是：创建实例 → 连接 → 通信 → `DisconnectNamedPipe` → 重用该实例。当前代码每次创建新实例，旧的通信用完后 handle 依赖 Drop 关闭，但缺少显式 `DisconnectNamedPipe`。对于多客户端场景，这会导致大量废弃 pipe 实例。

**影响**：多次连接/断开会累积废弃 Pipe 句柄。

**修复方向**：在 `handle_client` 退出前调用 `disconnect()`，或复用 pipe server 实例。

---

### L7 [🟠 High] `handle_client` 无消息帧定界 — JSON 片段风险

**文件**: [pipe_server.rs:L82-L103](file:///d:\desktop\project\Lexi\src-tauri\src\pipe_server.rs#L82-L103)

```rust
let request = String::from_utf8_lossy(&buffer[..n]).to_string();
let response = handler(request);
```

如果 TSF DLL 发送的消息超过 4096 字节，或 TCP/Pipe 层分片，一次 `read` 可能只收到部分 JSON（如 `{"type":"keyd`），导致 JSON 解析失败。当前没有长度前缀、分隔符或任何帧协议。

**影响**：大消息或网络异常时会丢失数据。当前键事件消息很短（<100字节），风险低但不是零。

**修复方向**：添加长度前缀（如 4 字节 LE u32）或使用换行符分隔。

---

### L8 [🟡 Medium] `RimeContext::candidates()` 负值 `num_candidates` 导致 UB

**文件**: [ffi.rs:L124-L143](file:///d:\desktop\project\Lexi\src-tauri\src\ime\rime\ffi.rs#L124-L143)

```rust
let num = self.menu.num_candidates as usize;  // c_int → usize
for i in 0..num {  // 如果 num_candidates 为负，转为巨大的 usize
    let cand = unsafe { &*self.menu.candidates.add(i) };
    ...
}
```

`RimeMenu.num_candidates` 是 `c_int`（i32），如果因 RIME 内部错误返回负值，`as usize` 会转换为一个巨大的正数（如 -1 → 4294967295），导致越界访问 `candidates` 指针数组 → **Undefined Behavior**。

**影响**：RIME 异常状态下可能崩溃。

**修复方向**：添加 `num_candidates <= 0` 的防御检查。

---

### L9 [🟡 Medium] `select_candidate` 不验证 index 范围

**文件**: [lib.rs:L56](file:///d:\desktop\project\Lexi\src-tauri\src\lib.rs#L56) / [mod.rs:L168-L209](file:///d:\desktop\project\Lexi\src-tauri\src\ime\rime\mod.rs#L168-L209)

`index` 参数是前端传入的 `i32`，负值或超大值直接透传给 RIME 的 `select_candidate`。RIME 内部可能返回错误但不一定崩溃，但这可能导致不可预期的行为。

**修复方向**：在 `select_candidate` 命令中验证 `0 <= index < len(candidates)`。

---

### L10 [🟡 Medium] `RimeEngine` 三个独立 `Mutex` — 潜在顺序死锁

**文件**: [mod.rs:L12-L16](file:///d:\desktop\project\Lexi\src-tauri\src\ime\rime\mod.rs#L12-L16)

```rust
pub struct RimeEngine {
    library: Mutex<Option<RimeLibrary>>,
    session_id: Mutex<Option<RimeSessionId>>,
    initialized: Mutex<bool>,
}
```

当前三个锁的获取顺序是：`session_id` → `library`（通过 `with_api`）。但如果未来有方法先获取 `library` 再获取 `session_id`，就会死锁。三把独立锁增加了维护负担。

**修复方向**：合并为 `Mutex<EngineInner>`。

---

### L11 [🟡 Medium] `bar_wndproc` 持有 Mutex 期间 clone 全量 BarData

**文件**: [candidate_bar.rs:L124](file:///d:\desktop\project\Lexi\src-tauri\src\candidate_bar.rs#L124)

```rust
let data = ctx.state.lock().unwrap().clone();
```

渲染线程持有 `bar_state` 锁的同时 clone 了包含所有候选词字符串的完整 `BarData`。这期间主线程的任何 `bar_state.lock()`（来自按键处理）都会被阻塞。对于高频打字场景，这会引起渲染延迟。

**修复方向**：使用 `Arc::make_mut` + 双缓冲，或先 drop lock 再 clone（先提取必要字段）。

---

### L12 [⚪ Low] `DllMain` 使用魔数代替常量

**文件**: [tsf-service/lib.rs:L31-L38](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\lib.rs#L31-L38)

```rust
match reason {
    1 => { ... }  // DLL_PROCESS_ATTACH
    0 => { ... }  // DLL_PROCESS_DETACH
    _ => {}
}
```

应使用 `windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH` 等常量。

---

## 二、🟠 使用问题（Usage / Correctness）

### U1 [🟠 High] `bar_state.lock().unwrap()` 全线崩溃风险

**文件**: [lib.rs](file:///d:\desktop\project\Lexi\src-tauri\src\lib.rs) 多处 (L37, L46, L61, L74, L88, L96, L104)

所有 Tauri 命令使用 `bar_state.lock().unwrap()`，如果候选栏线程 panic 导致 Mutex 中毒（poisoned），下一个按键事件会导致整个 Tauri 应用崩溃。

**修复方向**：使用 `lock().map_err(|e| e.to_string())?` 或 `lock().unwrap_or_else(|e| e.into_inner())`。

---

### U2 [🟠 High] `handle_client` 返回错误响应格式

**文件**: [pipe_server.rs:L89-L92](file:///d:\desktop\project\Lexi\src-tauri\src\pipe_server.rs#L89-L92)

```rust
let data = match response {
    Some(ref resp) => resp.as_bytes(),
    None => b"{}",  // ← 空 JSON，不是 {"handled":false}
};
```

当 handler 返回 `None` 时发送 `{}`，但 TSF 端期望 `{"handled":true/false}`。解析 `{}` 时 `msg.get("handled")` 会返回 `None`，可能触发意外行为。

---

### U3 [🟡 Medium] `OnKeyDown` 同步阻塞 TSF 线程最多 5 秒

**文件**: [text_service.rs:L148](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\text_service.rs#L148) / [pipe_client.rs:L59-L155](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\pipe_client.rs#L59-L155)

`pipe_client::send_message()` 是同步调用，使用 OVERLAPPED I/O + `WaitForSingleObject(5000ms)`。这意味着如果主进程繁忙，每次按键都可能阻塞 TSF 的键盘钩子线程长达 5 秒，导致系统输入完全冻结。

**影响**：用户打字时可能感到明显卡顿。

**修复方向**：缩短超时（如 200ms），或将 Pipe 通信改为纯异步。

---

### U4 [🟡 Medium] `expect("No font available")` 会 panic

**文件**: [candidate_bar.rs:L103](file:///d:\desktop\project\Lexi\src-tauri\src\candidate_bar.rs#L103)

```rust
let typeface = fm.match_family_style("Microsoft YaHei", FontStyle::default())
    .or_else(|| fm.match_family_style("Arial", FontStyle::default()))
    .expect("No font available");
```

如果系统缺少这两种字体（Windows PE 环境、某些精简版），候选栏线程直接 panic，导致主线程后续 `bar_state.lock()` 可能触发中毒。

**修复方向**：使用 `FontMgr::default().default_family()` 作为最终回退。

---

### U5 [⚪ Low] `CStr::from_ptr().to_str().unwrap_or("")` 吞掉内容

**文件**: [ffi.rs:L112](file:///d:\desktop\project\Lexi\src-tauri\src\ime\rime\ffi.rs#L112) 等处

如果 RIME 返回非 UTF-8 文本（极端情况），候选词内容会被静默替换为空字符串，用户看到空白候选词但不知道发生了什么。

**修复方向**：使用 `to_string_lossy()` 保留信息。

---

### U6 [⚪ Low] `#![allow(dead_code)]` 掩盖未使用导入

**文件**: [tsf-service/lib.rs:L1](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\lib.rs#L1)

`text_service.rs` 中 import 了很多未使用的 TSF 接口（`IEnumTfContexts`, `ITfCandidateListUIElement`, `ITfCompartmentMgr` 等 ~15 个），被 crate-level `allow(dead_code)` 掩盖。

**修复方向**：移除未使用的导入。

---

## 三、⚪ 完整性问题（Completeness / Missing）

### C1 [🔴 Critical] 缺少 TSF 文本组合管理 — 无法输入文本

**文件**: [text_service.rs](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\text_service.rs)

当前 TSF 服务只实现了 `ITfKeyEventSink::OnKeyDown` 将按键发送到主进程，但 **完全没有实现文本输入所需的 TSF 接口**：

| 缺少的接口 | 作用 |
|-----------|------|
| `ITfCompositionSink` | 管理组合（composition）生命周期 |
| `ITfTextEditSink` | 在文本上下文中执行编辑操作 |
| `ITfEditSession` | 通过 TSF 编辑会话将文本写入应用程序 |
| `ITfDisplayAttributeProvider` | 提供组合文本的下划线/高亮样式 |

**这意味着当前代码无法将任何文字实际输入到文本框中。** 按键可以截获，但没有代码调用 `ITfRange::SetText()` 或 `ITfInsertAtSelection::InsertTextAtSelection()` 将候选词写入目标应用。

**影响**：整个输入法无法工作 —— 选词后文字不会上屏。

**修复方向**：实现完整的 TSF 组合管理流程。

---

### C2 [🔴 Critical] 缺少优雅关闭流程

**文件**: [lib.rs](file:///d:\desktop\project\Lexi\src-tauri\src\lib.rs)

当用户退出应用时（托盘 `quit` → `app.exit(0)`）：

1. **RIME Engine 未销毁** — `destroy()` 从未被调用，RIME 会话泄露，用户词典可能未保存
2. **PipeServer 未停止** — `PipeServer::stop()` 从未调用，Tokio 任务继续运行
3. **候选栏线程未终止** — 线程被强制终止而无清理
4. **TSF DLL 仍连接** — 没有通知 TSF DLL 断开

**修复方向**：在 `app.exit(0)` 前添加清理逻辑（实现 `Drop` + 发出关闭信号）。

---

### C3 [🔴 Critical] 候选栏不跟随光标 — 固定位置

**文件**: [candidate_bar.rs](file:///d:\desktop\project\Lexi\src-tauri\src\candidate_bar.rs)

`BarData::default()` 中 `pos_x: 100, pos_y: 300` 是硬编码的。`update_bar_position` 命令存在，但没有任何代码从 TSF 获取光标位置并调用它。候选栏始终出现在 (100, 300) 而不是光标下方。

**影响**：候选栏位置完全错误，最基本的使用体验不成立。

**修复方向**：在 TSF 中调用 `ITfContext::GetSelection()` 获取光标屏幕坐标，通过 Pipe 传递给主进程。

---

### C4 [🔴 Critical] 预处理文本（preedit）存储但不渲染

**文件**: [candidate_bar.rs:L265-L324](file:///d:\desktop\project\Lexi\src-tauri\src\candidate_bar.rs#L265-L324)

```rust
let y_start = if data.preedit.is_empty() { 14.0 } else { 34.0 };
```

`preedit` 字段被正确存储，渲染时会影响候选词布局（y 偏移），但 **preedit 文本本身从未被绘制**（如 "ni'hao" 不显示）。用户看不到自己正在打的拼音。

**影响**：用户无法确认当前输入的内容。

**修复方向**：在候选栏上方绘制 preedit 文本。

---

### C5 [🟠 High] `RimeEngine` 缺少 `Drop` 实现

**文件**: [mod.rs](file:///d:\desktop\project\Lexi\src-tauri\src\ime\rime\mod.rs)

当 `AppState` 被 Tauri 销毁时，`RimeEngine` 的 `Drop` 不会自动调用 `destroy()` → `destroy_session()` → `finalize()`。RIME 会话和 DLL 资源不会正确释放。

**修复方向**：为 `RimeEngine` 实现 `Drop`。

---

### C6 [🟠 High] `DllRegisterServer` / `DllUnregisterServer` 为空实现

**文件**: [tsf-service/lib.rs:L89-L95](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\lib.rs#L89-L95)

```rust
fn register_ime() -> HRESULT { HRESULT(0) }
fn unregister_ime() -> HRESULT { HRESULT(0) }
```

COM 自注册函数仅为占位，依赖外部 PowerShell 脚本操作注册表。标准的 COM DLL 应该在 `DllRegisterServer` 中自行完成注册。

---

### C7 [🟡 Medium] 前端代码（HTML/CSS/JS）完全缺失

`tauri.conf.json` 中 `"frontendDist": "../src"` 指向的 `src/` 目录下没有任何 `.html`、`.css`、`.js` 文件。设置面板无法加载。

---

### C8 [🟡 Medium] 无错误日志/诊断机制

整个项目中，错误处理采用 `eprintln!` 打印到 stderr。Release 模式下 `windows_subsystem = "windows"` 没有控制台，错误信息完全不可见。没有日志文件、没有 Windows Event Log、没有崩溃报告。

---

## 四、🔵 可重构问题（Refactoring Needs）

### R1 [🟠 High] 主进程数据流过度绕弯

**文件**: [lib.rs:L110-L130](file:///d:\desktop\project\Lexi\src-tauri\src\lib.rs#L110-L130)

```
TSF DLL → Named Pipe → pipe_handler → Tauri emit → JS 前端 → Tauri command → RimeEngine
```

按键处理需要 6 跳，其中 JS 前端这一跳完全可以省略。pipe_handler 可以直接调用 `RimeEngine::process_key()` 并同步返回结果給 TSF DLL，无需经过 Tauri 事件系统和 WebView。

**修复方向**：pipe_handler 中直接调用引擎处理按键，将 Tauri emit 仅用于通知 UI 更新。

---

### R2 [🟡 Medium] `candidate_bar.rs` — GDI + Skia 紧耦合

**文件**: [candidate_bar.rs](file:///d:\desktop\project\Lexi\src-tauri\src\candidate_bar.rs)

Win32 窗口管理（`CreateWindowExW`、消息循环）和 Skia 渲染逻辑混在同一个文件中，难以测试，也无法复用渲染管线。

**修复方向**：抽离 `Renderer` trait，实现 `SkiaRenderer`，窗口管理独立。

---

### R3 [🟡 Medium] `render_frame()` — GDI 资源手动管理

**文件**: [candidate_bar.rs:L150-L194](file:///d:\desktop\project\Lexi\src-tauri\src\candidate_bar.rs#L150-L194)

```rust
let dib = CreateDIBSection(...);     // 手动创建
let dc = CreateCompatibleDC(...);    // 手动创建
// ... 使用 ...
SelectObject(dc, old_bmp);           // 手动恢复
DeleteDC(dc);                        // 手动删除
DeleteObject(dib);                   // 手动删除
```

GDI 对象没有 RAII 包装，panic 时必定泄漏。

**修复方向**：使用 `windows` crate 的 ownership 类型，或实现 RAII wrapper。

---

### R4 [🟡 Medium] `RimeEngine` 三锁 → 一锁

三把独立的 `Mutex` 增加死锁风险和认知负担。合并为 `Mutex<EngineInner>` 更安全。

---

### R5 [⚪ Low] IPC 协议用内联 JSON 字符串

**文件**: [lib.rs:L110-L130](file:///d:\desktop\project\Lexi\src-tauri\src\lib.rs#L110-L130) / [text_service.rs:L146](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\text_service.rs#L146)

```rust
// 手写 JSON 字符串拼接
format!(r#"{{"type":"keydown","keycode":{},"modifiers":0}}"#, keycode)
Some(r#"{"handled":false}"#.into())
```

应该定义 `#[derive(Serialize, Deserialize)]` 结构体，统一序列化/反序列化。

---

## 五、🟣 可重写问题（Rewrite Candidates）

### RW1 [🟡 Medium] PipeClient — raw Win32 OVERLAPPED

**文件**: [pipe_client.rs](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\pipe_client.rs)

当前实现使用手动 OVERLAPPED I/O + `CreateEventW` + `WaitForSingleObject`，代码冗长且容易出错。在拥有 `tokio` 和 `windows` crate 的环境中，可以改用 `tokio::net::windows::named_pipe::ClientOptions` 获得更简洁的异步实现。

注意：TSF DLL 在 COM STA 线程中运行，使用 `tokio` 需要独立的 runtime。

---

### RW2 [⚪ Low] 候选栏渲染 — 缺少平台抽象

当前完全绑定 Windows GDI + Skia，为 macOS 移植增加了难度。建议抽象渲染后端。

---

## 六、🟢 性能问题（Performance）

### P1 [🟡 Medium] 每帧渲染创建 GDI 对象

**文件**: [candidate_bar.rs:L150-L194](file:///d:\desktop\project\Lexi\src-tauri\src\candidate_bar.rs#L150-L194)

每次 `render_frame()` 调用都创建新的 DIB Section 和 CompatibleDC，绘制后立即销毁。对于高频打字（每秒 5-10 次渲染），GDI 对象的频繁创建/销毁会产生可感知的性能开销。

**修复方向**：在 `BarContext` 中缓存 DIB Section 和 DC，仅在窗口大小变化时重建。

---

### P2 [🟡 Medium] 每帧渲染创建 Skia Font 对象

**文件**: [candidate_bar.rs:L275-L277](file:///d:\desktop\project\Lexi\src-tauri\src\candidate_bar.rs#L275-L277)

```rust
let font_large = Font::from_typeface(typeface.clone(), 18.0);
let font_small = Font::from_typeface(typeface.clone(), 14.0);
let font_index = Font::from_typeface(typeface.clone(), 11.0);
```

三个 Font 对象每帧都重新创建。`typeface.clone()` 可能是引用计数操作，但 `Font` 构造仍有开销。

**修复方向**：在 `BarContext` 中缓存预创建的 Font 对象。

---

### P3 [⚪ Low] `process_key` 双锁开销

每次按键：`session_id.lock()` → `library.lock()`。对于 60+ WPM 打字，这会产生可测量的锁竞争。

**修复方向**：合并为单锁，或使用 `parking_lot::Mutex`。

---

## 七、缺陷汇总 Todo 列表

### 🔴 Critical（必须立即修复）

| # | 问题 | 文件 | 行号 |
|---|------|------|------|
| L1 | OnKeyDown 吞噬所有系统按键 | text_service.rs | L138-L157 |
| L2 | process_key 返回值语义混淆导致候选栏闪烁 | lib.rs / mod.rs | L33-L53 / L154-L166 |
| L3 | RimeLibrary::load() DLL 句柄泄漏 | ffi.rs | L163-L204 |
| L4 | total_pages 计算错误 | lib.rs | L42 |
| C1 | 缺少 TSF 文本组合管理，无法上屏文字 | text_service.rs | 全文 |
| C2 | 无优雅关闭流程 | lib.rs | L172 |
| C3 | 候选栏不跟随光标（固定位置） | candidate_bar.rs | L49-L50 |
| C4 | preedit 文本存储但不渲染 | candidate_bar.rs | L295 |

### 🟠 High（优先修复）

| # | 问题 | 文件 | 行号 |
|---|------|------|------|
| L5 | 修饰键硬编码为 0 | text_service.rs | L146 |
| L6 | PipeServer 循环泄漏 pipe 实例 | pipe_server.rs | L36-L61 |
| L7 | handle_client 无消息帧定界 | pipe_server.rs | L82-L103 |
| U1 | bar_state.lock().unwrap() 崩溃风险 | lib.rs | 多处 |
| U2 | handle_client 错误响应格式 | pipe_server.rs | L89-L92 |
| C5 | RimeEngine 缺少 Drop | mod.rs | L12-L17 |
| C6 | DllRegisterServer 为空 | lib.rs (tsf) | L89-L95 |
| R1 | 主进程数据流绕弯（6 跳） | lib.rs | L110-L130 |

### 🟡 Medium（应尽快修复）

| # | 问题 | 文件 | 行号 |
|---|------|------|------|
| L8 | RimeContext::candidates() 负值越界 | ffi.rs | L124-L143 |
| L9 | select_candidate 不验证 index 范围 | lib.rs | L56 |
| L10 | 三 Mutex 潜在死锁 | mod.rs | L12-L16 |
| L11 | bar_wndproc 持锁 clone | candidate_bar.rs | L124 |
| U3 | OnKeyDown 同步阻塞 5 秒 | text_service.rs | L148 |
| U4 | 字体缺失导致 panic | candidate_bar.rs | L103 |
| C7 | 前端代码缺失 | src/ | — |
| C8 | 无错误日志诊断 | 全局 | — |
| R2 | GDI+Skia 紧耦合 | candidate_bar.rs | 全文 |
| R3 | GDI 资源手动管理 | candidate_bar.rs | L150-L194 |
| R4 | RimeEngine 三锁 → 一锁 | mod.rs | L12-L16 |
| RW1 | PipeClient raw OVERLAPPED | pipe_client.rs | 全文 |
| P1 | 每帧创建 GDI 对象 | candidate_bar.rs | L150-L194 |
| P2 | 每帧创建 Skia Font | candidate_bar.rs | L275-L277 |

### ⚪ Low（改进项）

| # | 问题 | 文件 | 行号 |
|---|------|------|------|
| L12 | DllMain 魔数 | lib.rs (tsf) | L31-L38 |
| U5 | 非 UTF-8 文本静默丢失 | ffi.rs | L112 |
| U6 | allow(dead_code) 掩盖未使用导入 | lib.rs (tsf) | L1 |
| R5 | IPC 用内联 JSON 字符串 | lib.rs / text_service.rs | 多处 |
| RW2 | 缺少平台渲染抽象 | candidate_bar.rs | 全文 |
| P3 | process_key 双锁开销 | mod.rs | L154-L156 |

---

## 八、建议修复顺序

1. **第一优先级**（使输入法可工作）：
   - L1: 修复按键吞噬 → 恢复系统键盘功能
   - C1: 实现文本上屏 → 使输入法能实际输入文字
   - L5: 提取修饰键 → 中英文切换可用
   - C3 + C4: 光标跟随 + preedit 渲染 → 基本 UX 可用

2. **第二优先级**（稳定性）：
   - C2 + C5: 优雅关闭 + RimeEngine Drop
   - L2: 修复候选栏闪烁
   - L3: 修复 DLL 泄漏

3. **第三优先级**（质量改进）：
   - R1: 简化数据流
   - L6 + L7: Pipe 协议改进
   - U1: 崩溃容错
   - 性能优化项
