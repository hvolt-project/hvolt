// Turn V8 coverage into a per-module list of the loader functions that actually ran.
//
// `NODE_V8_COVERAGE` records every function's hit count, in the main process and in workers,
// which is why it is used instead of a `require` hook. For each loader module we parse its
// source, map each `exports.<name>` to the function it refers to, and report the name when a
// covered range falls inside that function.
//
// Usage: node covers-from-coverage.js <coverage-dir> <out.json>

'use strict';

const fs = require('fs');
const os = require('os');
const path = require('path');

function loaderRoot() {
  if (process.env.ETS_LOADER_ROOT) return process.env.ETS_LOADER_ROOT;
  const sdkRoot = path.join(os.homedir(), '.hvolt', 'sdk');
  const versions = fs.existsSync(sdkRoot)
    ? fs.readdirSync(sdkRoot).filter((v) => fs.statSync(path.join(sdkRoot, v)).isDirectory()).sort().reverse()
    : [];
  for (const version of versions) {
    const candidate = path.join(sdkRoot, version, 'openharmony', 'ets', 'build-tools', 'ets-loader');
    if (fs.existsSync(path.join(candidate, 'main.js'))) return candidate;
  }
  return null;
}

function collectHits(dir) {
  const hits = new Map();
  const walk = (current) => {
    let entries;
    try {
      entries = fs.readdirSync(current, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) {
        walk(full);
      } else if (entry.name.endsWith('.json')) {
        let data;
        try {
          data = JSON.parse(fs.readFileSync(full, 'utf8'));
        } catch {
          continue;
        }
        for (const script of data.result || []) {
          const url = script.url.startsWith('file://') ? decodeURIComponent(script.url.slice(7)) : script.url;
          if (!url.includes('/ets-loader/')) continue;
          const list = hits.get(url) || [];
          for (const fn of script.functions || []) {
            for (const range of fn.ranges || []) {
              list.push(range);
            }
          }
          hits.set(url, list);
        }
      }
    }
  };
  walk(dir);
  return hits;
}

function main() {
  const [dir, outPath] = process.argv.slice(2);
  if (!dir || !outPath) {
    console.error('usage: covers-from-coverage.js <coverage-dir> <out.json>');
    process.exit(2);
  }
  const root = loaderRoot();
  const ts = root ? require(path.join(root, 'node_modules', 'typescript')) : null;
  const hits = collectHits(dir);
  const result = {};
  for (const [file, ranges] of hits) {
    const marker = '/ets-loader/';
    const at = file.indexOf(marker);
    const moduleName = at >= 0 ? file.slice(at + marker.length) : path.basename(file);
    const covered = ranges.filter((r) => (r.count || 0) > 0);
    if (!covered.length || !ts) continue;
    let source;
    try {
      source = fs.readFileSync(file, 'utf8');
    } catch {
      continue;
    }
    const sf = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
    const spans = new Map();
    // `function name(…) { … }`
    const visit = (node) => {
      // A loader export may be a class as well as a function (`exports.ModuleBuildMode =
      // ModuleBuildMode`); both are recorded, or class exports never count as covered.
      if ((ts.isFunctionDeclaration(node) || ts.isClassDeclaration(node)) && node.name) {
        spans.set(node.name.text, [node.getStart(sf), node.getEnd()]);
      }
      if (ts.isVariableDeclaration(node) && node.name && ts.isIdentifier(node.name)) {
        const init = node.initializer;
        if (init && (ts.isArrowFunction(init) || ts.isFunctionExpression(init) || ts.isClassExpression(init))) {
          spans.set(node.name.text, [init.getStart(sf), init.getEnd()]);
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(sf);
    // `exports.name = …`
    const re = /exports\.([A-Za-z_$][\w$]*)\s*=\s*([A-Za-z_$][\w$]*)\s*[,;]/g;
    let match;
    const names = [];
    while ((match = re.exec(source))) {
      const [, exportName, ref] = match;
      const span = spans.get(ref);
      if (!span) continue;
      const hit = covered.some((r) => r.startOffset < span[1] && r.endOffset > span[0]);
      if (hit) names.push(exportName);
    }
    if (names.length) {
      result[moduleName] = Array.from(new Set(names)).sort();
    }
  }
  fs.writeFileSync(outPath, JSON.stringify(result, null, 2));
  console.error(`covers: ${Object.keys(result).length} modules`);
}

main();
