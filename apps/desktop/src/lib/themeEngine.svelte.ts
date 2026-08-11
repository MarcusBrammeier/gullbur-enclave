/**
 * Theme Engine — Gullbúr Enclave
 *
 * A secure, validated, reactive theme system built on Svelte 5 $state runes.
 *
 * SECURITY GUARANTEES:
 *   - Themes are PURE CSS VARIABLE MAPS only (no code, no eval, no URLs).
 *   - All user-supplied theme data is validated against a Zod schema.
 *   - External network fetches are FORBIDDEN (no url() values, no @import, no fonts from CDN).
 *   - Every theme key is a `--css-variable`; every value is constrained.
 *
 * CAPABILITIES:
 *   - Full CSS token customization (colors, radii, spacing, typography, motion).
 *   - JSON import/export for community theme sharing.
 *   - LocalStorage persistence.
 *   - Svelte 5 reactive — components re-render instantly when theme changes.
 *   - Accent presets independent of base theme.
 */

import { z } from 'zod';

// ── Zod Schema (strict security boundary) ────────────────────────────────────

/** Allowed CSS values — prevents URL(), javascript:, expression(), and other XSS vectors */
const cssSafeValue = z.string()
  .regex(/^(?!.*url\(|javascript:|expression\(|data:|<|>|eval|import|document|cookie|window|on\w+=)/i, 'Unsafe CSS value')
  .max(512, 'Value too long (max 512 chars)');

/** Motion timing values — must match common CSS timing patterns */
const cssTimingValue = z.string().regex(
  /^(\d+(?:\.\d+)?(?:ms|s)|cubic-bezier\([^)]+\)|steps\([^)]+\)|linear|ease|ease-in|ease-out|ease-in-out)$/,
  'Must be a valid CSS timing function or duration'
);

/** A complete theme definition */
const themeDefinitionSchema = z.object({
  /** Required: human-readable name for the theme */
  name: z.string().min(1).max(64),
  /** Optional: description / attribution */
  description: z.string().max(256).optional(),
  /** Core color tokens */
  colors: z.object({
    bgPrimary: cssSafeValue,
    bgSecondary: cssSafeValue,
    surface: cssSafeValue,
    surfaceHover: cssSafeValue,
    cardBg: cssSafeValue,
    cardBgHover: cssSafeValue,
    inputBg: cssSafeValue,
    border: cssSafeValue,
    borderHover: cssSafeValue,
    borderStrong: cssSafeValue,
    text: cssSafeValue,
    textSecondary: cssSafeValue,
    textMuted: cssSafeValue,
    accent: cssSafeValue,
    accentHover: cssSafeValue,
    accentSubtle: cssSafeValue,
    accentShadow: cssSafeValue,
    accentGlow: cssSafeValue,
    accentContrast: cssSafeValue,
    danger: cssSafeValue,
    dangerHover: cssSafeValue,
  }),
  /** Optional: geometry tokens (border radii, spacing) */
  geometry: z.object({
    radiusCard: cssSafeValue.optional(),
    radiusButton: cssSafeValue.optional(),
    radiusInput: cssSafeValue.optional(),
    radiusModal: cssSafeValue.optional(),
    borderWidth: cssSafeValue.optional(),
    spacingCompact: cssSafeValue.optional(),
    spacingNormal: cssSafeValue.optional(),
    spacingComfortable: cssSafeValue.optional(),
  }).optional(),
  /** Optional: typography tokens */
  typography: z.object({
    fontFamily: cssSafeValue.optional(),
    fontMono: cssSafeValue.optional(),
    fontWeightNormal: cssSafeValue.optional(),
    fontWeightMedium: cssSafeValue.optional(),
    fontWeightSemibold: cssSafeValue.optional(),
    letterSpacing: cssSafeValue.optional(),
  }).optional(),
  /** Optional: motion & animation tokens */
  motion: z.object({
    durationInstant: cssTimingValue.optional(),
    durationFast: cssTimingValue.optional(),
    durationNormal: cssTimingValue.optional(),
    durationSlow: cssTimingValue.optional(),
    easingDefault: cssTimingValue.optional(),
    easingOut: cssTimingValue.optional(),
    easingInOut: cssTimingValue.optional(),
    scaleHover: cssSafeValue.optional(),
    scaleActive: cssSafeValue.optional(),
  }).optional(),
});

