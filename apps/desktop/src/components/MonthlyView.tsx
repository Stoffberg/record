import type { AppUsage, DailySummary } from '@record/types'
import { createMemo, createResource, createSignal, For, onMount, Show } from 'solid-js'
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

function getDaysInMonth(): string[] {
  const now = new Date()
  const year = now.getFullYear()
  const month = now.getMonth()
  const today = now.getDate()
  const days: string[] = []
  for (let d = 1; d <= today; d++) {
    days.push(localDateStr(new Date(year, month, d)))
  }
  return days.reverse()
}

async function fetchMonth(): Promise<DailySummary[]> {
  const days = getDaysInMonth()
  return Promise.all(days.map((d) => getDailySummary(new Date(`${d}T12:00:00`))))
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

function DayApps(props: { apps: AppUsage[] }) {
  const visible = createMemo(() => props.apps.filter((a) => a.total_secs >= 60))
  const topSecs = createMemo(() => {
    const apps = visible()
    if (apps.length === 0) return 1
    return Math.max(apps[0].total_secs, 1)
  })

  return (
    <Show when={visible().length > 0} fallback={<div class="day-detail-empty">No activity</div>}>
      <div class="day-detail-apps">
        <For each={visible()}>
          {(app) => {
            const pct = () => Math.max(2, (app.total_secs / topSecs()) * 100)
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
  )
}

export default function MonthlyView() {
  const [summaries] = createResource(fetchMonth)
  const [selected, setSelected] = createSignal<string | null>(null)

  const maxActive = () => {
    const data = summaries()
    if (!data || data.length === 0) return 1
    return Math.max(...data.map((d) => d.total_active_secs), 1)
  }

  const toggle = (date: string) => {
    setSelected((prev) => (prev === date ? null : date))
  }

  return (
    <div class="monthly-view">
      <header class="monthly-header">
        <h1>{new Date().toLocaleDateString('en-US', { month: 'long', year: 'numeric' })}</h1>
      </header>

      <Show when={summaries()}>
        {(data) => (
          <div class="day-list">
            <For each={data()}>
              {(day) => {
                const pct = () => Math.max(2, (day.total_active_secs / maxActive()) * 100)
                const dayLabel = () => {
                  const d = new Date(`${day.date}T12:00:00`)
                  return d.toLocaleDateString('en-US', { weekday: 'short', day: 'numeric' })
                }
                const isSelected = () => selected() === day.date

                return (
                  <div>
                    <button type="button" class="day-row" onClick={() => toggle(day.date)}>
                      <span class="day-label mono">{dayLabel()}</span>
                      <div class="app-bar-track day-bar-track">
                        <div class="app-bar-fill" style={{ width: `${pct()}%` }} />
                      </div>
                      <span class="day-time mono">{formatTime(day.total_active_secs)}</span>
                    </button>
                    <div classList={{ 'day-expand': true, 'day-expand-open': isSelected() }}>
                      <div>
                        <DayApps apps={day.apps} />
                      </div>
                    </div>
                  </div>
                )
              }}
            </For>
          </div>
        )}
      </Show>
    </div>
  )
}
