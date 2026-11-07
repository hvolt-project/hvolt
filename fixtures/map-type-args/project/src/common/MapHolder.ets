/**
 * 保存一份带类型的缓存
 */
@Observed
export class MapHolder {
  private cache: Map<string, number> = new Map<string, number>([['seed', 1]])

  total(): number {
    return this.cache.size
  }
}
