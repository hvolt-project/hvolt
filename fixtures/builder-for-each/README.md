# builder-for-each

`xb-menu` 的最小复现（16 行）：全局 `@Builder function` 里有一个 `ForEach`。

## 形态

```ts
@Builder
export function ListBuilder(items: string[]) {
  ForEach(items, (item: string) => {
    Text(item)
  })
}
```

参考侧发射 `(parent ? parent : this).forEachUpdateFunction(elmtId, items, forEachItemGenFunction);`，
hvolt 发射 `this.forEachUpdateFunction(elmtId, items, forEachItemGenFunction);`。

触发条件是「`ForEach` 位于全局 `@Builder function` 内」：那种函数有 `parent` 参数，接收者要用
`(parent ? parent : this)`。组件方法里的 `ForEach` 不受影响。
