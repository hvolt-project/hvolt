# plain-ts-shorthand

`common-vpn-service` 的最小化版本：一个普通 `.ts` 模块（不是页面）在对象字面量里写同名的键与值。

## 形态

```ts
export function makeBag(): Bag {
  const destination = 'd';
  const gateway = 'g';
  return { destination: destination, gateway: gateway };
}
```

参考侧发射 `return { destination: destination, gateway: gateway };`，hvolt 发射
`return { destination, gateway };` —— 同名属性被缩写了。这条差异只在这个模块是**普通 `.ts` 文件**时出现；
同样的对象字面量写在页面 `.ets` 里时，双方都保留原写法（见 `fixtures/shorthand-property`）。
