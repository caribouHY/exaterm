import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

const SRC_DIR = path.join(process.cwd(), "src");
const INDEX_CSS = "src/index.css";

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
      message: "Use a design token from src/index.css instead of a raw color.",
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

export async function checkCssFiles(cssFiles) {
  const issues = [];

  for (const filePath of cssFiles) {
    const resolvedFilePath = path.resolve(filePath);
    const file = toRepoPath(resolvedFilePath);
    const content = await readFile(path.resolve(filePath), "utf8");
    const ignoredRanges = file === INDEX_CSS ? findRootTokenRanges(content) : [];
    const declarations = findDeclarations(content, ignoredRanges);

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
