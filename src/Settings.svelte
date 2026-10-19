<script lang="ts">
  import { Check, LogIn, LogOut, LoaderCircle, UserRound, Moon } from 'lucide-svelte';
  import AudioOutputs from './AudioOutputs.svelte';
  import Diagnostics from './Diagnostics.svelte';
  import MusicFolders from './MusicFolders.svelte';
  import Appearance from './Appearance.svelte';
  import type { Status, Preferences, Theme, SleepTimerState } from './types';
  let { status, authBusy, preferences, wallpaper, sleepState, onsleep, onlogin, onimport, onlogout, onappearance, onlocal }: {
    sleepState:SleepTimerState;onsleep:()=>void;status: Status | null; authBusy: boolean; preferences: Preferences; wallpaper: Theme | null;
    onlocal: () => void; onlogin: () => void; onimport: (browser: string) => void; onlogout: () => void; onappearance: (preferences: Preferences) => void;
  } = $props();
</script>
<div class="settings-page">
  <header class="settings-heading"><span class="eyebrow">ТВОЯ NEISORA</span><h1>Настройки</h1><p>Аккаунт, локальная музыка и оформление.</p></header>
  <section class="settings-section account-section" aria-labelledby="account-title">
    <div class="settings-section-heading"><span class="settings-symbol"><UserRound size={22}/></span><div><h2 id="account-title">Аккаунт</h2><p>Твои подборки, медиатека и миксы YouTube Music.</p></div><span class="connection-state" class:connected={status?.authenticated}>{#if status?.authenticated}<Check size={14}/>Подключён{:else}Не подключён{/if}</span></div>
    {#if status && !status.mpv}<p class="settings-warning">Не найден mpv. Установи его через пакетный менеджер своего дистрибутива.</p>{/if}
    {#if status && !status.extractor}<p class="settings-warning">Не найдены компоненты yt-dlp и Node. Переустанови neisora из полного пакета.</p>{/if}
    <div class="account-actions"><button class="primary" onclick={onlogin}><LogIn size={16}/>{status?.authenticated ? 'Обновить вход' : 'Войти через Google'}</button>{#if status?.authenticated}<button class="secondary disconnect-button" onclick={onlogout}><LogOut size={15}/>Отключить аккаунт</button>{/if}</div>
    {#if status?.browser.length}<details class="browser-import"><summary>Подключить вход из браузера</summary><p>Сначала войди в YouTube Music в браузере, затем выбери его здесь.</p><div class="browser-buttons">{#each status.browser as browser}<button class="secondary" disabled={authBusy} onclick={() => onimport(browser)}>{#if authBusy}<LoaderCircle size={14} class="spin"/>{/if}{browser[0].toUpperCase() + browser.slice(1)}</button>{/each}</div></details>{/if}
    <p class="account-privacy">Сессия хранится на этом компьютере. Пароль вводится на странице Google.</p>
  </section>
  <section class="settings-section sleep-settings"><div class="settings-section-heading"><span class="settings-symbol"><Moon size={22}/></span><div><h2>Таймер сна</h2><p>{sleepState.mode==='track'?'После текущего трека':sleepState.deadlineMs?`До ${new Date(sleepState.deadlineMs).toLocaleTimeString('ru-RU',{hour:'2-digit',minute:'2-digit'})}`:'Остановить музыку через время или после трека'}</p></div></div><button class="secondary" onclick={onsleep}>Настроить таймер</button></section>
  <AudioOutputs/>
  <MusicFolders onopen={onlocal}/>
  <Appearance {preferences} {wallpaper} onupdate={onappearance}/>
  <Diagnostics/>
  <details class="keyboard-help"><summary>Горячие клавиши</summary><dl><dt>Пробел</dt><dd>Пауза / воспроизведение</dd><dt>← / →</dt><dd>Перемотка на 5 секунд</dd><dt>R</dt><dd>Переключить повтор</dd><dt>M</dt><dd>Выключить / вернуть звук</dd><dt>Ctrl / ⌘ + K</dt><dd>Поиск музыки</dd><dt>Esc</dt><dd>Закрыть плеер, очередь или меню</dd></dl><p>Клавиши плеера не мешают вводу текста. Системные медиаклавиши работают и без фокуса окна.</p></details>
  {#if status?.version}<footer class="settings-about"><span>neisora. {status.version}</span>{#if status.safeRenderer}<span>Совместимый режим графики</span>{/if}</footer>{/if}
</div>
<style>.keyboard-help{padding:20px 0;color:var(--muted);font-size:12px}.keyboard-help summary{cursor:pointer;color:var(--text)}.keyboard-help dl{display:grid;grid-template-columns:minmax(80px,auto) 1fr;gap:12px 20px;margin:20px 0}.keyboard-help dt{color:var(--text)}.keyboard-help dd{margin:0}.keyboard-help p{line-height:1.7}.settings-about{display:flex;justify-content:space-between;gap:16px;color:var(--muted);font-size:11px;padding:24px 0;border-top:1px solid var(--line);margin-top:30px;}</style>
