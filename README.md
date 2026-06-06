# Lexi

> [!WARNING]
> This project is still under heavy development. It is not yet ready for daily use.
> Many core features are incomplete or non-functional.

> **中文版说明详见 [README_ZH.md](README_ZH.md)。**

A minimalist Chinese input method for Windows, built with Tauri v2 + Rust + Skia.

## Features

- **RIME-based** — leverages the proven librime engine for pinyin input
- **Skia-rendered** — custom-drawn candidate bar with frosted-glass aesthetic
- **Lightweight** — designed to be visually refined yet resource-efficient
- **Themeable** — light/dark themes with customizable accent colors

## Prerequisites

- [Rust toolchain](https://rustup.rs/) (edition 2021)
- Windows 10/11
- [librime.dll](https://rime.im/) placed in `src-tauri/target/debug/` or `PATH`

## Build & Run

```powershell
cd src-tauri

# Build TSF DLL
cargo build -p lexi-tsf

# Build main app
cargo build

# Run in development mode
cargo tauri dev
```

## Register / Unregister

```powershell
# Register as system input method (admin required)
.\scripts\register-tsf.ps1

# Unregister
.\scripts\unregister-tsf.ps1
```

## Contributing

Contributions are welcome! See **[CONTRIBUTING.md](CONTRIBUTING.md)** for details.

## License

Apache 2.0 — see [LICENSE](./LICENSE).
