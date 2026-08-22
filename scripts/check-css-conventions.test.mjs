import test from "node:test";
import assert from "node:assert/strict";
import {
  CSS_ARCHITECTURE,
  checkDeclaration,
  checkStylesheets,
  checkStylesheetStructure,
  findCustomPropertyDefinitions,
  findCustomPropertyUsages,
  findClassOwners,
  findDeclarations,
  findRootTokenRanges,
  findSelectors,
  isAllowedShadow,
  selectorDepth,
  splitSelectorList,
} from "./check-css-conventions.mjs";

function issueRules(content, file = "src/components/Test.css") {
  return findDeclarations(content).flatMap((declaration) =>
    checkDeclaration(file, declaration).map((issue) => issue.rule)
  );
}

test("flags raw colors outside token definitions", () => {
  assert.deepEqual(issueRules(".sample {\n  color: #fff;\n}"), ["raw-color"]);
});

test("allows raw colors inside :root token definitions", () => {
  const content =
    ":root {\n  --text-primary: #fff;\n}\n.sample {\n  color: var(--text-primary);\n}";
  const declarations = findDeclarations(content, findRootTokenRanges(content));

  assert.equal(declarations.length, 1);
  assert.equal(declarations.at(0)?.property, "color");
});

test("flags transition all usage", () => {
  assert.deepEqual(issueRules(".sample {\n  transition: all 120ms ease;\n}"), ["transition-all"]);
});

test("supports multiline declaration values", () => {
  const declarations = findDeclarations(".sample {\n  color:\n    rgba(255, 255, 255, 0.8);\n}");

  assert.equal(declarations.length, 1);
  assert.equal(declarations.at(0)?.value, "rgba(255, 255, 255, 0.8)");
  assert.deepEqual(
    checkDeclaration("src/components/Test.css", declarations.at(0)).map((issue) => issue.rule),
    ["raw-color"]
  );
});

test("checks longhand radius properties", () => {
  assert.deepEqual(issueRules(".sample {\n  border-bottom-right-radius: 2px;\n}"), [
    "border-radius",
  ]);
  assert.deepEqual(issueRules(".sample {\n  border-bottom-right-radius: var(--radius-sm);\n}"), []);
});

test("allows shadow tokens embedded in compound values and none", () => {
  assert.equal(
    isAllowedShadow("src/components/Test.css", {
      property: "box-shadow",
      value: "0 0 0 1px var(--shadow-sm)",
    }),
    true
  );
  assert.equal(
    isAllowedShadow("src/components/Test.css", {
      property: "box-shadow",
      value: "none",
    }),
    true
  );
});

test("finds custom property definitions and usages without reading comments", () => {
  const content = `
    /* --ignored: red; var(--also-ignored) */
    :root { --defined: #fff; }
    .sample { color: var(--defined); background: var(--missing, transparent); }
  `;

  assert.deepEqual(
    findCustomPropertyDefinitions(content).map(({ token }) => token),
    ["--defined"]
  );
  assert.deepEqual(
    findCustomPropertyUsages(content).map(({ token }) => token),
    ["--defined", "--missing"]
  );
});

test("extracts selectors inside conditional at-rules but not keyframe steps", () => {
  const selectors = findSelectors(`
    .sample { color: red; }
    @media (max-width: 760px) { .sample__child { color: blue; } }
    @keyframes pulse { from { opacity: 0; } to { opacity: 1; } }
  `).map(({ selector }) => selector);

  assert.deepEqual(selectors, [".sample", ".sample__child"]);
});

test("splits selector lists only at top-level commas", () => {
  assert.deepEqual(
    splitSelectorList('.dialog :is(.input, .select)[aria-label="Last, first"], .dialog__button'),
    ['.dialog :is(.input, .select)[aria-label="Last, first"]', ".dialog__button"]
  );
});

test("measures selector depth by compound selector levels", () => {
  assert.equal(selectorDepth('.sample .child > button[aria-pressed="true"]'), 3);
  assert.equal(selectorDepth(".sample:is(.active, .busy) .child"), 2);
});

test("rejects root token definitions outside approved token sources", () => {
  const issues = checkStylesheetStructure(
    "src/components/AI/AIChatPanel.css",
    ":root { --feature-color: #fff; }",
    new Set()
  );

  assert.deepEqual(
    issues.map(({ rule }) => rule),
    ["feature-root-token"]
  );
});

test("allows component-scoped custom properties", () => {
  const issues = checkStylesheetStructure(
    "src/components/Settings/SettingsPanel.css",
    ".settings-panel { --settings-gap: 12px; padding: var(--settings-gap); }",
    new Set()
  );

  assert.deepEqual(issues, []);
});

test("requires feature selectors to start from an owned class", () => {
  const issues = checkStylesheetStructure(
    "src/components/AI/AIChatPanel.css",
    ".connection-dialog .ai-panel { display: block; }",
    new Set()
  );

  assert.deepEqual(
    issues.map(({ rule }) => rule),
    ["class-ownership"]
  );
});

