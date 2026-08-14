---
name: exaterm-release-prep
description: Prepare ExaTerm release version bumps and release notes. Use when Codex is asked to prepare a new ExaTerm release, update release version metadata, move changelog entries from Unreleased to a release heading, verify release-preparation files, or decide what is in scope before commits, tags, artifacts, pushes, or pull requests.
---

# ExaTerm Release Prep

## Overview

Use this skill when preparing a new ExaTerm release version. Keep the agent workflow self-contained here; do not duplicate these release-preparation steps in human-facing `docs/`.

Run commands from the repository root in Windows PowerShell unless a user explicitly directs otherwise.

## Version Updates

- Update the app version in:
  - `package.json`
  - `pnpm-lock.yaml`
  - `src-tauri/tauri.conf.json`
  - `src-tauri/Cargo.toml`
  - the `name = "exaterm"` package entry in `src-tauri/Cargo.lock`
- Do not update unrelated dependency versions in lockfiles just because they match the old app version.
- Keep UI version displays derived from a single source when possible. The status bar currently reads `package.json` through `src/components/StatusBar/StatusBar.tsx`.
- Search for stale hardcoded app version text before finishing, replacing the placeholders with the old and new release versions:

```powershell
rg "ExaTerm v|<old-version>|<new-version>" src package.json pnpm-lock.yaml src-tauri/tauri.conf.json src-tauri/Cargo.toml
```

## Changelog

- Move the current `CHANGELOG.md` `## Unreleased` entries under a new release heading such as `## v0.2.0`.
- Leave an empty `## Unreleased` heading at the top for the next development cycle.
- Match the existing heading style. Do not add a release date unless the surrounding changelog convention uses dates.
- Confirm the release notes include user-visible additions, fixes, and notable behavior changes.

## Validation

- Use `$exaterm-validate-change` to choose release validation commands before opening or updating the release PR.
- Run `git status --short` and confirm only intended release-preparation files changed.
- Do not report release prep as complete while stale old-version or new-version references remain unexplained.

## Out of Scope Unless Requested

- Do not create commits, tags, release artifacts, pushes, or pull requests unless the user explicitly asks for them.
- Commit messages must be written in English when commits are requested.
- Do not add release-preparation instructions to user-facing `docs/`.
