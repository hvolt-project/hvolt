import {
  factor,
  label,
} from './Multi';

import { /* inline */ base } from './Multi'; // the base value

@Entry
@Component
struct MultilineImport {
  @State text: string = '';

  aboutToAppear(): void {
    this.text = String(factor) + label + String(base);
  }

  build() {
    Column() {
      Text(this.text)
    }
  }
}
