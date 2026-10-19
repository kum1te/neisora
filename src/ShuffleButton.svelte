<script lang="ts">
  import { Shuffle, LoaderCircle } from 'lucide-svelte';
  let {active=false,disabled=false,busy=false,onchange}:{active?:boolean;disabled?:boolean;busy?:boolean;onchange:()=>void}=$props();
</script>
<button class="icon shuffle-button" class:enabled={active} disabled={disabled||busy} aria-pressed={active} aria-label={active?'Выключить перемешивание':'Перемешать очередь'} title={disabled?'Перемешивание доступно для обычных и локальных очередей':busy?'Загружаем оставшиеся треки…':active?'Вернуть порядок следующих треков':'Перемешать следующие треки'} onclick={onchange}>{#if busy}<LoaderCircle size={18} class="spin"/>{:else}<Shuffle size={19}/>{/if}</button>
