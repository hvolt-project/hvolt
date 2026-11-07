import { InsightIntentLink } from '@ohos.app.ability.InsightIntentDecorator';

@InsightIntentLink({
  intentName: 'PlayMusic',
  domain: 'music',
  intentVersion: '1.0.0',
  displayName: '播放音乐',
  uri: 'music://play'
})
@Entry
@Component
struct IntentView {
  build() {
    Column() {
      Text('intent')
    }
  }
}
