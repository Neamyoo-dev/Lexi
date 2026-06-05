const { invoke } = window.__TAURI__.core;

const COLORS = [
  { name: '靛蓝', value: [74, 108, 247] },
  { name: '紫', value: [124, 58, 237] },
  { name: '粉', value: [236, 72, 153] },
  { name: '红', value: [239, 68, 68] },
  { name: '橙', value: [245, 158, 11] },
  { name: '绿', value: [16, 185, 129] },
  { name: '青', value: [6, 182, 212] },
  { name: '灰', value: [107, 114, 128] },
];

const DEFAULTS = {
  theme: 'light',
  primaryColor: [74, 108, 247],
  rgbStr: '74, 108, 247',
  candidateCount: 6,
  enableGradient: true,
};

let config = loadConfig();

function loadConfig() {
  try {
    const saved = localStorage.getItem('lexi_config');
    if (saved) return { ...DEFAULTS, ...JSON.parse(saved) };
  } catch (_) {}
  return { ...DEFAULTS };
}

function saveConfig() {
  try { localStorage.setItem('lexi_config', JSON.stringify(config)); } catch (_) {}
}

function applyConfig() {
  document.documentElement.style.setProperty('--lexi-primary', `rgb(${config.rgbStr})`);
  document.documentElement.style.setProperty('--lexi-primary-dim', `rgba(${config.rgbStr}, 0.1)`);
  document.documentElement.style.setProperty('--lexi-primary-glow', `rgba(${config.rgbStr}, 0.3)`);
  document.documentElement.classList.remove('light', 'dark');
  document.documentElement.classList.add(config.theme);

  invoke('update_bar_theme', { theme: config.theme }).catch(() => {});
  invoke('update_bar_color', {
    r: config.primaryColor[0],
    g: config.primaryColor[1],
    b: config.primaryColor[2],
  }).catch(() => {});

  saveConfig();
}

function renderSettings() {
  const body = document.getElementById('settings-body');
  if (!body) return;

  body.innerHTML = `
    <div class="settings-section">
      <div class="settings-section-title">外观</div>
      <div class="settings-row">
        <div>
          <div class="settings-label">主题</div>
          <div class="settings-desc">浅色 / 深色</div>
        </div>
        <select class="settings-select" data-key="theme">
          <option value="light" ${config.theme === 'light' ? 'selected' : ''}>☀️ 浅色</option>
          <option value="dark" ${config.theme === 'dark' ? 'selected' : ''}>🌙 深色</option>
        </select>
      </div>
      <div class="settings-row">
        <div>
          <div class="settings-label">主色调</div>
          <div class="settings-desc">候选栏高亮颜色</div>
        </div>
      </div>
      <div class="settings-color-grid" data-key="primaryColor">
        ${COLORS.map(c => {
          const rgb = `rgb(${c.value[0]}, ${c.value[1]}, ${c.value[2]})`;
          const active = config.primaryColor[0] === c.value[0]
            && config.primaryColor[1] === c.value[1]
            && config.primaryColor[2] === c.value[2];
          return `<button class="settings-color-swatch ${active ? 'active' : ''}"
                  style="background: ${rgb}; color: ${rgb}"
                  data-color="${c.value.join(',')}" title="${c.name}"></button>`;
        }).join('')}
      </div>
    </div>
    <div class="settings-section">
      <div class="settings-section-title">输入</div>
      <div class="settings-row">
        <div>
          <div class="settings-label">候选词数量</div>
          <div class="settings-desc">每页显示的候选词数</div>
        </div>
        <select class="settings-select" data-key="candidateCount">
          ${[4, 5, 6, 7, 8, 9].map(n =>
            `<option value="${n}" ${config.candidateCount === n ? 'selected' : ''}>${n} 个</option>`
          ).join('')}
        </select>
      </div>
    </div>
  `;

  body.querySelectorAll('[data-key]').forEach(el => {
    const key = el.dataset.key;
    if (el.tagName === 'SELECT') {
      el.addEventListener('change', () => {
        config[key] = key === 'candidateCount' ? parseInt(el.value) : el.value;
        applyConfig();
        renderSettings();
      });
    }
  });

  body.querySelectorAll('.settings-color-swatch').forEach(el => {
    el.addEventListener('click', () => {
      const [r, g, b] = el.dataset.color.split(',').map(Number);
      config.primaryColor = [r, g, b];
      config.rgbStr = `${r}, ${g}, ${b}`;
      applyConfig();
      renderSettings();
    });
  });
}

applyConfig();
renderSettings();
