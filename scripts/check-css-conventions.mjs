import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

const SRC_DIR = path.join(process.cwd(), "src");

const GLOBAL_STYLE_ENTRY_FILE = "src/styles/index.css";
const ORDERED_GLOBAL_STYLE_SOURCE_FILES = [
  "src/styles/tokens.css",
  "src/styles/foundation/reset.css",
  "src/styles/foundation/base.css",
  "src/styles/foundation/scrollbar.css",
  "src/styles/utilities.css",
  "src/styles/components/controls.css",
  "src/styles/components/shared-ui.css",
  "src/styles/motion.css",
];
const SETTINGS_STYLE_ENTRY_FILE = "src/components/Settings/SettingsPanel.css";
const ORDERED_SETTINGS_STYLE_SOURCE_FILES = [
  "src/components/Settings/SettingsLayout.css",
  "src/components/Settings/SettingsSidebar.css",
  "src/components/Settings/ShortcutsSettings.css",
  "src/components/Settings/SettingsFooter.css",
  "src/components/Settings/AiSettings.css",
  "src/components/Settings/ConnectionHistorySettings.css",
  "src/components/Settings/SshSettings.css",
  "src/components/Settings/SettingsToggle.css",
];

export const CSS_ARCHITECTURE = {
  globalStyleEntryFile: GLOBAL_STYLE_ENTRY_FILE,
  orderedGlobalStyleSourceFiles: ORDERED_GLOBAL_STYLE_SOURCE_FILES,
  featureStyleEntries: new Map([[SETTINGS_STYLE_ENTRY_FILE, ORDERED_SETTINGS_STYLE_SOURCE_FILES]]),
  globalStyleSourceFiles: new Set([GLOBAL_STYLE_ENTRY_FILE, ...ORDERED_GLOBAL_STYLE_SOURCE_FILES]),
  tokenSourceFiles: new Set(["src/styles/tokens.css"]),
  sharedStyleSourceFiles: new Set([
    "src/styles/utilities.css",
    "src/styles/components/controls.css",
    "src/styles/components/shared-ui.css",
    "src/styles/motion.css",
  ]),
  maxSelectorDepth: 4,
  featureStylesheets: new Map([
    [
      "src/App.css",
      {
        ownedPrefixes: ["app", "app-credential", "ssh-auth-prompt", "ssh-host-key-prompt"],
      },
    ],
    ["src/components/AI/AIAssistantLogo.css", { ownedPrefixes: ["ai-assistant-logo"] }],
    ["src/components/AI/AIChatPanel.css", { ownedPrefixes: ["ai"] }],
    ["src/components/Connection/ConnectionDialog.css", { ownedPrefixes: ["connection"] }],
    ["src/components/Log/LogViewer.css", { ownedPrefixes: ["log"] }],
    ["src/components/Settings/SettingsPanel.css", { ownedPrefixes: [] }],
    [
      "src/components/Settings/SettingsLayout.css",
      { ownedPrefixes: ["settings", "settings-section", "settings-help"] },
    ],
    ["src/components/Settings/SettingsSidebar.css", { ownedPrefixes: ["settings-category"] }],
    [
      "src/components/Settings/ShortcutsSettings.css",
      { ownedPrefixes: ["settings-shortcuts", "settings-section", "settings-help"] },
    ],
    [
      "src/components/Settings/SettingsFooter.css",
      {
        ownedPrefixes: ["settings-actions", "settings-saved", "settings-unsaved", "settings-error"],
      },
    ],
    [
      "src/components/Settings/AiSettings.css",
      { ownedPrefixes: ["settings-secret", "settings-provider", "settings-toggle"] },
    ],
    [
      "src/components/Settings/ConnectionHistorySettings.css",
      { ownedPrefixes: ["settings-connection-history", "settings-help"] },
    ],
    [
      "src/components/Settings/SshSettings.css",
      { ownedPrefixes: ["settings-ssh", "settings-help"] },
    ],
    [
      "src/components/Settings/SettingsToggle.css",
      { ownedPrefixes: ["settings-toggle", "toggle"] },
    ],
    ["src/components/StatusBar/StatusBar.css", { ownedPrefixes: ["statusbar"] }],
    ["src/components/StatusBar/StatusBarPalette.css", { ownedPrefixes: ["statusbar-palette"] }],
    [
      "src/components/Terminal/TerminalTabs.css",
      { ownedPrefixes: ["terminal-tab", "terminal-tabs"] },
    ],
    [
      "src/components/Terminal/TerminalView.css",
      {
        ownedPrefixes: ["terminal-view"],
        externalPrefixes: [
          {
            prefix: "xterm",
            reason: "xterm.js owns these elements; TerminalView scopes all overrides.",
          },
        ],
      },
    ],
    ["src/components/TitleBar/TitleBar.css", { ownedPrefixes: ["titlebar"] }],
    ["src/features/app-exit/AppExitDialog.css", { ownedPrefixes: ["app-exit"] }],
    ["src/features/app-update/AppUpdateDialog.css", { ownedPrefixes: ["app-update"] }],
  ]),
};

