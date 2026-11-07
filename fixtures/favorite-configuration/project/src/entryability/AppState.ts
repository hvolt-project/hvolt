export class AppConfig {
  currentProfileId: string = ''
}

export class UIConfig {
  isVibrate: boolean = false
}

export function isEnd(size: number, index: number): boolean {
  return index === size - 1
}
