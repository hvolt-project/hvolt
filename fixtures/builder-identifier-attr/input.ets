// `parseBuilderNode` (process_component_build.js) decides a builder-reference attribute argument
// by the name it holds (`storedFileInfo.builderLikeCollection`, which is the current struct's
// `@Builder` methods and `@BuilderParam` fields plus the global `@Builder` table):
//
//   * a name in the table is a builder, so `this.TabBar` — not called — still takes the
//     `{ builder: this.TabBar.bind(this) }` form (`processPropertyBuilder`);
//   * `showUnit` is in `CUSTOM_BUILDER_PROPERTIES_WITHOUTKEY`, so its builder argument is the
//     bare `this.Unit.bind(this)`, with no `builder:` key;
//   * a name that is not in the table is a value, so `.background(this.color)` stays as written;
//   * the same holds for a bare name rather than a `this.` member: an imported global `@Builder`
//     function written without a call is still a builder reference (`processIdentifierBuilder`).
import { panelBuilder } from '../common/Panel';

@Entry
@Component
struct Index {
  @State color: string = '#ffffff';

  @Builder TabBarBuilder() {
    Text('tab')
  }

  @Builder UnitBuilder() {
    Text('unit')
  }

  build() {
    Column() {
      Tabs() {
        TabContent() {
          Text('first')
        }
        .tabBar(this.TabBarBuilder)
      }

      TextInput({ placeholder: 'value' })
        .showUnit(this.UnitBuilder)

      Text('menu')
        .bindMenu(panelBuilder)

      Text('value')
        .background(this.color)
    }
  }
}

@Component
struct ActionPanel {
  @BuilderParam actions: () => void = this.emptyActions;

  @Builder
  emptyActions() {
    Text('none')
  }

  build() {
    Text('panel')
      .bindMenu(this.actions)
  }
}
