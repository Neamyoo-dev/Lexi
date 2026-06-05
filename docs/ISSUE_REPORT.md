# Lexi 输入法 — 代码问题全面审查报告

> 审查范围：全量 Rust 源码（8 个文件，~1300 行）+ 2 个部署脚本
> 审查维度：逻辑问题 | 使用问题 | 完整性问题 | 可重构问题 | 可重写问题 | 性能问题

---

## 修复状态总览

| 类别 | 问题数 | 已修复 |
|------|--------|--------|
| 🔴 逻辑问题 | 12 (L1-L12) | ✅ 全部修复 |
| 🟠 使用问题 | 1 (U1) | ✅ 已修复 |
| ⚪ 完整性问题 | 8 (C1-C8) | ✅ C4 已修复，其余待修复 |
| 🔵 可重构问题 | 5 (R1-R5) | ✅ R1 已修复，其余待重构 |
| 🟢 性能问题 | 3 (P1-P3) | ✅ P1-P2 已修复，P3 待优化 |

## 剩余待修复问题

### ⚪ 完整性问题

#### C1 [Critical] 缺少 TSF 文本组合管理，无法上屏文字
- **文件**: [text_service.rs](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\text_service.rs)
- **说明**: 未实现 `ITfCompositionSink`、`ITfTextEditSink`、`ITfEditSession` 接口

#### C2 [Critical] 无优雅关闭流程
- **文件**: [lib.rs](file:///d:\desktop\project\Lexi\src-tauri\src\lib.rs)
- **说明**: exit 前未清理 RIME 会话、PipeServer、候选栏线程

#### C3 [Critical] 候选栏不跟随光标（固定位置）
- **文件**: [candidate_bar.rs](file:///d:\desktop\project\Lexi\src-tauri\src\candidate_bar.rs)
- **说明**: 缺少 TSF 端获取光标屏幕坐标并传递

#### C5 [High] RimeEngine 缺少 Drop
- ✅ **已修复**

#### C6 [High] DllRegisterServer 为空
- **文件**: [tsf-service/lib.rs](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\lib.rs)

#### C7 [Medium] 前端代码缺失
- **说明**: `src/` 下缺少完整设置面板

#### C8 [Medium] 无错误日志/诊断机制
- **说明**: Release 模式下错误不可见

### 🔵 可重构问题

#### R2 [Medium] GDI+Skia 紧耦合
- **文件**: [candidate_bar.rs](file:///d:\desktop\project\Lexi\src-tauri\src\candidate_bar.rs)

#### R3 [Medium] GDI 资源手动管理（无 RAII）
- **文件**: [candidate_bar.rs](file:///d:\desktop\project\Lexi\src-tauri\src\candidate_bar.rs)

#### R4 [Medium] RimeEngine 三锁 → 一锁
- ✅ **已修复**

#### R5 [Low] IPC 用内联 JSON 字符串
- **文件**: [lib.rs](file:///d:\desktop\project\Lexi\src-tauri\src\lib.rs) / [text_service.rs](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\text_service.rs)

### 🟣 可重写问题

#### RW1 [Medium] PipeClient raw OVERLAPPED
- **文件**: [pipe_client.rs](file:///d:\desktop\project\Lexi\src-tauri\crates\tsf-service\src\pipe_client.rs)

#### RW2 [Low] 缺少平台渲染抽象
- **文件**: [candidate_bar.rs](file:///d:\desktop\project\Lexi\src-tauri\src\candidate_bar.rs)

### 🟢 性能问题

#### P3 [Low] process_key 双锁开销
- **文件**: [ime/rime/mod.rs](file:///d:\desktop\project\Lexi\src-tauri\src\ime\rime\mod.rs)
