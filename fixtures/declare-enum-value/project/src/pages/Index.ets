import { KeyCode } from '@kit.InputKit';

// `KeyCode` comes from a declaration-only system module (`export declare enum KeyCode`): its
// members are runtime constants, so the emitted file has to keep both the import and the
// `KeyCode.KEYCODE_…` reference instead of inlining the number.
@Entry
@Component
struct DeclareEnumValue {
  @State pressed: string = '';

  private escapePressed(code: number): boolean {
    return code === KeyCode.KEYCODE_ESCAPE;
  }

  build() {
    Column() {
      Text(`${this.escapePressed(2070) ? 'esc' : this.pressed}`)
    }
  }
}
