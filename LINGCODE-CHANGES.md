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

## Project templates (new `lingcode_templates` crate)

Adds a **New from Template** flow so users can start from a starter project instead of an empty
folder (upstream Zed only opens existing folders). New crate `crates/lingcode_templates/` (mirrors
the `lingcode_android` init+`register_action` pattern), wired via `lingcode_templates::init` in
`crates/zed/src/main.rs`.

- **Action:** `workspace::NewFromTemplate` (defined in the `workspace` crate next to `NewFile`, so the
  welcome screen can reference it without a dependency cycle; the handler is registered from the new
  crate via `workspace.register_action`).
- **Entry points:** the **Welcome** screen "Get Started" section (`crates/workspace/src/welcome.rs`,
  `Section<4>`→`Section<5>`, `IconName::FileCode`) and the **File** app menu ("New from Template…",
  `crates/zed/src/zed/app_menus.rs`).
- **Flow:** native multi-button prompt to pick a template → system folder picker for the parent dir →
  scaffold into a fresh non-colliding `<slug>` directory (via the `fs` abstraction) → open it with
  `open_workspace_for_paths`.
- **Templates (embedded via `include_str!`, fully offline)** under `crates/lingcode_templates/templates/`:
  **Python** (`main.py` + `pytest`), **Web / HTML5 game** (zero-dep canvas loop), **Android**
  (Kotlin + Gradle, builds via the Android menu), **Node / TypeScript** (`tsc` + npm scripts). Add a
  template by dropping files under `templates/<dir>/`, listing them with the `template_file!` macro, and
  adding a `Template` entry.
- **Known caveat:** the Android template omits the Gradle **wrapper jar** (a binary can't be embedded as
  text), so its README instructs running `gradle wrapper` once before the Android-menu build commands.

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

## Magic Install (new `lingcode_install` crate)

Ports the macOS app's **Magic Install** (`Services/Deploy/MagicInstallService.swift`): detect the project's
package manager(s) from marker files and run their install commands, streaming output into a modal. New crate
`crates/lingcode_install/` mirrors the `lingcode_cloud` action+streaming-modal pattern, wired via
`lingcode_install::init` in `crates/zed/src/main.rs` and an **Install Dependencies** item in the **Cloud** app
menu (`crates/zed/src/zed/app_menus.rs`).

- **Action:** `lingcode_install::MagicInstall` (registered on the workspace via `register_action`).
- **Native, no CLI dependency** — unlike Cloud/Push (which delegate to the `lingcode` CLI), detection is just
  marker-file existence checks and the install command is a plain subprocess, so it works even when the CLI
  isn't on PATH. Spawns via Zed's cross-platform `util::command::new_std_command` (no `/bin/zsh` assumption);
  `which::which` resolves Windows `.cmd`/`.bat` wrappers.
