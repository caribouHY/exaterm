import test from "node:test";
import assert from "node:assert/strict";
import {
  checkDeclaration,
  findDeclarations,
  findRootTokenRanges,
  isAllowedShadow,
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
