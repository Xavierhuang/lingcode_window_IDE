# LingCode

LingCode is a high-performance, multiplayer code editor built in Rust with a
GPU-accelerated renderer and a built-in, multi-provider AI agent.

## Download

Grab the latest Windows installer from the
[Releases](https://github.com/Xavierhuang/lingcode_window_IDE/releases) page:

- **LingCode-x86_64.exe** — Intel / AMD PCs
- **LingCode-aarch64.exe** — ARM64 PCs (Snapdragon, Surface Pro X, etc.)

> The build is currently unsigned, so Windows SmartScreen may warn on first launch —
> click **More info → Run anyway**.

## Features

- Fast, GPU-accelerated editing with full language-server (LSP) support
- A built-in AI agent with multiple model providers (LingModel, Kimi, Qwen, and more)
- Integrated cloud deploy, project sharing, and Android build/deploy tooling
- Real-time multiplayer collaboration
- Start from a starter project with **New from Template**

## Building from source

- [Building LingCode for Windows](./WINDOWS-BUILD.md)

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md).

## License

LingCode is open source. The applicable terms are in
[LICENSE-GPL](./LICENSE-GPL), [LICENSE-APACHE](./LICENSE-APACHE), and
[LICENSE-AGPL](./LICENSE-AGPL). Third-party dependency licenses are tracked with
[`cargo-about`](https://github.com/EmbarkStudios/cargo-about).