- **Detection table** (`MANAGERS`): pnpm / yarn / bun / npm (lockfile suppression so generics don't double up),
  cargo, poetry / pipenv / pip, go, bundler, composer, dotnet, maven, swift. Extend by adding a
  `PackageManager` entry.
- **Tests:** unit tests over the detect filter (suppression, multi-ecosystem, empty project) using a temp dir —
  pure logic, no spawn (`tempfile` dev-dependency).
- **Status:** code complete + `cargo metadata` validates the manifests; pending the ARM64 `build_lingcode.bat`
  compile + a manual run before it's trusted.

## Magic Push AI commit message

Brings `lingcode_cloud::PushToGithub` in line with the macOS Magic Push (`Services/Deploy/MagicPushService.swift`),
which generates a one-line commit message from the staged diff. **Editor-side change only** (in
`crates/lingcode_cloud/src/lingcode_cloud.rs`):
- `run_push` now passes `--ai-message` so the CLI generates the message when the user supplies none.
- The `PushEvent::Commit.message` field (previously `#[allow(dead_code)]`) is surfaced in the modal:
  `Committed N file(s): <message>`.

**Companion CLI change required (separate `lingcode` repo, `src/github/push.ts`):** honor `--ai-message` by
generating the commit message from `git diff --cached` and emitting it in the existing `commit` NDJSON event.
The two must ship together — on a CLI that predates the flag the push will error on the unknown argument.

## LingModel browser OAuth sign-in

Ports the macOS app's "Sign In with Browser" flow (`LingCodeAuthService.swift`) so LingModel can be
authenticated via a `lingcode://` OAuth round-trip instead of only a pasted API key. **Purely additive** — the
pasted-key path is untouched and remains the fallback; both land the token in the same keychain slot
(`ApiKeyState`), so all inference code is unchanged.

- **Callback parsing** (`crates/zed/src/zed/open_listener.rs`) — new `OpenRequestKind::LingModelAuthCallback`
  + a parse arm for `lingcode://auth/callback?code=…&state=…` (or `access_token=…` / `error=…`), with a unit
  test. Dispatched in `crates/zed/src/main.rs`'s `handle_open_request` to `deliver_ling_model_auth`.
- **Cross-crate bridge** (`crates/language_models/src/ling_model_auth.rs`, new) — a `LingModelAuthListener`
  global (modeled on `client::RefreshLlmTokenListener`) that the provider subscribes to; the last callback is
  buffered for late/cold-launch subscribers. Registered first in `language_models::init`.
- **Provider** (`crates/language_models/src/provider/ling_model.rs`) — `State` gains a PKCE
  `begin_browser_sign_in` (S256 via `sha2`, verifier/state via `rand`, base64url), an `on_auth_callback`
  (state validation → direct `access_token` store, or `exchange_code` against the token endpoint via
  `http_client`), and a **"Sign In with Browser"** button in the config view. New deps: `sha2`, `rand`, `url`.
- **Endpoints to confirm before shipping:** `OAUTH_AUTHORIZE_URL`, `OAUTH_TOKEN_URL`, `OAUTH_CLIENT_ID`, and
  whether the server returns `access_token` directly on the redirect or a `code` to exchange (both handled).
- **Branding rule honored:** no user-visible string names the upstream vendor.
- **Status:** code complete + `cargo metadata`/`cargo fmt` clean. **Higher verification risk than the Magic
  items** (crypto + cross-crate gpui globals + async, none compiled here) — must build with
  `build_lingcode.bat` and be exercised end-to-end (warm launch *and* cold launch) before it's trusted. The
  changelog originally flagged this port as "large and risky"; the API-key fallback contains that risk.

## Remote coding — client slice (host side staged)

The macOS "remote coding" feature (drive the agent from a phone, zero setup) has no Windows equivalent: the
serving component is macOS-native (`LingCodeServer` / `lingcode serve`, Darwin `NWListener`), so — unlike the
cloud actions — the fork can't just spawn the CLI to host. Full host support is **new networked Rust**
(estimated weeks); the file-level staged plan is in **`REMOTE-CODING-PLAN.md`**.

Shipped now (the tractable, working *client* half):
- **`lingcode_cloud::OpenRemoteControl`** — opens `https://lingcode.dev/remote-control.html`
  (`crates/lingcode_cloud/src/lingcode_cloud.rs`), so a Windows user can drive their *other* LingCode hosts
  from the web client (the relay + web UI are already deployed and platform-independent). One-liner action in
  the existing crate, mirroring `OpenBackendConsole`. Menu: **Cloud → Remote Control (Web)**
  (`crates/zed/src/zed/app_menus.rs`).
- Not yet done: making *this* Windows machine a drivable **host** (the agent HTTP+SSE server + relay bridge) —
  see the plan. Deliberately staged rather than written blind.

### Host server lifecycle (new `lingcode_remote` crate)

Key realization: the Windows **`lingcode` CLI already ships a complete cross-platform headless server**
(`lingcode serve` — sessions, SSE event streams, permissions, PTY, files). So the Windows host does **not**
need a from-scratch Rust HTTP/SSE server (the macOS Swift `LingCodeServer`/`NWListener` is Apple-only and
unusable here); it just manages the CLI server's lifecycle — the same "delegate to the CLI" approach as
`lingcode_cloud`.

New crate `crates/lingcode_remote/`, wired via `lingcode_remote::init` in `crates/zed/src/main.rs` and the
**Cloud** app menu (`Start Remote Server` / `Stop Remote Server`).
- **Actions** `lingcode_remote::{StartRemoteServer, StopRemoteServer}`.
- **Start** spawns `lingcode serve` (via `util::command::new_std_command` + `util::process::Child`), parses the
  `listening on http://host:port` line for the address, and opens a status modal. The running process is held
  in a **gpui global** so it survives the modal being closed (`StopRemoteServer` / the modal's Stop button
  `Child::kill()`s it). On exit it surfaces a hint to run `lingcode serve` in a terminal (e.g. not signed in).
- **Zero-setup phone reach is now wired:** the crate spawns **`lingcode remote`** (not bare `lingcode serve`),
  the new CLI command that registers this machine as a relay host, starts a private loopback server, and
  tunnels the hosted relay to it — so the web remote-control reaches it with no SSH/port config. (The relay
  bridge was built once in the cross-platform CLI so Mac and Windows share it — see below.)
- **Status:** code complete + `cargo metadata`/`cargo fmt` clean; pending the ARM64 `build_lingcode.bat`
  compile + a run.

### Relay bridge — `lingcode remote` (in the cross-platform CLI)

Closes the zero-setup gap. **Faithful port of the macOS app's `collab-bridge/bridge.mjs` serve-host logic**
into the Bun/TS `lingcode` CLI (a separate repo), so both platforms share one bridge:
- `packages/lingcode/src/remote/serve-tunnel.ts` — joins the relay room's `__serve` doc over y-websocket,
  announces `lc-serve-host-hello`, and answers `lc-serve-request` frames by proxying to the loopback server,
  streaming back `lc-serve-response-head`/`-chunk`/`-close`/`-error` (verbatim protocol from the Mac bridge,
  incl. the binary-JSON-frame trick that avoids the Yjs decoder).
- `packages/lingcode/src/cli/cmd/remote.ts` — `lingcode remote`: registers via `POST /api/remote/hosts`
  (LingCode Cloud token), starts a private loopback `lingcode serve --hostname 127.0.0.1` (per-run password;
  the tunnel authenticates with the matching `ServerAuth` Basic header), then runs the tunnel until Ctrl-C.
- New deps `yjs` / `y-websocket` / `ws` (Mac bridge versions); wired into `src/index.ts`.
- **Status:** typechecks clean except the three new imports (need `bun install`); the logic is a faithful port
  but **needs `bun install` + a live relay + sign-in to verify end-to-end** (none runnable in this session).
