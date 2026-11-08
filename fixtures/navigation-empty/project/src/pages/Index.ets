// `navigationCreateParam` (process_component_build.js) gives both the container and the
// destination a `create` argument that the source never wrote:
//
//   * a `Navigation` with no stack of its own starts one — `new NavPathStack()` — and its
//     `navigationOrNavDestinationCreateContent` object reports `isUserCreateStack: false`,
//     because the component built the stack instead of the caller;
//   * a `NavDestination` whose body has no statement still takes the render arrow: the create
//     call is `NavDestination.create(() => { }, { moduleName, pagePath })`.
//
// A `TabContent` takes neither, so the same empty body keeps `TabContent.create()`.
@Entry
@Component
struct Index {
  build() {
    Column() {
      Navigation() {
        NavDestination() {
          Text('destination')
        }
        .title('Dest')

        NavDestination() {
        }
        .title('Empty')
      }
      .title('Nav')

      Tabs({ barPosition: BarPosition.Start }) {
        TabContent() {
        }
      }
    }
  }
}
