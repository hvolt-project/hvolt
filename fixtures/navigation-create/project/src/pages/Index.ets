// `navigationCreateParam` (process_component_build.js) appends a trailing object to the `create`
// call of `Navigation` and `NavDestination`: both take `{ moduleName, pagePath }`, and
// `Navigation` additionally takes `isUserCreateStack: true` because the component supplies its
// own stack.  `TabContent` takes neither.
@Entry
@Component
struct Index {
  @State pageInfos: NavPathStack = new NavPathStack();

  build() {
    Column() {
      Navigation(this.pageInfos) {
        NavDestination() {
          Text('destination')
        }
        .title('Dest')
      }
      .title('Navigation')
      .mode(NavigationMode.Stack)
    }
  }
}