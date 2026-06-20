# macOS LingCode IDE → Windows fork — parity status

Snapshot of where the Windows Zed fork stands against the macOS LingCode app. Derived from
`LINGCODE-CHANGES.md` (what shipped) and `MAC-PARITY-IMPLEMENTATION-PLAN.md` (the plan). Keep this in
sync as features land/verify.

**Legend**
- ✅ **Ported** — code present (but see "Verified" column; most is not yet build/run-proven)
- ⚠️ **Partial** — some of the feature ships; pieces missing
- 🚧 **Staged** — designed/planned, not built (or only a slice built)
- ❌ **Not portable** — macOS/Apple-only by nature

## AI / providers

| macOS feature | Windows status | Verified | What's left |
|---|---|---|---|
| LingModel managed provider | ✅ Ported (`provider/ling_model.rs`) | ❌ not run | confirm endpoint + a real inference call |
| LingModel browser OAuth sign-in | ✅ Ported (PKCE + callback) | ❌ **high risk** | cold+warm launch end-to-end; confirm OAuth URLs/client_id |
| Kimi / Qwen / z.ai providers | ✅ Ported (openai_compatible presets) | ❌ not run | smoke-test each endpoint |
| z.ai native `thinking` toggle | 🚧 Not sent | — | needs a dedicated provider w/ custom request JSON |
| Cloud paywall / Zed Pro upsell | ✅ Removed (provider disabled) | ❌ not run | n/a |

## Cloud / deploy ("Magic")

| macOS feature | Windows status | Verified | What's left |
|---|---|---|---|
| Magic Install (detect + install deps) | ✅ Ported (`lingcode_install`) | ❌ not run | run on a real Node/cargo project |
| Magic Push (push to GitHub) | ✅ Ported (`lingcode_cloud::PushToGithub`) | ❌ not run | — |
| Magic Push **AI commit message** | ⚠️ Editor side done | ❌ not run | **companion `lingcode` CLI change** (`--ai-message`) must ship together |
| Cloud Console + Project Sharing | ✅ Ported (Cloud menu actions) | ❌ not run | — |
| Magic Ship / App Store upload | ❌ Apple-only | — | Android path covers the mobile case |

## Mobile (Android)

| macOS feature | Windows status | Verified | What's left |
|---|---|---|---|
| Toolchain check / build APK / bundle / run | ✅ Ported (`lingcode_android`) | ❌ not run | — |
| Logcat / Layout Inspector / Analyze APK | ✅ Ported (modal output) | ❌ not run | — |
| Deploy to Google Play (full API flow) | ✅ Ported | ❌ not run | service-account config + a real upload |
| Kotlin/Java **JDWP debugger** | 🚧 Staged | — | wire into Zed's DAP/debugger UI |
| Dockable logcat/layout **panels** | 🚧 Staged | — | currently modal-only |
| APK **diff** + richer analyzer UI | 🚧 Staged | — | needs a two-file picker |
| Run-destination toolbar picker | 🚧 Staged | — | targets first device only today |
| AVD create/delete UI | 🚧 Staged | — | needs text-input UI |
| GPUI deploy form (Keychain creds) | 🚧 Staged | — | uses `.lingcode/play-deploy.json` today |

## Remote coding ("drive the agent from your phone")

| macOS feature | Windows status | Verified | What's left |
|---|---|---|---|
| Web remote-control **client** | ✅ Ported (`OpenRemoteControl`) | ❌ not run | — |
| Host server lifecycle | ⚠️ Partial (`lingcode_remote` spawns `lingcode serve`/`remote`) | ❌ not run | build + run |
| Zero-setup relay bridge (`lingcode remote`) | ⚠️ In CLI (separate repo) | ❌ not run | `bun install` + live relay + sign-in to verify |
| Make **this** machine a drivable host | 🚧 Staged | — | the marquee gap — est. *weeks* of new networked Rust |

## Editor surface / branding

| Item | Status |
|---|---|
| Rebrand Zed → LingCode (chrome, menus, icons, identifiers) | ✅ Done |
| De-brand pass (hide upstream; repoint `zed.dev` links) | ✅ Done (reachable surfaces) |
| Auto-update (GitHub Releases) | ✅ Wired — **pending first real end-to-end update test** |
| Project templates (New from Template) | ✅ Ported (`lingcode_templates`) |

---

## Bottom line

- **Feature surface: ~80% present.** The two real gaps are the **remote-coding host** (only the client
  ships) and the **Android debugger / dockable panels**.
- **Verification is the gating risk:** almost nothing in the ported set has been compiled+run with the
  ARM64 toolchain yet. The release build now running on `main` is the first full compile; passing it only
  proves *compilation*, not behavior. Each ✅ above still needs its manual flow exercised before it's trusted.
- **To reach true full parity**, in rough order: (1) get the build green, (2) run each Magic/Android/LingModel
  flow once on a real machine, (3) ship the `lingcode` CLI `--ai-message` companion change, (4) build the
  remote-coding **host** (the only multi-week, architectural item), (5) finish the staged Android UI pieces.
