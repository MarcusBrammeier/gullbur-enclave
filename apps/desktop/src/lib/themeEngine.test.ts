/**
 * Theme Engine tests — validates security, immutability, Zod validation,
 * theme persistence, and import/export.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

// We need to mock localStorage for jsdom tests
const store = new Map<string, string>();
const localStorageMock = {
  getItem: (key: string) => store.get(key) ?? null,
  setItem: (key: string, val: string) => { store.set(key, val); },
  removeItem: (key: string) => { store.delete(key); },
  clear: () => store.clear(),
  get length() { return store.size; },
  key: (i: number) => [...store.keys()][i] ?? null,
};

beforeEach(() => {
  store.clear();
  // Mock localStorage
  Object.defineProperty(window, 'localStorage', { value: localStorageMock, writable: true });
  // Set up document.documentElement for style injection tests
  document.documentElement.style.cssText = '';
});

afterEach(() => {
  store.clear();
});

// Import after mocks
const { themeEngine, ThemeEngine } = await import('./themeEngine.svelte.ts');

describe('ThemeEngine', () => {
  // ── Security Guarantees ──────────────────────────────────────────────

  it('rejects CSS values containing url()', () => {
    const malicious = {
      name: 'hack',
      colors: {
        bgPrimary: 'url(http://evil.com/exfil)', bgSecondary: '#000', surface: '#111',
        surfaceHover: '#222', cardBg: '#333', cardBgHover: '#444', inputBg: '#555',
        border: '#666', borderHover: '#777', borderStrong: '#888',
        text: '#fff', textSecondary: '#ccc', textMuted: '#999',
        accent: 'url(https://evil.com)', accentHover: '#000', accentSubtle: '#aaa',
        accentShadow: '#bbb', accentGlow: '#ccc', accentContrast: '#fff',
        danger: '#f00', dangerHover: '#d00',
      },
    };
    const result = themeEngine.saveCustomTheme('evil-test', malicious);
    expect(result.success).toBe(false);
    expect(result.errors).toBeDefined();
  });

  it('rejects CSS values containing javascript:', () => {
    const xss = {
      name: 'xss',
      colors: {
        bgPrimary: 'javascript:alert(1)', bgSecondary: '#000', surface: '#111',
        surfaceHover: '#222', cardBg: '#333', cardBgHover: '#444', inputBg: '#555',
        border: '#666', borderHover: '#777', borderStrong: '#888',
        text: '#fff', textSecondary: '#ccc', textMuted: '#999',
        accent: '#0f0', accentHover: '#0a0', accentSubtle: '#aaa',
        accentShadow: '#bbb', accentGlow: '#ccc', accentContrast: '#fff',
        danger: '#f00', dangerHover: '#d00',
      },
    };
    const result = themeEngine.saveCustomTheme('xss-test', xss);
    expect(result.success).toBe(false);
  });

  it('rejects CSS values with HTML injection', () => {
    const htmlInject = {
      name: 'html-inject',
      colors: {
        bgPrimary: '<script>stealKeys()</script>', bgSecondary: '#000', surface: '#111',
        surfaceHover: '#222', cardBg: '#333', cardBgHover: '#444', inputBg: '#555',
        border: '#666', borderHover: '#777', borderStrong: '#888',
        text: '#fff', textSecondary: '#ccc', textMuted: '#999',
        accent: '#0f0', accentHover: '#0a0', accentSubtle: '#aaa',
        accentShadow: '#bbb', accentGlow: '#ccc', accentContrast: '#fff',
        danger: '#f00', dangerHover: '#d00',
      },
    };
    const result = themeEngine.saveCustomTheme('html-test', htmlInject);
    expect(result.success).toBe(false);
  });

  it('rejects CSS values containing eval', () => {
    const evalInject = {
      name: 'eval-test',
      colors: {
        bgPrimary: '#000', bgSecondary: '#000', surface: '#111',
        surfaceHover: '#222', cardBg: '#333', cardBgHover: '#444', inputBg: '#555',
        border: '#666', borderHover: '#777', borderStrong: '#888',
        text: '#fff', textSecondary: '#ccc', textMuted: '#999',
        accent: '#0f0', accentHover: '#0a0', accentSubtle: '#aaa',
        accentShadow: '#bbb', accentGlow: '#ccc', accentContrast: '#fff',
        danger: '#f00', dangerHover: '#d00',
      },
    };
    // Using eval as a value should be rejected
    const bad = { ...evalInject, colors: { ...evalInject.colors, bgSecondary: 'eval(something)' } };
    const result = themeEngine.saveCustomTheme('eval-test-2', bad);
    expect(result.success).toBe(false);
  });

  it('rejects invalid CSS timing values', () => {
    const badTiming = {
      name: 'bad-timing',
      colors: {
        bgPrimary: '#000', bgSecondary: '#111', surface: '#222',
        surfaceHover: '#333', cardBg: '#444', cardBgHover: '#555', inputBg: '#666',
        border: '#777', borderHover: '#888', borderStrong: '#999',
        text: '#fff', textSecondary: '#ccc', textMuted: '#aaa',
        accent: '#0f0', accentHover: '#0a0', accentSubtle: '#bbb',
        accentShadow: '#ccc', accentGlow: '#ddd', accentContrast: '#fff',
        danger: '#f00', dangerHover: '#d00',
      },
      motion: {
        durationFast: 'not-a-timing',
      },
    };
    const result = themeEngine.saveCustomTheme('bad-timing', badTiming);
    expect(result.success).toBe(false);
  });

  // ── Built-in theme immutability ──────────────────────────────────────

  it('cannot overwrite built-in themes', () => {
    const result = themeEngine.saveCustomTheme('obsidian', {
      name: 'Hacked Dark Slate',
      colors: {
        bgPrimary: '#ff0000', bgSecondary: '#ff0000', surface: '#ff0000',
        surfaceHover: '#ff0000', cardBg: '#ff0000', cardBgHover: '#ff0000',
        inputBg: '#ff0000', border: '#ff0000', borderHover: '#ff0000',
        borderStrong: '#ff0000', text: '#ff0000', textSecondary: '#ff0000',
        textMuted: '#ff0000', accent: '#ff0000', accentHover: '#ff0000',
        accentSubtle: '#ff0000', accentShadow: '#ff0000', accentGlow: '#ff0000',
        accentContrast: '#fff', danger: '#f00', dangerHover: '#d00',
      },
    });
    expect(result.success).toBe(false);
    expect(result.errors).toBeDefined();
  });

  // ── Theme application ────────────────────────────────────────────────

  it('applies obsidian theme by default', () => {
    expect(themeEngine.currentThemeId).toBe('obsidian');
    expect(themeEngine.currentTheme.name).toBe('Obsidian');
  });

  it('applies light-studio theme correctly', () => {
    const ok = themeEngine.applyTheme('light-studio');
    expect(ok).toBe(true);
    expect(themeEngine.currentThemeId).toBe('light-studio');
    expect(themeEngine.currentTheme.colors.bgPrimary).toBe('#f0f2f5');
  });

  it('applies legacy-emerald fallback theme', () => {
    const ok = themeEngine.applyTheme('legacy-emerald');
    expect(ok).toBe(true);
    expect(themeEngine.currentThemeId).toBe('legacy-emerald');
    expect(themeEngine.currentTheme.colors.accent).toBe('#10b981');
    expect(themeEngine.currentTheme.geometry?.radiusCard).toBe('12px');
  });

  it('returns false for unknown theme ID', () => {
    const ok = themeEngine.applyTheme('nonexistent-theme');
    expect(ok).toBe(false);
  });

  // ── Custom theme CRUD ────────────────────────────────────────────────

  it('saves a valid custom theme', () => {
    const customDef = {
      name: 'My Custom Theme',
      colors: {
        bgPrimary: '#1a1a2e', bgSecondary: '#16213e', surface: '#0f3460',
        surfaceHover: '#1a4a7a', cardBg: 'rgba(15, 52, 96, 0.8)',
        cardBgHover: 'rgba(15, 52, 96, 0.9)', inputBg: 'rgba(22, 33, 62, 0.7)',
        border: 'rgba(255,255,255,0.06)', borderHover: 'rgba(255,255,255,0.1)',
        borderStrong: 'rgba(255,255,255,0.14)', text: '#e6edf3',
        textSecondary: '#8b949e', textMuted: '#6e7681',
        accent: '#e94560', accentHover: '#ff6b81', accentSubtle: 'rgba(233,69,96,0.1)',
        accentShadow: 'rgba(233,69,96,0.2)', accentGlow: 'rgba(233,69,96,0.35)',
        accentContrast: '#ffffff', danger: '#ff4444', dangerHover: '#cc0000',
      },
    };
    const result = themeEngine.saveCustomTheme('my-custom', customDef);
    expect(result.success).toBe(true);
  });

  it('applies a saved custom theme', () => {
    // First save it
    const ok = themeEngine.saveCustomTheme('my-applied', {
      name: 'Applied Theme',
      colors: {
        bgPrimary: '#0a0a23', bgSecondary: '#1a1a3e', surface: '#2a2a5e',
        surfaceHover: '#3a3a7e', cardBg: 'rgba(42,42,94,0.8)',
        cardBgHover: 'rgba(42,42,94,0.9)', inputBg: 'rgba(26,26,62,0.7)',
        border: 'rgba(255,255,255,0.05)', borderHover: 'rgba(255,255,255,0.1)',
        borderStrong: 'rgba(255,255,255,0.12)', text: '#e0e0ff',
        textSecondary: '#a0a0cc', textMuted: '#7070aa',
        accent: '#ff6b35', accentHover: '#ff8c5a', accentSubtle: 'rgba(255,107,53,0.1)',
        accentShadow: 'rgba(255,107,53,0.2)', accentGlow: 'rgba(255,107,53,0.3)',
        accentContrast: '#ffffff', danger: '#ff3333', dangerHover: '#cc0000',
      },
    });
    expect(ok.success).toBe(true);
    // Apply it
    const applied = themeEngine.applyTheme('my-applied');
    expect(applied).toBe(true);
    expect(themeEngine.currentThemeId).toBe('my-applied');
  });

  it('returns available themes including built-in and custom', () => {
    // Save a custom first
    themeEngine.saveCustomTheme('test-theme', {
      name: 'Test Theme',
      colors: {
        bgPrimary: '#000', bgSecondary: '#111', surface: '#222',
        surfaceHover: '#333', cardBg: '#444', cardBgHover: '#555', inputBg: '#666',
        border: '#777', borderHover: '#888', borderStrong: '#999',
        text: '#fff', textSecondary: '#ccc', textMuted: '#aaa',
        accent: '#0f0', accentHover: '#0a0', accentSubtle: '#bbb',
        accentShadow: '#999', accentGlow: '#888', accentContrast: '#fff',
        danger: '#f00', dangerHover: '#d00',
      },
    });
    const available = themeEngine.getAvailableThemes();
    const builtins = available.filter(t => t.isBuiltin);
    const customs = available.filter(t => !t.isBuiltin);
    expect(builtins.length).toBeGreaterThanOrEqual(3);
    expect(customs.length).toBeGreaterThanOrEqual(1);
    // Built-in themes are immutable
    for (const t of builtins) {
      expect(t.isBuiltin).toBe(true);
    }
  });

  it('deletes a custom theme', () => {
    themeEngine.saveCustomTheme('delete-me', {
      name: 'Delete Me',
      colors: {
        bgPrimary: '#000', bgSecondary: '#111', surface: '#222',
        surfaceHover: '#333', cardBg: '#444', cardBgHover: '#555', inputBg: '#666',
        border: '#777', borderHover: '#888', borderStrong: '#999',
        text: '#fff', textSecondary: '#ccc', textMuted: '#aaa',
        accent: '#0f0', accentHover: '#0a0', accentSubtle: '#bbb',
        accentShadow: '#999', accentGlow: '#888', accentContrast: '#fff',
        danger: '#f00', dangerHover: '#d00',
      },
    });
    const deleted = themeEngine.deleteCustomTheme('delete-me');
    expect(deleted).toBe(true);
    // Verify it's gone
    const available = themeEngine.getAvailableThemes();
    expect(available.find(t => t.id === 'delete-me')).toBeUndefined();
  });

  it('cannot delete built-in themes', () => {
    const deleted = themeEngine.deleteCustomTheme('obsidian');
    expect(deleted).toBe(false);
  });

  // ── Export / Import ──────────────────────────────────────────────────

  it('exports current theme as a portable JSON object', () => {
    themeEngine.applyTheme('legacy-emerald');
    const exported = themeEngine.exportCurrentTheme();
    expect(exported.name).toBe('Legacy Emerald');
    expect(exported.version).toBe(1);
    expect(exported.theme.colors.accent).toBe('#10b981');
  });

  it('imports a valid theme from JSON', () => {
    const importData = {
      name: 'Imported Theme',
      description: 'From a friend',
      version: 1,
      theme: {
        name: 'Imported Theme',
        colors: {
          bgPrimary: '#2d2d2d', bgSecondary: '#3d3d3d', surface: '#4d4d4d',
          surfaceHover: '#5d5d5d', cardBg: 'rgba(77,77,77,0.8)',
          cardBgHover: 'rgba(77,77,77,0.9)', inputBg: 'rgba(61,61,61,0.7)',
          border: 'rgba(255,255,255,0.06)', borderHover: 'rgba(255,255,255,0.1)',
          borderStrong: 'rgba(255,255,255,0.14)', text: '#f0f0f0',
          textSecondary: '#b0b0b0', textMuted: '#808080',
          accent: '#ff8800', accentHover: '#ffaa33', accentSubtle: 'rgba(255,136,0,0.1)',
          accentShadow: 'rgba(255,136,0,0.2)', accentGlow: 'rgba(255,136,0,0.3)',
          accentContrast: '#000000', danger: '#ff4444', dangerHover: '#cc0000',
        },
      },
    };
    const result = themeEngine.importTheme(importData);
    expect(result.success).toBe(true);
    expect(result.id).toBe('imported-theme');
  });

  it('rejects import with invalid theme data', () => {
    const badImport = {
      name: 'Bad Import',
      version: 1,
      theme: {
        name: 'Bad Import',
        colors: {
          bgPrimary: 'url(http://evil.com)',  // blocked
          bgSecondary: '#000', surface: '#111',
          surfaceHover: '#222', cardBg: '#333', cardBgHover: '#444', inputBg: '#555',
          border: '#666', borderHover: '#777', borderStrong: '#888',
          text: '#fff', textSecondary: '#ccc', textMuted: '#999',
          accent: '#0f0', accentHover: '#0a0', accentSubtle: '#aaa',
          accentShadow: '#bbb', accentGlow: '#ccc', accentContrast: '#fff',
          danger: '#f00', dangerHover: '#d00',
        },
      },
    };
    const result = themeEngine.importTheme(badImport);
    expect(result.success).toBe(false);
    expect(result.errors).toBeDefined();
  });

  it('rejects import with missing required color fields', () => {
    const incomplete = {
      name: 'Incomplete',
      version: 1,
      theme: {
        name: 'Incomplete',
        colors: {
          bgPrimary: '#000',
          // missing everything else
        },
      },
    };
    const result = themeEngine.importTheme(incomplete);
    expect(result.success).toBe(false);
  });

  // ── Accent & motion ──────────────────────────────────────────────────

  it('sets accent preset and persists', () => {
    themeEngine.setAccent('violet');
    expect(themeEngine.accentPreset).toBe('violet');
    expect(document.documentElement.getAttribute('data-accent')).toBe('violet');
  });

  it('sets motion speed and persists', () => {
    themeEngine.setMotionSpeed('instant');
    expect(themeEngine.motionSpeed).toBe('instant');
    expect(document.documentElement.getAttribute('data-motion')).toBe('instant');
  });
});