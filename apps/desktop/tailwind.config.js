/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{svelte,ts,js}"],
  theme: {
    extend: {
      colors: {
        vault: {
          50: "#ecfdf5",
          100: "#d1fae5",
          200: "#a7f3d0",
          300: "#6ee7b7",
          400: "#34d399",
          500: "#10b981",
          600: "#059669",
          700: "#047857",
          800: "#065f46",
          900: "#064e3b",
          950: "#022c22",
        },
        surface: {
          50: "#f8fafc",
          100: "#f1f5f9",
          200: "#e2e8f0",
          300: "#cbd5e1",
          400: "#94a3b8",
          500: "#64748b",
          600: "#475569",
          700: "#334155",
          800: "#1e293b",
          850: "#172033",
          900: "#0f172a",
          950: "#020617",
        },
      },
      fontFamily: {
        mono: ["JetBrains Mono", "Fira Code", "monospace"],
        sans: ["Inter", "system-ui", "-apple-system", "sans-serif"],
      },
      boxShadow: {
        'glow-vault': '0 0 15px -3px rgba(16, 185, 129, 0.3)',
        'glow-vault-lg': '0 0 30px -5px rgba(16, 185, 129, 0.4)',
        'glow-vault-xl': '0 0 60px -10px rgba(16, 185, 129, 0.5)',
      },
      backdropBlur: {
        glass: '12px',
      },
    },
  },
  plugins: [
    function ({ addUtilities }) {
      addUtilities({
        '.glow-vault': {
          boxShadow: '0 0 15px -3px rgba(16, 185, 129, 0.3)',
        },
        '.glow-vault-lg': {
          boxShadow: '0 0 30px -5px rgba(16, 185, 129, 0.4)',
        },
        '.glow-vault-xl': {
          boxShadow: '0 0 60px -10px rgba(16, 185, 129, 0.5)',
        },
        '.glass': {
          backgroundColor: 'rgba(15, 23, 42, 0.6)',
          backdropFilter: 'blur(12px)',
          WebkitBackdropFilter: 'blur(12px)',
          border: '1px solid rgba(148, 163, 184, 0.1)',
        },
        '.glass-sm': {
          backgroundColor: 'rgba(15, 23, 42, 0.4)',
          backdropFilter: 'blur(8px)',
          WebkitBackdropFilter: 'blur(8px)',
          border: '1px solid rgba(148, 163, 184, 0.08)',
        },
        '.glass-lg': {
          backgroundColor: 'rgba(30, 41, 59, 0.5)',
          backdropFilter: 'blur(16px)',
          WebkitBackdropFilter: 'blur(16px)',
          border: '1px solid rgba(148, 163, 184, 0.15)',
        },
      });
    },
  ],
};
