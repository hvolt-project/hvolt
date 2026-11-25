interface NamedThing {
  label: string;
}

class LabelBox {
  label: string = "";
}

function readLabel(target: NamedThing): string {
  return target.label;
}

const box = new LabelBox();
const readBack = readLabel(box);

const primitive = 1;
const boxedCheck = primitive instanceof Number;

@Entry
@Component
struct StructuralProbe {
  build() {
    Column() {
      Text(`${readBack}${boxedCheck}`)
    }
  }
}
