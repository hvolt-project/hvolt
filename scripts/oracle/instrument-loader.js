// Record which functions of the official ets-loader a build actually calls.
//
// Preloaded into the hvigor process with `--require`, so the record comes from the real build
// rather than from someone's claim about it. Writes `<ORACLE_COVERS_OUT>` at exit:
// `{ "<module>": ["<export>", …] }`, module paths relative to the loader root.
//
// Skipped entirely when ORACLE_COVERS_OUT is unset.

'use strict';

const fs = require('fs');
const path = require('path');
const Module = require('module');

const out = process.env.ORACLE_COVERS_OUT;
const loaderRoot = process.env.ORACLE_LOADER_ROOT;
if (!out || !loaderRoot) {
  return;
}

const called = new Map();
const originalLoad = Module._load;

// The build may load the loader from the SDK root or from DevEco's bundled copy; identifying
// it by the directory name keeps both working.
function rel(file) {
  const normalized = file.split(path.sep).join('/');
  const marker = '/ets-loader/';
  const at = normalized.indexOf(marker);
  return at >= 0 ? normalized.slice(at + marker.length) : path.basename(file);
}

function wrap(exportsObject, file) {
  if (!exportsObject || (typeof exportsObject !== 'object' && typeof exportsObject !== 'function')) {
    return exportsObject;
  }
  const moduleName = rel(file);
  let record = called.get(moduleName);
  if (!record) {
    record = new Set();
    called.set(moduleName, record);
  }
  for (const key of Object.keys(exportsObject)) {
    const value = exportsObject[key];
    if (typeof value !== 'function' || value.__oracleWrapped) {
      continue;
    }
    const wrapped = function (...args) {
      record.add(key);
      return value.apply(this, args);
    };
    wrapped.__oracleWrapped = true;
    try {
      exportsObject[key] = wrapped;
    } catch {
      // Read-only export: leave it alone.
    }
  }
  return exportsObject;
}

Module._load = function (request, parent, isMain) {
  const result = originalLoad.apply(this, arguments);
  try {
    const resolved = Module._resolveFilename(request, parent, isMain);
    if (resolved.split(path.sep).join('/').includes('/ets-loader/')) {
      return wrap(result, resolved);
    }
  } catch {
    // Resolution failures belong to the build, not to the probe.
  }
  return result;
};

process.on('exit', () => {
  const payload = {};
  for (const [moduleName, names] of called) {
    if (names.size) {
      payload[moduleName] = Array.from(names).sort();
    }
  }
  try {
    fs.writeFileSync(out, JSON.stringify(payload, null, 2));
  } catch {
    // Nothing to do at exit; the run itself already happened.
  }
});
