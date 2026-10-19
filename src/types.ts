export type Endpoint = Record<string, unknown>;
export interface Item {
  id: string; title: string; subtitle: string; image: string; kind: string;
  endpoint: Endpoint; play: Endpoint | null; duration: string; selected: boolean;
  source?: 'local' | null; album?: string | null; available?: boolean; localRevision?: number; manual?: boolean; queueKey?: string;
  shuffleOrder?: number; setVideoId?: string; queuePosition?: number; isMusicVideo?: boolean; liked?: boolean | null;
}
export interface Section { title: string; subtitle: string; items: Item[] }
export interface HistoryEntry { item: Item; playedAt: number }
export interface Collection { browseId: string; playlistId: string; saved: boolean | null; editable?: boolean; description?: string; privacy?: string }
export interface Feed { collection?: Collection | null; title: string; sections: Section[]; continuation: Endpoint | null; authenticated: boolean }
export interface LocalFolder { id:string; path:string; available:boolean }
export interface LocalStatus { folders:LocalFolder[]; total:number; scanning:boolean; warning:string }
export interface LocalPage { items:Item[]; total:number; artists:string[]; albums:string[]; status:LocalStatus }
export interface Queue { shuffled?: boolean; excluded?:string[]; updateSection?: string | null; title: string; items: Item[]; continuation: Endpoint | null; context: Endpoint; selected: number; dynamic: boolean }
export interface Status { authenticated: boolean; browser: string[]; mpv: boolean; extractor: boolean; volume: number; safeRenderer?: boolean; version?: string }
export interface PlayerState { position: number; duration: number; paused: boolean; volume: number; buffering: boolean; active: boolean }
export interface LyricWord { time: number; end: number; text: string }
export interface LyricLine { time: number; text: string; words?: LyricWord[] }
export interface Lyrics { lines: LyricLine[]; plain: string; source: string; synced: boolean }
export interface Theme { id: string; name: string; accent: string; background: string; surface: string; text: string }
export interface VisualizerPreferences { mode: 'off' | 'ribbon' | 'rings' | 'waves'; intensity: number; speed: number }
export interface Preferences { selected: string; custom: Theme[]; followWallpaper: boolean; visualizer: VisualizerPreferences }
export interface AppearanceState { preferences: Preferences; wallpaper: Theme | null }

export interface SleepTimerState {mode:'off'|'track'|'minutes';deadlineMs:number|null}

export interface AudioOutputState {preferred:string;selected:string;fallback:boolean;silent:boolean;devices:{name:string;description:string}[]}
export interface LyricCandidate {id:number;title:string;artist:string;album:string;duration:number;synced:boolean;wordTimed:boolean}
export interface LyricSelection {record:LyricCandidate;lyrics:Lyrics|null}
