# bind-popup-global-builder

`bindPopup` 的第二实参是配置对象，其中 `builder:` 的初值是全局 `@Builder` 函数的调用。
参考的 `processBindPopupBuilderProperty`（`process_component_build.js`）先于
`processCustomBuilderProperty` 命中，并且只处理这个对象里的 `builder:` 属性：`PopupOptions.builder`
是 `CustomBuilder`，所以值被发成 `{ builder: … }` 对象，箭头里把全局 builder 以 `this` 为接收者调用。

**实参转发规则**（`processInitializer` → `parseBuilderNode` → `getParsedBuilderAttrArgumentWithParams`）：
`[createThis(), ...e.arguments]` —— 源代码写的**每一个**实参都原样跟在接收者后面，`this` 也不例外
（早期 hvolt 的实现会丢掉第一个实参，已按 golden 纠正）。三个样本：

```js
Text.bindPopup(this.shown, {
    builder: { builder: () => {
            popupContent.call(this, 'hello');
        } },
    onWillDismiss: false
});
// 箭头实参：实参整体原样转发
builder: { builder: () => {
        popupBuilder.call(this, this, this.tips[1], () => {
            Text('x');
        });
    } }
// 唯一的实参就是 this：仍然转发
builder: { builder: () => {
        selfArg.call(this, this);
    } }
// 多实参：一个都不丢
builder: { builder: () => {
        twoArgs.call(this, 'a', 'b');
    } }
```

以此为界，hvolt 的接收规则是「每个实参都能原样转发才包」（`forwardable_builder_argument`）：字面量、
标识符、`this`、成员/下标访问，或函数 / 箭头表达式（函数体不再深入）；**嵌套调用、对象字面量、赋值、
语句仍然放弃改写**——对象字面量还有额外理由：参考实现遇到「唯一的对象字面量实参」会把它换成
`__BuilderParamProxy(…)` 调用（`transDoubleDollarInCustomBuilder`），hvolt 不生成这条。

写法注意：全局 `@Builder` 的参数表末尾会被追加 `parent = null`，所以 fixture 里**不能**有名为 `parent`
的形参（否则 hvigor 报 `00305015 Rollup Error: Argument name clash`）。

说明写在这里而不是 `input.ets` 顶部：`@Builder` 函数前的注释在 `emit_builder` 重建声明时会丢，
那是另一条已知缺口，与本 fixture 要钉的 `bindPopup` 形态无关。