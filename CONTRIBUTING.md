# Contributing to Lexi

Lexi is a minimalist Chinese input method for Windows built with Tauri v2 + Rust + Skia. It's still in early development — every contribution helps get it closer to daily-driver quality.

## Setup

Required tools:

- Rust 1.85+ with Cargo
- Windows 10/11
- [librime.dll](https://rime.im/) placed in `src-tauri/target/debug/` or `PATH`
- Git

Build and check:

```powershell
cd src-tauri

# Build TSF DLL
cargo build -p lexi-tsf

# Build main app
cargo build

# Run in development mode
cargo tauri dev
```

Register the input method (admin required) before testing:

```powershell
.\scripts\register-tsf.ps1
```

## Project Layout

The workspace contains two crates:

```text
src-tauri/
├── src/                          # Main crate (lexi / lexi_lib)
│   ├── main.rs                   # Program entry point
│   ├── lib.rs                    # AppState, Tauri commands, startup
│   ├── pipe_server.rs            # Named Pipe server (tokio)
│   ├── candidate_bar.rs          # Skia + Win32 candidate window
│   ├── logging.rs                # File-based logging
│   └── ime/rime/
│       ├── mod.rs                # RimeEngine wrapper
│       └── ffi.rs                # RIME C API bindings
├── crates/tsf-service/           # TSF crate (lexi-tsf cdylib)
│   └── src/
│       ├── lib.rs                # DLL entry, COM ClassFactory, registry
│       ├── text_service.rs       # TSF text service & event sink
│       └── pipe_client.rs        # Named Pipe client (OVERLAPPED I/O)
├── scripts/                      # Registration scripts
│   ├── register-tsf.ps1
│   └── unregister-tsf.ps1
└── tauri.conf.json               # Tauri configuration
```

The data flow through the system:

```text
Key press
  → Windows TSF framework
  → lexi_tsf.dll (ITfKeyEventSink)
  → Named Pipe
  → pipe_server (tokio)
  → RimeEngine::process_key()
  → ContextData ← candidates
  → BarData update
  → PostMessage → Skia-rendered candidate bar
```

## Change Guidelines

- **TSF changes** affect system-level input handling. Test by registering the DLL and switching to Lexi as the active input method. Verify that key events reach RIME and candidates display.
- **RIME changes** affect the conversion engine. Prefer using librime's public API through the FFI bindings rather than working around it.
- **Candidate bar changes** affect the Skia rendering or window management. Verify that the bar positions correctly near the cursor and follows the active theme.
- **Pipe protocol changes** must keep backward compatibility with the TSF DLL. Both sides communicate via JSON over `\\.\pipe\LexiInputMethod`.
- **Logging** should use the `log` crate macros (`log::info!`, `log::error!`, etc.) rather than `eprintln!` or `println!`. Logs go to `%LOCALAPPDATA%\Lexi\logs\lexi.log`.

Keep modules focused. If a file grows too large, split by concept — for example, separate Skia drawing helpers from window message handling.

## Commits

Use Conventional Commits:

```text
type(scope): short description
```

Common types: `feat`, `fix`, `refactor`, `docs`, `test`, `ci`, `chore`.

Useful scopes: `tsf` (TSF DLL), `rime` (RIME engine), `bar` (candidate bar), `pipe` (IPC), `frontend` (settings UI), `scripts` (deployment).

Since the project has just two main crates but two repos (monorepo workspace + separate DLL), a commit may touch both sides when changing the pipe protocol. That is fine — just keep each commit logically coherent.

## Checklist Before Commit

```powershell
cargo fmt --all
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

If you modified the TSF DLL:

```powershell
cargo build -p lexi-tsf
```

If you modified the pipe protocol, test that the main app starts without errors and the DLL can connect:

```powershell
cargo build
.\scripts\register-tsf.ps1   # re-register if DLL changed
```

## License

By contributing, you agree that your contributions are licensed under the [Apache 2.0 License](LICENSE).
