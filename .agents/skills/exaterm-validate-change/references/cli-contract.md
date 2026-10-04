# CLI Contract Synchronization

For an `exaterm-cli` change, inspect the public contract before validation. Command names, options, defaults, JSON fields, errors or exit behavior, permissions, readiness or recovery guidance, and session lifecycle changes must update the installable Agent Skill in the same change:

- Update `skills/exaterm-cli/SKILL.md` for discovery, workflow, authorization or operating rules.
- Update `skills/exaterm-cli/references/cli-reference.md` for syntax, result fields, limits, defaults or troubleshooting.
- Update both when both agent decisions and exact command details change.

Updating `docs/CLI_GUIDE.*.md` does not update the Agent Skill. Before committing or publishing, confirm the relevant files were updated or explain why the CLI change has no user-visible or agent-visible contract impact.

When either installable Skill file changes, use `skill-creator` and run its `quick_validate.py` against `skills/exaterm-cli`, in addition to the validation entrypoint's formatting and code checks.
