#!/usr/bin/env node
// Count the AST tokens two TypeScript trees disagree about.
//
// The fixture oracle judges a file by whether its tree *is* the reference's; this tool says how
// far apart two trees are, in tokens, so a change that keeps a file different can still be shown
// to shrink the difference — and a change that enlarges one shows up as a number instead of a
// diff nobody reads.
//
// ## What a token is
//
// One canonical node of the TypeScript AST, as `typescript`'s own parser reads the emitted file.
// The reading is the same one `scripts/oracle/compare.js` and `scripts/debug/compare-trees.js`
// use, so the three tools cannot disagree about what a tree "says":
//
//   * a node contributes `SyntaxKind` plus its payload — an identifier's name, a numeric or
//     string literal's text;
//   * a string that names the project rather than the translation (`com.example.*`,
//     anything carrying `/src/main/ets/`) is `:<identity>`, because every fixture and every
//     build has its own bundle name and source root;
//   * `({ page, line, col })` — the source coordinate the compiler writes into a component's
//     `observeComponentCreation2` — is one `LocMeta` node, and so is the sixth constructor
//     argument of a component call; the two sides may carry it or omit it;
//   * grouping parentheses are not structure: `(a && b) ? x : y` and `a && b ? x : y` walk to
//     the same operator tree, so the wrapper is skipped and its expression walked;
//   * everything else the parser visits is counted, in `ts.forEachChild` order: keywords,
//     declarations, modifiers, operators and punctuation-bearing expressions alike. Nothing is
//     skipped except the comments (the parser attaches them to the source, not the tree) and
//     the empty file's EOF token.
//
// A token present on one side only counts once per side; the total is their sum, which is what
// makes the number comparable across runs.
//
// ## How two token streams are aligned
//
// Longest-common-subsequence (the classic table walk), which is order-preserving: a token that
// merely moved is reported as removed-and-added rather than matched across the move. The
// resulting count is therefore an upper bound on editing distance — stable, deterministic, and
// never zero for two different trees.
//
// ## Modes
//
//   token-diff.js <ref-root> <hvolt-root>     every `.ts` of the reference tree, per file
//   token-diff.js <ref.ts>  <ours.ts>         one pair
//
// The reference tree's relative paths are mapped onto the hvolt tree the way
// `compare-trees.js` does: `<module>/src/main/ets/<rest>` is `<module>/<rest>`, because hvolt
// stages a module's sources without its source root. A file the hvolt tree does not have is
// reported as missing rather than as silently agreeing.
//
// The output is one line per differing file and a `total` line, sorted by path so two runs on
// the same input print the same bytes; `--json` prints the same numbers as one object.
//
// Usage:
//   node scripts/debug/token-diff.js <ref-root> <hvolt-root> [--json]
//   node scripts/debug/token-diff.js <ref.ts> <ours.ts>

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

/// One canonical token per AST node, in `forEachChild` order.
function canonical(node, out) {
  const kind = ts.SyntaxKind[node.kind];
  let extra = '';
  if (ts.isIdentifier(node) || ts.isPrivateIdentifier(node)) {
    extra = `:${node.text}`;
  } else if (ts.isStringLiteralLike(node)) {
    extra = /^com\.example\./.test(node.text) || /\/src\/main\/ets\//.test(node.text)
      ? ':<identity>'
      : `:${JSON.stringify(node.text)}`;
  } else if (ts.isNumericLiteral(node)) {
    extra = `:${node.text}`;
  }
  // Grouping parentheses are not structure.
  if (ts.isParenthesizedExpression(node)) {
    canonical(node.expression, out);
    return;
  }
  // The source coordinate is one placeholder on both sides.
  if (ts.isObjectLiteralExpression(node) && node.properties.length > 0) {
    const first = node.properties[0];
    if (ts.isPropertyAssignment(first) && String(first.name.text || first.name.escapedText) === 'page') {
      out.push('LocMeta');
      return;
    }
  }
  out.push(kind + extra);
  // The sixth constructor argument of a component call is the same coordinate.
  if (ts.isNewExpression(node) && node.arguments && node.arguments.length >= 6) {
    node.arguments.forEach((argument, index) => {
      if (index === 5) out.push('LocMeta');
      else canonical(argument, out);
    });
    return;
  }
  ts.forEachChild(node, (child) => canonical(child, out));
}

function tokens(file) {
  const text = fs.readFileSync(file, 'utf8');
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.ES2017, true, ts.ScriptKind.TS);
  const parts = [];
  canonical(source, parts);
  return parts;
}

/// The tokens two streams disagree about, both sides summed.
function differing(a, b) {
  const n = a.length;
  const m = b.length;
  const table = Array.from({ length: n + 1 }, () => new Int32Array(m + 1));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      table[i][j] = a[i] === b[j] ? table[i + 1][j + 1] + 1 : Math.max(table[i + 1][j], table[i][j + 1]);
    }
  }
  let i = 0;
  let j = 0;
  let referenceOnly = 0;
  let hvoltOnly = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      i++;
      j++;
    } else if (table[i + 1][j] >= table[i][j + 1]) {
      referenceOnly++;
      i++;
    } else {
      hvoltOnly++;
      j++;
    }
  }
  referenceOnly += n - i;
  hvoltOnly += m - j;
  return { referenceOnly, hvoltOnly, total: referenceOnly + hvoltOnly };
}

/// The reference tree's path for one file, in the hvolt tree's layout.
function stagedPath(referenceRoot, file) {
  const parts = path.relative(referenceRoot, file).split(path.sep);
  if (parts.length > 4 && parts[1] === 'src' && parts[2] === 'main' && parts[3] === 'ets') {
    return [parts[0], ...parts.slice(4)].join('/');
  }
  return parts.join('/');
}

function walk(directory, out) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const full = path.join(directory, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else if (entry.name.endsWith('.ts')) out.push(full);
  }
}

function main() {
  const args = process.argv.slice(2).filter((arg) => arg !== '--json');
  const json = process.argv.includes('--json');
  const [left, right] = args;
  if (!left || !right) {
    console.error('usage: token-diff.js <ref-root|ref.ts> <hvolt-root|ours.ts> [--json]');
    process.exit(2);
  }
  if (!fs.existsSync(left) || !fs.existsSync(right)) {
    console.error(`missing input: ${!fs.existsSync(left) ? left : right}`);
    process.exit(2);
  }

  const single = fs.statSync(left).isFile();
  const per = {};
  let missing = [];
  if (single) {
    const { total } = differing(tokens(left), tokens(right));
    if (json) {
      console.log(JSON.stringify({ total, files: total ? 1 : 0, per: { [path.basename(left)]: total } }));
      return;
    }
    console.log(`${total} ${path.basename(left)}`);
    return;
  }

  const files = [];
  walk(left, files);
  files.sort();
  let total = 0;
  for (const file of files) {
    const staged = stagedPath(left, file);
    const mine = path.join(right, staged);
    if (!fs.existsSync(mine)) {
      missing.push(staged);
      continue;
    }
    const { total: count } = differing(tokens(file), tokens(mine));
    if (count) {
      per[staged] = count;
      total += count;
    }
  }
  if (json) {
    console.log(JSON.stringify({ total, files: Object.keys(per).length, missing: missing.length, per }));
    return;
  }
  for (const staged of Object.keys(per).sort()) {
    console.log(`${String(per[staged]).padStart(6)}  ${staged}`);
  }
  console.log(`total ${total}  (files ${Object.keys(per).length}, reference files ${files.length}, missing ${missing.length})`);
}

main();