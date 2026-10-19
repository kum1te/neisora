/** Coalesce wheel detents into one short animation, including nested horizontal shelves. */
export function smoothScroll(node: HTMLElement, enabled = true) {
  let frame = 0, target = node.scrollTop, written = target, previous = 0;
  const reduced = matchMedia('(prefers-reduced-motion: reduce)');
  function stop() { cancelAnimationFrame(frame); frame = 0; previous = 0; target = node.scrollTop; }
  function tick(now: number) {
    if (!enabled || document.hidden || Math.abs(node.scrollTop - written) > 1) { stop(); return; }
    target = Math.max(0, Math.min(target, node.scrollHeight - node.clientHeight));
    const before = node.scrollTop, distance = target - before;
    const dt = previous ? Math.min(40, now - previous) : 16.7; previous = now;
    written = Math.abs(distance) < .6 ? target : node.scrollTop + distance * (1 - Math.exp(-dt / 48));
    node.scrollTo({ top: written, behavior: 'instant' });
    written = node.scrollTop; // WebKit may quantize positions; never compare to a fractional write.
    if (Math.abs(target - written) < .6 || written === before) { node.scrollTo({ top: target, behavior: 'instant' }); stop(); }
    else frame = requestAnimationFrame(tick);
  }
  function wheel(event: WheelEvent) {
    if (!enabled || event.ctrlKey || event.shiftKey || Math.abs(event.deltaX) > Math.abs(event.deltaY) || !event.deltaY) return;
    event.preventDefault();
    const delta = event.deltaY * (event.deltaMode === 1 ? 32 : event.deltaMode === 2 ? node.clientHeight * .85 : 1);
    if (!frame) target = written = node.scrollTop;
    target = Math.max(0, Math.min(target + delta, node.scrollHeight - node.clientHeight));
    if (reduced.matches) { node.scrollTop = target; written = node.scrollTop; return; }
    if (!frame && Math.abs(target - written) > .6) frame = requestAnimationFrame(tick);
  }
  function update(value: boolean) { stop(); enabled = value; node.style.overflowY = value ? '' : 'hidden'; }
  node.addEventListener('wheel', wheel, { passive: false });
  node.addEventListener('pointerdown', stop);
  document.addEventListener('visibilitychange', stop);
  update(enabled);
  return { update, destroy() { stop(); node.removeEventListener('wheel', wheel); node.removeEventListener('pointerdown', stop); document.removeEventListener('visibilitychange', stop); node.style.overflowY = ''; } };
}
export function centerLine(container: HTMLElement, line: HTMLElement, reduced = false) {
  const top = container.scrollTop + line.getBoundingClientRect().top - container.getBoundingClientRect().top - container.clientHeight / 2 + line.clientHeight / 2;
  container.scrollTo({ top: Math.max(0, top), behavior: reduced ? 'instant' : 'smooth' });
}
