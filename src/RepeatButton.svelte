<script lang="ts">
  import { Repeat, Repeat1 } from 'lucide-svelte';
  import type { RepeatMode } from './playback';
  let { mode, disabled = false, onchange }: { mode: RepeatMode; disabled?: boolean; onchange: () => void } = $props();
  const label = $derived(mode === 'one' ? 'Повтор одного трека' : mode === 'all' ? 'Повтор очереди' : 'Повтор выключен');
</script>
<button class="icon repeat-button" class:enabled={mode !== 'off'} {disabled} aria-label={label} aria-pressed={mode !== 'off'} title={`${label} · Нажми, чтобы переключить`} onclick={onchange}>
  {#if mode === 'one'}<Repeat1 size={19}/>{:else}<Repeat size={19}/>{/if}
</button>
