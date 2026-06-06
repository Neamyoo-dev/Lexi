const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

async function initApp() {
  try {
    await invoke('init_ime');
  } catch (e) {
    console.error('Failed to initialize IME:', e);
  }
}

function applySystemTheme() {
  const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
  const savedConfig = (() => {
    try {
      const saved = localStorage.getItem('lexi_config');
      return saved ? JSON.parse(saved) : null;
    } catch (_) {
      return null;
    }
  })();

  if (savedConfig && savedConfig.theme) {
    document.documentElement.classList.remove('light', 'dark');
    document.documentElement.classList.add(savedConfig.theme);
  } else {
    document.documentElement.classList.remove('light', 'dark');
    document.documentElement.classList.add(prefersDark ? 'dark' : 'light');
  }
}

window.addEventListener("DOMContentLoaded", () => {
  applySystemTheme();
  initApp();
});
