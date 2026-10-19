export type RepeatMode = 'off' | 'all' | 'one';
export function readRepeat(value: string | null): RepeatMode {
  return value === 'all' || value === 'one' ? value : 'off';
}
export function cycleRepeat(mode: RepeatMode): RepeatMode {
  return mode === 'off' ? 'all' : mode === 'all' ? 'one' : 'off';
}
/** Explicit skips ignore repeat-one; a live radio is never rewound as a finite playlist. */
export function repeatTarget(mode: RepeatMode, ended: boolean, index: number, length: number, dynamic: boolean, hasContinuation: boolean): number | null {
  if (!length) return null;
  if (ended && mode === 'one') return index;
  if (index + 1 < length) return index + 1;
  return mode === 'all' && !dynamic && !hasContinuation ? 0 : null;
}

/** Resume only a restored session; Play at EOF must restart the track. */
export function resumePosition(restored:boolean,position:number,duration:number):number {
  return restored&&Number.isFinite(position)&&position>=0&&(!Number.isFinite(duration)||duration<=0||position<duration-.5)?position:0;
}
