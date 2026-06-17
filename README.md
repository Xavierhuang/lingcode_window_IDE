# LingCode

LingCode is a high-performance, multiplayer code editor — a rebranded fork of
[Zed](https://github.com/zed-industries/zed) (Rust + GPUI) wired to use LingCode's
multi-provider AI agent instead of Zed's hosted service.

---

### Building from source

- [Building LingCode for Windows](./WINDOWS-BUILD.md)

The upstream Zed build guides still apply for other platforms:

- [Building for macOS](./docs/src/development/macos.md)
- [Building for Linux](./docs/src/development/linux.md)

### What's different from upstream Zed

LingCode keeps changes minimal and centralized (branding, identifiers, assets, and the
built-in agent) to ease rebasing against upstream Zed. See
[LINGCODE-CHANGES.md](./LINGCODE-CHANGES.md) for the complete list.

Highlights:

- Rebranded **Zed → LingCode** across all user-visible chrome (app name, menus, About,
  welcome screen, settings, provider-configuration text, notifications).
- Ships **LingCode's ACP agent** as the built-in agent.
- The Zed-hosted cloud provider (Zed Pro/Business/AI paywall) is **disabled**, and
  built-in auto-update is **disabled** (LingCode manages its own updates).

### Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md).

### Licensing

LingCode is a fork of Zed and inherits its licensing. License information for third-party
dependencies must be correctly provided for CI to pass. We use
[`cargo-about`](https://github.com/EmbarkStudios/cargo-about) to comply with open-source
licenses; if CI fails on licensing, check `script/licenses/zed-licenses.toml` (see the
upstream guidance in the Zed repository for details).
