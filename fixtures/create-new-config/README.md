# create-new-config

整份复刻 `entry/components/Configuration/CreateNewConfig.ets`（91 行源码，对应 185 行 `.ts`）。
旧聚类里它与 `EditConfigContent`、`ImportConfigFromFile`、`ImportConfigFromURL` 同属一簇
（`CallExpression ↔ ObjectLiteralExpression`），覆盖潜力最大。

## 复刻方式

- 正文逐行保留，import 换成页面相对路径（`../`），末尾追加一个 `@Entry` 页面实例化该组件；
- 桩：`Constants.ets` 六个常量、`ResourceStringUtil.ets`、`BreakPoint.ets`、`breakpointsystem.ets`、
  `components/Common/Common.ets`（`BindSheetTitleCustomBuilder` 在真实工程里是**全局 `@Builder function`**，
  不是组件，桩按函数写）；
- 资源补了 2 个 string 与 2 个 color；顺序是 `new-fixture.sh` → 补资源 → `run-hvigor.sh`，
  且跑 hvigor 前用 `diff input.ets project/src/pages/Index.ets` 确认同步。

## 差异归类

1. `makeBuilderParameterProxy` 缺失——属于已知的 `builder-param-proxy` 规则（并发线在做）；
2. 源码坐标 `line: 19, col: 3` 对 `line: 2, col: 3`——子组件在全局 `@Builder function` 内实例化时
   参考侧给的是原文件行号，我们给的是另一处行号；
3. 注释条数 67 对 58；
4. 排版（空接口换行、else 折行、`LocMeta` 对象展开）。

结论：该文件卡在别处已存在的规则上，本轮不重复修。