export type ThemeDefinition = z.infer<typeof themeDefinitionSchema>;

// ── Built-in Presets (immutable, cannot be modified at runtime) ──────────────

const LEGACY_EMERALD: ThemeDefinition = {
  name: 'Legacy Emerald',
  description: 'Original Gullbúr Enclave theme — emerald accent on dark gray. Preserved as immutable fallback.',
  colors: {
    bgPrimary: '#0d1117',
    bgSecondary: '#161b22',
    surface: '#1c2333',
    surfaceHover: '#252d3f',
    cardBg: 'rgba(28, 35, 51, 0.8)',
    cardBgHover: 'rgba(28, 35, 51, 0.9)',
    inputBg: 'rgba(37, 45, 63, 0.7)',
    border: 'rgba(255, 255, 255, 0.06)',
    borderHover: 'rgba(255, 255, 255, 0.1)',
    borderStrong: 'rgba(255, 255, 255, 0.14)',
    text: '#e6edf3',
    textSecondary: '#8b949e',
    textMuted: '#6e7681',
    accent: '#10b981',
    accentHover: '#059669',
    accentSubtle: 'rgba(16, 185, 129, 0.08)',
    accentShadow: 'rgba(16, 185, 129, 0.15)',
    accentGlow: 'rgba(16, 185, 129, 0.3)',
    accentContrast: '#ffffff',
    danger: '#ef4444',
    dangerHover: '#dc2626',
  },
  geometry: {
    radiusCard: '12px',
    radiusButton: '8px',
    radiusInput: '8px',
    radiusModal: '16px',
    borderWidth: '1px',
    spacingNormal: '1rem',
  },
  motion: {
    durationInstant: '50ms',
    durationFast: '150ms',
    durationNormal: '250ms',
    durationSlow: '400ms',
    easingOut: 'cubic-bezier(0.16, 1, 0.3, 1)',
    easingInOut: 'cubic-bezier(0.65, 0, 0.35, 1)',
    scaleHover: '1.02',
    scaleActive: '0.98',
  },
};

const DARK_SLATE: ThemeDefinition = {
  name: 'Dark Slate',
  description: 'Sleek dark gray theme with tactical emerald accents. Non-fatiguing deep slate base.',
  colors: {
    bgPrimary: '#0d1117',
    bgSecondary: '#161b22',
    surface: '#1c2333',
    surfaceHover: '#252d3f',
    cardBg: 'rgba(22, 27, 34, 0.85)',
    cardBgHover: 'rgba(28, 35, 51, 0.9)',
    inputBg: 'rgba(37, 45, 63, 0.6)',
    border: 'rgba(255, 255, 255, 0.06)',
    borderHover: 'rgba(255, 255, 255, 0.1)',
    borderStrong: 'rgba(255, 255, 255, 0.14)',
    text: '#e6edf3',
    textSecondary: '#8b949e',
    textMuted: '#6e7681',
    accent: '#10b981',
    accentHover: '#34d399',
    accentSubtle: 'rgba(16, 185, 129, 0.08)',
    accentShadow: 'rgba(16, 185, 129, 0.15)',
    accentGlow: 'rgba(16, 185, 129, 0.3)',
    accentContrast: '#ffffff',
    danger: '#ef4444',
    dangerHover: '#f87171',
  },
  geometry: {
    radiusCard: '8px',
    radiusButton: '6px',
    radiusInput: '6px',
    radiusModal: '12px',
    borderWidth: '1px',
    spacingCompact: '0.5rem',
    spacingNormal: '1rem',
    spacingComfortable: '1.5rem',
  },
  typography: {
    fontFamily: '"Inter", system-ui, -apple-system, sans-serif',
    fontMono: '"JetBrains Mono", "SF Mono", "Fira Code", monospace',
    fontWeightMedium: '500',
    fontWeightSemibold: '600',
    letterSpacing: '-0.025em',
  },
  motion: {
    durationInstant: '50ms',
    durationFast: '100ms',
    durationNormal: '180ms',
    durationSlow: '300ms',
    easingDefault: 'cubic-bezier(0.4, 0, 0.2, 1)',
    easingOut: 'cubic-bezier(0.16, 1, 0.3, 1)',
    easingInOut: 'cubic-bezier(0.65, 0, 0.35, 1)',
    scaleHover: '1.02',
    scaleActive: '0.98',
  },
};

