// `process_sendable.js::transformOptionalMemberForSendable`: inside a `@Sendable` class a
// property written `name?: T` becomes `name: T | undefined = undefined`, and one that writes
// its own initializer keeps it (`init?: string = 'i'` → `init: string | undefined = 'i'`). A
// property without the question token is untouched.
@Sendable
class Box {
  value?: number;
  init?: string = 'i';
  plain: boolean = false;

  read(): number {
    return this.plain ? 1 : 0;
  }
}

@Sendable
function make(n: number): number {
  return n;
}

@Entry
@Component
struct SendableOptional {
  build() {
    Column() {
      Text(`${new Box().read()}${make(1)}`)
    }
  }
}
