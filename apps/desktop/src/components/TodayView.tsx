import type { AppUsage } from '@record/types'
import { createMemo, createSignal, For, onCleanup, onMount, Show } from 'solid-js'
import { createStore, reconcile } from 'solid-js/store'
import { getAppIcon, getDailySummary } from '../lib/api'
import AppDetailView from './AppDetailView'

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

function sameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  )
}

function addDays(date: Date, days: number): Date {
  const d = new Date(date)
  d.setDate(d.getDate() + days)
  return d
}

function formatDateLabel(date: Date): string {
  const now = new Date()
  if (sameDay(date, now)) return 'Today'
  if (sameDay(date, addDays(now, -1))) return 'Yesterday'
  return date.toLocaleDateString('en-US', {
    weekday: 'long',
    month: 'long',
    day: 'numeric',
  })
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
  const [selectedDate, setSelectedDate] = createSignal(new Date())
  const [selectedApp, setSelectedApp] = createSignal<AppUsage | null>(null)
  let lastFetchTime = 0

  const isToday = () => sameDay(selectedDate(), new Date())

  async function refresh() {
    try {
      const data = await getDailySummary(selectedDate())
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

  function goToPreviousDay() {
    setSelectedApp(null)
    setState('ready', false)
    setSelectedDate(addDays(selectedDate(), -1))
    refresh()
  }

  function goToNextDay() {
    if (isToday()) return
    setSelectedApp(null)
    setState('ready', false)
    setSelectedDate(addDays(selectedDate(), 1))
    refresh()
  }

  function goToToday() {
    if (isToday()) return
    setSelectedApp(null)
    setState('ready', false)
    setSelectedDate(new Date())
    refresh()
  }

  onMount(() => {
    refresh()
    const fetchId = setInterval(() => {
      if (isToday()) refresh()
    }, 5000)
    const tickId = setInterval(() => {
      if (isToday() && lastFetchTime > 0) {
        setElapsed(Math.floor((Date.now() - lastFetchTime) / 1000))
      }
    }, 1000)

    const keyHandler = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey) return
      if (selectedApp()) return
      if (e.key === 'ArrowLeft') {
        e.preventDefault()
        goToPreviousDay()
      } else if (e.key === 'ArrowRight') {
        e.preventDefault()
        goToNextDay()
      } else if (e.key === 't') {
        e.preventDefault()
        goToToday()
      }
    }
    window.addEventListener('keydown', keyHandler)

    onCleanup(() => {
      clearInterval(fetchId)
      clearInterval(tickId)
      window.removeEventListener('keydown', keyHandler)
    })
  })

  const liveElapsed = () => (isToday() ? elapsed() : 0)
  const activeTime = () => state.activeSecs + liveElapsed()
  const idleTime = () => state.idleSecs

  const visibleApps = createMemo(() => state.apps.filter((a) => a.total_secs >= 60))

  const topAppSecs = createMemo(() => {
    const apps = visibleApps()
    if (apps.length === 0) return 1
    return Math.max(apps[0].total_secs + liveElapsed(), 1)
  })

  return (
    <Show
      when={!selectedApp()}
      fallback={
        <AppDetailView
          app={selectedApp()!}
          date={selectedDate()}
          onBack={() => setSelectedApp(null)}
        />
      }
    >
      <div class="today-view">
        <header class="today-header">
          <div class="date-nav">
            <button type="button" class="date-nav-btn" onClick={goToPreviousDay}>
              <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
                <path
                  d="M10 3L5 8l5 5"
                  stroke="currentColor"
                  stroke-width="1.5"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              </svg>
            </button>
            <span class="date-nav-label">
              <span class="date-nav-heading">{formatDateLabel(selectedDate())}</span>
            </span>
            <button type="button" class="date-nav-btn" onClick={goToNextDay} disabled={isToday()}>
              <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
                <path
                  d="M6 3l5 5-5 5"
                  stroke="currentColor"
                  stroke-width="1.5"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              </svg>
            </button>
          </div>
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
              <span class="stat-value mono">{visibleApps().length}</span>
            </div>
          </div>

          <Show
            when={visibleApps().length > 0}
            fallback={
              <div class="today-empty">
                {isToday() ? 'No activity recorded yet. Keep working.' : 'No activity recorded.'}
              </div>
            }
          >
            <div class="app-list">
              <For each={visibleApps()}>
                {(app, i) => {
                  const liveSecs = () =>
                    i() === 0 ? app.total_secs + liveElapsed() : app.total_secs
                  const pct = () => Math.max(2, (liveSecs() / topAppSecs()) * 100)
                  return (
                    <button type="button" class="app-row" onClick={() => setSelectedApp(app)}>
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
                    </button>
                  )
                }}
              </For>
            </div>
          </Show>
        </Show>
      </div>
    </Show>
  )
}
