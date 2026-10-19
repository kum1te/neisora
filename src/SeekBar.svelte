<script lang="ts">
  let { position, duration, label, onseek }: { position: number; duration: number; label: string; onseek: (value: number) => void } = $props();
  let dragging = $state(false), preview = $state(0);
  const value = $derived(dragging ? preview : position);
  function commit(value: number) {
    preview = value;
    onseek(value);
    dragging = false;
  }
</script>
<input class="seek-range" type="range" min="0" max={duration || 1} step=".1" value={value} disabled={!duration} aria-label={label}
  onpointerdown={e => { dragging = true; preview = +e.currentTarget.value; }}
  oninput={e => { preview = +e.currentTarget.value; }}
  onchange={e => commit(+e.currentTarget.value)}
  onpointercancel={() => dragging = false}
  onblur={() => dragging = false}
  style={`--progress:${duration ? value / duration * 100 : 0}%`}/>
