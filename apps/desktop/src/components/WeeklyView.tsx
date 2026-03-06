import type { AppUsage, DailySummary } from '@record/types'
import { createMemo, createSignal, For, onCleanup, onMount, Show } from 'solid-js'
import { getAppIcon, getDailySummary, localDateStr } from '../lib/api'

function formatTime(secs: number): string {
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  return `${h}h ${m}m`
}

function formatDuration(secs: number): string {
  if (secs < 60) return `${secs}s`
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  if (h > 0) return `${h}h ${m}m`
  return `${m}m`
}

function getMonday(date: Date): Date {
  const d = new Date(date)
  const day = d.getDay()
  const diff = day === 0 ? -6 : 1 - day
  d.setDate(d.getDate() + diff)
  d.setHours(0, 0, 0, 0)
  return d
}

function addDays(date: Date, days: number): Date {
  const d = new Date(date)
  d.setDate(d.getDate() + days)
  return d
}

function sameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  )
}

function weekLabel(monday: Date): string {
  const now = new Date()
  const thisMonday = getMonday(now)
  if (sameDay(monday, thisMonday)) return 'This Week'
  if (sameDay(monday, addDays(thisMonday, -7))) return 'Last Week'
  const sunday = addDays(monday, 6)
  const fmt = (d: Date) => d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
  return `${fmt(monday)} \u2013 ${fmt(sunday)}`
}

const DAY_NAMES = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']

interface WeekData {
  days: DailySummary[]
  totalActive: number
  totalIdle: number
  appTotals: AppUsage[]
}

function aggregateWeek(summaries: DailySummary[]): WeekData {
  let totalActive = 0
  let totalIdle = 0
  const appMap = new Map<string, AppUsage>()

  for (const day of summaries) {
    totalActive += day.total_active_secs
    totalIdle += day.total_idle_secs
    for (const app of day.apps) {
      const existing = appMap.get(app.bundle_id)
      if (existing) {
        existing.total_secs += app.total_secs
        existing.session_count += app.session_count
      } else {
        appMap.set(app.bundle_id, { ...app })
      }
    }
  }

  const appTotals = [...appMap.values()]
    .filter((a) => a.total_secs >= 60)
    .sort((a, b) => b.total_secs - a.total_secs)

  return { days: summaries, totalActive, totalIdle, appTotals }
}

function emptySummary(date: Date): DailySummary {
  return { date: localDateStr(date), total_active_secs: 0, total_idle_secs: 0, apps: [] }
}

async function fetchWeek(monday: Date): Promise<WeekData> {
  const today = new Date()
  const allDays = Array.from({ length: 7 }, (_, i) => addDays(monday, i))
  const summaries = await Promise.all(
    allDays.map((d) => (d <= today ? getDailySummary(d) : Promise.resolve(emptySummary(d)))),
  )
  return aggregateWeek(summaries)
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
          <svg width="16" height="16" viewBox="0 0 20 20" fill="none">
            <rect x="2" y="2" width="16" height="16" rx="4" fill="currentColor" opacity="0.15" />
          </svg>
        </div>
      }
    >
      <img class="app-icon" src={src()!} alt="" width="16" height="16" />
    </Show>
  )
}

