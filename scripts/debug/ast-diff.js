#!/usr/bin/env node
// 打印 golden 与 actual 的第一个 AST 结构差异，供定位用；判据与 oracle 的 compare.js 一致。
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

function canonical(node, out) {
  if (ts.isParenthesizedExpression(node)) {
    canonical(node.expression, out);
    return;
  }
  let extra = '';
  if (ts.isIdentifier(node)) extra = `:${node.text}`;
  else if (ts.isStringLiteralLike(node)) extra = `:${JSON.stringify(node.text)}`;
  else if (ts.isNumericLiteral(node)) extra = `:${node.text}`;
  out.push(ts.SyntaxKind[node.kind] + extra);
  ts.forEachChild(node, (child) => canonical(child, out));
}

function fingerprint(file) {
  const text = fs.readFileSync(file, 'utf8');
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.ES2017, true, ts.ScriptKind.TS);
  const parts = [];
  canonical(source, parts);
  return parts;
}

const dir = process.argv[2];
const golden = fingerprint(path.join(dir, 'golden.ts'));
const actual = fingerprint(path.join(dir, 'actual.ts'));
console.log(`length ${golden.length} vs ${actual.length}`);
for (let i = 0; i < Math.max(golden.length, actual.length); i += 1) {
  if (golden[i] !== actual[i]) {
    console.log('first diff at', i);
    console.log(' golden:', golden.slice(Math.max(0, i - 6), i + 6).join(' | '));
    console.log(' actual:', actual.slice(Math.max(0, i - 6), i + 6).join(' | '));
    break;
  }
}
