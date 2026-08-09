<script lang="ts">
  import { vault, setTheme } from '../vault.svelte.ts';
  import { themeEngine } from '../themeEngine.svelte.ts';
  import type { AccentTheme } from '../vault.svelte.ts';

  function handleTestnetToggle() {
    if (vault.testnetOnly) {
      vault.showBetaWarning = true;
    } else {
      vault.testnetOnly = true;
    }
  }

  function confirmMainnet() {
    vault.testnetOnly = false;
    vault.showBetaWarning = false;
  }

  function cancelMainnet() {
    vault.showBetaWarning = false;
  }

  function handleAccentChange(accent: AccentTheme) {
    vault.accent = accent;
    themeEngine.setAccent(accent);
  }

  function handleMotionChange(speed: 'instant' | 'normal' | 'expressive') {
    themeEngine.setMotionSpeed(speed);
  }

  const builtinThemeIds = ['obsidian', 'dark-slate', 'light-studio'] as const;
  type BuiltinThemeId = (typeof builtinThemeIds)[number];
  const themeIcons: Record<BuiltinThemeId, string> = {
    obsidian: '🪨',
    'dark-slate': '🌙',
    'light-studio': '☀️',
  };

  function handleThemeChange(id: BuiltinThemeId) {
    themeEngine.applyTheme(id);
    const isDark = id !== 'light-studio';
    document.documentElement.setAttribute('data-theme', isDark ? 'dark' : 'light');
    localStorage.setItem('gullbur_theme', id);
  }

  $effect(() => {
    const id = themeEngine.currentThemeId;
    const isDark = id !== 'light-studio';
    document.documentElement.setAttribute('data-theme', isDark ? 'dark' : 'light');
  });

  // ── Density toggle ─────────────────────────────────────────────────
  let density = $state<'normal' | 'compact' | 'expanded'>('normal');

  function handleDensityChange(mode: 'normal' | 'compact' | 'expanded') {
    density = mode;
    document.documentElement.setAttribute('data-density', mode);
    localStorage.setItem('gullbur_density', mode);
  }

  // ── Command Palette (Cmd+K) ───────────────────────────────────────
  let commandPaletteOpen = $state(false);
  let paletteQuery = $state('');

  function handleKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
      e.preventDefault();
      commandPaletteOpen = true;
    }
  }

  interface PaletteItem {
    icon: string;
    label: string;
    desc?: string;
    shortcut?: string;
    keywords?: string[];
    action: () => void;
  }

  function getPaletteItems(): PaletteItem[] {
    const items: PaletteItem[] = [
      { icon: '🪨', label: 'Switch to Obsidian', desc: 'OLED tactical dark theme', keywords: ['obsidian', 'dark', 'theme', 'oled'], shortcut: '', action: () => handleThemeChange('obsidian') },
      { icon: '🌙', label: 'Switch to Dark Slate', desc: 'Legacy dark gray theme', keywords: ['dark', 'slate', 'theme', 'legacy'], shortcut: '', action: () => handleThemeChange('dark-slate') },
      { icon: '☀️', label: 'Switch to Light Studio', desc: 'Warm studio-slate light theme', keywords: ['light', 'studio', 'theme', 'bright'], shortcut: '', action: () => handleThemeChange('light-studio') },
    ];

    // Add network switches from the vault store
    for (const net of vault.networks) {
      const icon = net.id?.includes('ethereum') ? '⬡' : net.id?.includes('bitcoin') ? '₿' : net.id?.includes('monero') ? 'ɱ' : net.id?.includes('litecoin') ? 'Ł' : '◈';
      items.push({
        icon,
        label: `Switch to ${net.name ?? net.id}`,
        desc: `Select ${net.id} network`,
        keywords: [net.id, net.name ?? '', net.symbol ?? '', 'network', 'chain'],
        action: () => { vault.selectedNetwork = net.id; },
      });
    }
    return items;
  }

  let filteredItems = $derived.by(() => {
    const q = paletteQuery.toLowerCase().trim();
    const all = getPaletteItems();
    if (!q) return all;
    return all.filter(item =>
      item.keywords?.some(k => k.toLowerCase().includes(q)) ||
      item.label.toLowerCase().includes(q)
    );
  });

  function runPaletteAction(item: PaletteItem) {
    item.action();
    commandPaletteOpen = false;
    paletteQuery = '';
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="flex items-center gap-2 flex-wrap">
  <!-- Cmd+K hint -->
  <button
    class="hidden md:flex items-center gap-1 px-2 py-1 text-[11px] font-mono text-muted bg-surface border border-strong/50 rounded-md hover:bg-surface-hover hover:text-secondary transition-colors"
    title="Command Palette (Cmd+K / Ctrl+K)"
    onclick={() => { commandPaletteOpen = true; paletteQuery = ''; }}
  >
    <span class="text-xs">⌘K</span>
  </button>

  <!-- Testnet-Only Toggle -->
  <div class="flex items-center gap-1.5">
    <button
      class="relative w-9 h-5 rounded-full transition-colors {vault.testnetOnly ? 'bg-amber-600' : 'bg-surface-hover'}"
      onclick={handleTestnetToggle}
      role="switch"
      aria-checked={vault.testnetOnly}
      title={vault.testnetOnly ? 'Testnet-only mode — click to disable' : 'Enable testnet-only mode'}
    >
      <span
        class="absolute top-0.5 left-0.5 w-4 h-4 bg-white rounded-full transition-transform {vault.testnetOnly ? 'translate-x-4' : ''}"
      ></span>
    </button>
    {#if vault.testnetOnly}
      <span class="text-[10px] uppercase tracking-wider font-semibold text-amber-400 bg-amber-400/10 border border-amber-400/30 rounded px-1.5 py-0.5 leading-none">
        Testnet Only
      </span>
    {/if}
  </div>

  <!-- Theme Selector (Engine-driven) -->
  <div class="flex rounded-lg overflow-hidden border border-strong/50">
    {#each builtinThemeIds as id}
      <button
        class="px-2.5 py-1.5 text-xs font-medium transition-all
          {themeEngine.currentThemeId === id
            ? 'bg-accent text-white shadow-sm'
            : 'bg-surface text-secondary hover:bg-surface-hover hover:text-primary'}"
        onclick={() => handleThemeChange(id)}
        title={id === 'obsidian' ? 'OLED Tactical Dark' : id === 'dark-slate' ? 'Legacy Dark Slate' : 'Warm Light Studio'}
      >
        {themeIcons[id]}
      </button>
    {/each}
  </div>

  <!-- Accent Theme Selector -->
  <div class="flex items-center gap-1">
    {#each (['emerald', 'violet', 'amber', 'cyan', 'rose'] as const) as accent}
      <button
        class="w-5 h-5 rounded-full border border-border-strong transition-all hover:scale-110
          {vault.accent === accent ? 'ring-2 ring-accent' : ''}"
        style="background: {accent === 'emerald' ? '#10b981' : accent === 'violet' ? '#8b5cf6' : accent === 'amber' ? '#f59e0b' : accent === 'cyan' ? '#06b6d4' : '#f43f5e'}"
        title={`Accent: ${accent}`}
        aria-label={`Accent ${accent}`}
        onclick={() => handleAccentChange(accent)}
      ></button>
    {/each}
  </div>

  <!-- Motion Speed Selector -->
  <div class="hidden md:flex rounded-lg overflow-hidden border border-strong/30 text-[11px]">
    <button
      class="px-2 py-1 transition-colors {themeEngine.motionSpeed === 'instant' ? 'bg-accent text-white' : 'bg-surface text-secondary hover:text-primary'}"
      title="Instant transitions (0ms)"
      onclick={() => handleMotionChange('instant')}
    >⚡ 0ms</button>
    <button
      class="px-2 py-1 transition-colors {themeEngine.motionSpeed === 'normal' ? 'bg-accent text-white' : 'bg-surface text-secondary hover:text-primary'}"
      title="Tactical transitions (100ms)"
      onclick={() => handleMotionChange('normal')}
    >🎯 100ms</button>
    <button
      class="px-2 py-1 transition-colors {themeEngine.motionSpeed === 'expressive' ? 'bg-accent text-white' : 'bg-surface text-secondary hover:text-primary'}"
      title="Expressive transitions (200ms)"
      onclick={() => handleMotionChange('expressive')}
    >✨ 200ms</button>
  </div>

  <!-- Density Toggle -->
  <div class="hidden md:flex rounded-lg overflow-hidden border border-strong/30 text-[11px]">
    <button
      class="px-2 py-1 transition-colors {density === 'compact' ? 'bg-accent text-white' : 'bg-surface text-secondary hover:text-primary'}"
      title="Compact layout — dense Bloomberg terminal style"
      onclick={() => handleDensityChange('compact')}
    >📊 Compact</button>
    <button
      class="px-2 py-1 transition-colors {density === 'normal' ? 'bg-accent text-white' : 'bg-surface text-secondary hover:text-primary'}"
      title="Normal layout"
      onclick={() => handleDensityChange('normal')}
    >🎯 Normal</button>
    <button
      class="px-2 py-1 transition-colors {density === 'expanded' ? 'bg-accent text-white' : 'bg-surface text-secondary hover:text-primary'}"
      title="Expanded layout — spacious readability"
      onclick={() => handleDensityChange('expanded')}
    >🛋️ Expanded</button>
  </div>
</div>

<!-- ── Command Palette (Cmd+K) ─────────────────────────────────────── -->
{#if commandPaletteOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_interactive_supports_focus -->
  <div
    class="fixed inset-0 z-[200] flex items-start justify-center pt-[15vh] bg-black/60 backdrop-blur-sm"
    onclick={() => { commandPaletteOpen = false; paletteQuery = ''; }}
    onkeydown={(e) => { if (e.key === 'Escape') { commandPaletteOpen = false; paletteQuery = ''; } }}
    role="dialog"
    aria-modal="true"
    aria-label="Command palette"
    tabindex="-1"
  >
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="w-full max-w-lg mx-4 rounded-xl shadow-2xl border border-strong/50 overflow-hidden"
      style="background: var(--color-surface)"
      onclick={(e) => e.stopPropagation()}
    >
      <!-- Search input -->
      <div class="flex items-center gap-3 px-4 py-3 border-b border-strong/30">
        <span class="text-muted text-sm">🔍</span>
        <input
          class="flex-1 bg-transparent border-none outline-none text-primary text-sm placeholder:text-muted"
          placeholder="Search accounts, networks, or actions…"
          autofocus
          onkeydown={(e) => {
            if (e.key === 'Escape') { commandPaletteOpen = false; paletteQuery = ''; }
          }}
          bind:value={paletteQuery}
        />
        <kbd class="text-[10px] font-mono text-muted bg-bg-secondary px-1.5 py-0.5 rounded border border-strong/30">esc</kbd>
      </div>
      <!-- Results -->
      <div class="max-h-64 overflow-y-auto p-2">
        {#if filteredItems.length === 0}
          <div class="text-center py-6 text-muted text-sm">No matching commands</div>
        {:else}
          {#each filteredItems as item}
            <button
              class="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm text-left transition-colors hover:bg-surface-hover"
              onclick={() => runPaletteAction(item)}
            >
              <span class="text-base">{item.icon}</span>
              <div class="flex-1 min-w-0">
                <div class="text-primary font-medium">{item.label}</div>
                {#if item.desc}
                  <div class="text-muted text-[11px] truncate">{item.desc}</div>
                {/if}
              </div>
              {#if item.shortcut}
                <kbd class="text-[10px] font-mono text-muted bg-bg-secondary px-1.5 py-0.5 rounded border border-strong/30 shrink-0">{item.shortcut}</kbd>
              {/if}
            </button>
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}

<!-- Beta Warning Modal -->
{#if vault.showBetaWarning}
<!-- svelte-ignore a11y_click_events_have_key_events a11y_interactive_supports_focus -->
<div
  class="fixed inset-0 z-[100] flex items-center justify-center bg-black/50"
  onclick={cancelMainnet}
  onkeydown={(e) => { if (e.key === 'Escape') cancelMainnet(); }}
  role="dialog"
  aria-modal="true"
  aria-label="Mainnet beta warning"
  tabindex="-1"
>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="bg-vault-900 border border-strong rounded-xl shadow-2xl max-w-sm w-full mx-4 p-6"
    onclick={(e) => e.stopPropagation()}
  >
    <div class="flex items-start gap-3 mb-4">
      <span class="text-2xl">⚠️</span>
      <div>
        <h3 class="text-base font-semibold text-primary">Mainnet is in Beta</h3>
        <p class="text-sm text-secondary mt-1 leading-relaxed">
          Real assets are at risk. Are you sure?
        </p>
      </div>
    </div>
    <div class="flex gap-3 justify-end">
      <button
        class="px-4 py-2 rounded-lg text-sm font-medium bg-surface text-primary hover:bg-surface-hover border border-strong/50 transition-colors"
        onclick={cancelMainnet}
      >
        Cancel
      </button>
      <button
        class="btn-danger"
        onclick={confirmMainnet}
      >
        Continue
      </button>
    </div>
  </div>
</div>
{/if}