<script lang="ts">
  import { Music2 } from 'lucide-svelte';
  import { localArtwork } from './localArtwork';
  import type { Item } from './types';
  let {item}: {item:Item} = $props();
  let element = $state<HTMLElement>(), image = $state<string | null>(null);
  $effect(() => {
    const target = element, track = item;
    image = null;
    if (!target) return;
    let active = true;
    const observer = new IntersectionObserver(entries => {
      if (!entries.some(e=>e.isIntersecting)) return;
      observer.disconnect();
      void localArtwork(track).then(value=>{if(active)image=value;});
    },{rootMargin:'100px'});
    observer.observe(target);
    return () => {active=false;observer.disconnect();};
  });
</script>
<span class="local-artwork" bind:this={element}>{#if image}<img src={image} alt=""/>{:else}<Music2 size={20}/>{/if}</span>
<style>.local-artwork{display:grid;place-items:center;width:100%;height:100%;overflow:hidden;border-radius:inherit;color:var(--muted);background:color-mix(in srgb,var(--text) 4%,transparent)}.local-artwork img{display:block;object-fit:cover;width:100%;height:100%}</style>
