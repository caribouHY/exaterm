---
name: exaterm-ui-change
description: Apply ExaTerm-specific desktop UI, React, and CSS conventions while preserving terminal sessions and sensitive-data boundaries. Use when Codex changes ExaTerm layouts, dialogs, menus, controls, styling, design tokens, component CSS, visual states, or screenshot-led UI behavior.
---

# ExaTerm UI Change

## Prepare

- Inspect the target React component, its CSS, and the relevant shared tokens in `src/index.css` before editing.
- Preserve the current VS Code-inspired direction: compact, restrained, task-oriented, and suitable for repeated terminal work.
- Treat screenshots as layout evidence. Do not copy terminal output, connection targets, usernames, prompts, logs, or secrets into examples or diagnostics.
- Identify whether the change can remount a `TerminalView`, alter terminal-area dimensions, or reset tab, focus, scrollback, connection, or log state.

## Style the Existing System

- Use semantic tokens from `src/index.css` before adding colors, shadows, spacing, or radii.
- Add a token only when it has reusable meaning across components.
- Keep component-specific layout rules in the component CSS file and follow existing BEM-style class names.
- List specific properties in transitions; do not use `transition: all`.
- Use `--radius-sm` for controls, `--radius-md` for popovers and compact grouped surfaces, and `--radius-lg` only for dialogs or intentionally elevated empty states.
- Limit raw colors and repeated `rgba(...)` values to token definitions or unavoidable third-party overrides.
- Do not introduce Tailwind, shadcn/ui, CSS modules, or a new styling system without a separately approved migration.

## Preserve Desktop Interaction

- Keep buttons, inputs, menus, tabs, and status controls compact and keyboard accessible.
- Prefer icon buttons only when the action is obvious, and provide an accessible label.
- Use menus for option sets, tabs for major views, toggles for binary settings, and lists or tables for log-like data.
- Keep hover states subtle and focus states visible.
- Use overlays or delegation to an already mounted terminal for transient menus and commands when practical.
- Do not clear, recreate, or disconnect a terminal session as a side effect of a visual change. Do not resize it solely as a visual workaround.

## Keep Feedback Safe

- Use generic, privacy-safe text for status, diagnostics, screenshots, and empty states.
- Update both `src/locales/en.json` and `src/locales/ja.json` when user-visible text changes.
- Preserve existing confirmation behavior for destructive actions.

## Validate

- Use `$exaterm-validate-change` to select and report validation commands.
- Inspect the final diff for one-off values, accidental styling-system changes, terminal remounts, and English/Japanese text drift.
- Distinguish automated checks from GUI, connected-session, hardware, and screenshot verification that was not performed.