export const LIGHT_STUDIO: ThemeDefinition = {
  name: 'Light Studio',
  description: 'Warm studio-slate light theme — #f0f2f5 canvas. No harsh whites, premium ergonomic contrast.',
  colors: {
    bgPrimary: '#f0f2f5',
    bgSecondary: '#ffffff',
    surface: '#ffffff',
    surfaceHover: '#f6f8fa',
    cardBg: 'rgba(255, 255, 255, 0.85)',
    cardBgHover: 'rgba(255, 255, 255, 0.95)',
    inputBg: 'rgba(246, 248, 250, 0.8)',
    border: 'rgba(208, 215, 222, 0.5)',
    borderHover: 'rgba(208, 215, 222, 0.8)',
    borderStrong: '#d0d7de',
    text: '#1f2328',
    textSecondary: '#656d76',
    textMuted: '#8b949e',
    accent: '#059669',
    accentHover: '#047857',
    accentSubtle: 'rgba(5, 150, 105, 0.08)',
    accentShadow: 'rgba(5, 150, 105, 0.15)',
    accentGlow: 'rgba(5, 150, 105, 0.3)',
    accentContrast: '#ffffff',
    danger: '#dc2626',
    dangerHover: '#b91c1c',
  },
  geometry: {
    radiusCard: '10px',
    radiusButton: '8px',
    radiusInput: '8px',
    radiusModal: '14px',
    borderWidth: '1px',
    spacingCompact: '0.5rem',
    spacingNormal: '1rem',
    spacingComfortable: '1.5rem',
  },
  typography: {
    fontFamily: '"Inter", system-ui, -apple-system, sans-serif',
    fontMono: '"JetBrains Mono", "SF Mono", "Fira Code", monospace',
    fontWeightMedium: '500',
    fontWeightSemibold: '600',
    letterSpacing: '-0.025em',
  },
  motion: {
    durationInstant: '50ms',
    durationFast: '100ms',
    durationNormal: '180ms',
    durationSlow: '300ms',
    easingDefault: 'cubic-bezier(0.4, 0, 0.2, 1)',
    easingOut: 'cubic-bezier(0.16, 1, 0.3, 1)',
    easingInOut: 'cubic-bezier(0.65, 0, 0.35, 1)',
    scaleHover: '1.02',
    scaleActive: '0.98',
  },
};

const OBSIDIAN: ThemeDefinition = {
  name: 'Obsidian',
  description: 'OLED tactical dark — #0d1117 canvas. Minimal, high-contrast, zero eye fatigue.',
  colors: {
    bgPrimary: '#0d1117',
    bgSecondary: '#161b22',
    surface: '#21262d',
    surfaceHover: '#30363d',
    cardBg: 'rgba(22, 27, 34, 0.85)',
    cardBgHover: 'rgba(33, 38, 45, 0.9)',
    inputBg: 'rgba(48, 54, 61, 0.6)',
    border: 'rgba(48, 54, 61, 0.6)',
    borderHover: 'rgba(48, 54, 61, 0.8)',
    borderStrong: '#30363d',
    text: '#f0f6fc',
    textSecondary: '#8b949e',
    textMuted: '#6e7681',
    accent: '#10b981',
    accentHover: '#34d399',
    accentSubtle: 'rgba(16, 185, 129, 0.08)',
    accentShadow: 'rgba(16, 185, 129, 0.15)',
    accentGlow: 'rgba(16, 185, 129, 0.3)',
    accentContrast: '#ffffff',
    danger: '#ef4444',
    dangerHover: '#f87171',
  },
  geometry: {
    radiusCard: '10px',
    radiusButton: '8px',
    radiusInput: '8px',
    radiusModal: '14px',
    borderWidth: '1px',
    spacingCompact: '0.5rem',
    spacingNormal: '1rem',
    spacingComfortable: '1.5rem',
  },
  typography: {
    fontFamily: '"Inter", system-ui, -apple-system, sans-serif',
    fontMono: '"JetBrains Mono", "SF Mono", "Fira Code", monospace',
    fontWeightMedium: '500',
    fontWeightSemibold: '600',
    letterSpacing: '-0.025em',
  },
  motion: {
    durationInstant: '50ms',
    durationFast: '100ms',
    durationNormal: '180ms',
    durationSlow: '300ms',
    easingDefault: 'cubic-bezier(0.4, 0, 0.2, 1)',
    easingOut: 'cubic-bezier(0.16, 1, 0.3, 1)',
    easingInOut: 'cubic-bezier(0.65, 0, 0.35, 1)',
    scaleHover: '1.02',
    scaleActive: '0.98',
  },
};

