import { invoke } from './backend';
import type { Item } from './types';
const cache = new Map<string,Promise<string | null>>();
export function localArtwork(item: Item, large = false): Promise<string | null> {
  const key = `${item.id}:${item.localRevision ?? 0}:${large}`;
  let value = cache.get(key);
  if (!value) {
    value = invoke<string | null>('local_artwork',{id:item.id,large}).catch(() => null);
    if (cache.size >= 128) cache.delete(cache.keys().next().value!);
    cache.set(key,value);
  }
  return value;
}
