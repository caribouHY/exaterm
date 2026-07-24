# ExaTerm UI Design Guidelines

This document is AI-facing implementation guidance. User-facing documentation belongs in `docs/`.

## Design Direction

ExaTerm is a Windows-focused desktop GUI tool, not a web landing page. Keep the current VS Code-inspired dark theme: compact, restrained, task-oriented, and optimized for repeated terminal work.

Prioritize:

- Dense but readable desktop layouts.
- Stable title bar, tab, workspace, side panel, dialog, and status bar regions.
- Low visual noise around terminal content.
- Clear affordances for commands, menus, inputs, toggles, status, and destructive actions.
- Small, consistent motion that does not distract from terminal output.

Avoid:

- Marketing-style hero sections, large decorative headings, and presentation layouts.
- Heavy cards, nested cards, strong gradients, large rounded surfaces, and ornamental backgrounds.
- One-off colors, shadows, spacing, or radii outside the shared CSS tokens.
- UI examples or debug text that expose terminal output, connection targets, usernames, prompts, logs, or secrets.

## CSS Structure

`src/index.css` is the primary design-system source. It owns shared tokens, reset/base styles, common controls, and simple utilities.

Component CSS files should:

- Use semantic tokens from `src/index.css` before adding new values.
- Add new tokens only when a value has reusable meaning across components.
- Keep component-specific layout rules in the component CSS file.
- Prefer BEM-style component class names already used in the codebase.
- Avoid `transition: all`; list the properties being animated.

Do not introduce Tailwind, shadcn/ui, CSS modules, or file restructuring without a separate design-system migration plan.

## Tokens

Use semantic tokens rather than raw colors for component styling:

- Backgrounds: `--bg-base`, `--bg-surface`, `--bg-elevated`, `--bg-input`, `--bg-hover`, `--bg-active`, `--bg-overlay`, `--bg-menu`, `--bg-code`.
- Text: `--text-primary`, `--text-secondary`, `--text-muted`, `--text-link`, `--text-on-accent`, `--text-shortcut`.
- Borders and focus: `--border-primary`, `--border-secondary`, `--border-subtle`, `--border-focus`, `--focus-ring`.
- Status and feedback: status tokens plus muted accent surfaces for danger, warning, success, and protocol badges.
- Radius: use `--radius-sm` for controls, `--radius-md` for popovers and compact grouped surfaces, and `--radius-lg` only for dialogs or intentionally elevated empty states.

Raw `#fff`, `#ccc`, repeated `rgba(...)`, and local shadows are acceptable only inside token definitions or unavoidable third-party overrides.

## Desktop Interaction Rules

- Keep primary controls compact. Buttons and inputs should fit a toolbar/dialog workflow rather than a web form.
- Prefer icon buttons for obvious tool actions, with accessible labels in React when available.
- Use menus for option sets, tabs for major views, toggles for binary settings, and tables/lists for log-like data.
- Keep hover states subtle and focus states visible.
- Do not remount terminal views, clear terminal buffers, or reset sessions as a side effect of visual updates.

## Privacy Boundaries

Terminal buffers, session logs, connection targets, usernames, prompts, command output, and API keys are sensitive. UI guidance, sample text, diagnostics, screenshots, and changelog entries must not reveal them.

When adding diagnostic or status UI, use generic labels and privacy-safe summaries unless the user explicitly requested otherwise and the existing privacy model supports it.
