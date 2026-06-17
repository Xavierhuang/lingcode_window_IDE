# LingCode changes vs. upstream Zed

This fork rebrands Zed → **LingCode** and wires in LingCode's multi-provider agent. All edits are
string/asset/config swaps (no architectural changes), kept deliberately minimal to ease rebasing against
upstream Zed.

## Visible name / chrome
- `crates/release_channel/src/lib.rs` — `display_name()` → LingCode (all channels); `app_identifier()` →
  `LingCode-Editor-*` (runtime single-instance mutex base)
- `crates/zed/src/zed/app_menus.rs` — app-menu name + About/Hide/Quit → LingCode
- `crates/zed/Cargo.toml` — `[package.metadata.bundle-*]` `name` → LingCode (macOS .app name)
- `crates/zed/build.rs` — Windows `.exe` FileDescription / ProductName → LingCode
- `crates/onboarding/src/onboarding.rs` — "Welcome to LingCode"
- `crates/paths/src/paths.rs` — config/data/state/cache dirs → `LingCode` / `lingcode`; log → `LingCode.log`

## Identifiers
- **Bundle id:** `dev.zed.Zed*` → `dev.lingcode.LingCode*` (`release_channel` `app_id()`, `Cargo.toml`
  bundle identifiers, notification id in `crates/zed/src/main.rs`, flatpak checks in `crates/cli/src/main.rs`)
- **URL scheme:** `zed://` → `lingcode://`, `zed-cli://` → `lingcode-cli://` (all handlers in
  `open_listener.rs`, `open_url_modal.rs`, `crates/zed/src/main.rs`, `crates/cli/src/main.rs` incl. tests;
  macOS `osx_url_schemes`; Windows registry in `zed.iss`). `zed-dock-action://` left (internal IPC).
- **Single-instance mutex:** runtime `LingCode-Editor-*-Instance-Mutex`, installer `$appMutex` set to match
  (upstream had these mismatched).

## Windows installer (`script/bundle-windows.ps1`, `crates/zed/resources/windows/zed.iss`)
- App name / display name / shell "Open with" name / setup filename → LingCode
- Exe basename `Zed.exe` → `LingCode.exe` (build copy, sign list, installer source + registry)
- AppPublisher → LingCode; publisher/support/update URLs → lingcode.dev
- AUMID `ZedIndustries.Zed*` → `LingCode.LingCode*`; file-assoc registry prefix → `LingCode*`
- **Left (needs your cert):** `$appAppxFullName` (appx package identity, cert-hash-tied)

## Logo / icons (binary assets)
- App icon: the LingCode Mac app icon → all 8 macOS/Linux PNGs (`crates/zed/resources/app-icon*.png`, all
  channels) + 4 Windows `.ico` (`crates/zed/resources/windows/app-icon*.ico`, multi-res)
- In-app logo: LingCode brand SVG → `assets/images/zed_logo.svg` (filename kept so `VectorName::ZedLogo`
  resolves; the `image.rs:178` path test still passes)

## Agent wiring
- `assets/settings/default.json` — `agent_servers.LingCode` (custom ACP, `command: lingcode`, `args: [acp]`)
  ships LingCode as the built-in agent.
- (In the separate `LingCodeCLIv2` repo) `packages/lingcode/src/config/config.ts` first-run seed writes a
  curated default: `model: lingmodel/lingmodel-standard` + `enabled_providers` allowlist of LingCode's 16
  providers, plus ACP agent identity / TUI strings rebranded OpenCode → LingCode.

## Windows build enablement + deeper rebrand (2026-06)
Done after the first successful Windows (aarch64-msvc) build:
- **Welcome screen** — `crates/workspace/src/welcome.rs` "Welcome (back) to Zed" → LingCode (the classic
  Welcome tab; the `crates/onboarding` one was already done).
- **~110 user-visible UI strings** rebranded Zed → LingCode across ~50 crates: window/menu text
  (`About LingCode`, `LingCode Repository/Twitter`), launch/error/log messages, collab + title-bar text,
  update-status labels, settings descriptions (`settings_ui/src/page_data.rs`), every model-provider config
  helper (`language_models/src/provider/*.rs` except `cloud.rs`), extensions/CLI text, and the `LingCode Agent`
  display labels. URLs/emails (`zed.dev`), font/theme/icon load-keys (`Zed Mono`, `Zed Plex`, `Zed (Default)`),
  internal identifiers (`ZED_AGENT_ID`, window-class/UA/protocol strings), and tests were left untouched.
- **Cloud paywall removed** — `CloudLanguageModelProvider` registration disabled in
  `crates/language_models/src/language_models.rs`, so the Zed Pro/Business/AI/Agent upsell UI no longer appears.
- **Auto-update disabled** — `ZED_UPDATE_EXPLANATION` set at build time (see `build_lingcode.bat`); the app
  won't phone home or update into upstream Zed.
- **Binary renamed** — `crates/zed/Cargo.toml` bin target `zed` → `lingcode` (+ `default-run`), so the build
  produces `lingcode.exe`. The Rust *package* is still named `zed` (internal, not user-visible).
- **Build fixes for this toolchain** — `.cargo/config.toml` adds `+fp16` to the Windows `target-feature`
  (gemm-common half-precision asm on aarch64); `build_lingcode.bat` runs the build inside `vcvarsarm64.bat`
  with LLVM/clang on PATH (clang needed by `ring`); a junction supplies the MSVC ARM64 spectre libs. See
  `WINDOWS-BUILD.md`.

## Intentionally NOT changed (and why)
- **Zed cloud-commerce strings** ("Zed Pro", "Zed's hosted models", trial upsells) — left in place in
  `crates/language_models/src/provider/cloud.rs`, `crates/ai_onboarding`, `crates/agent_ui`, but no longer
  reachable since `CloudLanguageModelProvider` is disabled (above). Renaming them would advertise a product
  you don't sell.
- **Font/theme asset names** ("Zed Mono", "Zed Icons" in `crates/settings`) — these are load keys mapping to
  bundled font/theme files; renaming the string without the assets breaks loading.
- **`appAppxFullName`** — cert-tied (see above).
- **Help-menu repo/Twitter links** — actions open Zed's real GitHub/Twitter; need your URLs.
