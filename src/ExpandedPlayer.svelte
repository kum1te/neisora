<script lang="ts">
  import { flushSync, onDestroy } from 'svelte';
  import { fly } from 'svelte/transition';
  import { invoke } from './backend';
  import { X, Play, Pause, SkipBack, SkipForward, LoaderCircle, Music2, ListMusic, RefreshCw, Captions, ListPlus, ListFilter } from 'lucide-svelte';
  import SeekBar from './SeekBar.svelte';
  import RepeatButton from './RepeatButton.svelte';
  import ShuffleButton from './ShuffleButton.svelte';
  import type { RepeatMode } from './playback';
  import Visualizer from './Visualizer.svelte';
  import LyricVersions from './LyricVersions.svelte';
  import WordLine from './WordLine.svelte';
  import type { Item, Lyrics, LyricSelection, LyricCandidate, PlayerState, VisualizerPreferences } from './types';
  import { smoothScroll, centerLine } from './scroll';
  let { track, cover, player, resolving, onclose, ontoggle, onprevious, onnext, onseek, onqueue, visualizer, repeatMode, onrepeat, onplaylist, shuffled, shuffleDisabled, shuffleBusy, onshuffle }: { track: Item; cover?: string; player: PlayerState; resolving: boolean; onclose: () => void; ontoggle: () => void; onprevious: () => void; onnext: () => void; onseek: (time: number) => void; onqueue: () => void; shuffled:boolean;shuffleDisabled:boolean;shuffleBusy:boolean;onshuffle:()=>void;repeatMode: RepeatMode; onrepeat: () => void; onplaylist: () => void; visualizer: VisualizerPreferences } = $props();
  let lyrics = $state<Lyrics | null>(null), loading = $state(false), error = $state('');
  let chooser=$state(false),choice=$state<LyricCandidate|null>(null);
  let showLyrics=$state(localStorage.getItem('player:lyrics-visible')!=='false');
  let controls: HTMLElement | undefined;
  let layoutAnimation: Animation | undefined;
  function toggleLyrics() {
    // Measure the current visual position before cancelling, so rapid toggles reverse smoothly.
    const before = controls?.getBoundingClientRect();
    layoutAnimation?.cancel();
    flushSync(() => { showLyrics = !showLyrics; });
    localStorage.setItem('player:lyrics-visible', String(showLyrics));
    const after = controls?.getBoundingClientRect();
    if (!controls || !before || !after || !after.width || !after.height || matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    const animation = controls.animate([
      { transform: `translate(${before.left-after.left}px, ${before.top-after.top}px) scale(${before.width/after.width}, ${before.height/after.height})` },
      { transform: 'none' }
    ], { duration: 420, easing: 'cubic-bezier(.22,1,.36,1)' });
    layoutAnimation = animation;
    animation.onfinish = () => { if (layoutAnimation === animation) layoutAnimation = undefined; };
  }
  let panel = $state<HTMLDivElement>();
  let manualUntil = 0, generation = 0, loaded = '', retry = $state(0);
  let offset = $state(0);
  let resumeTimer: ReturnType<typeof setTimeout>;
  function holdScroll() {
    manualUntil = Date.now() + 3500; clearTimeout(resumeTimer);
    resumeTimer = setTimeout(() => { manualUntil = 0; if (panel && active >= 0) { const line = panel.querySelector<HTMLElement>(`[data-line="${active}"]`); if (line) centerLine(panel, line); } }, 3550);
  }
  function offsetKey(){return `lyrics-offset:${track.id}${choice?`:lrclib:${choice.id}`:''}`;}
  function readOffset(){const value=Number(localStorage.getItem(offsetKey()));return Number.isFinite(value)?Math.max(-30,Math.min(30,value)):0;}
  function chosen(value:LyricSelection|null){generation++;choice=value?.record??null;lyrics=value?.lyrics??null;error=value&&!value.lyrics?'Сохранённая версия сейчас недоступна. Выбери другую или верни автоматический подбор.':'';loading=false;offset=readOffset();if(!value){loaded='';retry++;}}
  function shift(value: number) { offset = Math.round(Math.max(-30, Math.min(30, offset + value)) * 10) / 10; localStorage.setItem(offsetKey(), String(offset)); }
  onDestroy(() => { generation++; clearTimeout(resumeTimer); layoutAnimation?.cancel(); });
  const active = $derived.by(() => {
    if (!showLyrics || !lyrics?.synced) return -1;
    let low = 0, high = lyrics.lines.length;
    while (low < high) { const mid = (low + high) >>> 1; if (lyrics.lines[mid].time <= player.position - offset + .08) low = mid + 1; else high = mid; }
    return low - 1;
  });
  function time(v: number) { const n = Math.max(0, Math.floor(v || 0)); return `${Math.floor(n / 60)}:${String(n % 60).padStart(2, '0')}`; }
  $effect(() => {
    track.id; generation++; loaded = ''; lyrics = null; loading = true; error = '';chooser=false;choice=null;
    manualUntil = 0; retry = 0; clearTimeout(resumeTimer);
    const saved = Number(localStorage.getItem(`lyrics-offset:${track.id}`)); offset = Number.isFinite(saved) ? Math.max(-30, Math.min(30, saved)) : 0;
  });
  $effect(() => {
    const key = `${track.id}:${retry}`;
    if (!showLyrics || resolving || !player.active || !player.duration || loaded === key) return;
    loaded = key; const version = ++generation;
    lyrics = null; loading = true; error = '';
    if (track.source === 'local') {
      void invoke<Lyrics>('local_lyrics',{id:track.id}).then(value=>{if(version===generation)lyrics=value;}).catch(e=>{if(version===generation)error=String(e);}).finally(()=>{if(version===generation)loading=false;});
      return;
    }
    const metadata = { id: track.id, title: track.title, artist: track.subtitle, duration: player.duration, musicVideo: track.isMusicVideo ?? /official music video/i.test(track.title), refresh: retry > 0 };
    void (async()=>{
      try {
        const selected=await invoke<LyricSelection|null>('get_lyric_choice',{id:track.id});
        if(version!==generation)return;
        choice=selected?.record??null;offset=readOffset();
        if(selected){lyrics=selected.lyrics;loading=false;if(!lyrics)error='Сохранённая версия сейчас недоступна. Выбери другую или верни автоматический подбор.';return;}
      }catch(e){if(version===generation){error=String(e);loading=false;}return;}
    void invoke<Lyrics | null>('get_word_lyrics', metadata).then(value => {
      if (version === generation && value?.lines.some(line => line.words?.length)) { lyrics = value; loading = false; error = ''; }
    }).catch(() => {});
    void invoke<Lyrics>('get_lyrics', metadata)
      .then(value => { if (version === generation && !lyrics?.lines.some(line => line.words?.length)) lyrics = value; })
      .catch(e => { if (version === generation) error = String(e); })
      .finally(() => { if (version === generation) loading = false; });
    })();
  });
  $effect(() => {
    const line = active;
    if (panel && line >= 0 && Date.now() > manualUntil) {
      const element = panel.querySelector<HTMLElement>(`[data-line="${line}"]`);
      if (element) centerLine(panel, element, matchMedia('(prefers-reduced-motion: reduce)').matches);
    }
  });
</script>

<div class="expanded-player" class:lyrics-hidden={!showLyrics}>
  <Visualizer preferences={visualizer} {player}/>
  <header class="expanded-header"><span class="expanded-wordmark">neisora<span>.</span></span><span class="eyebrow">СЕЙЧАС ИГРАЕТ</span><div class="expanded-header-actions">{#if showLyrics&&track.source!=='local'}<button class="icon" aria-label="Выбрать версию текста" title={choice?`Выбрана версия: ${choice.artist}`:'Выбрать версию текста'} onclick={()=>chooser=true}><ListFilter size={21}/></button>{/if}<button class="icon lyrics-toggle" class:enabled={showLyrics} aria-label="Показывать текст песни" aria-pressed={showLyrics} title={showLyrics?'Скрыть текст песни':'Показать текст песни'} onclick={toggleLyrics}><Captions size={23}/></button><button class="icon" aria-label="Свернуть плеер" onclick={onclose}><X size={23}/></button></div></header>
  <section class="expanded-left" bind:this={controls}><div class="expanded-cover">{#if cover || track.image}<img src={cover || track.image} alt={track.title}/>{:else}<Music2 size={70}/>{/if}</div><div class="expanded-track"><h1>{track.title}</h1><p>{track.subtitle.split('•')[0].trim()}</p></div><div class="expanded-seek"><SeekBar position={player.position} duration={player.duration} label="Позиция трека" {onseek}/><div><span>{time(player.position)}</span><span>{time(player.duration)}</span></div></div><div class="expanded-controls"><ShuffleButton active={shuffled} disabled={shuffleDisabled} busy={shuffleBusy} onchange={onshuffle}/><RepeatButton mode={repeatMode} onchange={onrepeat}/><button class="icon" aria-label="Предыдущий трек" onclick={onprevious}><SkipBack size={25} fill="currentColor"/></button><button class="main-play" aria-label={player.paused ? 'Воспроизвести' : 'Пауза'} disabled={resolving} onclick={ontoggle}>{#if resolving || player.buffering}<LoaderCircle class="spin" size={26}/>{:else if player.paused}<Play size={25} fill="currentColor"/>{:else}<Pause size={25} fill="currentColor"/>{/if}</button><button class="icon" aria-label="Следующий трек" onclick={onnext}><SkipForward size={25} fill="currentColor"/></button>{#if track.source !== 'local'}<button class="icon" aria-label="Добавить текущий трек в плейлист" title="Добавить в плейлист" onclick={onplaylist}><ListPlus size={22}/></button>{/if}<button class="icon expanded-queue" aria-label="Открыть очередь" onclick={onqueue}><ListMusic size={22}/></button></div></section>
  {#if showLyrics}<section class="expanded-right" aria-label="Текст песни" transition:fly={{x:24,duration:matchMedia('(prefers-reduced-motion: reduce)').matches?0:240}}>
    {#if loading}<div class="lyrics-message"><LoaderCircle class="spin" size={25}/><p>Ищем текст</p></div>
    {:else if lyrics?.synced}<div class="lyrics-scroll" role="region" aria-label="Синхронный текст" use:smoothScroll bind:this={panel} onwheel={holdScroll} onpointerdown={holdScroll}><div class="lyrics-spacer"></div>{#each lyrics.lines as line, n}<button data-line={n} class="lyric-line" class:lyric-active={n === active} class:lyric-past={n < active} class:word-timed={n === active && !!line.words?.length} onclick={() => { manualUntil = 0; onseek(Math.max(0, line.time + offset)); }}>{#if n === active && line.words?.length}<WordLine words={line.words} position={player.position - offset} paused={player.paused} buffering={player.buffering}/>{:else}{line.text || '♪'}{/if}</button>{/each}<div class="lyrics-spacer"></div></div><div class="lyrics-source lyrics-timing"><span>{lyrics.source}{choice?' · Твой выбор':''}{#if lyrics.lines.some(line => line.words?.length)} · По словам{/if}</span><div class="lyrics-offset" aria-label="Смещение текста"><button aria-label="Текст раньше на полсекунды" title="Подсвечивать раньше" onclick={() => shift(-.5)}>−</button><button class="offset-reset" title="Сбросить смещение" aria-label="Сбросить смещение текста" onclick={() => shift(-offset)}>{offset > 0 ? '+' : ''}{offset.toFixed(1)} с</button><button aria-label="Текст позже на полсекунды" title="Подсвечивать позже" onclick={() => shift(.5)}>+</button></div></div>
    {:else if lyrics?.plain}<div class="lyrics-scroll plain-lyrics" use:smoothScroll>{lyrics.plain}</div><div class="lyrics-source">{lyrics.source}{choice?' · Твой выбор':''} · Текст без таймкодов <button class="lyrics-retry" onclick={() => retry++}>Искать снова</button></div>
    {:else}<div class="lyrics-message"><Music2 size={32}/><p>{error || 'Текст появится после запуска трека'}</p>{#if error}<button class="secondary" onclick={() => retry++}><RefreshCw size={14}/>Ещё раз</button>{/if}</div>{/if}
  </section>{/if}
</div>

{#if chooser}{#key track.id}<LyricVersions {track} selectedId={choice?.id} onclose={()=>chooser=false} onchanged={chosen}/>{/key}{/if}

<style>
  .expanded-header-actions{display:flex;align-items:center;gap:14px}
  .lyrics-toggle.enabled{color:var(--accent)}
  .expanded-left{grid-column:1;grid-row:1;transform-origin:top left}
  .expanded-right{grid-column:2;grid-row:1;min-width:0}
  .lyrics-hidden .expanded-left{grid-column:1 / -1;justify-self:center;max-width:510px}
  .lyrics-hidden .expanded-right{pointer-events:none}
</style>
