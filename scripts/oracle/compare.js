#!/usr/bin/env node
// Compare hvolt's emitted TypeScript with the loader's golden at AST level.
//
// Formatting and comments do not count as differences; comments are compared separately so a
// text-level bug cannot hide behind the AST comparison. Project identity (bundle name, page
// path) belongs to the fixture's project, not to the translation, so it is normalised.
//
// Usage: node compare.js <fixture-dir>

'use strict';

const fs = require('fs');
const os = require('os');
const path = require('path');

function loaderRoot() {
  const sdkRoot = path.join(os.homedir(), '.hvolt', 'sdk');
  const versions = fs.existsSync(sdkRoot)
    ? fs.readdirSync(sdkRoot).filter((v) => fs.statSync(path.join(sdkRoot, v)).isDirectory()).sort().reverse()
    : [];
  for (const version of versions) {
    const candidate = path.join(sdkRoot, version, 'openharmony', 'ets', 'build-tools', 'ets-loader');
    if (fs.existsSync(path.join(candidate, 'main.js'))) return candidate;
  }
  throw new Error('ets-loader not found');
}

const ts = require(path.join(loaderRoot(), 'node_modules', 'typescript'));

const IDENTITY_FIELDS = new Set(['bundleName', 'pageFullPath', 'pagePath', 'moduleName']);

function normaliseProjectIdentity(node, out) {
  if (ts.isPropertyAssignment(node) && node.name && (node.name.text || node.name.escapedText)) {
    const name = String(node.name.text || node.name.escapedText);
    if (IDENTITY_FIELDS.has(name) && ts.isStringLiteralLike(node.initializer)) {
      out.push(`${name}: <identity>`);
      return;
    }
  }
  ts.forEachChild(node, (child) => normaliseProjectIdentity(child, out));
}

// A canonical walk of the tree: node kinds plus the text of identifiers and literals.
// Printing would preserve each side's original line breaks, which is formatting, not meaning.
function canonical(node, out) {
  const kind = ts.SyntaxKind[node.kind];
  let extra = '';
  if (ts.isIdentifier(node) || ts.isPrivateIdentifier(node)) {
    extra = `:${node.text}`;
  } else if (ts.isStringLiteralLike(node)) {
    // Bundle names and page paths identify the fixture's project, not the translation.
    if (/^com\.example\./.test(node.text) || /\/src\/main\/ets\//.test(node.text)) {
      extra = ':<identity>';
    } else {
      extra = `:${JSON.stringify(node.text)}`;
    }
  } else if (ts.isNumericLiteral(node)) {
    extra = `:${node.text}`;
  }
  // Grouping parentheses are not structure: `(a && b) ? x : y` and `a && b ? x : y` walk to
  // the same operator tree. hvolt prints the tree with the minimum grouping its printer needs
  // while the reference's emitter writes the parentheses it was handed, so the wrapper is
  // skipped — the inner expression is still walked, so a grouping that really differs
  // (`(a + b) * c` vs `a + b * c`) still comes out as a different tree.
  if (ts.isParenthesizedExpression(node)) {
    canonical(node.expression, out);
    return;
  }
  // Source coordinates only feed DevTools; the translation must match in structure.
  // `{ page, line, col }` is the component's source coordinate; both sides may carry it or
  // omit it, and it does not change behaviour.
  if (ts.isObjectLiteralExpression(node) && node.properties.length > 0) {
    const first = node.properties[0];
    if (ts.isPropertyAssignment(first) && String(first.name.text || first.name.escapedText) === 'page') {
      out.push('LocMeta');
      return;
    }
  }
  out.push(kind + extra);
  // The sixth constructor argument of a component call is its source coordinate; the two
  // sides may emit it or an empty literal. Treat both as one placeholder.
  if (ts.isNewExpression(node) && node.arguments && node.arguments.length >= 6) {
    node.arguments.forEach((arg, index) => {
      if (index === 5) {
        out.push('LocMeta');
      } else {
        canonical(arg, out);
      }
    });
    return;
  }
  ts.forEachChild(node, (child) => canonical(child, out));
}

