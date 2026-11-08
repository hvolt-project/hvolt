#!/usr/bin/env node
// Drive the official ets-loader the way hvigor does: through its rollup plugin.
//
// Feeding `processUISyntax` into a plain TypeScript emit is rejected by the loader's own
// TypeScript fork ("Class and object literal elements must be visited with their respective
// visitors") because struct members must go through its own visitor. hvigor never does that:
// it runs the loader as a rollup plugin (`etsTransform()`) and hands it a `share` object.
// This driver mirrors that contract.
//
// Usage: node run-ets-loader.js <input.ets> [--out <file>]

'use strict';

const fs = require('fs');
const os = require('os');
const path = require('path');

function sdkLoaderRoot() {
  if (process.env.ETS_LOADER_ROOT) return process.env.ETS_LOADER_ROOT;
  const sdkRoot = path.join(os.homedir(), '.hvolt', 'sdk');
  const versions = fs.existsSync(sdkRoot)
    ? fs.readdirSync(sdkRoot).filter((v) => fs.statSync(path.join(sdkRoot, v)).isDirectory()).sort().reverse()
    : [];
  for (const version of versions) {
    const candidate = path.join(sdkRoot, version, 'openharmony', 'ets', 'build-tools', 'ets-loader');
    if (fs.existsSync(path.join(candidate, 'main.js'))) return candidate;
  }
  throw new Error('ets-loader not found; set ETS_LOADER_ROOT');
}

const loaderRoot = sdkLoaderRoot();
// The loader ships only its rollup *plugins*; hvigor supplies rollup itself. Fall back to the
// copy that ships inside DevEco Studio when the configured toolchain has none.
function resolveRollup() {
  const candidates = [
    process.env.ETS_ROLLUP,
    path.join(loaderRoot, 'node_modules', 'rollup'),
    path.join(__dirname, '..', '..', 'tools', 'oracle', 'node_modules', 'rollup'),
    '/Applications/DevEco-Studio.app/Contents/plugins/codescanner/code-scanner-agent/node_modules/rollup',
  ].filter(Boolean);
  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) return require(candidate);
  }
  throw new Error('rollup not found; set ETS_ROLLUP to a rollup package directory');
}
const rollup = resolveRollup();
const main = require(path.join(loaderRoot, 'main.js'));
const fileInfoModule = require(path.join(loaderRoot, 'lib', 'file_info_cache'));
const { etsTransform, getCompilerHost } = require(path.join(
  loaderRoot,
  'lib',
  'fast_build',
  'ets_ui',
  'rollup-plugin-ets-typescript',
));
const ts = require(path.join(loaderRoot, 'node_modules', 'typescript'));
const etsChecker = require(path.join(loaderRoot, 'lib', 'ets_checker'));

function configure(projectDir) {
  const cfg = main.projectConfig;
  cfg.projectPath = projectDir;
  cfg.projectRootPath = projectDir;
  cfg.buildPath = path.join(projectDir, 'build');
  cfg.cachePath = path.join(projectDir, 'build', 'cache');
  cfg.moduleName = 'entry';
  cfg.bundleName = 'com.example.fixture';
  cfg.compileMode = 'esmodule';
  cfg.processTs = true;
  cfg.useNormalizedOHMUrl = true;
  cfg.compileHar = false;
  cfg.compileShared = false;
  cfg.minAPIVersion = 20;
  cfg.compatibleSdkVersion = '6.0.0(20)';
  cfg.targetAPIVersion = 20;
  cfg.runtimeOS = 'HarmonyOS';
  cfg.entryObj = {};
  cfg.cardObj = {};
  cfg.entryArrayForObf = [];
  cfg.aceBuildJson = {};
  cfg.hashProjectPath = projectDir;
  cfg.singleFileEmit = false;
  // arkguard (obfuscation) runs inside the plugin chain and reads the build mode.
  cfg.buildMode = 'Debug';
  main.partialUpdateConfig.partialUpdateMode = true;
  process.env.compileMode = 'esmodule';
}

function shareFor(projectDir) {
  return {
    projectConfig: {
      expandImportPath: { enable: false },
      compileMode: 'esmodule',
      useNormalizedOHMUrl: true,
      moduleName: 'entry',
      bundleName: 'com.example.fixture',
    },
    arkProjectConfig: { buildMode: 'Debug', obfuscationMergedObConfig: undefined },
    projectRootPath: projectDir,
    moduleRootPath: projectDir,
    aceModuleBuild: path.join(projectDir, 'build'),
    resourceTableTs: path.join(projectDir, 'build', 'ResourceTable.ts'),
    rawfilechanged: false,
    // Logging and file-info hooks hvigor injects; a fixture run needs them to be callable.
    getLogger: () => ({
      debug() {},
      info() {},
      warn() {},
      error() {},
      printError() {},
    }),
    getHvigorConsoleLogger: () => ({
      debug() {},
      info() {},
      warn() {},
      error() {},
      printError() {},
    }),
    getCachedFileInfo: (file) => ({ file, mtimeMs: 0, hash: '', children: [] }),
  };
}

function withShare(plugin, share) {
  const wrapped = { name: plugin.name };
  for (const [hook, value] of Object.entries(plugin)) {
    if (hook === 'name') continue;
    if (typeof value !== 'function') {
      wrapped[hook] = value;
      continue;
    }
    wrapped[hook] = function (...args) {
      this.share = share;
      if (process.env.ORACLE_TRACE) {
        console.error(`[oracle] ${plugin.name}.${hook}`, String(args[0] || '').slice(0, 80));
      }
      return value.apply(this, args);
    };
  }
  return wrapped;
}

async function run() {
  const argv = process.argv.slice(2);
  const input = argv[0];
  if (!input) {
    console.error('usage: run-ets-loader.js <input.ets> [--out <file>]');
    process.exit(2);
  }
  const outIndex = argv.indexOf('--out');
  const outPath = outIndex >= 0 ? argv[outIndex + 1] : null;

  const file = path.resolve(input);
  const projectDir = fs.mkdtempSync(path.join(os.tmpdir(), 'ets-loader-fixture-'));
  configure(projectDir);
  const share = shareFor(projectDir);
  if (fileInfoModule && typeof fileInfoModule.fileInfoCache?.setShare === 'function') {
    fileInfoModule.fileInfoCache.setShare(share);
  }

  // hvigor owns the global program; the plugin's hooks read it during transform and clear it
  // at build end. Build it with the loader's own compiler options and compiler host.
  const host = getCompilerHost();
  const program = ts.createProgram({
    rootNames: [file],
    options: etsChecker.compilerOptions,
    host,
  });
  main.globalProgram = { program };

  const bundle = await rollup.rollup({
    input: file,
    plugins: [withShare(etsTransform(), share)],
    onwarn: () => {},
  });
  const { output } = await bundle.generate({ format: 'es' });
  const code = output.map((chunk) => chunk.code || '').join('\n');
  if (outPath) {
    fs.writeFileSync(outPath, code);
    console.error(`wrote ${outPath}`);
  } else {
    process.stdout.write(code);
  }
}

run().catch((error) => {
  console.error('driver failed:', error && error.stack ? error.stack : error);
  process.exit(1);
});
