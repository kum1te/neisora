import type { Theme } from './types';
export const presets: Theme[] = [
  { id: 'violet', name: 'neisora', accent: '#d5c6ff', background: '#0c0d10', surface: '#141318', text: '#f4f3f6' },
  { id: 'ocean', name: 'Прилив', accent: '#8cced8', background: '#0a1014', surface: '#111c22', text: '#eef7f8' },
  { id: 'forest', name: 'Мох', accent: '#b0d19b', background: '#0c110e', surface: '#172018', text: '#f0f5ed' },
  { id: 'rose', name: 'Розовый кварц', accent: '#e5b1be', background: '#140e12', surface: '#23171e', text: '#faf0f4' },
  { id: 'amber', name: 'Закат', accent: '#e5be87', background: '#13100b', surface: '#221b13', text: '#faf4ea' },
  { id: 'mono', name: 'Графит', accent: '#d4d6db', background: '#0e1012', surface: '#1a1d21', text: '#f2f3f5' },
];
export function applyTheme(theme: Theme) {
  const root = document.documentElement;
  for (const [key, color] of Object.entries({ accent: theme.accent, bg: theme.background, surface: theme.surface, text: theme.text })) root.style.setProperty(`--${key}`, color);
  const hex = theme.accent.slice(1);
  const [r,g,b] = [0,2,4].map(n => parseInt(hex.slice(n,n+2),16));
  root.style.setProperty('--on-accent', .2126*r + .7152*g + .0722*b > 145 ? '#18151d' : '#ffffff');
  root.style.setProperty('--muted', `color-mix(in srgb, ${theme.text} 48%, ${theme.surface})`);
}
