import type { AppSession, AppUsage } from '@record/types'
import { createMemo, createResource, createSignal, For, onMount, Show } from 'solid-js'
import { addExclusion, getAppAverages, getAppIcon, getAppSessions } from '../lib/api'

function formatDuration(secs: number): string {
  if (secs < 60) return `${secs}s`
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  if (h > 0) return `${h}h ${m}m`
  return `${m}m`
}

function formatHour(h: number): string {
  if (h === 0) return '12a'
  if (h < 12) return `${h}a`
  if (h === 12) return '12p'
  return `${h - 12}p`
}

function AppIcon(props: { bundleId: string; size?: number }) {
  const [src, setSrc] = createSignal<string | null>(null)
  const size = () => props.size ?? 32

  onMount(async () => {
    const b64 = await getAppIcon(props.bundleId)
    if (b64) setSrc(`data:image/png;base64,${b64}`)
  })

  return (
    <Show
      when={src()}
      fallback={
        <div class="app-icon-placeholder">
          <svg width={size()} height={size()} viewBox="0 0 20 20" fill="none">
            <rect x="2" y="2" width="16" height="16" rx="4" fill="currentColor" opacity="0.15" />
          </svg>
        </div>
      }
    >
      <img class="app-icon" src={src()!} alt="" width={size()} height={size()} />
    </Show>
  )
}

function Timeline(props: { sessions: AppSession[] }) {
  const range = createMemo(() => {
    const sessions = props.sessions
    if (sessions.length === 0) return { startHour: 8, endHour: 20 }

    const firstStart = new Date(sessions[0].started_at)
    const lastEnd = new Date(sessions[sessions.length - 1].ended_at)
    const now = new Date()
    const endTime = lastEnd > now ? lastEnd : now

    const startHour = Math.max(0, firstStart.getHours() - 1)
    const endHour = Math.min(24, endTime.getHours() + 2)

    return { startHour, endHour }
  })

  const blocks = createMemo(() => {
    const { startHour, endHour } = range()
    const totalMinutes = (endHour - startHour) * 60

    return props.sessions.map((s) => {
      const start = new Date(s.started_at)
      const end = new Date(s.ended_at)
      const startMin = (start.getHours() - startHour) * 60 + start.getMinutes()
      const endMin = (end.getHours() - startHour) * 60 + end.getMinutes()
      const left = Math.max(0, (startMin / totalMinutes) * 100)
      const width = Math.max(0.5, ((endMin - startMin) / totalMinutes) * 100)
      return { left, width }
    })
  })

  const hourMarks = createMemo(() => {
    const { startHour, endHour } = range()
    const totalHours = endHour - startHour
    const marks: { label: string; left: number }[] = []
    const step = totalHours > 12 ? 3 : totalHours > 6 ? 2 : 1
    for (let h = startHour; h <= endHour; h += step) {
      marks.push({
        label: formatHour(h),
        left: ((h - startHour) / totalHours) * 100,
      })
    }
    return marks
  })

  return (
    <div class="timeline">
      <div class="timeline-track">
        <For each={blocks()}>
          {(block) => (
            <div
              class="timeline-block"
              style={{ left: `${block.left}%`, width: `${block.width}%` }}
            />
          )}
        </For>
      </div>
      <div class="timeline-labels">
        <For each={hourMarks()}>
          {(mark) => (
            <span class="timeline-hour mono" style={{ left: `${mark.left}%` }}>
              {mark.label}
            </span>
          )}
        </For>
      </div>
    </div>
  )
}

interface IgnoreModalProps {
  appName: string
  bundleId: string
  onClose: () => void
  onIgnored: () => void
}

function IgnoreModal(props: IgnoreModalProps) {
  const handleIgnore = async (hours?: number) => {
    let expiresAt: string | undefined
    if (hours) {
      const d = new Date()
      d.setHours(d.getHours() + hours)
      expiresAt = d.toISOString()
    }
    await addExclusion(props.bundleId, props.appName, expiresAt)
    props.onIgnored()
  }

  return (
    <div class="modal-overlay" role="dialog" onClick={props.onClose} onKeyDown={props.onClose}>
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: modal stop propagation */}
      <div class="modal" role="dialog" onClick={(e) => e.stopPropagation()}>
        <h3 class="modal-title">Ignore {props.appName}?</h3>
        <p class="modal-desc">This app won't appear in your activity tracking.</p>
        <div class="modal-actions">
          <button type="button" class="modal-btn" onClick={() => handleIgnore(1)}>
            For 1 hour
          </button>
          <button type="button" class="modal-btn" onClick={() => handleIgnore(4)}>
            For 4 hours
          </button>
          <button type="button" class="modal-btn modal-btn-accent" onClick={() => handleIgnore()}>
            Always
          </button>
          <button type="button" class="modal-btn modal-btn-dim" onClick={props.onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  )
}

