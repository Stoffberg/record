import { createResource, createSignal, For, Show } from 'solid-js'
import { backfillProjects, getVersion } from '../lib/api'
import type { Theme } from '../lib/context'
import { useApp } from '../lib/context'

const themes: { value: Theme; label: string }[] = [
  { value: 'system', label: 'System' },
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
]

export default function SettingsView() {
  const { autoStart, toggleAutoStart, theme, setTheme } = useApp()
  const [version] = createResource(getVersion)
  const [backfillState, setBackfillState] = createSignal<'idle' | 'running' | number>('idle')

  const runBackfill = async () => {
    setBackfillState('running')
    try {
      const updated = await backfillProjects()
      setBackfillState(updated)
    } catch {
      setBackfillState('idle')
    }
  }

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
          <Show when={version()}>
            <span class="setting-value mono">v{version()}</span>
          </Show>
        </div>
      </div>

      <div class="settings-section">
        <h2>Debug</h2>
        <div class="setting-row">
          <div class="setting-info">
            <span class="setting-label">Backfill projects</span>
            <span class="setting-desc">
              Re-detect projects for all recorded sessions using latest adapter logic.
            </span>
          </div>
          <Show
            when={typeof backfillState() === 'number'}
            fallback={
              <button
                class="setting-badge"
                disabled={backfillState() === 'running'}
                onClick={runBackfill}
              >
                {backfillState() === 'running' ? 'Running…' : 'Run'}
              </button>
            }
          >
            <span class="setting-value mono">{backfillState() as number} updated</span>
          </Show>
        </div>
      </div>
    </div>
  )
}
