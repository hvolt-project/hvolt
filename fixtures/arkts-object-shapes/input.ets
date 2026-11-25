function objSpread(a: Record<string, number>) {
  return { ...a };
}

function stdlib(v: Record<string, number>) {
  return Object.keys(v);
}

function reassign() {
  const o = { m: () => 1 };
  o.m = () => 2;
}

function standalone() {
  return this;
}

@Entry
@Component
struct ArktsObjectShapes {
  build() {
    Column() {
      Text(`${objSpread({})}`)
    }
  }
}
