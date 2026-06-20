# Building LingCode for Windows

This guide gets LingCode building on a Windows machine.

## 1. Prerequisites (Windows 10 1809+ / Windows 11)

- **Rust (stable)** via [rustup](https://rustup.rs): `rustup default stable`
- **MSVC C++ build tools** — install Visual Studio 2022 (Community is fine) with the
  **"Desktop development with C++"** workload. This provides the MSVC toolchain, Windows
  SDK, and the DirectX/DirectWrite headers the Windows rendering backend needs.
- **A DirectX 11–capable GPU** (the renderer uses D3D11 on Windows; required at runtime).
- **Hardware:** ~16 GB+ RAM and ~50 GB free disk. The first build is large.
- **Git**.

## 2. Get the code

```powershell
git clone <repo-url> lingcode
cd lingcode
```

## 3. Dev build (do this first)

```powershell
cargo run
```

- **First build is slow** (~20–40 min) — it compiles a large dependency graph.
  Subsequent builds are incremental.
- Goal: the window opens and shows LingCode branding (title, menus, About, Welcome
  screen) with the LingCode icon.

## 4. Wire up the agent (so the agent panel works)

LingCode's agent runs as a **separate process** spoken over ACP. The shipped default
config (`assets/settings/default.json` → `agent_servers.LingCode`) launches it as
`lingcode acp`, so the `lingcode` CLI must be **on PATH**:

1. Build/install the `lingcode` CLI so `lingcode.exe` is on PATH.
2. In the running IDE, open the agent panel → pick **LingCode**. First run with no key
   falls back to the free tier; sign in / add keys via `lingcode providers` to surface
   the full provider set (LingModel needs no key).

## 5. Building the installer (only after the dev build works)

```powershell
pwsh script/bundle-windows.ps1 -Architecture x86_64 -channel stable
```

Needs [Inno Setup](https://jrsoftware.org/isinfo.php) and the MSVC toolchain. CI produces
a signed installer when the Azure code-signing variables are set; locally it builds an
unsigned installer.

> Releases are built automatically by the GitHub Actions release workflow on a tag push
> (`v*`), which publishes the per-arch installers (`LingCode-x86_64.exe` /
> `LingCode-aarch64.exe`) to the Releases page.
