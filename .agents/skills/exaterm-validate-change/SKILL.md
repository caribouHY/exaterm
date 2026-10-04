---
name: exaterm-validate-change
description: Select, run, and report validation for ExaTerm changes, including code, documentation, and Skills. Use before marking changes complete or PR-ready.
---

# ExaTerm Validate Change

Choose checks from the final diff. Run from the repository root in Windows PowerShell; command definitions and conditional procedures are maintained in this Skill and its references.

## Command Rules

- Use `pnpm run`, not `npm run` or `corepack`. Use Cargo with `--manifest-path src-tauri/Cargo.toml`; do not change into `src-tauri` for validation.
- Run `pnpm`, `cargo`, and `npm` with the required elevated execution permissions.
- Keep a failed standard command in the report; do not substitute another command to make the result appear green.
- Do not run application builds for documentation, agent guidance, or Skill-only changes unless requested or application behavior is affected.

## Required Checks

Apply every matching category. These frontend and Rust checks include their required CI tests and lint checks.

- **Frontend code, tests, React, TypeScript, CSS, locales, frontend dependencies or build/test configuration:** `pnpm run format`, `pnpm run test:frontend`, `pnpm run build`.
- **Rust code, tests, Tauri commands, Cargo files or backend dependencies:** `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `cargo test --manifest-path src-tauri/Cargo.toml`, `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`.
- **Frontend/backend boundary, shared payloads or invoke surface:** both frontend and Rust checks.
- **Installer, sidecar, CLI binary, runtime packaging or Tauri bundle integration:** relevant code checks plus `pnpm run tauri build --debug`.
- **Documentation, agent guidance or Skills only:** `pnpm run format` and `pnpm run format:check`; check changed paths, links and frontmatter. For changed Skills, use `skill-creator` and its `quick_validate.py`.

## Conditional References

Read only matching references; their requirements add to the checks above.

- For any `exaterm-cli` change, read [CLI contract synchronization](references/cli-contract.md), including when determining that no public contract changed.
- For protocol setup, cancellation, timeouts or a Tauri command's async call chain, read [optimized runtime validation](references/optimized-runtime.md).
- Before publishing or claiming PR readiness, read [PR-ready validation](references/pr-ready.md).

## Completion and Reporting

- Apply the changelog rule in `AGENTS.md`: report the checked entry or why none is required. For release preparation, check the target release heading, not a new `Unreleased` entry.
- Report each executed command and its result, plus skipped required checks and the reason. Summarize failures and distinguish environment failures from code failures when supported by evidence.
- Resolve failures caused by the requested change and rerun affected checks. For retries requiring approval, use the normal permission flow; do not silently change commands.
- Distinguish automated checks from unperformed GUI, connected-session, hardware and optimized-executable verification. Keep terminal content, targets, usernames, prompts, paths to private data and secrets out of reports.
