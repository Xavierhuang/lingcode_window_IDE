# Remote coding on Windows — implementation plan (host side)

Status: **plan + a shipped client slice.** Companion to `MAC-PARITY-IMPLEMENTATION-PLAN.md`.

## What shipped now (client slice)

`lingcode_cloud::OpenRemoteControl` (menu: **Cloud → Remote Control (Web)**) opens
`https://lingcode.dev/remote-control.html`. This is the **client**: a Windows user can drive their *other*
LingCode hosts (a Mac, or a future Windows host) from the web UI, zero setup. It works today because the relay
and web client are platform-independent and already deployed.

What it does **not** do: make *this* Windows machine a drivable **host**. That's the work below.

## Why the host side is real work (the wall)

The macOS app hosts remote coding with an in-app **`LingCodeServer`** (Swift, Darwin `NWListener`) exposing an
HTTP+SSE agent contract, plus a Node **collab-bridge** that joins the hosted relay. Confirmed facts:

- The serving component is **macOS-native** (`LingCodeServer/`, and `lingcode serve` in the Swift CLI). There
  is **no cross-platform `serve`** — so unlike the cloud actions, the fork cannot just spawn the `lingcode`
  CLI to host. (Verified: `lingcode serve` lives in `LingCodeCLI/Sources/lingcode/Commands/Serve.swift`,
  macOS-only.)
- The fork's agent is **ACP-based** (`lingcode acp` / `claude-acp` / `codex-acp` / `gemini`), rendered
  natively by Zed. There is no HTTP surface over it.

So hosting on Windows requires writing a **new Rust HTTP+SSE server** in the fork that bridges to an ACP agent.
This is the large piece; estimated multiple weeks; must be built + run to verify (no part compiles here blind).

## The macOS server contract to mirror

From `LingCodeServer/` (the API the web client + iPad already speak):

| Endpoint | Purpose |
|---|---|
| `POST /v1/agent/ask` | SSE stream: queryStarted, assistantText, thinking, toolUse, permissionRequest, toolResult, queryFinished |
| `POST /v1/agent/permission/:id` | approve/deny a tool call |
| `POST /v1/agent/cancel/:queryId` | cancel a running query |
| `GET  /v1/health` | server version + concurrency limits |
| `POST /v1/workspace/{read,write,list,delete,stat,exec}` | sandboxed file + shell I/O (path-jailed to the workspace root) |
| `GET  /v1/workspace/watch` | SSE file-change stream |

Auth: bearer token tied to the user's LingCode account (the same account the web client signs into).

## UPDATE — the server already exists; don't rewrite it

The Windows **`lingcode` CLI already ships `lingcode serve`** — a complete cross-platform headless server
(sessions, SSE event streams, permissions, PTY, files). So Stages 1 (and most of 3) below — re-implementing
the agent/workspace HTTP+SSE server in Rust — are **unnecessary**. The shipped `crates/lingcode_remote/` crate
just **spawns and manages `lingcode serve`** (see `LINGCODE-CHANGES.md`), reachable now on LAN / SSH / `--attach`.

**Stage 2 (the zero-setup relay bridge) is now built** — `lingcode remote` in the cross-platform CLI
(`src/remote/serve-tunnel.ts` + `src/cli/cmd/remote.ts`), a faithful port of the macOS `collab-bridge`
serve-host. The IDE's `lingcode_remote` crate spawns it. Needs `bun install` (3 new deps) + a live relay to
verify end-to-end. The from-scratch Rust server design below is retained only as a fallback if the CLI server
can't be used.

## Fallback design — from-scratch `lingcode_remote` Rust server (Option A, relay-backed)

Preferred over Option B (extending Zed's SSH `remote_server`) because it **keeps the zero-setup property** the
macOS feature is built around (no SSH for the user) and **reuses the already-deployed relay + the web client
this PR's client slice opens**. Build the host natively in Rust:

```
crates/lingcode_remote/Cargo.toml
crates/lingcode_remote/src/lingcode_remote.rs
```

Dependencies available in-tree: **axum** (HTTP/SSE server — already used by `crates/collab`), **tokio**,
`gpui`, `workspace`, `agent`/ACP client crates, `serde`/`serde_json`, `util`. Wire via `lingcode_remote::init`
in `crates/zed/src/main.rs` and a **Remote** app menu in `crates/zed/src/zed/app_menus.rs`
(Enable / Disable / Copy Pairing Link / Status), mirroring the `lingcode_cloud`/`lingcode_android` crates.

### Staged build (each stage independently shippable + testable)

**Stage 1 — local agent HTTP+SSE server.**
- `EnableRemoteCoding` action starts an axum server on a loopback/LAN port.
- Implement `GET /v1/health` and `POST /v1/agent/ask` (SSE). `ask` drives an **ACP agent session** (reuse the
  fork's existing ACP client — the same path the agent panel uses) and maps ACP events → the SSE event names
  in the contract table.
- Implement `POST /v1/agent/permission/:id` and `/cancel/:queryId` against the live session registry.
- Bearer-auth middleware. Reachable on LAN or via the user's own SSH/tunnel. **Testable with `curl`.**

**Stage 2 — zero-setup reach via the relay.**
- Spawn/port the host **bridge** that registers via `POST /api/remote/hosts` and joins
  `wss://lingcode.dev/ws/collab/<room>` as host, demuxing the `lc-serve-*` frames to the local Stage-1 server.
  Two routes: (a) ship the existing Node `bridge.mjs` as a bundled subprocess (fast, needs Node), or (b)
  reimplement the bridge in Rust (`tokio-tungstenite`, no Node dependency — preferred long-term).
- After this, the **web client (already wired via `OpenRemoteControl`) reaches the Windows host with zero
  setup**, closing the core parity gap.

**Stage 3 — workspace endpoints.** `POST /v1/workspace/{read,write,list,delete,stat,exec}` +
`GET /v1/workspace/watch`, path-jailed to the open worktree (use the `fs`/`worktree` abstractions + the
project's existing file watcher). Gives the phone a file tree + terminal, not just chat.

**Stage 4 — live-session mirror.** Mirror an *open* agent tab to the phone (the macOS `lc-agent-*` frames):
a `LiveSessionRegistry` of open agent threads, snapshot streaming, and `cmd` (send/stop/approve) routed into
the same in-process agent session. Highest effort; do last.

## Option B (fallback) — extend Zed's `remote_server`

If zero-setup is dropped, add the same `/v1/agent/*` endpoints to `crates/remote_server` and let the phone
reach it over the user's SSH tunnel. Reuses Zed's heartbeat/reconnect/transport (`crates/remote`), but the
user must set up SSH, and it doesn't reuse the relay/web-client. Lower product value; only pick if Option A's
relay/bridge can't be used.

## Risk / discipline

Every stage is new networked Rust that **cannot be compiled here** — build with `build_lingcode.bat` and test
each stage end-to-end before the next. Path-jail and bearer-auth must be reviewed carefully (this exposes file
+ shell I/O over the network). Start at Stage 1 behind the action so it's opt-in and off by default.
