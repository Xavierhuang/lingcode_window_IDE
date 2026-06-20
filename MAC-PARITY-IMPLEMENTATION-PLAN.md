# Mac-parity implementation plan — Magic utilities, LingModel OAuth, Remote coding

Status: **plan only, no code committed.** Targets the Windows Zed fork in this repo.
Companion to `LINGCODE-CHANGES.md`. File paths are relative to the repo root unless absolute.

## Scope

Three gaps identified against the macOS LingCode app:

1. **Magic utilities** — Magic Install (portable), Magic Push AI commit message (portable enhancement), Magic Ship/App Store (NOT portable — Apple-only; Android already covered by `lingcode_android::AndroidDeployToPlay`).
2. **LingModel OAuth** — browser sign-in via `lingcode://auth/callback`, replacing/augmenting the pasted-API-key path.
3. **Remote coding** — "drive the agent from a phone." Large/architectural; planned as a later staged effort.

Recommended sequence by value/effort: **Magic Push AI → LingModel OAuth → Magic Install → Remote coding.**

## Build / verify prerequisites (read first)

Per `WINDOWS-BUILD.md` this fork only builds with the ARM64 MSVC + LLVM/clang toolchain
(`build_lingcode.bat` inside `vcvarsarm64.bat`, `+fp16` target-feature, MSVC ARM64 spectre-libs junction).
**Every change below must be compiled with that toolchain before commit** — the project's standing rule
(`LINGCODE-CHANGES.md`) is no unbuildable blind edits. The OAuth item especially must be run end-to-end
(cold launch + warm launch callback) before it is trusted.

Branding rule (load-bearing): **no user-visible string may name the upstream model vendor or "Zed".**
Use "LingModel" / "LingCode" / generic "your API key".

---

## Feature 1 — Magic Install (new `lingcode_install` crate)

Mirrors the macOS `LingCode/Services/Deploy/MagicInstallService.swift`: scan the project for package-manager
marker files, run the matching install command(s), stream output to a modal. Pattern is an exact clone of
`crates/lingcode_cloud/` (the streaming-modal + `actions!` + `register_action` shape).

### Files to create

```
crates/lingcode_install/Cargo.toml
crates/lingcode_install/src/lingcode_install.rs
```

### Files to edit

| File | Change |
|---|---|
| `Cargo.toml` (workspace root) | add `"crates/lingcode_install"` to `members` (keep alpha order with the other `lingcode_*` crates) |
| `crates/zed/Cargo.toml` | add `lingcode_install.workspace = true` to `[dependencies]` (next to `lingcode_cloud`) — and add `lingcode_install = { path = "crates/lingcode_install" }` to the root `[workspace.dependencies]` |
| `crates/zed/src/main.rs` (line 722-724) | add `lingcode_install::init(app_state.clone(), cx);` |
| `crates/zed/src/zed/app_menus.rs` (Cloud menu, ~line 288-303) | add `MenuItem::action("Install Dependencies", lingcode_install::MagicInstall)` |

### `crates/lingcode_install/Cargo.toml`

Copy `crates/lingcode_cloud/Cargo.toml` verbatim, changing `name`, `[lib].path`. `editor` and `menu` deps are
only needed if you add a text input (Magic Install has none), so they can be dropped:

```toml
[package]
name = "lingcode_install"
version = "0.1.0"
edition.workspace = true
publish.workspace = true
license = "GPL-3.0-or-later"

[lints]
workspace = true

[lib]
path = "src/lingcode_install.rs"
doctest = false

[dependencies]
anyhow.workspace = true
futures.workspace = true
gpui.workspace = true
log.workspace = true
serde.workspace = true
ui.workspace = true
util.workspace = true
which.workspace = true
workspace.workspace = true
```

### `crates/lingcode_install/src/lingcode_install.rs` (skeleton)

Detection table ported from `MagicInstallService.swift` lines 34-191. Decision: **run detection natively in
Rust** (no CLI dependency — unlike Cloud/Push which delegate to the `lingcode` CLI), because detection is just
"does file X exist" + "run command Y", and keeping it native means Magic Install works even where the CLI
is not on PATH.