const RAW_COLOR_PATTERN = /(?:#[0-9a-fA-F]{3,8}\b|\b(?:rgb|rgba|hsl|hsla)\s*\()/;
const ALLOWED_RADIUS_TOKENS = new Set([
  "--radius-sm",
  "--radius-md",
  "--radius-lg",
  "--radius-full",
]);
const ZERO_RADIUS = /^(?:0|0px|0rem|0em)$/;

const SHADOW_ALLOWLIST = [
  {
    file: "src/components/AI/AIChatPanel.css",
    property: "box-shadow",
    value: "0 0 0 2px var(--bg-surface)",
    reason: "keeps the active context indicator readable on the panel surface",
  },
  {
    file: "src/components/Terminal/TerminalView.css",
    property: "box-shadow",
    value: "inset 0 -1px 0 var(--accent-blue-muted)",
    reason: "marks detected Cisco hostnames without adding a new global token",
  },
  {
    file: "src/components/Terminal/TerminalView.css",
    property: "box-shadow",
    value: "inset 0 -1px 0 var(--accent-yellow-muted)",
    reason: "marks Cisco config-mode hostnames without adding a new global token",
  },
];

export async function collectCssFiles(directory) {
  const resolvedDirectory = path.resolve(directory);
  // eslint-disable-next-line security/detect-non-literal-fs-filename -- Scans resolved repository paths for the local CSS convention check.
  const entries = await readdir(path.resolve(directory), { withFileTypes: true });
  const files = await Promise.all(
    entries.map(async (entry) => {
      const fullPath = path.join(resolvedDirectory, entry.name);
      if (entry.isDirectory()) {
        return collectCssFiles(fullPath);
      }
      return entry.isFile() && entry.name.endsWith(".css") ? [fullPath] : [];
    })
  );

  return files.flat();
}

export function toRepoPath(filePath) {
  return path.relative(process.cwd(), filePath).replaceAll(path.sep, "/");
}

export function stripCommentsPreservingLines(content) {
  return content.replace(/\/\*[\s\S]*?\*\//g, (match) => match.replace(/[^\n]/g, " "));
}

export function findStylesheetImports(file, content) {
  const imports = [];
  const cleanContent = stripCommentsPreservingLines(content);
  const importPattern = /^\s*@import\s+(?:url\()?['"]([^'"]+)['"]\)?\s*;/gm;
  let match;

  while ((match = importPattern.exec(cleanContent)) !== null) {
    const importedFile = path.posix.normalize(path.posix.join(path.posix.dirname(file), match[1]));
    imports.push({ file: importedFile, line: lineForIndex(cleanContent, match.index) });
  }

  return imports;
}

export function checkGlobalStyleEntry(file, content) {
  return checkImportOnlyStyleEntry(
    file,
    content,
    CSS_ARCHITECTURE.orderedGlobalStyleSourceFiles,
    "global"
  );
}

export function checkImportOnlyStyleEntry(file, content, expectedFiles, rulePrefix) {
  const issues = [];
  const imports = findStylesheetImports(file, content);
  const actualFiles = imports.map((entry) => entry.file);

  if (
    actualFiles.length !== expectedFiles.length ||
    actualFiles.some((importedFile, index) => importedFile !== expectedFiles.at(index))
  ) {
    issues.push({
      file,
      line: 1,
      rule: `${rulePrefix}-style-import-order`,
      property: "@import",
      value: actualFiles.join(", "),
      message: `Import each registered ${rulePrefix} source once in this order: ${expectedFiles.join(", ")}.`,
    });
  }

  const importPattern = /^\s*@import\s+(?:url\()?['"][^'"]+['"]\)?\s*;/gm;
  const remainingContent = stripCommentsPreservingLines(content).replace(importPattern, "").trim();
  if (remainingContent.length > 0) {
    issues.push({
      file,
      line: 1,
      rule: `${rulePrefix}-style-entry-content`,
      property: "stylesheet",
      value: file,
      message: `Keep the ${rulePrefix} style entry import-only so cascade order remains explicit.`,
    });
  }

  return issues;
}

export function checkFeatureStyleEntries(stylesheets, { requireEntries = false } = {}) {
  const issues = [];
  const stylesheetFiles = new Set(stylesheets.map(({ file }) => file));

  for (const [entryFile, sourceFiles] of CSS_ARCHITECTURE.featureStyleEntries) {
    const entry = stylesheets.find(({ file }) => file === entryFile);

    if (!entry) {
      if (requireEntries) {
        issues.push({
          file: entryFile,
          line: 1,
          rule: "feature-style-entry-missing",
          property: "stylesheet",
          value: entryFile,
          message: "Keep every registered feature stylesheet entry present in the source tree.",
        });
      }
      continue;
    }

    issues.push(...checkImportOnlyStyleEntry(entry.file, entry.content, sourceFiles, "feature"));

    if (requireEntries) {
      for (const sourceFile of sourceFiles) {
        if (!stylesheetFiles.has(sourceFile)) {
          issues.push({
            file: sourceFile,
            line: 1,
            rule: "feature-style-source-missing",
            property: "stylesheet",
            value: sourceFile,
            message: "Keep every registered feature style source present in the source tree.",
          });
        }
      }
    }
  }

  return issues;
}

export function checkGlobalStyleSources(stylesheets) {
  const issues = [];
  const stylesheetFiles = new Set(stylesheets.map(({ file }) => file));
  const globalEntry = stylesheets.find(
    ({ file }) => file === CSS_ARCHITECTURE.globalStyleEntryFile
  );

  if (!globalEntry) {
    issues.push({
      file: CSS_ARCHITECTURE.globalStyleEntryFile,
      line: 1,
      rule: "global-style-entry-missing",
      property: "stylesheet",
      value: CSS_ARCHITECTURE.globalStyleEntryFile,
      message: "Keep one registered global stylesheet entry imported by src/main.tsx.",
    });
  } else {
    issues.push(...checkGlobalStyleEntry(globalEntry.file, globalEntry.content));
  }

  for (const sourceFile of CSS_ARCHITECTURE.orderedGlobalStyleSourceFiles) {
    if (!stylesheetFiles.has(sourceFile)) {
      issues.push({
        file: sourceFile,
        line: 1,
        rule: "global-style-source-missing",
        property: "stylesheet",
        value: sourceFile,
        message: "Keep every registered global style source present in the source tree.",
      });
    }
  }

  return issues;
}

export function findRootTokenRanges(content) {
  const ranges = [];
  const rootPattern = /:root\s*\{/g;
  let match;

  while ((match = rootPattern.exec(content)) !== null) {
    const start = match.index;
    let depth = 0;

    for (let index = match.index; index < content.length; index += 1) {
      const char = content.charAt(index);
      if (char === "{") {
        depth += 1;
      } else if (char === "}") {
        depth -= 1;
        if (depth === 0) {
          ranges.push([start, index + 1]);
          rootPattern.lastIndex = index + 1;
          break;
        }
      }
    }
  }

  return ranges;
}

export function findCustomPropertyDefinitions(content) {
  const cleanContent = stripCommentsPreservingLines(content);
  const definitions = [];
  const pattern = /(--[\w-]+)\s*:/g;
  let match;

  while ((match = pattern.exec(cleanContent)) !== null) {
    definitions.push({
      token: match[1],
      line: lineForIndex(cleanContent, match.index),
      index: match.index,
    });
  }

  return definitions;
}

export function findCustomPropertyUsages(content) {
  const cleanContent = stripCommentsPreservingLines(content);
  const usages = [];
  const pattern = /var\(\s*(--[\w-]+)/g;
  let match;

  while ((match = pattern.exec(cleanContent)) !== null) {
    usages.push({
      token: match[1],
      line: lineForIndex(cleanContent, match.index),
    });
  }

  return usages;
}

function isRuleContainer(prelude) {
  return /^@(media|supports|container|layer)\b/i.test(prelude);
}

export function splitSelectorList(prelude) {
  const selectors = [];
  let start = 0;
  let parenthesisDepth = 0;
  let bracketDepth = 0;
  let quote = null;

  for (let index = 0; index < prelude.length; index += 1) {
    const char = prelude.charAt(index);
    if (quote) {
      if (char === quote && prelude.charAt(index - 1) !== "\\") {
        quote = null;
      }
    } else if (char === '"' || char === "'") {
      quote = char;
    } else if (char === "(") {
      parenthesisDepth += 1;
    } else if (char === ")") {
      parenthesisDepth -= 1;
    } else if (char === "[") {
      bracketDepth += 1;
    } else if (char === "]") {
      bracketDepth -= 1;
    } else if (char === "," && parenthesisDepth === 0 && bracketDepth === 0) {
      selectors.push(prelude.slice(start, index).trim());
      start = index + 1;
    }
  }

  selectors.push(prelude.slice(start).trim());
  return selectors.filter(Boolean);
}

export function findSelectors(content) {
  const cleanContent = stripCommentsPreservingLines(content);
  const selectors = [];
  const stack = [{ acceptsRules: true }];
  let statementStart = 0;

  for (let index = 0; index < cleanContent.length; index += 1) {
    const char = cleanContent.charAt(index);
    if (char === "{") {
      const prelude = cleanContent.slice(statementStart, index).trim();
      const parent = stack.at(-1);
      const atRule = prelude.startsWith("@");

      if (parent?.acceptsRules && prelude && !atRule) {
        const preludeOffset = cleanContent.indexOf(prelude, statementStart);
        for (const selector of splitSelectorList(prelude)) {
          const trimmedSelector = selector.trim();
          if (trimmedSelector) {
            selectors.push({
              selector: trimmedSelector,
              line: lineForIndex(cleanContent, preludeOffset),
            });
          }
        }
      }

      stack.push({ acceptsRules: atRule && isRuleContainer(prelude) });
      statementStart = index + 1;
    } else if (char === "}") {
      stack.pop();
      statementStart = index + 1;
    }
  }

  return selectors;
}

export function findSelectorClasses(selector) {
  return [...selector.matchAll(/\.([_a-zA-Z][\w-]*)/g)].map((match) => match[1]);
}

export function selectorDepth(selector) {
  const withoutAttributes = selector.replace(/\[[^\]]*\]/g, "");
  const withoutFunctionalPseudos = withoutAttributes.replace(/:(?:is|where|not|has)\([^)]*\)/g, "");
  return withoutFunctionalPseudos
    .trim()
    .split(/\s+(?![^()]*\))|\s*[>+~]\s*/)
    .filter(Boolean).length;
}

function matchesPrefix(className, prefix) {
  return (
    className === prefix ||
    className.startsWith(`${prefix}-`) ||
    className.startsWith(`${prefix}__`) ||
    className.startsWith(`${prefix}--`)
  );
}

export function findClassOwners(className) {
  let longestPrefixLength = -1;
  const owners = new Set();

  for (const [file, policy] of CSS_ARCHITECTURE.featureStylesheets) {
    for (const prefix of policy.ownedPrefixes) {
      if (!matchesPrefix(className, prefix)) {
        continue;
      }

      if (prefix.length > longestPrefixLength) {
        longestPrefixLength = prefix.length;
        owners.clear();
      }
      if (prefix.length === longestPrefixLength) {
        owners.add(file);
      }
    }
  }

  return owners;
}

export function checkStylesheetStructure(file, content, sharedClasses = new Set()) {
  const issues = [];
  const rootRanges = findRootTokenRanges(content);
  const rootDefinitions = findCustomPropertyDefinitions(content).filter((definition) =>
    isInsideRange(definition.index, rootRanges)
  );
  const tokenSource = CSS_ARCHITECTURE.tokenSourceFiles.has(file);

  if (!tokenSource) {
    for (const definition of rootDefinitions) {
      issues.push({
        file,
        line: definition.line,
        rule: "feature-root-token",
        property: definition.token,
        value: ":root",
        message: "Define global design tokens in an approved token source, not feature CSS.",
      });
    }
  }

  const policy = CSS_ARCHITECTURE.featureStylesheets.get(file);
  if (!policy && !CSS_ARCHITECTURE.globalStyleSourceFiles.has(file)) {
    issues.push({
      file,
      line: 1,
      rule: "missing-style-ownership",
      property: "stylesheet",
      value: file,
      message: "Register the stylesheet and its owned class prefixes in CSS_ARCHITECTURE.",
    });
    return issues;
  }

  if (!policy) {
    return issues;
  }

  for (const { selector, line } of findSelectors(content)) {
    const classes = findSelectorClasses(selector);
    const firstClass = classes.at(0);

    if (selector === ":root" && rootDefinitions.length > 0) {
      continue;
    }

    if (selectorDepth(selector) > CSS_ARCHITECTURE.maxSelectorDepth) {
      issues.push({
        file,
        line,
        rule: "selector-depth",
        property: "selector",
        value: selector,
        message: `Keep selectors at ${CSS_ARCHITECTURE.maxSelectorDepth} compound levels or fewer.`,
      });
    }

    if (!firstClass) {
      issues.push({
        file,
        line,
        rule: "class-ownership",
        property: "selector",
        value: selector,
        message: `Start the selector with a class owned by ${file}.`,
      });
      continue;
    }

    if (sharedClasses.has(firstClass)) {
      issues.push({
        file,
        line,
        rule: "shared-class-root",
        property: "selector",
        value: selector,
        message: "Scope shared-class adjustments beneath a feature-owned class.",
      });
      continue;
    }

    const externalPrefixes = policy.externalPrefixes ?? [];
    const firstClassOwners = findClassOwners(firstClass);
    if (!firstClassOwners.has(file)) {
      issues.push({
        file,
        line,
        rule: "class-ownership",
        property: "selector",
        value: selector,
        message: `Start the selector with a class owned by ${file}.`,
      });
      continue;
    }

    const unownedClass = classes.find(
      (className) =>
        !sharedClasses.has(className) &&
        !externalPrefixes.some(({ prefix }) => matchesPrefix(className, prefix)) &&
        !findClassOwners(className).has(file)
    );
    if (unownedClass) {
      issues.push({
        file,
        line,
        rule: "class-ownership",
        property: "selector",
        value: selector,
        message: `Class .${unownedClass} is not owned, shared, or an approved external class.`,
      });
    }
  }

  return issues;
}

export function isInsideRange(index, ranges) {
  return ranges.some(([start, end]) => index >= start && index < end);
}

export function lineForIndex(content, index) {
  let line = 1;
  for (let cursor = 0; cursor < index; cursor += 1) {
    if (content.charAt(cursor) === "\n") {
      line += 1;
    }
  }
  return line;
}

export function findDeclarations(content, ignoredRanges = []) {
  const cleanContent = stripCommentsPreservingLines(content);
  const lines = cleanContent.split("\n");
  const lineOffsets = [];
  const declarations = [];
  let offset = 0;

  for (const line of lines) {
    lineOffsets.push(offset);
    offset += line.length + 1;
  }

  for (let lineIndex = 0; lineIndex < lines.length; lineIndex += 1) {
    const line = lines.at(lineIndex) ?? "";
    const match = line.match(/^\s*([a-zA-Z-]+)\s*:\s*(.*)$/);
    const propertyStart = (lineOffsets.at(lineIndex) ?? 0) + line.search(/\S/);

    if (!match || isInsideRange(propertyStart, ignoredRanges) || match[1].startsWith("--")) {
      continue;
    }

    let value = match[2];
    while (!value.includes(";") && lineIndex < lines.length - 1) {
      lineIndex += 1;
      value += ` ${(lines.at(lineIndex) ?? "").trim()}`;
    }

    declarations.push({
      line: lineForIndex(cleanContent, propertyStart),
      property: match[1].toLowerCase(),
      value: value.replace(/;.*/, "").replace(/\s+/g, " ").trim(),
    });
  }

  return declarations;
}

export function stripVarFunctions(value) {
  let result = "";
  for (let index = 0; index < value.length; index += 1) {
    if (value.slice(index, index + 4).toLowerCase() !== "var(") {
      result += value.charAt(index);
      continue;
    }

    let depth = 0;
    for (; index < value.length; index += 1) {
      const char = value.charAt(index);
      if (char === "(") {
        depth += 1;
      } else if (char === ")") {
        depth -= 1;
        if (depth === 0) {
          break;
        }
      }
    }
  }
  return result;
}

export function findVarToken(value) {
  return value.match(/var\(\s*(--[\w-]+)/)?.[1];
}

export function isAllowedShadow(file, declaration) {
  if (declaration.value === "none" || /var\(\s*--shadow-[\w-]+\s*\)/.test(declaration.value)) {
    return true;
  }

  return SHADOW_ALLOWLIST.some(
    (entry) =>
      entry.file === file &&
      entry.property === declaration.property &&
      entry.value === declaration.value
  );
}

export function checkDeclaration(file, declaration) {
  const issues = [];
  const valueWithoutVars = stripVarFunctions(declaration.value);

  if (RAW_COLOR_PATTERN.test(valueWithoutVars)) {
    issues.push({
      ...declaration,
      rule: "raw-color",
      message: "Use a design token from src/styles/tokens.css instead of a raw color.",
    });
  }

  if (declaration.property === "transition" && /\ball\b/i.test(declaration.value)) {
    issues.push({
      ...declaration,
      rule: "transition-all",
      message: "List the animated properties instead of using transition: all.",
    });
  }

  if (declaration.property.endsWith("radius")) {
    const token = findVarToken(declaration.value);
    if (!ZERO_RADIUS.test(declaration.value) && (!token || !ALLOWED_RADIUS_TOKENS.has(token))) {
      issues.push({
        ...declaration,
        rule: "border-radius",
        message: "Use a shared radius token; raw or unknown radius values need review.",
      });
    }
  }

  if (
    ["box-shadow", "text-shadow", "filter"].includes(declaration.property) &&
    /\b(?:drop-shadow|shadow)\b/i.test(`${declaration.property}: ${declaration.value}`) &&
    !isAllowedShadow(file, declaration)
  ) {
    issues.push({
      ...declaration,
      rule: "local-shadow",
      message: "Use a shared --shadow-* token, or add a narrow allowlist entry with a reason.",
    });
  }

  return issues;
}

export function formatIssue(issue) {
  return `${issue.file}:${issue.line} [${issue.rule}] ${issue.property}: ${issue.value}\n  ${issue.message}`;
}

export function checkStylesheets(stylesheets, { requireGlobalStyleSources = false } = {}) {
  const issues = [];

  if (requireGlobalStyleSources) {
    issues.push(...checkGlobalStyleSources(stylesheets));
  } else {
    const globalEntry = stylesheets.find(
      ({ file }) => file === CSS_ARCHITECTURE.globalStyleEntryFile
    );
    if (globalEntry) {
      issues.push(...checkGlobalStyleEntry(globalEntry.file, globalEntry.content));
    }
  }

  issues.push(
    ...checkFeatureStyleEntries(stylesheets, {
      requireEntries: requireGlobalStyleSources,
    })
  );

  const globalTokens = new Set(
    stylesheets
      .filter(({ file }) => CSS_ARCHITECTURE.tokenSourceFiles.has(file))
      .flatMap(({ content }) => findCustomPropertyDefinitions(content).map(({ token }) => token))
  );
  const sharedClasses = new Set(
    stylesheets
      .filter(({ file }) => CSS_ARCHITECTURE.sharedStyleSourceFiles.has(file))
      .flatMap(({ content }) =>
        findSelectors(content).flatMap(({ selector }) => findSelectorClasses(selector))
      )
  );

  for (const { file, content } of stylesheets) {
    const ignoredRanges = CSS_ARCHITECTURE.tokenSourceFiles.has(file)
      ? findRootTokenRanges(content)
      : [];
    const declarations = findDeclarations(content, ignoredRanges);
    const availableTokens = new Set([
      ...globalTokens,
      ...findCustomPropertyDefinitions(content).map(({ token }) => token),
    ]);

    issues.push(...checkStylesheetStructure(file, content, sharedClasses));

    for (const usage of findCustomPropertyUsages(content)) {
      if (!availableTokens.has(usage.token)) {
        issues.push({
          file,
          line: usage.line,
          rule: "undefined-custom-property",
          property: "custom-property",
          value: usage.token,
          message: "Define the custom property before using it with var().",
        });
      }
    }

    for (const declaration of declarations) {
      issues.push(
        ...checkDeclaration(file, declaration).map((issue) => ({
          ...issue,
          file,
        }))
      );
    }
  }

  return issues;
}

export async function checkCssFiles(cssFiles) {
  const stylesheets = [];

  for (const filePath of cssFiles) {
    const resolvedFilePath = path.resolve(filePath);
    const file = toRepoPath(resolvedFilePath);
    // eslint-disable-next-line security/detect-non-literal-fs-filename -- Reads resolved repository CSS files discovered by collectCssFiles.
    const content = await readFile(path.resolve(filePath), "utf8");
    stylesheets.push({ file, content });
  }

  return checkStylesheets(stylesheets, { requireGlobalStyleSources: true });
}

export async function run() {
  const cssFiles = await collectCssFiles(SRC_DIR);
  const issues = await checkCssFiles(cssFiles);

  if (issues.length > 0) {
    console.error("CSS convention check failed:\n");
    console.error(issues.map(formatIssue).join("\n\n"));
    process.exitCode = 1;
  } else {
    console.log(`CSS convention check passed (${cssFiles.length} files).`);
  }
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  await run();
}
