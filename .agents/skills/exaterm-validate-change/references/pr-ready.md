# PR-Ready Validation

Before publishing or claiming PR readiness:

- Run `pnpm run format`, then `pnpm run format:check`.
- Apply every matching category in the validation entrypoint's `Required Checks`. PR preparation must include the frontend tests and Rust Clippy when their respective areas changed; formatting or builds alone are insufficient.
- Apply the entrypoint's conditional CLI and optimized-runtime references when relevant. Packaging changes also require the debug Tauri build selected by the entrypoint.
- For documentation, agent guidance or Skills only, verify changed content, paths, links and frontmatter; do not add application builds by default.
- Repeat the `AGENTS.md` changelog decision against the final publication diff, including the release-preparation exception.
- Resolve required check failures caused by the intended change before committing or publishing; report remaining blockers and skipped checks honestly.

The entrypoint owns the command selection. This reference adds publication checks without maintaining a second command list.