```rust
//! Magic Install: detect the project's package manager(s) and run their install
//! commands, streaming output into a modal. Native port of the macOS
//! MagicInstallService (no `lingcode` CLI dependency).

use std::{path::PathBuf, process::Stdio, sync::Arc};

use anyhow::{Context as _, Result};
use futures::{AsyncBufReadExt as _, StreamExt as _};
use gpui::{
    App, AppContext as _, Context, DismissEvent, EventEmitter, FocusHandle, Focusable,
    Render, SharedString, Task, Window, actions,
};
use ui::prelude::*;
use util::process::Child;
use workspace::{AppState, DismissDecision, ModalView, Workspace};

actions!(
    lingcode_install,
    [
        /// Detect package managers in the current project and install dependencies.
        MagicInstall,
    ]
);

pub fn init(_: Arc<AppState>, cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, _window, _cx: &mut Context<Workspace>| {
        workspace.register_action(|workspace, _: &MagicInstall, window, cx| {
            let cwd = workspace
                .project()
                .read(cx)
                .visible_worktrees(cx)
                .next()
                .map(|wt| wt.read(cx).abs_path().to_path_buf());
            let Some(cwd) = cwd else {
                log::error!("Magic Install: no open project");
                return;
            };
            workspace.toggle_modal(window, cx, move |_window, cx| InstallModal::new(cwd, cx));
        });
    })
    .detach();
}

/// One package manager: the marker files that imply it, the command to run, and
/// any markers that suppress it (e.g. plain `npm` suppressed when `yarn.lock`
/// is present). Port the full table from MagicInstallService.swift:34-191.
struct PackageManager {
    name: &'static str,
    markers: &'static [&'static str],
    suppressed_by: &'static [&'static str],
    program: &'static str,
    args: &'static [&'static str],
}

const MANAGERS: &[PackageManager] = &[
    PackageManager { name: "npm", markers: &["package.json"], suppressed_by: &["yarn.lock", "pnpm-lock.yaml", "bun.lockb"], program: "npm", args: &["install"] },
    PackageManager { name: "yarn", markers: &["yarn.lock"], suppressed_by: &[], program: "yarn", args: &["install"] },
    PackageManager { name: "pnpm", markers: &["pnpm-lock.yaml"], suppressed_by: &[], program: "pnpm", args: &["install"] },
    PackageManager { name: "cargo", markers: &["Cargo.toml"], suppressed_by: &[], program: "cargo", args: &["fetch"] },
    PackageManager { name: "pip", markers: &["requirements.txt"], suppressed_by: &[], program: "python", args: &["-m", "pip", "install", "-r", "requirements.txt"] },
    // … port the remaining ~24 managers (gradle, pod, bundler, go mod, composer, etc.)
];

// Status + InstallModal struct + new() + push_line() are a straight copy of
// CloudModal (lingcode_cloud.rs:158-209), minus the `url` field.

async fn run_install(
    cwd: PathBuf,
    this: gpui::WeakEntity<InstallModal>,
    cx: &mut gpui::AsyncApp,
) -> Result<()> {
    // 1. detect: collect managers whose markers exist and aren't suppressed.
    let present: Vec<&PackageManager> = MANAGERS
        .iter()
        .filter(|m| m.markers.iter().any(|f| cwd.join(f).exists()))
        .filter(|m| !m.suppressed_by.iter().any(|f| cwd.join(f).exists()))
        .collect();

    if present.is_empty() {
        this.update(cx, |modal, cx| {
            modal.push_line("No recognized package manager in this project.", cx);
        }).ok();
        return Ok(());
    }

    // 2. run each detected install command, streaming stdout to the modal.
    for pm in present {
        this.update(cx, |modal, cx| modal.push_line(format!("→ {} {}", pm.program, pm.args.join(" ")), cx)).ok();

        // which::which falls back to the bare name so PATH resolution matches Cloud.
        let program = which::which(pm.program).unwrap_or_else(|_| PathBuf::from(pm.program));
        let mut command = util::command::new_std_command(&program);
        command.args(pm.args);
        command.current_dir(&cwd);

        let mut child = Child::spawn(command, Stdio::null(), Stdio::piped(), Stdio::piped())
            .with_context(|| format!("failed to launch `{}`", pm.program))?;
        let stdout = child.stdout.take().context("no stdout")?;
        let mut lines = futures::io::BufReader::new(stdout).lines();
        while let Some(Ok(line)) = lines.next().await {
            if line.is_empty() { continue; }
            this.update(cx, |modal, cx| modal.push_line(format!("  {line}"), cx)).ok();
        }
        let status = child.status().await.context("await exit")?;
        this.update(cx, |modal, cx| {
            modal.push_line(format!("{} {}", pm.name, if status.success() { "✓" } else { "failed" }), cx)
        }).ok();
    }
    Ok(())
}

// ModalView / Focusable / EventEmitter<DismissEvent> / Render: copy CloudModal's
// impls (lingcode_cloud.rs:296-358), dropping the `url`/Open-button branch.
```

