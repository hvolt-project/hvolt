# ADR: product 变体

状态：已裁决，待实现
日期：2026-11-08

## 背景

`hvolt.toml` 的 `[app] products` 目前只是一串名字（`src/project/import.rs:208-222` 从
`build-profile.json5` 只取 `products[].name`），构建里 `product` 硬编码为 `"default"`
（`src/cli/build.rs:93`、`:278`），没有任何覆盖语义。留着这个字段等于对外宣称支持多产品变体，
而实际做不到。

hvigor 的 product 管四类东西：app 身份覆盖、签名配置、资源覆盖目录、以及
`modules[].targets`（每个模块是否参与、各自的资源与依赖覆盖）。第四类属于模块装配裁剪，
hvolt 没有这个概念。

## 裁决

1. **覆盖范围**：app 身份 + 签名 + 资源 + 输出目录。模块集合全项目一致，不支持
   `modules[].targets` 级别的模块装配差异。
2. **配置形态**：`[products.<name>]` 子表，与现有 `[app]` / `[app.signing]` 同构：

   ```toml
   [app]
   bundle = "org.example.app"
   version = "1.0.0"
   version_code = 1000000
   products = ["default", "release"]

   [products.release.app]
   bundle = "org.example.app.release"
   version_code = 2000000

   [products.release.resources]
   directories = ["app-resources", "res/release"]

   [products.release.signing]
   store_file = "signing/release/release.p12"
   # 其余字段省略则从 [app.signing.default] 继承
   ```

3. **继承**：product 子表未写的字段一律从根 `[app]` / `[app.signing.default]` 继承。
4. **可覆盖字段**：`bundle` / `name` / `version` / `version_code` / `vendor` / `icon`。
   `sdk_version`、`runtime_os`、`members`、`dependencies` 不随 product 变——它们决定工具链与
   模块集合，属于工程级而非产物级。
5. **资源**：`directories` 是有序列表，后者同名覆盖前者，与 hvigor 的
   `resource.directories` 语义一致。
6. **产物落点**：默认 product 仍写 `output/<name>-signed.hap`（现有脚本与文档不用改）；
   其它 product 写 `output/<product>/<name>-signed.hap`，互不覆盖。
7. **选择**：`hvolt build --product <name>`；不传则取配置里的 `default_product`，未写则
   `"default"`。

## 验收

两个 product 各出一包，四条读数全部成立才算完成：

- 两个包的 `bundleName` 不同；
- 两个包的 `versionCode` 不同；
- 同一个资源条目（例如 `string.json` 里的某个键）在两个包里内容不同；
- 两个包的签名证书主体不同；
- 两个包能在同一台设备上共存安装（不同 bundleName）。

## 迁移

`hvolt migrate` 遇到多 product 工程时，从 `app.products[]` 读
`name` / `bundleName` / `versionCode` / `versionName` / `resource.directories` / `signingConfig`
写进 `[products.<name>]`；读不到的（如 `modules[].targets` 差异）逐个报 gap，不静默丢弃。