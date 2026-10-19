<script lang="ts">
  import { onMount } from 'svelte';
  let { value, onvolume }: { value: number; onvolume: (value: number, commit: boolean) => Promise<void> } = $props();
  let editing = $state(false), draft = $state(0), busy = false;
  let pending: { value: number; commit: boolean } | null = null;
  const displayed = $derived(Math.round(editing ? draft : value));
  async function send(value: number, commit = false) {
    pending = { value, commit: commit || !!pending?.commit };
    if (busy) return;
    busy = true;
    try { while (pending) { const update = pending; pending = null; await onvolume(update.value, update.commit); } }
    finally { busy = false; }
  }
  function input(event: Event) { draft = +(event.currentTarget as HTMLInputElement).value; void send(draft); }
  function finish() { if (editing) { void send(draft, true); editing = false; } }
  onMount(() => {
    window.addEventListener('pointerup', finish); window.addEventListener('pointercancel', finish);
    return () => { window.removeEventListener('pointerup', finish); window.removeEventListener('pointercancel', finish); };
  });
</script>
<div class="volume-slider" class:adjusting={editing} style={`--progress:${displayed}%;--fraction:${displayed / 100}`}>
  {#if editing}<output class="volume-percent">{displayed}%</output>{/if}
  <input type="range" min="0" max="100" step="1" value={displayed} aria-label="Громкость" aria-valuetext={`${displayed}%`}
    onpointerdown={() => { draft = value; editing = true; }} oninput={input}
    onkeydown={e => { if (['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End', 'PageUp', 'PageDown'].includes(e.key)) { if (!editing) draft = value; editing = true; } }}
    onkeyup={finish} onblur={finish} onchange={() => void send(draft, true)}/>
</div>
