# Lexi

> [!WARNING]
> 本项目仍在积极开发中，尚不适合日常使用。
> 许多核心功能尚未完善或不可用。

> **English version available at [README.md](README.md).**

一款极简的 Windows 拼音输入法，基于 Tauri v2 + Rust + Skia 构建。

## 特性

- **RIME 驱动** — 依托成熟的中州韵输入引擎
- **Skia 自绘** — 候选栏像素级渲染，毛玻璃极简风格
- **轻量** — 追求视觉精致的同时保持资源效率
- **主题可配** — 支持浅色/深色主题，可自定义主色调

## 前置要求

- [Rust 工具链](https://rustup.rs/) (edition 2021)
- Windows 10/11
- [librime.dll](https://rime.im/) 需放置在 `src-tauri/target/debug/` 或系统 `PATH` 中

## 构建与运行

```powershell
cd src-tauri

# 构建 TSF 输入法 DLL
cargo build -p lexi-tsf

# 构建主程序
cargo build

# 开发模式运行
cargo tauri dev
```

## 注册 / 注销

```powershell
# 注册为系统输入法（需要管理员权限）
.\scripts\register-tsf.ps1

# 注销
.\scripts\unregister-tsf.ps1
```

## 贡献指南

欢迎参与 Lexi 的开发！详情请阅读 **[CONTRIBUTING.md](CONTRIBUTING.md)**。

## 许可

Apache 2.0 — 详见 [LICENSE](./LICENSE)。
