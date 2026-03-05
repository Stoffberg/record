import { For } from 'solid-js'
import type { Theme } from '../lib/context'
import { useApp } from '../lib/context'

const themes: { value: Theme; label: string }[] = [
  { value: 'system', label: 'System' },
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
]

export default function SettingsView() {
  const { autoStart, toggleAutoStart, theme, setTheme } = useApp()

  return (
    <div class="settings-view">
      <header class="settings-header">
        <h1>Settings</h1>
      </header>

      <div class="settings-section">
        <h2>Appearance</h2>
        <div class="setting-row">
          <div class="setting-info">
            <span class="setting-label">Theme</span>
            <span class="setting-desc">Choose your preferred appearance</span>
          </div>
          <div class="segment-control">
            <For each={themes}>
              {(t) => (
                <button
                  classList={{ active: theme() === t.value }}
                  onClick={() => setTheme(t.value)}
                >
                  {t.label}
                </button>
              )}
            </For>
          </div>
        </div>
      </div>

      <div class="settings-section">
        <h2>System</h2>
        <div class="setting-row">
          <div class="setting-info">
            <span class="setting-label">Launch at login</span>
            <span class="setting-desc">Start tracking when you log in</span>
          </div>
          <button
            class={`toggle ${autoStart() ? 'toggle-on' : ''}`}
            onClick={toggleAutoStart}
            aria-label="Toggle launch at login"
          >
            <span class="toggle-knob" />
          </button>
        </div>
      </div>

      <div class="settings-section">
        <h2>About</h2>
        <div class="setting-row">
          <div class="setting-info">
            <span class="setting-label">Record</span>
            <span class="setting-desc">
              Privacy-first activity tracker. All data stays on your machine.
            </span>
          </div>
          <span class="setting-value mono">v0.1.0</span>
        </div>
      </div>
    </div>
  )
}
