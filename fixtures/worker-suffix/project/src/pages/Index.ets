// `processWorker` (process_ui_syntax.js) rewrites the script URL of `new <ns>.Worker(…)` from
// `.ts` to `.js`.
import worker from '@ohos.worker';

const task = new worker.Worker('entry/src/main/ets/workers/task.ts');

@Entry
@Component
struct WorkerPage {
  build() {
    Column() {
      Text('worker')
    }
  }
}