test("rejects unscoped element selectors in feature stylesheets", () => {
  const issues = checkStylesheetStructure(
    "src/components/AI/AIChatPanel.css",
    "button { display: block; }",
    new Set()
  );

  assert.deepEqual(
    issues.map(({ rule }) => rule),
    ["class-ownership"]
  );
});

test("rejects unowned descendant classes", () => {
  const issues = checkStylesheetStructure(
    "src/components/AI/AIChatPanel.css",
    ".ai-panel .connection-dialog { display: block; }",
    new Set()
  );

  assert.deepEqual(
    issues.map(({ rule }) => rule),
    ["class-ownership"]
  );
});

test("uses the most specific registered prefix for app feature ownership", () => {
  assert.deepEqual(
    [...findClassOwners("app-update-dialog")],
    ["src/features/app-update/AppUpdateDialog.css"]
  );

  const issues = checkStylesheetStructure(
    "src/App.css",
    ".app-update-dialog { display: block; }",
    new Set()
  );
  assert.deepEqual(
    issues.map(({ rule }) => rule),
    ["class-ownership"]
  );
});

test("uses the most specific registered prefix for AI component ownership", () => {
  assert.deepEqual(
    [...findClassOwners("ai-assistant-logo__star")],
    ["src/components/AI/AIAssistantLogo.css"]
  );

  const issues = checkStylesheetStructure(
    "src/components/AI/AIChatPanel.css",
    ".ai-panel .ai-assistant-logo { display: block; }",
    new Set()
  );
  assert.deepEqual(
    issues.map(({ rule }) => rule),
    ["class-ownership"]
  );
});

test("allows approved external classes only below the owning feature", () => {
  assert.deepEqual(
    checkStylesheetStructure(
      "src/components/Terminal/TerminalView.css",
      ".terminal-view .xterm-viewport { overflow: hidden; }",
      new Set()
    ),
    []
  );

  const issues = checkStylesheetStructure(
    "src/components/Terminal/TerminalView.css",
    ".xterm-viewport { overflow: hidden; }",
    new Set()
  );
  assert.deepEqual(
    issues.map(({ rule }) => rule),
    ["class-ownership"]
  );
});

test("allows a shared class only when scoped beneath an owned class", () => {
  const sharedClasses = new Set(["btn"]);
  assert.deepEqual(
    checkStylesheetStructure(
      "src/components/Connection/ConnectionDialog.css",
      ".connection-dialog__footer .btn { display: block; }",
      sharedClasses
    ),
    []
  );

  const issues = checkStylesheetStructure(
    "src/components/Connection/ConnectionDialog.css",
    ".btn.connection-dialog__submit { display: block; }",
    sharedClasses
  );
  assert.deepEqual(
    issues.map(({ rule }) => rule),
    ["shared-class-root"]
  );
});

test("rejects selectors beyond the configured depth", () => {
  const selector = [".ai-panel", ".ai-message", ".ai-markdown", "ul", "li"].join(" ");
  const issues = checkStylesheetStructure(
    "src/components/AI/AIChatPanel.css",
    `${selector} { display: block; }`,
    new Set()
  );

  assert.equal(CSS_ARCHITECTURE.maxSelectorDepth, 4);
  assert.deepEqual(
    issues.map(({ rule }) => rule),
    ["selector-depth"]
  );
});

test("requires new feature stylesheets to declare ownership", () => {
  const issues = checkStylesheetStructure(
    "src/components/NewFeature.css",
    ".new-feature { display: block; }",
    new Set()
  );

  assert.deepEqual(
    issues.map(({ rule }) => rule),
    ["missing-style-ownership"]
  );
});

test("reports custom property usages with no definition across stylesheets", () => {
  const issues = checkStylesheets([
    { file: "src/index.css", content: ":root { --defined: #fff; }" },
    {
      file: "src/components/AI/AIChatPanel.css",
      content: ".ai-panel { color: var(--defined); background: var(--missing); }",
    },
  ]);

  assert.deepEqual(
    issues.map(({ rule }) => rule),
    ["undefined-custom-property"]
  );
  assert.equal(issues.at(0)?.value, "--missing");
});

test("allows a feature-scoped custom property within its own stylesheet", () => {
  const issues = checkStylesheets([
    { file: "src/index.css", content: ":root { --global: #fff; }" },
    {
      file: "src/components/Settings/SettingsPanel.css",
      content:
        ".settings-panel { --settings-gap: 12px; color: var(--global); gap: var(--settings-gap); }",
    },
  ]);

  assert.deepEqual(issues, []);
});

test("rejects a feature-scoped custom property used by another stylesheet", () => {
  const issues = checkStylesheets([
    { file: "src/index.css", content: ":root { --global: #fff; }" },
    {
      file: "src/components/Settings/SettingsPanel.css",
      content: ".settings-panel { --settings-gap: 12px; gap: var(--settings-gap); }",
    },
    {
      file: "src/components/AI/AIChatPanel.css",
      content: ".ai-panel { gap: var(--settings-gap); }",
    },
  ]);

  assert.deepEqual(
    issues.map(({ rule }) => rule),
    ["undefined-custom-property"]
  );
  assert.equal(issues.at(0)?.value, "--settings-gap");
});
