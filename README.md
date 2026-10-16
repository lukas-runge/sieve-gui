# sieve-gui

A lightweight, cross-platform desktop editor for **server-side Sieve mail filters**. It talks to your mail server via **ManageSieve** (RFC 5804), so filters keep running even when no mail client is open. Built with Tauri and Svelte.

> **Status:** early scaffold — no ManageSieve functionality yet.

---

## Stack

| Layer | Technology |
|---|---|
| Desktop shell | [Tauri](https://tauri.app) |
| UI framework | [SvelteKit](https://kit.svelte.dev) + [Svelte](https://svelte.dev) |
| Styling | [Tailwind CSS](https://tailwindcss.com) + [daisyUI](https://daisyui.com) |
| Language (frontend) | TypeScript |
| Language (backend) | Rust |
| TS bindings | [`tauri-ts-generator`](https://github.com/lukas-runge/tauri-codegen) |
| Package manager | pnpm |

---

## Prerequisites

| Tool | Install |
|---|---|
| Node.js | [nodejs.org](https://nodejs.org) |
| pnpm | `npm i -g pnpm` |
| Rust | [rustup.rs](https://rustup.rs) |
| Tauri CLI v2 | bundled via `pnpm tauri` |

**Linux only** — install WebKit and related system libraries:

```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev \
  librsvg2-dev \
  patchelf
```

---

## Getting started

```bash
# 1. Clone
git clone https://github.com/lukas-runge/sieve-gui.git
cd sieve-gui

# 2. Install frontend dependencies (also installs tauri-ts-generator and generates TS bindings)
pnpm install

# 3. Start in development mode (hot-reload frontend + Rust backend)
pnpm tauri dev
```

The app window opens automatically. The Vite dev server runs on `http://localhost:1420`.

---

## Building

```bash
# Production build for the current platform
pnpm tauri build
```

Artifacts land in `src-tauri/target/release/bundle/`.

---

## Project structure

```
sieve-gui/
├── src/                        # SvelteKit frontend
│   ├── app.css                 # Tailwind + daisyUI imports
│   ├── app.html                # HTML shell
│   └── routes/
│       ├── +layout.svelte      # Imports app.css
│       └── +page.svelte        # Main UI
├── src-tauri/                  # Tauri + Rust backend
│   ├── src/
│   │   └── lib.rs              # Tauri commands
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/generated/tauri/        # Auto-generated TS bindings (do not edit)
├── app-icon.svg                # Icon source (`pnpm tauri icon app-icon.svg`)
└── .github/workflows/
    ├── ci.yml                  # Type-check + cargo check + Clippy on every PR
    └── release.yml             # Cross-platform builds triggered by version tags
```

---

## How it works

Tauri exposes Rust functions as IPC commands. `tauri-ts-generator` parses `src-tauri/src` and generates typed TypeScript wrappers into `src/generated/tauri/`, importable via the `tauri/*` alias:

```ts
import { getAppInfo } from "tauri/commands";
import type { AppInfo } from "tauri/types";
```

Bindings are regenerated automatically before `dev`, `build` and `check`.

---

## Releases

Pushing a `v*` tag builds draft releases for:

| Platform | Architecture |
|---|---|
| Linux | x86-64, ARM64 |
| Windows | x86-64, ARM64 |
| macOS | x86-64 (Intel), ARM64 (Apple Silicon) |

> **Note:** Builds are not code-signed. macOS builds are ad-hoc signed so Apple Silicon runs them, but Gatekeeper will still warn on first launch; Windows shows a SmartScreen prompt.

---

## Contributing

Found a bug? Have an idea? 😃 Just open an issue or send a PR — all help is genuinely appreciated. 🙏