### Windows-specific notes
- `util::command::new_std_command()` is the cross-platform spawner Cloud/Android already use — handles the
  `.bat` wrapper case on Windows. **Do not** hardcode `/bin/zsh` (the Swift service's macOS assumption).
- Home dir / temp dir: if needed use `std::env::temp_dir()` and the `home_dir()` helper already in
  `crates/lingcode_android/src/lingcode_android.rs`.

### Test
- Unit-test the detection filter (markers present + suppression) with a temp dir fixture — pure logic, no spawn.
- Manual: open a Node project → Cloud menu → Install Dependencies → confirm `npm install` streams.

---

## Feature 2 — Magic Push AI commit message (enhance existing)

`lingcode_cloud::PushToGithub` already exists (`crates/lingcode_cloud/src/lingcode_cloud.rs:367-694`) and
runs `lingcode github push . --ndjson`. The only macOS feature missing is the **AI-generated commit message**
from the diff (`MagicPushService.swift`).

### Recommended approach: put the AI step in the CLI, not the Rust modal
The commit currently happens **inside the `lingcode` CLI** (`src/github/push.ts`, emits the `commit` NDJSON
event). The cleanest parity is to add the message generation there (the CLI already has model access and the
diff), gated behind a flag the Rust side passes:

| File | Change |
|---|---|
| `lingcode` CLI `src/github/push.ts` (separate repo) | when no message supplied, call the model on `git diff --cached` for a one-line imperative message; emit it in the existing `commit` event |
| `crates/lingcode_cloud/src/lingcode_cloud.rs:516-521` | append `"--ai-message"` to the args so the CLI knows to generate |
| `crates/lingcode_cloud/src/lingcode_cloud.rs:565-571` (`PushEvent::Commit`) | the `message` field is already received (currently `#[allow(dead_code)]`) — surface it: `modal.push_line(format!("Committed {changed} file(s): {message}"), cx)` |

Rationale: keeps the model call cross-platform and out of GPUI; the Rust modal only needs a one-word arg
change + showing the message it already receives. **Effort: low.**

Alternative (if CLI can't be touched): generate in Rust before the push using the `LanguageModelRegistry`
default model — heavier (needs a model handle inside the cloud crate) and duplicates CLI logic. Not recommended.

---

## Feature 3 — LingModel OAuth sign-in

Mac flow (`LingCodeAuthService.swift`): open `https://lingcode.dev/oauth/authorize` with PKCE+state →
OS delivers `lingcode://auth/callback?code=…&state=…` → exchange code at the token endpoint → store token in
Keychain → use as the LingModel bearer. Windows has the scheme registered (`zed.iss:1262-1265`) and the same
Keychain store (`ApiKeyState`), but **`open_listener.rs` does not parse `auth/callback`**, and there is no
OAuth state machine.

### The 3 edits

#### (a) `crates/zed/src/zed/open_listener.rs` — parse the callback

Add a variant to `OpenRequestKind` (after `GitCommit`, line ~79) and its `Debug` arm (line ~115):

```rust
LingModelAuthCallback {
    code: Option<String>,
    state: Option<String>,
    access_token: Option<String>,
    error: Option<String>,
},
```

Add a parse arm in `OpenRequest::parse` (in the `url.strip_prefix` chain, alongside the other
`lingcode://…` arms, ~line 174). Use `url::Url` to read the query (the file already depends on the `url`
crate — see `parse_agent_url`):

```rust
} else if url.starts_with("lingcode://auth/callback") {
    if let Ok(parsed) = url::Url::parse(&url) {
        let mut code = None; let mut state = None;
        let mut access_token = None; let mut error = None;
        for (k, v) in parsed.query_pairs() {
            match k.as_ref() {
                "code" => code = Some(v.into_owned()),
                "state" => state = Some(v.into_owned()),
                "access_token" => access_token = Some(v.into_owned()),
                "error" => error = Some(v.into_owned()),
                _ => {}
            }
        }
        this.kind = Some(OpenRequestKind::LingModelAuthCallback { code, state, access_token, error });
    }
}
```

#### (b) Dispatch — route the callback to a global the provider listens on

The parsed `OpenRequestKind` is consumed in `handle_open_request` (in `crates/zed/src/zed.rs`; the other
variants like `GitClone`/`SharedAgentThread` are matched there). Add a match arm that pushes the callback into
a **global event channel** the provider subscribes to. Model this on the cloud provider's
`RefreshLlmTokenListener` pattern (`crates/language_models/src/provider/cloud.rs`) — a `Global` entity that
emits an event:

```rust
// new, in language_model crate (so both zed + language_models can see it):
pub struct LingModelAuthCallback { pub code: Option<String>, pub state: Option<String>,
    pub access_token: Option<String>, pub error: Option<String> }
// a GlobalLingModelAuth entity that EventEmitter<LingModelAuthCallback>; zed.rs emits, provider observes.
```

This cross-crate hop (zed crate → language_models crate) is the main structural work; the listener entity is
~40 lines.

#### (c) `crates/language_models/src/provider/ling_model.rs` — OAuth state machine + UI

Extend `State` (line 65) with pending PKCE fields and a sign-in method. Reuse `ApiKeyState::store` (line 77)
to persist the resulting token so **inference code at line 205-225 needs zero changes** — it still reads
`api_key_state.key()`.

```rust
// add to State:
pending: Option<PendingAuth>,   // { state: String, verifier: String }

impl State {
    fn begin_browser_sign_in(&mut self, cx: &mut Context<Self>) {
        let verifier = random_url_safe(64);
        let challenge = base64_url(sha256(&verifier));   // PKCE S256
        let state = random_url_safe(32);
        self.pending = Some(PendingAuth { state: state.clone(), verifier });
        let url = format!(
            "https://lingcode.dev/oauth/authorize?response_type=code\
             &client_id=lingcode-ide&redirect_uri=lingcode://auth/callback\
             &code_challenge={challenge}&code_challenge_method=S256&state={state}"
        );
        cx.open_url(&url);
    }

    // called when the global LingModelAuthCallback fires (provider observes it in new()):
    fn complete_sign_in(&mut self, cb: LingModelAuthCallback, cx: &mut Context<Self>) -> Task<Result<()>> {
        // 1. validate cb.state == self.pending.state (reject mismatch)
        // 2. if cb.access_token present → store it directly
        // 3. else POST {grant_type, code, redirect_uri, code_verifier} to the token endpoint,
        //    parse access_token, then self.set_api_key(Some(token), cx)
    }
}
```

Add a "Sign In with Browser" button to `ConfigurationView::render` (line 422 branch, above the API-key
input) calling `begin_browser_sign_in`. Keep the pasted-key path as the fallback — both write the same
`ApiKeyState`, so `is_authenticated()` and inference are unchanged.

Endpoints to confirm against the server before building: the authorize URL, the token URL, the `client_id`,
and whether the server returns `access_token` directly on the callback or an exchangeable `code`
(the Mac code handles both — mirror that).

### Risks (why this was punted in `LINGCODE-CHANGES.md`)
- **Cold-launch race:** the browser can deliver `lingcode://auth/callback` before the app/listener is ready.
  The global listener entity must be installed early (during app init, before `on_open_urls`) and must
  buffer a callback that arrives before the provider subscribes.
- **Cross-crate plumbing** (zed → language_models) is the bulk of the work; the OAuth math (PKCE/state) is
  standard.
- Must respect `LINGMODEL_API_KEY` env var: if set, skip the browser flow (same as macOS).
- `redirect_uri` string must **exactly** equal what the OAuth server has registered.

### Test
- Unit: parse `lingcode://auth/callback?code=x&state=y` → correct variant (add to the existing
  `test_parse_*` suite in open_listener.rs).
- Manual, both paths: cold launch (app closed) and warm launch (app open) → click Sign In → complete in
  browser → confirm token lands in keychain and an inference request succeeds.

---

## Feature 4 — Remote coding (later, staged)

No native equivalent on Windows. Mac runs an in-app HTTP+SSE server (`LingCodeServer`, Darwin `NWListener`) +
a Node collab-bridge over a hosted relay, so a phone drives the agent with zero setup. Zed offers SSH
remoting (`crates/remote`, `crates/remote_server`) and collab, but neither exposes the agent.

**Recommended path (Option B): extend Zed's `remote_server` with agent RPC** mirroring Mac's
`/v1/agent/ask` SSE contract, phone connects over the SSH tunnel. Reuses Zed's heartbeat/reconnect; loses the
zero-setup property. Rough estimate 2-3 weeks. **Defer until items 1-3 land** — it's the only one requiring
new architecture rather than the established crate/provider patterns.

Option A (port serve+relay to keep zero-setup) is higher effort and re-validates against Zed's RPC; only
pursue if zero-setup is a hard product requirement.

---

## Wiring checklist (all features)

- [ ] Magic Install: new crate + 4 edits (workspace `Cargo.toml`, `zed/Cargo.toml`, `main.rs:722`, `app_menus.rs:288`)
- [ ] Magic Push AI: CLI `push.ts` + 2 small `lingcode_cloud.rs` edits
- [ ] OAuth: `open_listener.rs` (variant + parse + Debug), `zed.rs` dispatch, new global listener in `language_model`, `ling_model.rs` (state machine + button)
- [ ] Build with `build_lingcode.bat` (ARM64 toolchain) — must compile clean
- [ ] Run the relevant manual flow for each before commit
- [ ] Append a section to `LINGCODE-CHANGES.md` describing what shipped (matches existing discipline)
