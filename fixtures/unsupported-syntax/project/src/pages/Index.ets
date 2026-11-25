function drop(o: Record<string, number>) {
  delete o["a"];
}

function touchGlobal() {
  globalThis.anything = 1;
}

function typedCatch() {
  try {
    return 1;
  } catch (e: Error) {
    return 0;
  }
}

@Entry
@Component
struct UnsupportedSyntax {
  build() {
    Column() {
      Text(`${drop({} as Record<string, number>)}`)
    }
  }
}
