enum Tone {
  Warm,
  Cool,
}

function enumWrong() {
  const t: Tone = "s";
  return t;
}

function enumOk() {
  const t: Tone = Tone.Warm;
  return t;
}

@Entry
@Component
struct TypeEnum {
  build() {
    Column() {
      Text("x")
    }
  }
}