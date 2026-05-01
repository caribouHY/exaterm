# ExaTerm Release Checklist

Use this checklist when preparing a new ExaTerm release version.

## Version Updates

- Update the app version in:
  - `package.json`
  - `package-lock.json`
  - `src-tauri/tauri.conf.json`
  - `src-tauri/Cargo.toml`
  - the `name = "exaterm"` package entry in `src-tauri/Cargo.lock`
- Do not update unrelated dependency versions in lockfiles just because they match the old app version.
- Keep UI version displays derived from a single source when possible. The status bar currently reads `package.json` through `src/components/StatusBar/StatusBar.tsx`.
- Search for stale hardcoded app version text before finishing, replacing the placeholders with the old and new release versions:
  - `rg "ExaTerm v|<old-version>|<new-version>" src package.json package-lock.json src-tauri/tauri.conf.json src-tauri/Cargo.toml`

## Changelog

- Move the current `CHANGELOG.md` `## Unreleased` entries under a new release heading such as `## v0.2.0`.
- Leave an empty `## Unreleased` heading at the top for the next development cycle.
- Match the existing heading style. Do not add a release date unless the surrounding changelog convention uses dates.
- Confirm the release notes include user-visible additions, fixes, and notable behavior changes.

## Validation

- Run `npm run format:check` before opening or updating the release PR. Editor format-on-save is not enough because CI also checks Markdown, JSON, and files that may not have been saved through the editor.
- Run `npm run build` for frontend and TypeScript validation.
- Run `cargo test` from `src-tauri/` or use `cargo test --manifest-path src-tauri/Cargo.toml`.
- Run `git status --short` and confirm only intended release-preparation files changed.
- Use `npm run tauri -- build --debug` only when installer/runtime integration or distributable bundle validation is part of the task.

## Out of Scope Unless Requested

- Do not create commits, tags, or release artifacts unless the user explicitly asks for them.
- Commit messages must be written in English when commits are requested.
