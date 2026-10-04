# ExaTerm Styling Guidance

Read this only for CSS, tokens, shared UI, overlays or motion. `docs/development/CSS_ARCHITECTURE.md` owns the CSS structure and contracts; consult its matching sections rather than a duplicate rule list:

- Always read `Direction` for existing-system styling work.
- Choose the affected `Ownership` subsection by its exact heading: `Feature and component CSS` for local layout, `Global design system` for token/global layers, `Shared UI` for shared controls or overlays, and `xterm third-party overrides` for terminal overrides. Do not read the whole `Ownership` section for a local feature change. Inspect `src/styles/tokens.css` and relevant semantic/component token definitions for token work.
- Choose the affected `Migration contracts` subsection: `Terminal rendering` for terminal/layout changes, `Settings layout` for Settings scrolling/footer, `Overlays and motion` for focus, stacking or motion, and `Sensitive data` for examples, screenshots or data presentation. Settings layout changes also read `Terminal rendering` to preserve the surrounding terminal.
- Read `Convention checks` for stylesheet registration or checker changes, and `Staged migration` only for migration work.

Apply these additional implementation choices:

- Add tokens only for reusable meaning across components; keep component-specific layout in the component's stylesheet.
- Use `--radius-control` for controls, `--radius-surface` for popovers or compact groups, `--radius-dialog` for dialogs or intentionally elevated empty states, and `--radius-pill` for fully rounded shapes.
- List specific transition properties. Keep raw colors or repeated `rgba(...)` values in token definitions, except unavoidable third-party overrides.
- Reuse shared motion tokens and keyframes. Follow the global reduced-motion policy; add feature-specific adaptations only to preserve final-state visibility, focus or keyboard feedback.

Review the affected ownership and migration contracts in the final diff, including token dependencies/aliases, feature scoping, overlay tiers and reduced motion. Do not weaken CSS checks to admit one-off values or overrides.