function fingerprint(file) {
  const text = fs.readFileSync(file, 'utf8');
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.ES2017, true, ts.ScriptKind.TS);
  // Printing the tree normalises whitespace and drops comments.
  const parts = [];
  canonical(source, parts);
  // Project identity strings differ by construction (each fixture has its own bundle name and
  // source layout); everything else in the tree has to match exactly.
  const structure = parts.join('\n');
  const identity = [];
  normaliseProjectIdentity(source, identity);
  const comments = [];
  const collect = (node) => {
    const ranges = ts.getLeadingCommentRanges(text, node.pos) || [];
    for (const range of ranges) {
      comments.push(text.slice(range.pos, range.end).trim());
    }
    ts.forEachChild(node, collect);
  };
  collect(source);
  return { structure, identity, comments };
}

// What two emitted files disagree about, as a list of messages.
function problemsBetween(goldenFile, actualFile) {
  const golden = fingerprint(goldenFile);
  const actual = fingerprint(actualFile);
  const problems = [];
  if (golden.structure !== actual.structure) {
    problems.push('AST differs');
  }
  if (golden.identity.join('|') !== actual.identity.join('|')) {
    problems.push(`project identity differs: ${golden.identity.join(',')} vs ${actual.identity.join(',')}`);
  }
  const goldenComments = golden.comments.filter((c) => c.startsWith('//') || c.startsWith('/*'));
  const actualComments = actual.comments.filter((c) => c.startsWith('//') || c.startsWith('/*'));
  if (goldenComments.length && JSON.stringify(goldenComments) !== JSON.stringify(actualComments)) {
    problems.push(`comment text differs (${goldenComments.length} vs ${actualComments.length})`);
  }
  return problems;
}

// The emitted paths a fixture declares beyond the page (`extra-files.json`), relative to the
// module root: `common/ObservedModel.ets` is compared as `common/ObservedModel.ts`.
function declaredExtras(dir) {
  const declaration = path.join(dir, 'extra-files.json');
  if (!fs.existsSync(declaration)) return [];
  const data = JSON.parse(fs.readFileSync(declaration, 'utf8'));
  return (data.files || []).map((source) => `${source.replace(/\.(ets|ts)$/, '')}.ts`);
}

// Every `.ts` under the golden tree other than the page, as a path relative to the fixture.
function goldenTypeScripts(dir) {
  const root = path.join(dir, 'golden');
  const found = [];
  const walk = (current) => {
    if (!fs.existsSync(current)) return;
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.name.endsWith('.ts')) found.push(path.relative(root, full).split(path.sep).join('/'));
    }
  };
  walk(root);
  return found;
}

function main() {
  const dir = process.argv[2];
  if (!dir) {
    console.error('usage: compare.js <fixture-dir>');
    process.exit(2);
  }
  const problems = problemsBetween(path.join(dir, 'golden.ts'), path.join(dir, 'actual.ts'));
  // Each declared file is its own pair, and the failure names it: a fixture that compares
  // several files has to say which one the reference and hvolt disagree about.
  const extras = declaredExtras(dir);
  for (const rel of extras) {
    const goldenFile = path.join(dir, 'golden', rel);
    const actualFile = path.join(dir, 'actual', rel);
    if (!fs.existsSync(goldenFile) || !fs.existsSync(actualFile)) {
      const missing = !fs.existsSync(goldenFile) ? 'golden' : 'actual';
      problems.push(`${rel}: missing from ${missing}`);
      continue;
    }
    for (const problem of problemsBetween(goldenFile, actualFile)) {
      problems.push(`${rel}: ${problem}`);
    }
  }
  // A `.ts` in the golden tree that nothing declares would be a checked-in expectation no
  // comparison looks at — the anchor would silently stop existing. Name it instead.
  for (const rel of goldenTypeScripts(dir)) {
    if (rel !== 'Index.ts' && !extras.includes(rel)) {
      problems.push(`${rel}: golden file is not declared in extra-files.json`);
    }
  }
  if (problems.length) {
    console.error(`FAIL ${dir}\n  ` + problems.join('\n  '));
    process.exit(1);
  }
  console.log(`PASS ${dir}`);
}

main();
