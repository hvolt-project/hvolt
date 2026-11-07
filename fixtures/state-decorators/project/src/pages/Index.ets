@Observed
class Task {
  title: string = ''
}

@Entry
@Component
struct StateHost {
  @State count: number = 0
  @State tasks: Task[] = []
  @State current: Task = new Task()
  @Provide('theme') theme: string = 'light'
  @Provide plain: string = 'p'

  build() {
    Column() {
      StateChild({ shared: $count, task: this.current })
    }
  }
}

@Component
struct StateChild {
  @Link shared: number
  @Consume('theme') theme: string
  @ObjectLink task: Task
  @StorageLink('appKey') storageLinked: number = 0
  @StorageProp('appKey') storagePropped: number = 0
  @LocalStorageLink('localKey') localStorageLinked: number = 0
  @LocalStorageProp('localKey') localStoragePropped: number = 0

  build() {
    Text(this.theme)
  }
}