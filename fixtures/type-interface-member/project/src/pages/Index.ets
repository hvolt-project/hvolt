interface WidgetMetrics {
  width: number;

  height: number;
}

function useMetrics(metrics: WidgetMetrics) {
  return `${metrics.width}${metrics.notAReal}`;
}

@Entry
@Component
struct TypeInterfaceMember {
  build() {
    Column() {
      Text("x")
    }
  }
}
