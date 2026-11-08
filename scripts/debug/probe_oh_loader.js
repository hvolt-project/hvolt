// 驱动 OhLocalComponentLoader：打印每个组件的 expected vs actual 位置。
const fs = require("fs");
const D = "/home/hongyuncong/.hvolt/tools/hvigor/node_modules/@ohos/hvigor-ohos-plugin/node_modules/@ohos/sdkmanager-common/build/src";
const { OhLocalComponentLoader } = require(D + "/ohos/loader/oh-local-component-loader.js");
const { SimpleProgress } = require(D + "/core/log/simple-progress.js");
const p = new SimpleProgress();
p.debug = m => console.log("DBG:", m);
p.warn = m => console.log("WRN:", m);
p.info = () => {};
p.error = m => console.log("ERR:", m);

const root = process.env.OHOS_BASE_SDK_HOME || "/home/hongyuncong/.hvolt/sdk";
const l = new OhLocalComponentLoader(root, p);
const files = l.findPotentialSdks(root);
console.log("scanned:", files.length, "files");
for (const f of files) {
  const json = JSON.parse(fs.readFileSync(f, "utf8"));
  const c = l.convertComponent(json, f);
  const fav = c.getFullApiVersion ? c.getFullApiVersion() : undefined;
  console.log("----", c.path, c.version);
  console.log("  apiVersion=", json.apiVersion, "platformVersion=", json.platformVersion, "fullApiVersion=", json.fullApiVersion);
  console.log("  fullApi(dto)=", fav ? fav.getValue() : "none", "valid=", fav ? require(D + "/core/util/utils.js").isValidApiVersion(fav) : "-");
  console.log("  expected=", l._getExpectedLocation(c));
  console.log("  actual  =", c.getLocation());
}
