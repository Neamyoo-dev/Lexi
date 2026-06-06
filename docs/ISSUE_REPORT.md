# Lexi 输入法 — 代码问题全面审查报告

> 审查范围：全量 Rust 源码（8 个文件，~1300 行）+ 2 个部署脚本
> 审查维度：逻辑问题 | 使用问题 | 完整性问题 | 可重构问题 | 可重写问题 | 性能问题

---

## 修复状态总览

| 类别 | 问题数 | 已修复 |
|------|--------|--------|
| 🔴 逻辑问题 | 12 (L1-L12) | ✅ 全部修复 |
| 🟠 使用问题 | 1 (U1) | ✅ 已修复 |
| ⚪ 完整性问题 | 8 (C1-C8) | ✅ C1-C4, C6-C8 已修复 |
| 🔵 可重构问题 | 5 (R1-R5) | ✅ R1 已修复，其余待重构 |
| 🟢 性能问题 | 3 (P1-P3) | ✅ P1-P2 已修复，P3 待优化 |

## 剩余待修复问题

### ⚪ 完整性问题

#### C1 [Critical] 缺少 TSF 文本组合管理，无法上屏文字
- ✅ **已修复**

#### C2 [Critical] 无优雅关闭流程
- ✅ **已修复**

#### C3 [Critical] 候选栏不跟随光标（固定位置）
- ✅ **已修复**

#### C5 [High] RimeEngine 缺少 Drop
- ✅ **已修复**

#### C6 [High] DllRegisterServer 为空
- ✅ **已修复**

#### C7 [Medium] 前端代码缺失
- ✅ **已修复**

#### C8 [Medium] 无错误日志/诊断机制
- ✅ **已修复**

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
