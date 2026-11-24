@Observed
class TrackedModel {
  @Track value: number = 1;
}

@Entry
@Component
struct TrackProbe {
  model: TrackedModel = new TrackedModel();

  build() {
    Column() {
      Text(`${this.model.value}`)
    }
  }
}