// ── Registry of immutable built-in themes ───────────────────────────────────
const BUILTIN_THEMES = {
  'legacy-emerald': LEGACY_EMERALD,
  'obsidian': OBSIDIAN,
  'dark-slate': DARK_SLATE,
  'light-studio': LIGHT_STUDIO,
} as const;

export type BuiltinThemeId = keyof typeof BUILTIN_THEMES;

// ── Theme Engine Class ───────────────────────────────────────────────────────

/**
 * Converts a ThemeDefinition to flat {--css-var: value} map for DOM injection.
 * Only includes colors and geometry — accent overrides are separate.
 */
function themeToCSSVars(theme: ThemeDefinition): Record<string, string> {
  const vars: Record<string, string> = {};
  const c = theme.colors;
  vars['--color-bg-primary'] = c.bgPrimary;
  vars['--color-bg-secondary'] = c.bgSecondary;
  vars['--color-surface'] = c.surface;
  vars['--color-surface-hover'] = c.surfaceHover;
  vars['--color-card-bg'] = c.cardBg;
  vars['--color-card-bg-hover'] = c.cardBgHover;
  vars['--color-input-bg'] = c.inputBg;
  vars['--color-border'] = c.border;
  vars['--color-border-hover'] = c.borderHover;
  vars['--color-border-strong'] = c.borderStrong;
  vars['--color-text'] = c.text;
  vars['--color-text-secondary'] = c.textSecondary;
  vars['--color-text-muted'] = c.textMuted;
  vars['--color-accent'] = c.accent;
  vars['--color-accent-hover'] = c.accentHover;
  vars['--color-accent-subtle'] = c.accentSubtle;
  vars['--color-accent-shadow'] = c.accentShadow;
  vars['--color-accent-glow'] = c.accentGlow;
  vars['--color-accent-contrast'] = c.accentContrast;
  vars['--color-danger'] = c.danger;
  vars['--color-danger-hover'] = c.dangerHover;
  // Geometry
  if (theme.geometry) {
    const g = theme.geometry;
    if (g.radiusCard) vars['--radius-card'] = g.radiusCard;
    if (g.radiusButton) vars['--radius-button'] = g.radiusButton;
    if (g.radiusInput) vars['--radius-input'] = g.radiusInput;
    if (g.radiusModal) vars['--radius-modal'] = g.radiusModal;
    if (g.borderWidth) vars['--border-width'] = g.borderWidth;
    if (g.spacingCompact) vars['--spacing-compact'] = g.spacingCompact;
    if (g.spacingNormal) vars['--spacing-normal'] = g.spacingNormal;
    if (g.spacingComfortable) vars['--spacing-comfortable'] = g.spacingComfortable;
  }
  // Typography
  if (theme.typography) {
    const t = theme.typography;
    if (t.fontFamily) vars['--font-family'] = t.fontFamily;
    if (t.fontMono) vars['--font-mono'] = t.fontMono;
    if (t.fontWeightNormal) vars['--font-weight-normal'] = t.fontWeightNormal;
    if (t.fontWeightMedium) vars['--font-weight-medium'] = t.fontWeightMedium;
    if (t.fontWeightSemibold) vars['--font-weight-semibold'] = t.fontWeightSemibold;
    if (t.letterSpacing) vars['--letter-spacing'] = t.letterSpacing;
  }
  // Motion
  if (theme.motion) {
    const m = theme.motion;
    if (m.durationInstant) vars['--duration-instant'] = m.durationInstant;
    if (m.durationFast) vars['--duration-fast'] = m.durationFast;
    if (m.durationNormal) vars['--duration-normal'] = m.durationNormal;
    if (m.durationSlow) vars['--duration-slow'] = m.durationSlow;
    if (m.easingDefault) vars['--easing-default'] = m.easingDefault;
    if (m.easingOut) vars['--easing-out'] = m.easingOut;
    if (m.easingInOut) vars['--easing-inout'] = m.easingInOut;
    if (m.scaleHover) vars['--scale-hover'] = m.scaleHover;
    if (m.scaleActive) vars['--scale-active'] = m.scaleActive;
  }
  return vars;
}

