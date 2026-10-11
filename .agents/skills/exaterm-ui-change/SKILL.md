---
name: exaterm-ui-change
description: Change the existing ExaTerm React UI, CSS, or design tokens while preserving terminal state. Excludes standalone creative assets.
---

# ExaTerm UI Change

## Prepare

- Inspect the target component and its styles. Keep the compact, restrained VS Code-inspired desktop direction.
- For CSS, tokens, shared UI, overlays or motion, read [styling guidance](references/styling.md) and the relevant CSS architecture sections it selects. React-only behavior changes do not require styling references.
- Identify changes that can remount `TerminalView`, alter terminal dimensions, or reset tabs, focus, scrollback, connections or logs.
- Follow the session, sensitive-data and locale synchronization rules in `AGENTS.md`. Screenshots are layout evidence; do not reproduce private terminal content in examples or diagnostics.
- Do not introduce Tailwind, shadcn/ui, CSS modules or another styling system without a separately approved migration.

## Preserve Desktop Interaction

- Keep controls compact and keyboard accessible, with visible focus and subtle hover feedback. Icon buttons need accessible labels.
- Prefer menus for option sets, tabs for major views, toggles for binary settings and lists or tables for log-like data.
- Use overlays or an already mounted terminal for transient commands when practical. Styling must not remount terminals or issue resize operations merely as a visual workaround.
- For Settings changes, preserve normal-width `.settings-content` scrolling, compact-window `.settings-layout` scrolling and `SettingsFooter` outside the scrolling region.
- Preserve overlay stacking, focus containment or restoration, keyboard handling, dismissal and existing destructive-action confirmations.
- Keep status, diagnostic and empty-state text generic and privacy-safe.

## Validate

- For CSS changes, run `pnpm run check:css` while iterating.
- Use `.agents/skills/exaterm-validate-change/SKILL.md` for final validation and changelog decisions.
- Review the relevant CSS architecture contracts and any styling reference used, terminal mount/state preservation, Settings scrolling/footer behavior and locale synchronization.
- Distinguish automated checks from unperformed GUI, connected-session, hardware and screenshot verification.
