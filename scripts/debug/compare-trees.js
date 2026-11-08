#!/usr/bin/env node
// Compare every emitted `.ts` of the reference loader with hvolt's, at AST level.
//
// The fixture oracle (`scripts/oracle/compare.js`) does this per fixture; this one does it for
// a whole project, which is what a real app needs: a build that packs and installs still runs
// wrong if a field, an enum member or a whole declaration never made it into the output.
//
// Usage: node compare-trees.js <reference-root> <hvolt-root>
//   reference-root: `<module>/build/.../esmodule/<variant>` of a DevEco build
//   hvolt-root:     the staged `.ts` tree hvolt writes (HVOLT_KEEP_TS=1)

'use strict';

const fs = require('fs');
const os = require('os');
const path = require('path');

function loaderRoot() {
  const sdkRoot = path.join(os.homedir(), '.hvolt', 'sdk');
  const versions = fs.existsSync(sdkRoot)
    ? fs
        .readdirSync(sdkRoot)
        .filter((v) => fs.statSync(path.join(sdkRoot, v)).isDirectory())
        .sort()
        .reverse()
    : [];
  for (const version of versions) {
    for (const layout of [
      ['openharmony', 'ets', 'build-tools', 'ets-loader'],
      ['ets', 'build-tools', 'ets-loader'],
    ]) {
      const candidate = path.join(sdkRoot, version, ...layout);
      if (fs.existsSync(path.join(candidate, 'main.js'))) return candidate;
    }
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

function canonical(node, out) {
  const kind = ts.SyntaxKind[node.kind];
  let extra = '';
  if (ts.isIdentifier(node) || ts.isPrivateIdentifier(node)) {
    extra = `:${node.text}`;
  } else if (ts.isStringLiteralLike(node)) {
    if (/^com\.example\./.test(node.text) || /\/src\/main\/ets\//.test(node.text)) {
      extra = ':<identity>';
    } else {
      extra = `:${JSON.stringify(node.text)}`;
    }
  } else if (ts.isNumericLiteral(node)) {
    extra = `:${node.text}`;
  }
  if (ts.isParenthesizedExpression(node)) {
    canonical(node.expression, out);
    return;
  }
  if (ts.isObjectLiteralExpression(node) && node.properties.length > 0) {
    const first = node.properties[0];
    if (ts.isPropertyAssignment(first) && String(first.name.text || first.name.escapedText) === 'page') {
      out.push('LocMeta');
      return;
    }
  }
  out.push(kind + extra);
  if (ts.isNewExpression(node) && node.arguments && node.arguments.length >= 6) {
    node.arguments.forEach((arg, index) => {
      if (index === 5) out.push('LocMeta');
      else canonical(arg, out);
    });
    return;
  }
  ts.forEachChild(node, (child) => canonical(child, out));
}

function nodes(file) {
  const text = fs.readFileSync(file, 'utf8');
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.ES2017, true, ts.ScriptKind.TS);
  const parts = [];
  canonical(source, parts);
  return parts;
}

function key_of(root, file) {
  const rel = path.relative(root, file);
  const parts = rel.split(path.sep);
  // entry/src/main/ets/pages/Index.ts -> entry/pages/Index.ts
  if (parts.length > 4 && parts[1] === 'src' && parts[2] === 'main' && parts[3] === 'ets') {
    return [parts[0], ...parts.slice(4)].join('/');
  }
  return parts.join('/');
}

function walk(dir, out) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else if (entry.name.endsWith('.ts')) out.push(full);
  }
}

function main() {
  const [referenceRoot, hvoltRoot] = process.argv.slice(2);
  if (!referenceRoot || !hvoltRoot) {
    console.error('usage: compare-trees.js <reference-root> <hvolt-root>');
    process.exit(2);
  }
  const files = [];
  walk(referenceRoot, files);

  let same = 0;
  const missing = [];
  const differing = [];
  for (const file of files) {
    const key = key_of(referenceRoot, file);
    const mine = path.join(hvoltRoot, key);
    if (!fs.existsSync(mine)) {
      missing.push(key);
      continue;
    }
    const a = nodes(file);
    const b = nodes(mine);
    if (a.length === b.length && a.every((value, index) => value === b[index])) {
      same += 1;
      continue;
    }
    let at = 0;
    while (at < a.length && at < b.length && a[at] === b[at]) at += 1;
    differing.push({
      key,
      at,
      reference: a.slice(Math.max(0, at - 2), at + 4),
      hvolt: b.slice(Math.max(0, at - 2), at + 4),
    });
  }

  console.log(`reference files: ${files.length}`);
  console.log(`same: ${same}`);
  console.log(`missing in hvolt: ${missing.length}`);
  for (const key of missing.slice(0, 20)) console.log(`  MISS ${key}`);
  console.log(`differing: ${differing.length}`);
  for (const d of differing.slice(0, 20)) {
    console.log(`  DIFF ${d.key} @node ${d.at}`);
    console.log(`    reference: ${d.reference.join(' | ')}`);
    console.log(`    hvolt:     ${d.hvolt.join(' | ')}`);
  }
}

main();