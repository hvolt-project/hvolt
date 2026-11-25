declare module "virtual" {
  export const virtualValue: number;
}

@Entry
@Component
struct ArktsAmbientDecl {
  build() {
    Column() {
      Text("x")
    }
  }
}