export default function WeeklyView() {
  const [monday, setMonday] = createSignal(getMonday(new Date()))
  const [week, setWeek] = createSignal<WeekData | null>(null)
  const [prevWeek, setPrevWeek] = createSignal<WeekData | null>(null)
  const [loading, setLoading] = createSignal(true)

  const isThisWeek = () => sameDay(monday(), getMonday(new Date()))

  async function load() {
    setLoading(true)
    const [current, prior] = await Promise.all([
      fetchWeek(monday()),
      fetchWeek(addDays(monday(), -7)),
    ])
    setWeek(current)
    setPrevWeek(prior)
    setLoading(false)
  }

  function goToPreviousWeek() {
    setMonday(addDays(monday(), -7))
    load()
  }

  function goToNextWeek() {
    if (isThisWeek()) return
    setMonday(addDays(monday(), 7))
    load()
  }

  onMount(() => {
    load()
    const keyHandler = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey) return
      if (e.key === 'ArrowLeft') {
        e.preventDefault()
        goToPreviousWeek()
      } else if (e.key === 'ArrowRight') {
        e.preventDefault()
        goToNextWeek()
      }
    }
    window.addEventListener('keydown', keyHandler)
    onCleanup(() => window.removeEventListener('keydown', keyHandler))
  })

  const maxDayActive = createMemo(() => {
    const w = week()
    if (!w) return 1
    return Math.max(...w.days.map((d) => d.total_active_secs), 1)
  })

  const topAppSecs = createMemo(() => {
    const w = week()
    if (!w || w.appTotals.length === 0) return 1
    return Math.max(w.appTotals[0].total_secs, 1)
  })

  const comparison = createMemo(() => {
    const curr = week()
    const prev = prevWeek()
    if (!curr || !prev || prev.totalActive === 0) return null
    const diff = curr.totalActive - prev.totalActive
    const pct = Math.round(Math.abs(diff / prev.totalActive) * 100)
    if (pct === 0) return null
    return { diff, pct, direction: diff > 0 ? 'up' : 'down' }
  })

  return (
    <div class="weekly-view">
      <header class="weekly-header">
        <div class="date-nav">
          <button type="button" class="date-nav-btn" onClick={goToPreviousWeek}>
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
            <span class="date-nav-heading">{weekLabel(monday())}</span>
          </span>
          <button type="button" class="date-nav-btn" onClick={goToNextWeek} disabled={isThisWeek()}>
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

      <Show when={!loading() && week()} fallback={<div class="today-empty">Loading...</div>}>
        {(w) => (
          <>
            <div class="weekly-stats">
              <div class="stat-card">
                <span class="stat-label">Active</span>
                <span class="stat-value mono">{formatTime(w().totalActive)}</span>
              </div>
              <div class="stat-card">
                <span class="stat-label">Idle</span>
                <span class="stat-value mono">{formatTime(w().totalIdle)}</span>
              </div>
              <div class="stat-card">
                <span class="stat-label">Daily avg</span>
                <span class="stat-value mono">
                  {formatTime(Math.round(w().totalActive / Math.max(w().days.length, 1)))}
                </span>
              </div>
            </div>

            <Show when={comparison()}>
              {(c) => (
                <div class="weekly-comparison">
                  <span class="weekly-comparison-text">
                    {c().pct}% {c().direction === 'up' ? 'more' : 'less'} than the prior week
                  </span>
                </div>
              )}
            </Show>

            <div class="weekly-bars">
              <For each={DAY_NAMES}>
                {(name, i) => {
                  const day = () => w().days[i()]
                  const secs = () => day()?.total_active_secs ?? 0
                  const pct = () => Math.max(0, (secs() / maxDayActive()) * 100)
                  const today = new Date()
                  const dayDate = () => addDays(monday(), i())
                  const isFuture = () => dayDate() > today
                  const isToday = () => sameDay(dayDate(), today)

                  return (
                    <div
                      classList={{
                        'weekly-bar-col': true,
                        'weekly-bar-future': isFuture(),
                        'weekly-bar-today': isToday(),
                      }}
                    >
                      <div class="weekly-bar-track">
                        <div class="weekly-bar-fill" style={{ height: `${pct()}%` }} />
                      </div>
                      <span class="weekly-bar-label mono">{name}</span>
                      <Show when={!isFuture()}>
                        <span class="weekly-bar-time mono">{formatTime(secs())}</span>
                      </Show>
                    </div>
                  )
                }}
              </For>
            </div>

            <Show when={w().appTotals.length > 0}>
              <h2 class="weekly-section-title">Top Apps</h2>
              <div class="weekly-app-list">
                <For each={w().appTotals.slice(0, 10)}>
                  {(app) => {
                    const pct = () => Math.max(2, (app.total_secs / topAppSecs()) * 100)
                    return (
                      <div class="day-detail-row">
                        <AppIcon bundleId={app.bundle_id} />
                        <span class="day-detail-name">{app.app_name}</span>
                        <div class="app-bar-track day-detail-bar">
                          <div class="app-bar-fill" style={{ width: `${pct()}%` }} />
                        </div>
                        <span class="day-detail-time mono">{formatDuration(app.total_secs)}</span>
                      </div>
                    )
                  }}
                </For>
              </div>
            </Show>
          </>
        )}
      </Show>
    </div>
  )
}
