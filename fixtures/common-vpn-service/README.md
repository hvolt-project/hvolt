# common-vpn-service

把 `proxy_core/rpc/CommonVpnService.ts` 整份原文复刻成 fixture，用于定位「简写属性」那一簇。

## 复刻方式

- 正文**逐字节**取自 `clashbox-flat/proxy_core/rpc/CommonVpnService.ts`，可用
  `cmp fixtures/common-vpn-service/project/src/common/CommonVpnService.ts <原文件>` 复核（当前一致）；
- **没有做 import 换桩**：原文只 import `@kit.NetworkKit` 与 `@kit.AbilityKit`，两者按规则保留，
  参考侧与 hvolt 都能解析；
- `extra-files.json` 里写的是 `common/CommonVpnService.ets` —— `extra-files.sh` 删除 4 个字符再拼
  `.ts`，写 `.ts` 会被截成 `CommonVpnServic.ts`。文件名本身仍是 `.ts`。

## 结论

参考侧保留 `destination: destination`，hvolt 在这个**普通 `.ts` 模块**里把它缩写成 `destination`。
这是 `PropertyAssignment ↔ ShorthandPropertyAssignment` 那一簇的真实成因；页面（`.ets`）里的对象字面量
不走这条路径，所以 `fixtures/shorthand-property` 复现不到。

最小可复现形态见 `fixtures/plain-ts-shorthand`（6 行模块）。
