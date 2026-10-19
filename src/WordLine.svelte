<script lang="ts">
  import type { LyricWord } from './types';
  let { words, position, paused, buffering }: { words: LyricWord[]; position: number; paused: boolean; buffering: boolean } = $props();
  let element: HTMLSpanElement;
  $effect(() => {
    const anchor = performance.now(), base = position, running = !paused && !buffering;
    const nodes = element?.querySelectorAll<HTMLElement>('.lyric-word');
    if (!nodes) return;
    let frame = 0, last = -Infinity;
    const tick = (now: number) => {
      if (now - last >= 32) {
        last = now;
        const clock = base + (running ? Math.min(.5, (now - anchor) / 1000) : 0);
        words.forEach((word, n) => {
          const value = `${Math.max(0, Math.min(100, (clock - word.time) / Math.max(.01, word.end - word.time) * 100))}%`;
          // Only the currently changing word invalidates style; completed words stay cached.
          if (nodes[n]?.style.getPropertyValue('--word-progress') !== value) nodes[n]?.style.setProperty('--word-progress', value);
        });
      }
      if (running && now - anchor < 500 && !document.hidden) frame = requestAnimationFrame(tick);
    };
    tick(anchor);
    return () => cancelAnimationFrame(frame);
  });
</script>
<span class="word-line" bind:this={element}>{#each words as word}<span class="lyric-word" style="--word-progress:0%"><span>{word.text}</span><span class="word-fill" aria-hidden="true">{word.text}</span></span>{/each}</span>
