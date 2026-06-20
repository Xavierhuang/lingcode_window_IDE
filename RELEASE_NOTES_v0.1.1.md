# LingCode v0.1.1

First cross-platform LingCode IDE release — Windows (x86_64 + ARM64) and Linux (x86_64), with a batch of features brought over from the macOS app.

## Downloads

| Platform | File | Notes |
|---|---|---|
| Windows (Intel/AMD) | `LingCode-x86_64.exe` | installer |
| Windows (ARM64 — Snapdragon/Surface) | `LingCode-aarch64.exe` | installer |
| Linux (x86_64) | `lingcode-linux-x86_64.tar.gz` | extract & run; built on glibc 2.35 (recent distros) |

> ⚠️ **Unsigned builds.** Windows SmartScreen may warn on first launch — click **More info → Run anyway**. (A code-signing cert removes the warning.)

## What's new

**Magic Install** — One action detects your project's package manager(s) from their lockfiles (npm/yarn/pnpm/bun, cargo, pip/poetry/pipenv, go, bundler, composer, dotnet, maven, swift) and runs the right install command, streaming output. Menu: **Cloud → Install Dependencies**.

**AI commit messages for Push to GitHub** — "Push to GitHub" can now generate a one-line commit message from your staged diff (falls back to a sensible default if unavailable). *Requires the matching `lingcode` CLI update.*

**LingModel sign-in with your browser** — The LingModel provider now supports a one-click **"Sign In with Browser"** OAuth flow (PKCE), in addition to pasting an API key. Both paths are supported; the key path is unchanged.

**Remote coding (zero-setup)** — New **Cloud → Start Remote Server** shares this machine so you can drive its agent from the LingCode web remote-control on your phone — no SSH or port config. It registers a relay host, runs a private local server, and tunnels the two. **Cloud → Remote Control (Web)** opens the web client to drive your other machines.

Plus the cross-platform editor, terminal, git, LSP, and multi-provider AI inherited from the LingCode/Zed core.

## Known limitations

- **Sign-in required** for LingModel OAuth and remote coding (a LingCode Cloud account).
- **AI commit messages** and **remote coding** also rely on the companion `lingcode` CLI being installed/updated.
- This is an early build — verify behavior on your setup before relying on it for critical work.

## Verify your download

After download, confirm the file matches your platform from the table above. Report issues at the repo's Issues tab.
