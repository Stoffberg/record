import type { DailySummary } from '@record/types'
import { createResource, For, Show } from 'solid-js'
import { getDailySummary } from '../lib/api'

function formatTime(secs: number): string {
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  return `${h}h ${m}m`
}

function getDaysInMonth(): string[] {
  const now = new Date()
  const year = now.getFullYear()
  const month = now.getMonth()
  const today = now.getDate()
  const days: string[] = []
  for (let d = 1; d <= today; d++) {
    const date = new Date(year, month, d)
    days.push(date.toISOString().split('T')[0])
  }
  return days.reverse()
}

async function fetchMonth(): Promise<DailySummary[]> {
  const days = getDaysInMonth()
  const results = await Promise.all(days.map((d) => getDailySummary(new Date(`${d}T12:00:00`))))
  return results
}

export default function MonthlyView() {
  const [summaries] = createResource(fetchMonth)

  const maxActive = () => {
    const data = summaries()
    if (!data || data.length === 0) return 1
    return Math.max(...data.map((d) => d.total_active_secs), 1)
  }

  return (
    <div class="monthly-view">
      <header class="monthly-header">
        <h1>{new Date().toLocaleDateString('en-US', { month: 'long', year: 'numeric' })}</h1>
      </header>

      <Show when={summaries()} fallback={<div class="today-empty">Loading...</div>}>
        {(data) => (
          <div class="day-list">
            <For each={data()}>
              {(day) => {
                const pct = () => Math.max(2, (day.total_active_secs / maxActive()) * 100)
                const dayLabel = () => {
                  const d = new Date(`${day.date}T12:00:00`)
                  return d.toLocaleDateString('en-US', { weekday: 'short', day: 'numeric' })
                }

                return (
                  <div class="day-row">
                    <span class="day-label mono">{dayLabel()}</span>
                    <div class="app-bar-track day-bar-track">
                      <div class="app-bar-fill" style={{ width: `${pct()}%` }} />
                    </div>
                    <span class="day-time mono">{formatTime(day.total_active_secs)}</span>
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
