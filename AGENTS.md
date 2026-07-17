# Repository Guidelines

## Project Structure & Module Organization
- `src/`: React + TypeScript UI. Use `src/*` imports for root-relative frontend modules.
- `src/api/`: frontend wrappers/listeners for Tauri commands and WASM calls; keep these in sync with backend command changes.
- `src/modules/` and `src/prototypes/`: reusable UI modules and current app surfaces, including Pixi-based canvas renderers.
- `src-tauri/`: Tauri desktop backend crate (`thesia`) and native audio/spectrogram logic in `src-tauri/src/core/`.
- `src-wasm/`: WebAssembly Rust crate used by the frontend; generated package is `src-wasm/pkg`.
- `src-common/`: shared Rust utilities used across workspace crates.
- `public/`: static web assets. `samples/`: audio fixtures for manual checks and test inputs.
- `dist/`, `target/`, `src-wasm/pkg/`, and `node_modules/`: generated outputs/dependencies; do not hand-edit.

## Build, Test, and Development Commands
- Prerequisites: Node.js >= 22 and Rust stable. `wasm-pack` is installed locally with the npm dev dependencies.
- First install flow: run `npm install`, then `npm run build:wasm` to generate the local `src-wasm/pkg` package.
- `npm run tauri dev`: run the desktop app in development (launches Vite through Tauri config).
- `npm run dev`: run frontend only at `http://localhost:1420`; Vite uses a strict fixed port for Tauri.
- `npm run build:wasm`: build the WASM module (run before full production builds).
- `npm run build:wasm.debug`: build the WASM module without release optimizations for debugging.
- `npm run build`: TypeScript compile check plus Vite production build.
- `npm run tauri build`: produce installable desktop bundles.
- `cargo test`: run Rust workspace unit tests (`src-common`, `src-wasm`, `src-tauri`).
- `npm run lint` and `npm exec tsc`: run frontend linting and strict type checks.
- `npm run clean`: remove cargo artifacts, `src-wasm/pkg`, and `node_modules`; use only when intentionally resetting local build state.

## Coding Style & Naming Conventions
- TypeScript/React: 2-space indentation, strict typing, `PascalCase` components, `camelCase` functions, `useXxx` hooks.
- Run ESLint (`npm run lint`) and Prettier before opening PRs; use the repository's existing formatting conventions.
- Prefer the configured `src/*` alias for cross-directory frontend imports, and relative imports for colocated files such as SCSS modules.
- Rust: workspace uses edition 2024; keep `snake_case` for functions/modules and `CamelCase` for types; format with `cargo fmt`.
- Keep SCSS modules colocated with components using `*.module.scss`.
- Do not use variables from `color-system.scss` directly in components or mixins. Define purpose-specific semantic color variables in `colors.scss`, then use those variables instead.
- When adding or renaming `#[tauri::command]` functions, update the matching TypeScript wrappers/types in `src/api/`.
- For Pixi renderer changes, keep initialization, resize, render, and destroy paths explicit; avoid work in idle frames unless there is visible state to update.

## Testing Guidelines
- Rust tests are inline with `#[cfg(test)]` and `#[test]`; prefer deterministic fixtures from `samples/` when relevant.
- Frontend currently uses linting and TypeScript checks instead of a dedicated unit test framework.
- If changes touch `src-wasm/`, rebuild `src-wasm/pkg` with `npm run build:wasm` before frontend type/build checks.
- Baseline pre-PR check: `cargo test && npm run lint && npm exec tsc && npm run tauri build`.

## Commit & Pull Request Guidelines
- Match current history style: short, imperative commit subjects (example: `fix panic when removing track quickly`).
- Keep commits focused on one logical change and explain non-obvious decisions in the body.
- PRs should include behavior summary, linked issue (if any), platform notes (macOS/Windows/Linux), and UI screenshots or recordings for visual changes.
- Ensure local checks pass before requesting review.
