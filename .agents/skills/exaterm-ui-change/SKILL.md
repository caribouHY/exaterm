---
name: exaterm-ui-change
description: Apply ExaTerm-specific desktop UI, React, and CSS conventions while preserving terminal sessions and sensitive-data boundaries. Use when Codex changes ExaTerm layouts, dialogs, menus, controls, styling, design tokens, component CSS, visual states, or screenshot-led UI behavior.
---

# ExaTerm UI Change

## Prepare

- Before changing CSS, tokens, shared UI, overlays, or motion, read `docs/development/CSS_ARCHITECTURE.md`.
- Inspect the target React component, its CSS, the layer order in `src/styles/tokens.css`, and the relevant definitions in `src/styles/tokens/semantic.css` or `src/styles/tokens/components.css` before editing.
- Preserve the current VS Code-inspired direction: compact, restrained, task-oriented, and suitable for repeated terminal work.
- Treat screenshots as layout evidence. Do not copy terminal output, connection targets, usernames, prompts, logs, or secrets into examples or diagnostics.
- Identify whether the change can remount a `TerminalView`, alter terminal-area dimensions, or reset tab, focus, scrollback, connection, or log state.

## Style the Existing System

- Use semantic tokens for feature styling. Use component tokens only for the reusable component contract they name, and do not consume primitive tokens from feature CSS.
- Add a token only when it has reusable meaning across components.
- Keep component-specific layout rules in the component CSS file and follow existing BEM-style class names.
- List specific properties in transitions; do not use `transition: all`.
- Use `--radius-control` for controls, `--radius-surface` for popovers and compact grouped surfaces, `--radius-dialog` for dialogs or intentionally elevated empty states, and `--radius-pill` for fully rounded shapes.
- Limit raw colors and repeated `rgba(...)` values to token definitions or unavoidable third-party overrides.
- Do not reintroduce removed compatibility aliases.
- Do not introduce Tailwind, shadcn/ui, CSS modules, or a new styling system without a separately approved migration.

## Share Cross-Feature UI Decisions

- Keep shared control and UI root classes in the registered global stylesheets. Adjust them from feature CSS only beneath a feature-owned selector.
- Use `.ui-overlay` for full-window dialog backdrops. Feature overlay classes own only their shared stack tier and feature-specific motion.
- Use an existing `--stack-*` token for `z-index`. If a new tier is necessary, update the token layers and CSS architecture and review the complete overlay stack instead of adding a numeric feature-local value.
- Use shared motion tokens and keyframes. Treat the global reduced-motion policy as the default; add a feature-specific adaptation only when needed to preserve final-state visibility, focus indication, or keyboard feedback.

## Preserve Desktop Interaction

- Keep buttons, inputs, menus, tabs, and status controls compact and keyboard accessible.
- Prefer icon buttons only when the action is obvious, and provide an accessible label.
- Use menus for option sets, tabs for major views, toggles for binary settings, and lists or tables for log-like data.
- Keep hover states subtle and focus states visible.
- Use overlays or delegation to an already mounted terminal for transient menus and commands when practical.
- Do not clear, recreate, or disconnect a terminal session as a side effect of a visual change. Do not resize it solely as a visual workaround.
- When changing Settings layout, preserve normal-width `.settings-content` scrolling, compact-window `.settings-layout` scrolling, and `SettingsFooter` outside the scrolling region.
- Preserve overlay stacking, focus containment or restoration, keyboard behavior, and dismissal behavior.

## Keep Feedback Safe

- Use generic, privacy-safe text for status, diagnostics, screenshots, and empty states.
- Update both `src/locales/en.json` and `src/locales/ja.json` when user-visible text changes.
- Preserve existing confirmation behavior for destructive actions.

## Validate

- Run `pnpm run check:css` while iterating on CSS changes.
- Use `$exaterm-validate-change` to select and report validation commands.
- Inspect the final diff for legacy aliases, numeric z-index values, duplicated overlay ownership, reduced-motion exceptions, one-off values, accidental styling-system changes, terminal remounts, Settings scroll or footer regressions, and English/Japanese text drift.
- Distinguish automated checks from GUI, connected-session, hardware, and screenshot verification that was not performed.
