import { useApp } from '../lib/context'

export default function SettingsView() {
  const { autoStart, toggleAutoStart } = useApp()

  return (
    <div class="settings-view">
      <header class="settings-header">
        <h1>Settings</h1>
      </header>

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
