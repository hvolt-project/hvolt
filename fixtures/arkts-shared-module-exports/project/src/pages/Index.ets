
"use shared";

export let counter = 0;
export function bump(): void {
}

@Entry
@Component
struct BatchProbe14 {
  build() {
    Column() {
      Text("x")
    }
  }
}
