---
name: exaterm-publish-pr
description: Review, validate, commit, push, and open a draft ExaTerm PR to dev when publication is requested.
---

# ExaTerm Publish PR

Use this repository procedure instead of the user-level publication Skill. Publication requires the user's request; this Skill does not authorize publishing an implementation-only task.

## Prepare and Review

- Read `AGENTS.md` and the `Branches and Commits` and `Pull Request Checks` sections of `docs/development/DEVELOPMENT_GUIDE.md`.
- Inspect the branch, working-tree diff, staged diff, and current `origin/dev`. Preserve unrelated user changes and keep an existing working branch unless instructed otherwise. When starting on `dev`, create `codex/<short-description>` from the latest `dev`; never commit directly to `dev`.
- Review the intended diff for regressions, privacy, session ownership, missing tests, and documentation. Fix findings within scope before publication.
- Confirm the required `CHANGELOG.md` entry under `Unreleased`, or state why the change needs no entry under `AGENTS.md`.

## Validate and Publish

- Use `.agents/skills/exaterm-validate-change/SKILL.md` for validation commands. Resolve required check failures caused by the change before committing; report unresolved failures.
- Stage only confirmed paths and inspect the staged diff. Use focused English commits with the repository's change-type prefix. Preserve an explicitly requested commit split.
- Push the branch to `origin` with upstream tracking. Do not force-push without explicit authorization.
- Create a draft PR to `dev` in `caribouHY/exaterm` using the connected GitHub app. Never use `gh`, the GitHub website, or an API fallback. Use ready-for-review only when requested.
- Attach every created PR with `attach_artifact`. If PR creation is unavailable, report the successful push and the remaining blocker.

## PR Content and Completion

Write the title and body in English. Describe the final problem and behavior, self-review findings, validation results, and any material limitations. Include the changelog decision. Keep credentials, terminal content, connection details, and personal paths out of the PR.

Report the branch, commits, push destination, draft PR link, and validation. Distinguish completed, skipped, and blocked steps.
