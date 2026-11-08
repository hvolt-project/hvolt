# observed-property-kind

状态字段的 backstore 类型由**声明的类型**决定：`ObservedPropertySimplePU` 还是
`ObservedPropertyObjectPU`，以及 `@Prop`/`@Link` 那族的 Simple/Object 变体。判错的后果不是排版，
而是运行期的深度观察语义。

## golden 形态（`golden/Index.ts`）

`@State` 一族：

| 字段 | 声明类型 | backstore | 行 |
| --- | --- | --- | --- |
| count | `number` | Simple | L299 |
| label | `string` | Simple | L306 |
| flag | `boolean` | Simple | L313 |
| config | `AppConfig`（本地 class） | Object | L321 |
| item | `SomeClass`（`@Observed` class） | Object | L329 |
| list | `number[]` | Object | L337 |
| objs | `SomeClass[]` | Object | L344 |
| m | `Map<string, number>` | Object | L352 |
| maybe | `string \| undefined` | Object | L360 |
| maybeBool | `boolean \| undefined` | **Simple** | L464 |
| opt | `number`（可选字段） | Simple | L367 |
| kind | `LocalKind`（本地 enum） | Simple | L375 |
| localAlias | `LocalAlias` = `'a' \| 'b'` | Simple | L383 |
| numberAlias | `NumberAlias` = `number` | **Object** | L390 |
| stringPrimAlias | `StringPrimAlias` = `string` | **Object** | L397 |
| boolAlias | `BoolAlias` = `boolean` | **Simple** | L404 |
| enumAlias | `EnumAlias` = `LocalKind` | **Simple** | L411 |
| twoPrimAlias | `TwoPrimAlias` = `string \| number` | Simple | L418 |
| inlineLiteralUnion | `'a' \| 'b'`（直接写） | Simple | L425 |
| inlinePrimUnion | `number \| string`（直接写） | Simple | L432 |
| iface | `SomeInterface` | Object | L440 |
| angleList | `Array<number>` | Object | L447 |

`@Prop` / `@Link` / `@ObjectLink` 一族：

| 字段 | 构造实参里的类 | 声明里的字段类型 | 行 |
| --- | --- | --- | --- |
| propObject（`SomeClass`） | `SynchedPropertyObjectOneWayPU` | `SynchedPropertySimpleOneWayPU<SomeClass>` | L98 |
| propNumber（`number`） | `SynchedPropertySimpleOneWayPU` | 同上形态 | L105 |
| linkObject（`AppConfig`） | `SynchedPropertyObjectTwoWayPU` | `SynchedPropertySimpleOneWayPU<AppConfig>` | L112 |
| objectLink（`SomeClass`） | `SynchedPropertyNesedObjectPU` | 同名 | L119 |

注意最后两列：**声明里的字段类型永远是 `SynchedPropertySimpleOneWayPU<…>`**，即使构造实参是
Object 变体 —— 这是参考的既有形态，只有构造那一行区分种类。

## 判据

参考侧按类型判定，等价于「合成出来的类型是 union 且每个成员都是基本类型」：

1. 语法节点本身是 `string` / `number` / `boolean` 关键字 → Simple；
2. 类型名是 enum（成员是 enum 字面量，合成类型是 union）→ Simple；
3. 类型名是**类型别名**时按别名体判：`boolean`（TS 里是 `true | false` 的 union）、union、
   或 enum 名 → Simple；**别名体是单独的 `number` / `string` → Object**（这条不对称是实测，
   见 `numberAlias` / `stringPrimAlias` 与 `boolAlias` 的对比）；
4. union：先丢掉 `undefined` / `null`。只剩一个幸存者时按「单节点」再判：`boolean` 简单
   （解析出来仍是 `true | false` 的 union）、enum 简单、别名按别名体判，`string` / `number`
   不简单——`maybe` 与 `maybeBool` 这两行就是这条的证据。剩多个时每个成员都必须是基本类型。

`@Provide` 与 `@State` 同规（`consumedConfig` → `ObservedPropertyObjectPU`、
`consumedNumber` → `ObservedPropertySimplePU`）；`@Consume` 的声明字段类型固定是
`ObservedPropertyAbstractPU<T>`，构造函数里走 `initializeConsume`，不按 simple/object 分家。

判据落在 `src/build/stage/process_component_class.rs` 的 `is_simple_type_text`，别名表来自同文件
`file_facts` 收集的 `aliases`。

## 边界

本 fixture 只放**文件内**能判定的形态。跨文件的类型名（导出 enum、导出别名、SDK 枚举）在
hvolt 里由项目级类型表回答，钉在 `fixtures/observed-property-kind-import`（已通过）。仍判不了的
只有第三方包里声明的类型名（如 `ColorPickerType` 来自 `@linke/ui-color-picker`），它不在项目
源码里，见该 fixture 的遗留说明。

## 复现

```sh
scripts/oracle/new-fixture.sh observed-property-kind
HOME=/tmp/hvigor-home scripts/oracle/run-hvigor.sh observed-property-kind
scripts/oracle/all.sh observed-property-kind     # PASS
```