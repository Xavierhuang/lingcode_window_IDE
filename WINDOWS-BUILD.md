# Building LingCode for Windows

This repo is **LingCode** — a fork of the [Zed](https://github.com/zed-industries/zed) editor (Rust + GPUI),
rebranded and wired to use LingCode's multi-provider agent. This guide gets it building on a Windows machine.

> **Status:** the rebrand is source-complete but **has never been compiled**. The first build is expected to
> surface issues — that is the point of this step. Get a `cargo run` dev build rendering "LingCode" before
> touching the installer or signing.

---

## 1. Prerequisites (Windows 10 1809+ / Windows 11)

- **Rust (stable)** via [rustup](https://rustup.rs): `rustup default stable`
- **MSVC C++ build tools** — install Visual Studio 2022 (Community is fine) with the
  **"Desktop development with C++"** workload. This provides the MSVC toolchain, Windows SDK, and the
  DirectX/DirectWrite headers GPUI's Windows backend needs.
- **A DirectX 11–capable GPU** (GPUI renders with D3D11 on Windows; required at runtime, not just build).
- **Hardware:** ~16 GB+ RAM and ~50 GB free disk. The first build is large.
- **Git**.

## 2. Get the code

```powershell
git clone <your-fork-url> lingcode
cd lingcode
```
(Or, if transferring without a remote, copy the `lingcode.bundle` produced on the Mac and
`git clone lingcode.bundle lingcode`.)

## 3. Dev build (do this first)

```powershell
cargo run
```
- **First build is slow** (~20–40 min) — it compiles ~230 crates. Subsequent builds are incremental.
- Goal: the window opens and shows **LingCode** branding (title, menus, About, Welcome screen) with the
  LingCode icon. Verify that before anything else.

## 4. Wire up the agent (so the agent panel works)

LingCode's agent is a **separate process** spoken over ACP. The shipped default config
(`assets/settings/default.json` → `agent_servers.LingCode`) launches it as `lingcode acp`, so the
`lingcode` CLI must be **on PATH**:

1. Build the agent for Windows from `LingCodeCLIv2/` (Bun): it already targets `win32-x64`/`win32-arm64`.
2. Install it so `lingcode.exe` is on PATH (the LingCodeCLIv2 installer puts it in
   `%LOCALAPPDATA%\Programs\lingcode\`).
3. In the running IDE, open the agent panel → pick **LingCode**. First run with no key falls back to the
   free tier; sign in / add keys via `lingcode providers` to surface the full 16-provider set
   (LingModel needs no key).

> For a true zero-install experience, bundle `lingcode.exe` alongside the app and change the
> `agent_servers.LingCode.command` in `assets/settings/default.json` to a path relative to the install dir.
> (Not done yet — PATH lookup is the current default.)

## 5. Building the installer (only after the dev build works)

```powershell
pwsh script/bundle-windows.ps1 -Architecture x86_64 -channel stable
```
Needs [Inno Setup](https://jrsoftware.org/isinfo.php) and the MSVC toolchain. Produces a signed installer
**only in CI** (it expects Azure code-signing vars); locally it will build an unsigned installer.

## Known first-build gotchas / TODO

These are expected and called out so you don't chase them blind:

- **Tests that assert old Zed branding.** Some unit tests may hard-code `"Zed"`, `dev.zed.Zed`, or
  `zed://`. If `cargo test` fails on string assertions, update the expected values — the production code
  was rebranded consistently, but test fixtures may lag. (Deep-link tests in
  `crates/zed/src/zed/open_listener.rs` were already updated to `lingcode://`.)
- **Zed cloud provider.** The native model provider still references Zed's hosted service ("Zed Pro",
  "Zed's hosted models" — ~25 strings in `crates/language_models` + `crates/agent_ui`). These were left
  intentionally: the right fix is to **disable `CloudLanguageModelProvider`** (in
  `crates/language_models/src/language_models.rs`) since LingCode uses its own ACP agent — not to rename
  them. Do this once the build is green so you can verify it compiles.
- **appx context-menu identity** (`$appAppxFullName` in `script/bundle-windows.ps1`) still says
  `ZedIndustries.Zed_…` — its hash suffix is tied to a code-signing certificate and **requires your own
  Windows publisher cert** to change. Blocks the Explorer "Open with" appx, not the app itself.
- **Help-menu links** ("LingCode Repository"/"Twitter" actions in `crates/zed/src/zed/app_menus.rs`) still
  open Zed's GitHub/Twitter. Repoint them to your URLs (in the `feedback` crate) or remove the items.
- **Auto-update** (`crates/auto_update`) points at the Zed/`ZED_SERVER_URL` endpoint. Repoint to a LingCode
  endpoint or disable via `ZED_UPDATE_EXPLANATION`.

## What was rebranded

See [`LINGCODE-CHANGES.md`](./LINGCODE-CHANGES.md) for the full list of LingCode edits vs. upstream Zed.

## Tracking upstream Zed (optional, later)

To pull future Zed fixes:
```powershell
git remote add upstream https://github.com/zed-industries/zed.git
git fetch upstream
git rebase upstream/main      # resolve conflicts in the rebranded files
```
Keep LingCode edits minimal and centralized (branding/identifiers/assets) to ease rebases.
