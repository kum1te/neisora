<script lang="ts">
  import { Check, Plus, Trash2, Image, Palette } from 'lucide-svelte';
  import { presets } from './themes';
  import type { Preferences, Theme, VisualizerPreferences } from './types';
  let { preferences, wallpaper, onupdate }: { preferences: Preferences; wallpaper: Theme | null; onupdate: (preferences: Preferences) => void } = $props();
  let editor = $state(false);
  let draft = $state<Theme>({ ...presets[0], id: '', name: 'Моя тема' });
  function setVisualizer(patch: Partial<VisualizerPreferences>) { onupdate({ ...preferences, visualizer: { ...preferences.visualizer, ...patch } }); }
  function select(id: string) { onupdate({ ...preferences, selected: id, followWallpaper: false }); }
  function create() {
    const selected = [...presets, ...preferences.custom].find(t => t.id === preferences.selected) ?? presets[0];
    draft = { ...selected, id: '', name: 'Моя тема' }; editor = true;
  }
  function save() {
    const theme = { ...draft, name: draft.name.trim() || 'Моя тема', id: `custom-${Date.now()}` };
    onupdate({ ...preferences, custom: [...preferences.custom, theme], selected: theme.id, followWallpaper: false }); editor = false;
  }
</script>

<section class="settings-section appearance-panel" aria-labelledby="appearance-title">
    <div class="settings-section-heading"><span class="settings-symbol"><Palette size={22}/></span><div><h2 id="appearance-title">Оформление</h2><p>Выбери палитру или создай свою.</p></div></div>
    <div class="theme-grid">{#each [...presets, ...preferences.custom] as theme}
      <div class="theme-option"><button class:chosen={preferences.selected === theme.id && !preferences.followWallpaper} class="theme-preview" onclick={() => select(theme.id)} aria-label={`Тема ${theme.name}`} style={`--preview-bg:${theme.background};--preview-surface:${theme.surface};--preview-accent:${theme.accent}`}><span class="preview-sidebar"></span><span class="preview-hero"></span><span class="preview-cards"><i></i><i></i><i></i></span>{#if preferences.selected === theme.id && !preferences.followWallpaper}<span class="theme-check"><Check size={15}/></span>{/if}</button><span class="theme-name">{theme.name}</span>{#if theme.id.startsWith('custom-')}<button class="delete-theme icon" aria-label={`Удалить ${theme.name}`} onclick={() => onupdate({ ...preferences, custom: preferences.custom.filter(t => t.id !== theme.id), selected: preferences.selected === theme.id ? 'violet' : preferences.selected })}><Trash2 size={12}/></button>{/if}</div>
    {/each}<button class="new-theme" onclick={create}><Plus size={23}/><span>Своя тема</span></button></div>
    <div class="wallpaper-toggle"><span><Image size={20}/><span><strong>В цвет обоев</strong><small>{wallpaper ? 'Палитра меняется вместе с рабочим столом' : 'Не удалось определить текущие обои'}</small></span></span><button class="toggle-switch" role="switch" aria-label="В цвет обоев" aria-checked={preferences.followWallpaper} disabled={!wallpaper && !preferences.followWallpaper} onclick={() => onupdate({ ...preferences, followWallpaper: !preferences.followWallpaper })}><span></span></button></div>
    <div class="visualizer-settings"><h3>Фон большого плеера</h3><p>Реагирует на уровень звука. Цвет берётся из выбранной темы.</p><div class="visualizer-modes">{#each [{id:'off',name:'Выключен'},{id:'ribbon',name:'Лента'},{id:'rings',name:'Кольца'},{id:'waves',name:'Волны'}] as mode}<button class:chosen={preferences.visualizer.mode === mode.id} aria-pressed={preferences.visualizer.mode === mode.id} onclick={() => setVisualizer({ mode: mode.id as VisualizerPreferences['mode'] })}>{mode.name}</button>{/each}</div><div class="visualizer-sliders"><label>Интенсивность <output>{Math.round(preferences.visualizer.intensity * 100)}%</output><input type="range" min="0" max="1" step=".05" value={preferences.visualizer.intensity} aria-label="Интенсивность визуализатора" onchange={e => setVisualizer({ intensity: +e.currentTarget.value })}/></label><label>Скорость <output>{preferences.visualizer.speed.toFixed(1)}×</output><input type="range" min=".5" max="2" step=".1" value={preferences.visualizer.speed} aria-label="Скорость визуализатора" onchange={e => setVisualizer({ speed: +e.currentTarget.value })}/></label></div></div>
    {#if editor}<form class="theme-editor" onsubmit={e => { e.preventDefault(); save(); }}><label class="theme-name-field">Название<input maxlength="40" bind:value={draft.name}/></label><div class="color-fields">{#each [{ key: 'accent', label: 'Акцент' }, { key: 'background', label: 'Фон' }, { key: 'surface', label: 'Панели' }, { key: 'text', label: 'Текст' }] as field}<label><input type="color" value={draft[field.key as keyof Theme]} oninput={e => draft = { ...draft, [field.key]: e.currentTarget.value }}/><span>{field.label}</span></label>{/each}</div><div class="editor-actions"><button class="secondary" type="button" onclick={() => editor = false}>Отмена</button><button class="primary" type="submit">Сохранить тему</button></div></form>{/if}

</section>
