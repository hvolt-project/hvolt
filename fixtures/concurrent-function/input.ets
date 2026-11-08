// `processConcurrent` (process_ui_syntax.js) prepends the `"use concurrent"` directive to the
// body of a `@Concurrent` function declaration.
@Concurrent
function computeTask(value: number): number {
  return value * 2;
}

@Entry
@Component
struct ConcurrentPage {
  build() {
    Column() {
      Text('concurrent')
    }
  }
}