interface Props {
  app: AppUsage
  onBack: () => void
}

export default function AppDetailView(props: Props) {
  const [showIgnore, setShowIgnore] = createSignal(false)
  const [sessions] = createResource(
    () => props.app.bundle_id,
    (bundleId) => getAppSessions(new Date(), bundleId),
  )
  const [averages] = createResource(
    () => props.app.bundle_id,
    (bundleId) => getAppAverages(new Date(), bundleId),
  )

  const stats = createMemo(() => {
    const s = sessions()
    if (!s || s.length === 0) return null
    const longest = Math.max(...s.map((x) => x.duration_secs))
    const first = new Date(s[0].started_at)
    const last = new Date(s[s.length - 1].ended_at)
    return {
      sessions: s.length,
      longest,
      firstUsed: first.toLocaleTimeString('en-US', {
        hour: 'numeric',
        minute: '2-digit',
        hour12: true,
      }),
      lastUsed: last.toLocaleTimeString('en-US', {
        hour: 'numeric',
        minute: '2-digit',
        hour12: true,
      }),
    }
  })

  return (
    <div class="app-detail-view">
      <header class="app-detail-header">
        <button type="button" class="app-detail-back" onClick={props.onBack}>
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
        <AppIcon bundleId={props.app.bundle_id} size={28} />
        <div class="app-detail-title">
          <h1>{props.app.app_name}</h1>
          <span class="app-detail-subtitle mono">{formatDuration(props.app.total_secs)} today</span>
        </div>
        <div class="app-detail-spacer" />
        <button
          type="button"
          class="app-detail-ignore"
          onClick={() => setShowIgnore(true)}
          title="Ignore this app"
        >
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
            <circle cx="8" cy="8" r="6.5" stroke="currentColor" stroke-width="1.5" />
            <line
              x1="3.5"
              y1="12.5"
              x2="12.5"
              y2="3.5"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
            />
          </svg>
        </button>
      </header>

      <Show
        when={sessions() && sessions()!.length > 0}
        fallback={<div class="today-empty">No sessions recorded today.</div>}
      >
        <Timeline sessions={sessions()!} />

        <Show when={stats()}>
          {(s) => (
            <div class="detail-stats">
              <div class="detail-stat">
                <span class="detail-stat-label">Sessions</span>
                <span class="detail-stat-value mono">{s().sessions}</span>
              </div>
              <div class="detail-stat">
                <span class="detail-stat-label">Longest</span>
                <span class="detail-stat-value mono">{formatDuration(s().longest)}</span>
              </div>
              <div class="detail-stat">
                <span class="detail-stat-label">First used</span>
                <span class="detail-stat-value mono">{s().firstUsed}</span>
              </div>
              <div class="detail-stat">
                <span class="detail-stat-label">Last used</span>
                <span class="detail-stat-value mono">{s().lastUsed}</span>
              </div>
              <Show when={averages()}>
                {(avg) => (
                  <>
                    <div class="detail-stat">
                      <span class="detail-stat-label">7 day avg</span>
                      <span class="detail-stat-value mono">
                        {formatDuration(Math.round(avg()[0]))}
                      </span>
                    </div>
                    <div class="detail-stat">
                      <span class="detail-stat-label">30 day avg</span>
                      <span class="detail-stat-value mono">
                        {formatDuration(Math.round(avg()[1]))}
                      </span>
                    </div>
                  </>
                )}
              </Show>
            </div>
          )}
        </Show>
      </Show>

      <Show when={showIgnore()}>
        <IgnoreModal
          appName={props.app.app_name}
          bundleId={props.app.bundle_id}
          onClose={() => setShowIgnore(false)}
          onIgnored={props.onBack}
        />
      </Show>
    </div>
  )
}
