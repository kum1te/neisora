<script lang="ts">
  import { onMount } from 'svelte';
  import { visualizerRenderer } from './visualizerRenderer';
  import { audioLevel } from './audio';
  import { invoke } from './backend';
  import type { PlayerState, VisualizerPreferences } from './types';
  let { preferences, player }: { preferences: VisualizerPreferences; player: PlayerState } = $props();
  let canvas = $state<HTMLCanvasElement>();
  let force2d = $state(false);
  let visible = $state(true), reduced = $state(false), ready = $state(false), contextLost = $state(false);
  const enabled = $derived(ready && visible && !reduced && preferences.mode !== 'off');
  const running = $derived(!contextLost && enabled && player.active && !player.paused && !player.buffering);
  onMount(() => {
    const media = matchMedia('(prefers-reduced-motion: reduce)');
    const visibility = () => visible = !document.hidden;
    const motion = () => reduced = media.matches;
    visibility(); motion(); document.addEventListener('visibilitychange', visibility); media.addEventListener('change', motion);
    ready = true;
    return () => {
      document.removeEventListener('visibilitychange', visibility); media.removeEventListener('change', motion);
      // Detached WebKit canvases can retain native drawing surfaces until JS GC.
      // Release the context only on unmount; ordinary playback/theme changes reuse it.
      if (canvas) { canvas.width = canvas.height = 1; canvas.getContext('webgl')?.getExtension('WEBGL_lose_context')?.loseContext(); }
      void invoke('set_visualizer', { enabled: false }).catch(() => {});
    };
  });
  $effect(() => {
    const element = canvas;
    if (!element) return;
    contextLost = false;
    const lost = (event: Event) => { event.preventDefault(); contextLost = true; };
    const restored = () => { contextLost = false; };
    element.addEventListener('webglcontextlost', lost);
    element.addEventListener('webglcontextrestored', restored);
    return () => { element.removeEventListener('webglcontextlost', lost); element.removeEventListener('webglcontextrestored', restored); };
  });
  $effect(() => { const active = enabled; if (ready) void invoke('set_visualizer', { enabled: active }).catch(() => {}); });
  $effect(() => {
    if (contextLost) return;
    const active = running;
    const mode = preferences.mode, intensity = preferences.intensity, speed = preferences.speed;
    const element = canvas, fallback = force2d;
    if (!element) return;
    const ctx = (() => {
      try { return visualizerRenderer(element, fallback); }
      catch { if (!fallback) force2d = true; return null; }
    })();
    if (!ctx) return;
    let frame = 0, last = 0, phase = 0, level = 0, attack = 0;
    // Match the actual display instead of stretching a low-resolution bitmap.
    let width = 0, height = 0, pixelRatio = 0;
    const resize = () => {
      width = element.clientWidth; height = element.clientHeight;
      pixelRatio = window.devicePixelRatio || 1;
      element.width = Math.max(1, Math.round(width * pixelRatio));
      element.height = Math.max(1, Math.round(height * pixelRatio));
      ctx.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
    };
    resize(); const observer = new ResizeObserver(resize); observer.observe(element);
    let color = getComputedStyle(element).getPropertyValue('--accent').trim() || '#d5c6ff';
    const theme = new MutationObserver(() => { color = getComputedStyle(element).getPropertyValue('--accent').trim() || '#d5c6ff'; });
    theme.observe(document.documentElement, { attributes: true, attributeFilter: ['style'] });
    function draw(now: number) {
      if (!ctx) return;
      if (pixelRatio !== (window.devicePixelRatio || 1)) resize();
      const dt = last ? Math.min(.08, (now - last) / 1000) : 1 / 60; last = now;
      const audio = audioLevel(), live = now - audio.received < 450;
      const wanted = live ? audio.rms : 0;
      level += (wanted - level) * (1 - Math.exp(-dt / (wanted > level ? .05 : .22)));
      attack += ((live ? audio.peak : 0) - attack) * (1 - Math.exp(-dt / .16));
      phase += dt * speed * (.35 + level);
      const w = width, h = height, strokeScale = w / 640;
      ctx.clearRect(0, 0, w, h); ctx.strokeStyle = color; ctx.lineCap = 'round';
      const opacity = intensity * (.1 + level * .28);
      if (mode === 'rings') {
        const rings = 5, points = 256;
        for (let ring = 0; ring < rings; ring++) {
          ctx.beginPath(); ctx.lineWidth = (1.5 + attack * 2) * strokeScale; ctx.globalAlpha = opacity * (1 - ring * .12);
          for (let n = 0; n <= points; n++) {
            const angle = n / points * Math.PI * 2;
            const radius = Math.min(w, h) * (.22 + ring * .055) + Math.sin(angle * 5 + phase * 2 + ring) * level * h * .035;
            const x = w * .36 + Math.cos(angle) * radius, y = h * .55 + Math.sin(angle) * radius;
            if (!n) ctx.moveTo(x, y); else ctx.lineTo(x, y);
          } ctx.closePath(); ctx.stroke();
        }
      } else {
        const count = mode === 'ribbon' ? 8 : 4, points = 160;
        for (let line = 0; line < count; line++) {
          ctx.beginPath(); ctx.lineWidth = (mode === 'ribbon' ? 2 : 2.5) * strokeScale; ctx.globalAlpha = opacity * (1 - line / (count + 2));
          for (let n = 0; n <= points; n++) {
            const x = n / points * w, t = n / points;
            const envelope = Math.sin(t * Math.PI);
            const wave = Math.sin(t * Math.PI * (mode === 'ribbon' ? 3 : 5) - phase * 2 + line * .4);
            const y = h * (.64 + line * .017) + wave * envelope * h * (level * .245) + Math.sin(t * 9 + phase + line * .15) * attack * h * .04;
            if (!n) ctx.moveTo(x, y); else ctx.lineTo(x, y);
          } ctx.stroke();
        }
      }
      frame = requestAnimationFrame(draw);
    }
    if (active) frame = requestAnimationFrame(draw); else ctx.clearRect(0, 0, element.width, element.height);
    return () => { cancelAnimationFrame(frame); observer.disconnect(); theme.disconnect(); ctx.clearRect(0, 0, element.width, element.height); ctx.dispose(); };
  });
</script>
{#key force2d}<canvas class="audio-visualizer" aria-hidden="true" bind:this={canvas}></canvas>{/key}
