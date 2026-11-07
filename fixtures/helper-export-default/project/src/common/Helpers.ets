@Builder
function header() {
  Text('header')
}

export default header

@Extend(Text)
function loud() {
  .fontWeight(FontWeight.Bold)
}
