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
- Copilot co-brand strip: `assets/images/zed_x_copilot.svg` regenerated as **LingCode × Copilot** — the
  Zed mark replaced with the LingCode `{}` logo (purple gradient), keeping the `+` and the GitHub
  Copilot mark. Filename kept (`VectorName` + the "Zed X Copilot" reference are internal).

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
  display labels. URLs/emails (`zed.dev`), font load-keys (`Zed Mono`, `Zed Plex`),
  internal identifiers (`ZED_AGENT_ID`, window-class/UA/protocol strings), and tests were left untouched.
  The default icon-theme display name was renamed `Zed (Default)` → `LingCode (Default)` (in
  `crates/theme/src/icon_theme.rs` `DEFAULT_ICON_THEME_NAME` + `assets/settings/default.json`, changed
  together); it is defined in code (id `zed`), so this is display-only and does not touch any asset file.
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

## De-brand pass (hide the Zed-fork origin)

Repointed/removed user-visible surfaces that revealed the upstream:
- **Help menu** (`crates/zed/src/zed/app_menus.rs`) — Documentation → `lingcode.dev/docs.html`;
  removed "LingCode Twitter" (`twitter.com/zeddotdev`) and "Join the Team" (`zed.dev/jobs`); the
  repo item now "LingCode Website" → `lingcode.dev`.
- **Feedback actions** (`crates/feedback/src/feedback.rs`) — bug report / request feature / email
  now target `lingcode.dev` / `mailto:support@lingcode.dev` (were `github.com/zed-industries` +
  `hi@zed.dev`). Action `OpenZedRepo` → `OpenLingCodeWebsite`.
- **Error dialogs** (`crates/zed/src/zed.rs`, `crates/zed/src/main.rs`) — file-watcher / unsupported-GPU /
  window-open failures: `zed.dev/docs/*` → `lingcode.dev/docs.html`; "Zed uses … for rendering" → "LingCode …".
