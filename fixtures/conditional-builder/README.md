# conditional-builder

`processCustomBuilderProperty`（`process_component_build.js`）把 builder 属性（`bindMenu` /
`bindContextMenu` / `bindContentCover` …）的每个实参交给 `parseBuilderNode`；实参本身是条件表达式时
先过 `processConditionalBuilder`，把**每个分支单独**再走一遍 `parseBuilderNode`。golden 量到的形态：

```js
Text.bindMenu(this.cond ? { builder: () => {
        MyMenu.call(this);
    } } : null);
Text.bindContextMenu(this.cond ? { builder: () => {
        MyMenu.call(this);
    } } : { builder: () => {
        OtherMenu.call(this);
    } }, ResponseType.RightClick);
Text.bindMenu(this.cond ? { builder: () => {
        this.ThisMenu.call(this);
    } } : null);
Text.bindMenu(this.cond ? { builder: this.actions.bind(this) } : null); // 无调用的 builder 引用
Text.bindMenu(this.cond ? null : { builder: () => {
        MyMenu.call(this);
    } });
TextInput.showUnit(this.cond ? () => {
        UnitOne.call(this);
    } : null); // WITHOUTKEY 属性：只给箭头，不包 builder: 键
// 下面四条是 bindPopup 配置对象里的 builder: 属性（processBindPopupBuilderProperty 之后的
// parseBuilderNode 通路），量到的形态与上面的实参形态**同一套渲染**：
Text.bindPopup(this.cond, { builder: this.cond ? { builder: () => {
            MyMenu.call(this);
        } } : null, onWillDismiss: false });
Text.bindPopup(this.cond, { builder: this.cond ? { builder: () => {
            this.ThisMenu.call(this);
        } } : null, onWillDismiss: false });
Text.bindPopup(this.cond, { builder: { builder: this.actions.bind(this) }, onWillDismiss: false });
Text.bindPopup(this.cond, { builder: { builder: MyMenu.bind(this) }, onWillDismiss: false });
```

从这个 fixture 量到、写进 `rewrite_conditional_builder` / `parse_builder_node` 的规则：

* 有调用的分支：`{ builder: () => { X.call(this, …) } }`，实参原样跟在接收者后面
  （`ArgMenu('y')` → `ArgMenu.call(this, 'y')`，`this.ThisArg('x')` → `this.ThisArg.call(this, 'x')`）；
* 无调用的 builder 引用：`{ builder: X.bind(this) }`，既覆盖 `this.actions`
  （`@BuilderParam`），也覆盖裸全局名 `MyMenu`；
* `CUSTOM_BUILDER_PROPERTIES_WITHOUTKEY` 属性（`showUnit` / `create`）只给箭头：全局调用是
  `() => { UnitOne.call(this) }`，`this.` 调用是 `() => { this.UnitTwo.call(this) }`，都不带
  `builder:` 键（与这些属性的非条件写法同一条规则）；
* `null` / `undefined` 分支原样保留，且 `null` 在前在后都一样；
* **不递归**：`cond ? MyMenu() : cond ? OtherMenu() : null` 只包第一个分支，内层条件表达式整段原样
  （golden 如此），不要"顺手"递归下去；
* 条件表达式出现在非第一个实参位（`bindMenu(isShow, content)`）时同样按分支处理。

`rewrite_builder_properties` 的 `builder:` 属性位另有两条规则（同样由 `parse_builder_node` 判定，
`fixtures/conditional-builder` 的 `popup-*` 四行是量的来源）：

* **条件值**按上面的实参规则逐分支渲染（带 `builder:` 键，即使属性本身是 `builder:`）；
* **无调用的 builder 引用**包成 `{ builder: X.bind(this) }`；注意这与同位置的非条件调用写法不同——
  `builder: this.X()` 仍是 `this.X.bind(this)`，`builder: this.X(args)` 仍是
  `() => { this.X.call(this, args) }`（这是 `processBindPopupBuilderProperty` 的另一条渲染）。

一处与本 fixture 相关但**不在** `rewrite_conditional_builder` 职责内的实测结论：

* `bindPopup(this.cond ? this.ThisMenu() : null, …)` 不是合法 ArkTS：`bindPopup` 第一形参是
  `boolean`，builder 调用返回 `void`，hvigor 报 `10505001 Argument of type 'void' is not assignable
  to parameter of type 'boolean'`。`this.` 前缀的对照只能放在 builder 位（`bindMenu` 第一参、
  `bindContentCover` 第二参），本 fixture 用的是后者。

说明写在这里而不是 `input.ets` 顶部：`@Builder` 函数前的注释在 `emit_builder` 重建声明时会丢，
那是另一条已知缺口，与本 fixture 要钉的形态无关。