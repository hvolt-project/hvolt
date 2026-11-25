import helper = require("../lib/helper");

@Entry
@Component
struct ImportEqualsProbe {
  build() {
    Column() {
      Text(`${helper.run()}`)
    }
  }
}