- **Scattered visible strings** — agent upgrade prompt (`agent_ui/src/conversation_view.rs`), thread-rating
  consent (`thread_view.rs`), crash GPU context (`zed/src/reliability.rs`), OpenRouter `X-Title`/`HTTP-Referer`
  (`open_router.rs`, shown in the user's OpenRouter dashboard), debug-config placeholder (`debugger_ui`).

## Android tooling (new `lingcode_android` crate)

Ports the build/run/deploy-prep essentials of the macOS app's Android tier. New crate
`crates/lingcode_android/` (mirrors the `lingcode_cloud` action+streaming-modal pattern), wired via
`lingcode_android::init` in `crates/zed/src/main.rs` and a new **Android** app menu in
`app_menus.rs`. Actions (each streams the tool's output in a modal):
- **Check Android Toolchain** — detects SDK / JDK / adb / emulator / gradle (Windows-aware:
  `%LOCALAPPDATA%\Android\Sdk`, `ANDROID_HOME`, etc.) and prints paths or install hints.
- **Build Debug APK** / **Build Release Bundle** / **Run on Device** — `gradlew assembleDebug` /
  `bundleRelease` / `installDebug`. On Windows the `gradlew.bat` wrapper is run via `cmd /C`
  (CreateProcess can't execute a `.bat` directly).
- **List Devices** (`adb devices -l`), **List Emulators** (`emulator -list-avds`), **Start Emulator**
  (launches the first AVD, detached), **Open Play Console** (`cx.open_url`).
- **Deploy to Google Play** — full Play Developer API flow (mirrors the macOS `GooglePlayDeployService`):
  optional `versionCode` auto-bump → `gradlew bundleRelease` → locate the `.aab` → service-account JWT
  (RS256 via `jsonwebtoken`) → OAuth2 token → create edit → upload bundle (`uploadType=media`) →
  assign track → commit. HTTP goes through the app's executor-safe `http_client`. Config is read from
  `<project>/.lingcode/play-deploy.json` (`service_account_json_path`, `package_name`, `track`,
  `auto_bump_version_code`, optional `aab_path`); the AAB must be signed by the project's
  `signingConfigs`. No GPUI form yet — the config file is the input surface.

- **Logcat** (`AndroidLogcat`) — streams `adb logcat -v threadtime` from the first device into the modal.
- **Layout Inspector** (`AndroidLayoutInspector`) — `uiautomator dump` + display the view hierarchy.
- **Analyze APK / AAB** (`AndroidAnalyzeApk`) — locates the built artifact, shows size + `aapt2 dump badging`.

STAGED / not ported (each needs a GPUI panel/form/toolbar or DAP integration that can't be written
responsibly without a build): the **Kotlin/Java JDWP debugger** (needs wiring into Zed's DAP/debugger
UI), **dockable** logcat/layout panels (currently modal output), **APK diff** + a richer analyzer UI
(needs a two-file picker), a Zed **run-destination toolbar** picker (the crate targets the first
connected device via `installDebug`), AVD **create/delete** UI (needs text input), and a GPUI **deploy
form** with Keychain-stored credentials (currently the `.lingcode/play-deploy.json` config +
project `signingConfigs`).

## Intentionally NOT changed (and why)
- **`zed:` action namespace (display)** — now de-branded everywhere it's shown: `debrand_action_name`
  is applied inside `command_palette::humanize_action_name`, which is the single function the command
  palette, **keymap editor**, which-key, and settings keybinding UI all route through. So the visible
  `zed:` prefix reads `lingcode:` across all of them. The raw `zed::Action` *identifiers* (in keymap
  JSON, `dispatch_action`, `actions!(zed, …)`) are deliberately left — they're load-bearing
  (every default+user keybinding references them); renaming the namespace would break all keymaps.
- **Telemetry event names** ("Zed Agent …") — invisible unless inspecting telemetry; renaming breaks
  analytics continuity and isn't user-facing.
- **User-Agent / protocol strings** (`"Zed/{}"`, window-class) — network-only, not casual-user visible,
  and extension/registry endpoints may key on them; changing risks breaking fetches.
- **Font / theme / grammar load-keys** ("Zed Mono", "Zed Plex", "Zed Keybind Context") — these map to
  bundled asset files; renaming the key without renaming the asset breaks loading. Not user-facing.
- **z.ai explicit "thinking" param** — the models work via the generic OpenAI-compatible path, but
  z.ai's native `{"thinking":{"type":"enabled"}}` toggle isn't sent (the OpenAI request shape has no
  such field). Replicating it needs a dedicated provider with custom request JSON; doing that unverified
  risks 400-ing a currently-working provider, so it's left until it can be tested.
- **LingModel OAuth sign-in** — macOS auths LingModel via the `lingcode://` OAuth session; the Windows
  provider uses a pasted LingCode API key (keychain-backed). Porting the full OAuth subsystem blind is
  large and risky; the API-key path is functional.
- **Zed cloud-commerce strings** ("Zed Pro", "Zed's hosted models", trial upsells) — left in place in
  `crates/language_models/src/provider/cloud.rs`, `crates/ai_onboarding` (no render site — unreachable),
  `crates/agent_ui`, no longer reachable since `CloudLanguageModelProvider` is disabled (above).
- **Font/theme asset names** ("Zed Mono", "Zed Icons", "Zed Keybind Context" language) — load keys mapping to
  bundled font/theme/grammar files; renaming the string without the assets breaks loading.
- **User-Agent / protocol strings** (`"Zed/{}"` in `main.rs`, extension UA) — network-only (not casual-user
  visible); registries may key on them, so left to avoid breaking extension/registry fetches.
- **Telemetry event names** ("Zed Agent …") — invisible unless inspecting telemetry; renaming breaks
  analytics continuity.
- **`auto_update_helper` `Zed.exe` paths / logs** — auto-update is disabled; not exercised.
- **`appAppxFullName`** — cert-tied (see above).

## Mac-IDE parity additions

Bringing the Windows IDE in line with the macOS LingCode IDE's AI/Cloud surface. The agent
experience already reaches functional parity through Zed's native ACP agents (`claude-acp`,
`codex-acp`, `gemini`) — Zed renders these natively, so the macOS "Claude Code (Web)" / "Codex
(Web)" WKWebView tabs map to the native agent panel rather than an embedded web view.

- **LingModel provider** — `crates/language_models/src/provider/ling_model.rs` (new). A branded,
  managed Anthropic-Messages provider pointing at `https://lingcode.dev/api/inference/anthropic`
  (the Anthropic client appends `/v1/messages`). Self-contained: hardcoded endpoint + single
  `lingmodel` model, no settings plumbing. Reuses `anthropic` crate `into_anthropic` /
  `AnthropicEventMapper` / `stream_completion`. Auth = a pasted LingCode API key via the shared
  keychain-backed `ApiKeyState` (env fallback `LINGMODEL_API_KEY`). Registered FIRST in
  `register_language_model_providers` for prominence. **Branding rule honored: no user-visible
  string names the upstream vendor.** Icon `AiLingModel` added to `crates/icons/src/icons.rs` +
  `assets/icons/ai_ling_model.svg`.
- **Kimi / Qwen / z.ai providers** — added as `openai_compatible` presets in
  `assets/settings/default.json` (zero Rust; auto-registered by `register_openai_compatible_providers`).
  Models/URLs mirror the macOS provider set (Kimi → `api.moonshot.ai/v1`, Qwen →
  `dashscope-intl.aliyuncs.com/compatible-mode/v1`, z.ai → `api.z.ai/api/paas/v4`).
- **Cloud Console + Project Sharing** — `crates/lingcode_cloud/src/lingcode_cloud.rs` adds
  `OpenBackendConsole` + `ShareCloudProject` actions. These open the LingCode Cloud web apps
  directly (`lingcode.dev/backends.html`, `lingcode.dev/project.html`) via `cx.open_url` —
  **no CLI dependency** (auth is the browser session, exactly like the macOS app, which opens the
  same pages). New **Cloud** app menu in `crates/zed/src/zed/app_menus.rs` groups all five cloud
  actions (Deploy / Connect / Disconnect / Open Backend Console / Share).
- **Branded provider icons** — `IconName::{AiKimi, AiQwen, AiZai}` + `assets/icons/ai_{kimi,qwen,zai}.svg`;
  `open_ai_compatible.rs` `icon()` maps the preset ids to them (others keep the generic glyph).