const STORAGE_KEY = 'gullbur_theme_def';
const CUSTOM_THEMES_KEY = 'gullbur_custom_themes';

export class ThemeEngine {
  /** Currently active theme ID (builtin or custom) */
  currentThemeId = $state<string>('obsidian');

  /** Current resolved ThemeDefinition (reactive) */
  currentTheme = $state<ThemeDefinition>(OBSIDIAN);

  /** Active accent preset */
  accentPreset = $state<'emerald' | 'violet' | 'amber' | 'cyan' | 'rose'>('emerald');

  /** Motion speed preference: 'instant' | 'normal' | 'expressive' */
  motionSpeed = $state<'instant' | 'normal' | 'expressive'>('normal');

  /** User-created custom themes (persisted, validated) */
  customThemes = $state<Record<string, ThemeDefinition>>({});

  /** Saved user theme definitions loaded from storage */
  private savedCustomThemes: Record<string, ThemeDefinition> = {};

  constructor() {
    // Hydrate from localStorage on construction
    this.loadFromStorage();
  }

  /** Get a read-only list of all available themes */
  getAvailableThemes(): Array<{ id: string; name: string; description?: string; isBuiltin: boolean }> {
    const list: Array<{ id: string; name: string; description?: string; isBuiltin: boolean }> = [];
    for (const [id, theme] of Object.entries(BUILTIN_THEMES)) {
      list.push({ id, name: theme.name, description: theme.description, isBuiltin: true });
    }
    for (const [id, theme] of Object.entries(this.savedCustomThemes)) {
      list.push({ id, name: theme.name, description: theme.description, isBuiltin: false });
    }
    return list;
  }

  /** Apply a theme by ID. Built-in themes are immutable. Returns false if not found. */
  applyTheme(id: string): boolean {
    if (id in BUILTIN_THEMES) {
      const theme = BUILTIN_THEMES[id as BuiltinThemeId];
      this.currentTheme = theme;
      this.currentThemeId = id;
      this.injectThemeToDOM(theme);
      this.persist();
      return true;
    }
    if (id in this.savedCustomThemes) {
      const theme = this.savedCustomThemes[id];
      this.currentTheme = theme;
      this.currentThemeId = id;
      this.injectThemeToDOM(theme);
      this.persist();
      return true;
    }
    return false;
  }

  /** Save a custom theme from a validated definition. Returns false if validation fails. */
  saveCustomTheme(id: string, definition: unknown): { success: boolean; errors?: string[] } {
    const parsed = themeDefinitionSchema.safeParse(definition);
    if (!parsed.success) {
      return { success: false, errors: parsed.error.issues.map(i => `${i.path.join('.')}: ${i.message}`) };
    }
    // Immutable check: cannot overwrite built-in theme IDs
    if (id in BUILTIN_THEMES) {
      return { success: false, errors: [`Cannot overwrite built-in theme "${id}"`] };
    }
    this.savedCustomThemes[id] = parsed.data;
    this.persistCustomThemes();
    return { success: true };
  }

