import type { AppUsage } from '@record/types'
import { createMemo, createSignal, For, onCleanup, onMount, Show } from 'solid-js'
import { createStore, reconcile } from 'solid-js/store'
import { getAppIcon, getDailySummary } from '../lib/api'

function formatDuration(secs: number): string {
  if (secs < 60) return `${secs}s`
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  if (h > 0) return `${h}h ${m}m`
  return `${m}m`
}

function formatTime(secs: number): string {
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  const s = secs % 60
  return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
}

function AppIcon(props: { bundleId: string }) {
  const [src, setSrc] = createSignal<string | null>(null)

  onMount(async () => {
    const b64 = await getAppIcon(props.bundleId)
    if (b64) setSrc(`data:image/png;base64,${b64}`)
  })

  return (
    <Show
      when={src()}
      fallback={
        <div class="app-icon-placeholder">
          <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
            <rect x="2" y="2" width="16" height="16" rx="4" fill="currentColor" opacity="0.15" />
          </svg>
        </div>
      }
    >
      <img class="app-icon" src={src()!} alt="" width="20" height="20" />
    </Show>
  )
}

interface StoreState {
  activeSecs: number
  idleSecs: number
  apps: AppUsage[]
  ready: boolean
}

export default function TodayView() {
  const [state, setState] = createStore<StoreState>({
    activeSecs: 0,
    idleSecs: 0,
    apps: [],
    ready: false,
  })
  const [elapsed, setElapsed] = createSignal(0)
  let lastFetchTime = 0

  async function refresh() {
    try {
      const data = await getDailySummary(new Date())
      setState({
        activeSecs: data.total_active_secs,
        idleSecs: data.total_idle_secs,
        ready: true,
      })
      setState('apps', reconcile(data.apps, { key: 'bundle_id' }))
      lastFetchTime = Date.now()
      setElapsed(0)
    } catch {
      // backend not ready yet
    }
  }

  onMount(() => {
    refresh()
    const fetchId = setInterval(refresh, 5000)
    const tickId = setInterval(() => {
      if (lastFetchTime > 0) {
        setElapsed(Math.floor((Date.now() - lastFetchTime) / 1000))
      }
    }, 1000)
    onCleanup(() => {
      clearInterval(fetchId)
      clearInterval(tickId)
    })
  })

  const activeTime = () => state.activeSecs + elapsed()
  const idleTime = () => state.idleSecs

  const topAppSecs = createMemo(() => {
    if (state.apps.length === 0) return 1
    return Math.max(state.apps[0].total_secs + elapsed(), 1)
  })

  return (
    <div class="today-view">
      <header class="today-header">
        <h1>
          {new Date().toLocaleDateString('en-US', {
            weekday: 'long',
            month: 'long',
            day: 'numeric',
          })}
        </h1>
      </header>

      <Show when={state.ready} fallback={<div class="today-empty">Waiting for data...</div>}>
        <div class="today-stats">
          <div class="stat-card">
            <span class="stat-label">Active</span>
            <span class="stat-value mono">{formatTime(activeTime())}</span>
          </div>
          <div class="stat-card">
            <span class="stat-label">Idle</span>
            <span class="stat-value mono">{formatTime(idleTime())}</span>
          </div>
          <div class="stat-card">
            <span class="stat-label">Apps</span>
            <span class="stat-value mono">{state.apps.length}</span>
          </div>
        </div>

        <Show
          when={state.apps.length > 0}
          fallback={<div class="today-empty">No activity recorded yet. Keep working.</div>}
        >
          <div class="app-list">
            <For each={state.apps}>
              {(app, i) => {
                const liveSecs = () => (i() === 0 ? app.total_secs + elapsed() : app.total_secs)
                const pct = () => Math.max(2, (liveSecs() / topAppSecs()) * 100)
                return (
                  <div class="app-row">
                    <div class="app-bar-accent" style={{ height: `${pct()}%` }} />
                    <AppIcon bundleId={app.bundle_id} />
                    <div class="app-body">
                      <div class="app-info">
                        <span class="app-name">{app.app_name}</span>
                        <span class="app-meta mono">
                          {formatDuration(liveSecs())}
                          <span class="app-sessions">
                            {app.session_count} session{app.session_count !== 1 ? 's' : ''}
                          </span>
                        </span>
                      </div>
                      <div class="app-bar-track">
                        <div class="app-bar-fill" style={{ width: `${pct()}%` }} />
                      </div>
                    </div>
                  </div>
                )
              }}
            </For>
          </div>
        </Show>
      </Show>
    </div>
  )
}
