/** Feather-style inline SVG icons for Gullbúr Enclave. 16x16 viewBox, 1.5px stroke. */

const S = (d: string) =>
  `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">${d}</svg>`;

export const ICONS = {
  wallet: S('<path d="M21 12v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4m18 0V8a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v4m18 0h-3a2 2 0 0 0-2 2v0a2 2 0 0 0 2 2h3M13 8h2"/>'),
  globe: S('<path d="M22 12c0 5.52-4.48 10-10 10S2 17.52 2 12 6.48 2 12 2s10 4.48 10 10z"/><path d="M2 12h20"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>'),
  bolt: S('<path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z"/>'),
  layout: S('<rect x="3" y="3" width="18" height="18" rx="2" ry="2"/><line x1="3" y1="9" x2="21" y2="9"/><line x1="9" y1="21" x2="9" y2="9"/>'),
  send: S('<line x1="12" y1="5" x2="12" y2="19"/><polyline points="19 12 12 5 5 12"/>'),
  receive: S('<line x1="12" y1="5" x2="12" y2="19"/><polyline points="5 12 12 19 19 12"/>'),
  settings: S('<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>'),
  lock: S('<rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>'),
  close: S('<line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>'),
  plus: S('<line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>'),
  refresh: S('<polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>'),
  scan: S('<path d="M3 7V5a2 2 0 0 1 2-2h2"/><path d="M17 3h2a2 2 0 0 1 2 2v2"/><path d="M21 17v2a2 2 0 0 1-2 2h-2"/><path d="M7 21H5a2 2 0 0 1-2-2v-2"/><line x1="7" y1="12" x2="17" y2="12"/>'),
  book: S('<path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/>'),
  check: S('<polyline points="20 6 9 17 4 12"/>'),
  copy: S('<rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>'),
  chevronRight: S('<polyline points="9 18 15 12 9 6"/>'),
  alertCircle: S('<circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/>'),
  externalLink: S('<path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/>'),
  disconnect: S('<line x1="1" y1="1" x2="23" y2="23"/><path d="M16.72 7.76A6.5 6.5 0 0 1 21 12a6.5 6.5 0 0 1-5.18 6.33"/><path d="M6.22 6.22A6.5 6.5 0 0 0 3 12a6.5 6.5 0 0 0 4.31 6.1"/><path d="M12 15a3 3 0 0 0 2.12-5.12"/><line x1="12" y1="2" x2="12" y2="6"/><line x1="12" y1="18" x2="12" y2="22"/>'),
};

/** Render an icon as HTML string (for svelte {@html}). Pass optional class. */
export function icon(name: keyof typeof ICONS, cls = ''): string {
  return ICONS[name].replace('<svg', `<svg class="${cls}"`);
}

/** Render an inline SVG component snippet. Usage: {@html iconHtml('wallet')} */
export const iconHtml = icon;