# CSS Architecture

This document defines the styling ownership and migration contracts for ExaTerm. The goal is to make CSS changes predictable without replacing the current visual system or changing application behavior during the migration.

## Direction

ExaTerm continues to use CSS custom properties, BEM-style class names, and component-local stylesheets. This matches the compact desktop UI and lets the codebase improve incrementally.

Tailwind CSS and shadcn/ui are not part of the current migration. A later proposal may map the semantic custom properties into a Tailwind theme and may adopt selected accessible primitives for new or substantially revised shared controls. CSS custom properties remain the design source of truth so that such an adoption does not require a second visual system.

## Ownership

### Global design system

`src/styles/index.css` is the only global entry point. It imports the global layers in this cascade order:

1. `tokens.css`: an import-only entry for the ordered token layers;
2. `foundation/reset.css`: universal reset rules;
3. `foundation/base.css`: root sizing and base document styles;
4. `foundation/scrollbar.css`: global scrollbar styling;
5. `utilities.css`: shared utility classes;
6. `components/controls.css`: shared buttons, inputs, selects, and labels;
7. `components/shared-ui.css`: shared messages, modals, empty states, and popover menus; and
8. `motion.css`: global keyframes and animation utilities.

`src/main.tsx` imports only this entry point. Keep the entry import-only and register global source files in their intended order in `CSS_ARCHITECTURE`; feature code must not import an individual global layer directly.

Global tokens must be defined in an approved token source. Feature stylesheets may define a custom property only when its meaning and lifetime are scoped to that component. Such a property is private to its stylesheet and must not be referenced by another feature. The existing `--settings-scrollbar-gap` is such a layout-private property; it is not a reusable design token.

`src/styles/tokens.css` imports the token layers in dependency order:

1. `tokens/primitives.css` contains context-free values such as palette colors, spacing, sizes, radii, durations, and raw effects;
2. `tokens/semantic.css` maps primitives to application intent such as surfaces, text, borders, status, and motion;
3. `tokens/components.css` contains the intentionally limited dimensions and effects that belong to a reusable UI boundary; and
4. `tokens/compatibility.css` maps the previous flat custom-property names to the canonical layers while feature styles migrate incrementally.

A token layer may reference only earlier layers. New feature CSS should prefer semantic tokens, using component tokens only for the component contract they name. Primitive tokens are implementation values rather than the application-facing styling contract. Existing flat names remain supported through the compatibility layer during migration; do not add new feature usage of those aliases.

### Shared UI

Classes such as `.btn`, `.input`, `.select`, `.ui-modal`, and `.ui-popover-menu` are shared contracts. Their root definitions belong to the global shared-UI layer. A feature may adjust a shared class only beneath a feature-owned selector, for example `.connection-dialog__file-row .btn`. A feature stylesheet must not redefine a shared class at the selector root.

### Feature and component CSS

Styles under `src/components/`, `src/features/`, and `src/App.css` own layout and visual states for their React boundary. Selectors start with a registered feature-owned class prefix and use BEM-style block, element, and modifier names. When registered prefixes overlap, the longest and most specific matching prefix owns the class. Cross-feature selectors and unscoped element rules are not allowed. New stylesheets must register their owned prefixes in `CSS_ARCHITECTURE` in `scripts/check-css-conventions.mjs`.

Settings uses `src/components/Settings/SettingsPanel.css` as an import-only feature entry. Its source files follow the existing React responsibilities: panel layout, category navigation, shortcuts, footer, AI, connection history, SSH, and toggles. Keep normal-width and compact-window rules for a responsibility in the same source file; the `760px` breakpoint describes a narrow desktop window, not a separate mobile application.

AI uses `src/components/AI/AIChatPanel.css` as an import-only feature entry for panel layout, messages and Markdown, command suggestions, and composer styles. Connection uses `src/components/Connection/ConnectionDialog.css` as an import-only feature entry for the main dialog, connection progress, SSH diagnostics, and credential prompt boundaries. Keep these source files aligned with their existing React responsibilities rather than grouping rules only by visual property.

Selectors should remain shallow. The automated check permits at most four compound levels; a component boundary or shared primitive should be introduced before a selector grows beyond that limit.

### xterm third-party overrides

xterm.js owns the `.xterm*` DOM. Overrides belong only in `src/components/Terminal/TerminalView.css` and remain scoped below `.terminal-view`. The xterm class prefix is the only current external-class compatibility entry, and its reason is recorded in the CSS convention configuration. Feature code must not depend on additional xterm DOM details merely to achieve layout changes.

## Migration contracts

CSS migration pull requests must preserve these behaviors:

- `TerminalView` and xterm remain mounted. Styling work must not clear buffers, disconnect sessions, recreate sessions, remount terminal views, or issue resize operations that are unnecessary for an actual size change.
- At normal window widths, `.settings-content` owns Settings scrolling.
- At compact window widths of `760px` or less, `.settings-layout` owns Settings scrolling.
- `SettingsFooter` remains outside the scrolling region and stays at the bottom of the Settings panel.
- Overlays retain their stacking relationship, focus containment or restoration, keyboard behavior, and dismissal behavior. z-index changes must be evaluated as a complete overlay stack rather than as isolated numbers.
- Motion changes preserve existing feedback and add a coherent `prefers-reduced-motion` path. Reduced motion must not remove state visibility or keyboard feedback.
- Terminal buffers, scrollback, session logs, connection targets, usernames, prompts, command output, and API keys remain sensitive. Styling, examples, diagnostics, screenshots, generated content, and CSS attributes must not expose or persist them.

Automated CSS checks do not prove GUI appearance, focus behavior, active-session preservation, or connected terminal behavior. Migration pull requests that move active styles require proportionate GUI and session checks in addition to automated validation.

## Convention checks

`pnpm run check:css` checks all source CSS for:

- raw colors outside approved token definitions;
- `transition: all`, unapproved radii, and local shadows;
- custom-property uses without a definition in the source CSS set;
- `:root` token definitions outside approved token sources;
- token-layer imports outside the declared order and dependencies on the same or a later token layer;
- missing or invalid feature class ownership;
- feature-root redefinitions of shared classes; and
- selectors deeper than four compound levels.

Token, global-layer, feature-entry, and shared-style source paths are centralized in `CSS_ARCHITECTURE`. The convention check verifies that `src/styles/index.css` and every registered feature entry import each source exactly once and in the declared cascade order. A new file under `src/styles/` must be registered as a global source or as a feature-owned stylesheet; do not weaken individual rules to admit it. Compatibility entries must be narrow, include a reason, and represent an actual external or migration boundary rather than a way to silence a finding.

## Staged migration

1. **Completed:** physically split the global stylesheet into tokens, foundation, shared controls, shared UI, utilities, and motion without changing names or values.
2. **Completed:** split Settings styles along existing React ownership while preserving its normal-width and compact-window scrolling contracts.
3. **Completed:** split AI styles into panel, messages and Markdown, commands, and composer responsibilities, and split Connection styles along its dialog, progress, diagnostics, and credential boundaries.
4. **Completed:** introduce primitive, semantic, and intentionally limited component token layers, retain the flat names as compatibility aliases, and make semantic tokens the application-facing contract.
5. Replace isolated z-index and motion decisions with shared layers, consolidate shared UI ownership, remove obsolete aliases, and update this document to the resulting structure.

Each stage should be a display-preserving refactor except for separately reviewed accessibility improvements such as reduced motion.
