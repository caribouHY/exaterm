# ExaTerm AI Change Checklist

Use this checklist before and after code changes. It is intentionally short and practical.

## Before Changing Code

- Identify whether the change touches frontend only, backend only, or the Tauri command boundary.
- Check whether active terminal sessions, tabs, scrollback, or terminal buffers could be lost.
- Check whether the change touches logs, secrets, connection details, terminal output, prompts, or API keys.
- For config changes, inspect both `src-tauri/src/config.rs` and `src/types/index.ts`.
- For UI text changes, inspect both `src/locales/en.json` and `src/locales/ja.json`.
- For Tauri command changes, inspect the command module and `src-tauri/src/lib.rs`.
- Use the current config guide paths: `docs/CONFIG_JSON_GUIDE.en.md` and `docs/CONFIG_JSON_GUIDE.ja.md`.
- Do not rely on a root-level `CONFIG_JSON_GUIDE.md`.

## During Implementation

- Preserve terminal state across settings changes and ordinary UI updates.
- Keep logging opt-in and make sensitive-data implications explicit when behavior changes.
- Keep cloud API keys in the OS credential store, not in `config.json` or logs.
- Keep Rust and TypeScript payload shapes synchronized.
- Update both English and Japanese locale files for user-visible text.
- Add a concise entry to `CHANGELOG.md` for user-visible behavior changes, fixes, and notable internal changes.
- For release version bumps, also use `ai-docs/RELEASE_CHECKLIST.md`.
- Register new backend commands in `src-tauri/src/lib.rs`.
- Keep changes scoped to the subsystem requested by the task.

## After Changing Code

- Format the changed area before validation, using `npm run format` or the narrower formatter command when appropriate.
- Do not rely on editor format-on-save as the only formatting gate. Before opening or updating a PR, run `npm run format:check` so Markdown, JSON, and files not saved in the editor are checked the same way CI checks them.
- Confirm AI-facing implementation notes stayed in `ai-docs/` or `AGENTS.md`, not in `docs/`.
- Confirm user-facing documentation stayed in `docs/`.
- Confirm Markdown links and referenced paths exist.
- Confirm active terminal tabs would not be remounted or cleared by the change.
- Confirm log and secret handling still match the privacy model.
- Run the smallest relevant validation set:
  - `npm run build` for frontend and TypeScript changes.
  - `cargo test` for Rust backend changes.
  - `npm run tauri -- build --debug` for installer/runtime integration changes.

Documentation-only changes do not require a full application build.
