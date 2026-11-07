// RE_EXPORT_COMMENT
export { Helper } from './Helper'

// FUNCTION_COMMENT
export function helperFn(): number {
  return 1
}

// CONST_COMMENT
export const helperConst: number = 2

// CLASS_COMMENT
export class HelperClass {
  value: number = 3
}

@Entry
@Component
struct BarrelPage {
  build() {
    Column() {
      Text(`${helperFn()}${helperConst}`)
    }
  }
}
