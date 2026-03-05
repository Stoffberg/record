import { createSignal, onMount } from 'solid-js'
import { getAutoStartEnabled, setAutoStart } from '../lib/api'

export default function SettingsView() {
  const [autoStart, setAutoStartState] = createSignal(false)
  const [loading, setLoading] = createSignal(true)

  onMount(async () => {
    try {
      setAutoStartState(await getAutoStartEnabled())
    } finally {
      setLoading(false)
    }
  })

  const toggleAutoStart = async () => {
    const next = !autoStart()
    setAutoStartState(next)
    try {
      await setAutoStart(next)
    } catch {
      setAutoStartState(!next)
    }
  }

  return (
    <div class="settings-view">
      <header class="settings-header">
        <h1>Settings</h1>
      </header>

      <div class="settings-section">
        <h2>Tracking</h2>
        <div class="setting-row">
          <div class="setting-info">
            <span class="setting-label">Poll interval</span>
            <span class="setting-desc">How often to check the active app</span>
          </div>
          <span class="setting-value mono">5 seconds</span>
        </div>
        <div class="setting-row">
          <div class="setting-info">
            <span class="setting-label">Idle threshold</span>
            <span class="setting-desc">Time before marking as idle</span>
          </div>
          <span class="setting-value mono">5 minutes</span>
        </div>
        <div class="setting-row">
          <div class="setting-info">
            <span class="setting-label">Merge gap</span>
            <span class="setting-desc">Max gap between heartbeats to merge</span>
          </div>
          <span class="setting-value mono">10 seconds</span>
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
            disabled={loading()}
            aria-label="Toggle launch at login"
          >
            <span class="toggle-knob" />
          </button>
        </div>
        <div class="setting-row">
          <div class="setting-info">
            <span class="setting-label">Data location</span>
            <span class="setting-desc">Where your activity data is stored</span>
          </div>
          <span class="setting-value mono" style={{ 'font-size': '0.75rem' }}>
            ~/Library/Application Support/dev.stoff.record/record.db
          </span>
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
