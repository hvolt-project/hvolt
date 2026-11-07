// Which exports of each loader module the loader itself can actually reach from `main.js`.
//
// The point of the inventory is to know what a fixture *could* cover. Every assignment
// (`exports.foo = …`) is a much wider set than what any build path touches, so the list is
// narrowed to exports that some other loader file reads off the module, discovered by walking
// the require graph from the entry.
//
// Usage: node reachable-exports.js <loader-root> <out.json>

'use strict';

const fs = require('fs');
const path = require('path');

const [rootArg, outPath] = process.argv.slice(2);
if (!rootArg || !outPath) {
  console.error('usage: reachable-exports.js <loader-root> <out.json>');
  process.exit(2);
}
const root = path.resolve(rootArg);

function files(dir, acc = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === 'node_modules') continue;
      files(full, acc);
    } else if (entry.name.endsWith('.js')) {
      acc.push(full);
    }
  }
  return acc;
}

// `require('./x')` resolves without the extension the map records.
const rel = (file) => {
  let name = path.relative(root, file).split(path.sep).join('/');
  if (!name.endsWith('.js')) name += '.js';
  return name;
};

// local binding → module path, per file, for `x = require('./y')` and destructuring forms.
function bindings(text, file) {
  const map = new Map();
  const simple = /([A-Za-z_$][\w$]*)\s*=\s*require\(\s*["']([^"']+)["']\s*\)/g;
  let m;
  while ((m = simple.exec(text))) {
    const target = path.resolve(path.dirname(file), m[2]);
    map.set(m[1], target);
  }
  const destructured = /(?:const|let|var)\s*\{([^}]+)\}\s*=\s*require\(\s*["']([^"']+)["']\s*\)/g;
  while ((m = destructured.exec(text))) {
    const target = path.resolve(path.dirname(file), m[2]);
    for (const part of m[1].split(',')) {
      const name = part.split(':').pop().trim();
      if (/^[A-Za-z_$][\w$]*$/.test(name)) {
        map.set(name, target);
      }
    }
  }
  return map;
}

const graph = new Map(); // module → Map(export → Set(modules that read it))
const allFiles = files(root);
const texts = new Map();
for (const file of allFiles) {
  texts.set(file, fs.readFileSync(file, 'utf8'));
}

for (const [file, text] of texts) {
  for (const [local, target] of bindings(text, file)) {
    const re = new RegExp(`\\b${local.replace(/\$/g, '\\$')}\\.([A-Za-z_$][\\w$]*)`, 'g');
    let m;
    while ((m = re.exec(text))) {
      const moduleName = rel(target);
      const slot = graph.get(moduleName) || new Map();
      const readers = slot.get(m[1]) || new Set();
      readers.add(rel(file));
      slot.set(m[1], readers);
      graph.set(moduleName, slot);
    }
  }
}

// Only callable exports can be observed by function coverage; a constant or a plain object
// never shows up as a hit, so counting it would make `covered` unreachable.
function callableNames(file) {
  const ts = requireCachedTypescript();
  if (!ts) return null;
  let source;
  try {
    source = fs.readFileSync(path.join(root, file), 'utf8');
  } catch {
    return null;
  }
  const sf = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  const names = new Set();
  const visit = (node) => {
    if ((ts.isFunctionDeclaration(node) || ts.isClassDeclaration(node)) && node.name) {
      names.add(node.name.text);
    }
    if (ts.isVariableDeclaration(node) && node.name && ts.isIdentifier(node.name)) {
      const init = node.initializer;
      if (init && (ts.isArrowFunction(init) || ts.isFunctionExpression(init) || ts.isClassExpression(init))) {
        names.add(node.name.text);
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sf);
  return names;
}

let cachedTs;
function requireCachedTypescript() {
  if (cachedTs !== undefined) return cachedTs;
  try {
    cachedTs = require(path.join(root, 'node_modules', 'typescript'));
  } catch {
    cachedTs = null;
  }
  return cachedTs;
}

const result = {};
for (const [moduleName, exportsMap] of graph) {
  const callables = callableNames(moduleName);
  const names = Array.from(exportsMap.keys());
  // The export name and the internal binding usually match; when the source cannot be read,
  // keep the name as written rather than silently dropping it.
  const filtered = callables ? names.filter((name) => callables.has(name)) : names;
  if (filtered.length) {
    result[moduleName] = filtered.sort();
  }
}
fs.writeFileSync(outPath, JSON.stringify(result, null, 2));
console.error(`reachable exports: ${Object.keys(result).length} modules`);
