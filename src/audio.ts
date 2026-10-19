/** One app-wide native event subscription; opening the player creates no IPC listeners. */
let level = { rms: 0, peak: 0, received: -Infinity };
const bounded = (n: number) => Number.isFinite(n) ? Math.max(0, Math.min(1, n)) : 0;
export function updateAudioLevel(value: { rms: number; peak: number }) {
  level = { rms: bounded(value.rms), peak: bounded(value.peak), received: performance.now() };
}
export function audioLevel() { return level; }