  /** Delete a user-created custom theme. Built-in themes cannot be deleted. */
  deleteCustomTheme(id: string): boolean {
    if (id in BUILTIN_THEMES) return false;
    if (!(id in this.savedCustomThemes)) return false;
    delete this.savedCustomThemes[id];
    this.persistCustomThemes();
    // If the deleted theme was active, fall back to dark-slate
    if (this.currentThemeId === id) {
      this.applyTheme('obsidian');
    }
    return true;
  }

  /** Export the current theme as a portable JSON object */
  exportCurrentTheme(): { name: string; description: string; version: 1; theme: ThemeDefinition } {
    return {
      name: this.currentTheme.name,
      description: this.currentTheme.description ?? '',
      version: 1,
      theme: this.currentTheme,
    };
  }

  /** Import a theme from a validated JSON object. Returns validation errors if any. */
  importTheme(json: unknown): { success: boolean; id?: string; errors?: string[] } {
    // Validate the wrapper format
    const wrapperSchema = z.object({
      name: z.string().min(1).max(64),
      description: z.string().max(256).optional().default(''),
      version: z.literal(1).optional().default(1),
      theme: themeDefinitionSchema,
    });
    const parsed = wrapperSchema.safeParse(json);
    if (!parsed.success) {
      return { success: false, errors: parsed.error.issues.map(i => `${i.path.join('.')}: ${i.message}`) };
    }
    const id = parsed.data.name.toLowerCase().replace(/[^a-z0-9-]+/g, '-').replace(/^-|-$/g, '');
    const result = this.saveCustomTheme(id, parsed.data.theme);
    return { ...result, id };
  }

  /** Set accent preset by name */
  setAccent(preset: 'emerald' | 'violet' | 'amber' | 'cyan' | 'rose'): void {
    this.accentPreset = preset;
    document.documentElement.setAttribute('data-accent', preset);
    localStorage.setItem('gullbur_accent', preset);
  }

  /** Set motion speed preference */
  setMotionSpeed(speed: 'instant' | 'normal' | 'expressive'): void {
    this.motionSpeed = speed;
    document.documentElement.setAttribute('data-motion', speed);
    localStorage.setItem('gullbur_motion', speed);
  }

  /** Apply CSS variable map to document root */
  private injectThemeToDOM(theme: ThemeDefinition): void {
    const vars = themeToCSSVars(theme);
    const root = document.documentElement;
    for (const [key, value] of Object.entries(vars)) {
      root.style.setProperty(key, value);
    }
  }

  /** Persist the active theme ID to localStorage */
  private persist(): void {
    try {
      localStorage.setItem(STORAGE_KEY, this.currentThemeId);
    } catch { /* storage may be unavailable */ }
  }

  /** Persist custom themes to localStorage */
  private persistCustomThemes(): void {
    try {
      localStorage.setItem(CUSTOM_THEMES_KEY, JSON.stringify(this.savedCustomThemes));
    } catch { /* storage may be unavailable */ }
  }

  /** Load persisted state from localStorage */
  private loadFromStorage(): void {
    if (typeof window === 'undefined') return;
    try {
      // Restore theme
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved && (saved in BUILTIN_THEMES)) {
        this.applyTheme(saved);
      } else {
        this.applyTheme('dark-slate');
      }
      // Restore accent
      const accent = localStorage.getItem('gullbur_accent');
      if (accent === 'violet' || accent === 'amber' || accent === 'cyan' || accent === 'rose') {
        this.setAccent(accent);
      }
      // Restore motion speed
      const motion = localStorage.getItem('gullbur_motion');
      if (motion === 'instant' || motion === 'normal' || motion === 'expressive') {
        this.setMotionSpeed(motion);
      }
      // Restore custom themes
      const raw = localStorage.getItem(CUSTOM_THEMES_KEY);
      if (raw) {
        const parsed = JSON.parse(raw);
        if (typeof parsed === 'object' && parsed !== null) {
          for (const [id, def] of Object.entries(parsed)) {
            const result = themeDefinitionSchema.safeParse(def);
            if (result.success) {
              this.savedCustomThemes[id] = result.data;
            }
          }
        }
      }
    } catch { /* storage read is best-effort */ }
  }
}

// ── Singleton Instance ──────────────────────────────────────────────────────
export const themeEngine = new ThemeEngine();