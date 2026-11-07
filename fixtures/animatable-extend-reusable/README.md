# animatable-extend-reusable

An `@AnimatableExtend` attribute inside a `@Reusable` view: the call is a free function that
branches on `isInitialRender` itself, so it must not be wrapped in the recycled view's
`if (isInitialRender)` attribute guard. This fixture pins its position relative to that guard.
