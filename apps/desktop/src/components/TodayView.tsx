import type { AppUsage, SpaceUsage } from '@record/types'
import { createMemo, createSignal, For, onCleanup, onMount, Show } from 'solid-js'
import { createStore, reconcile } from 'solid-js/store'
import { addProjectExclusion, getAppIcon, getDailySpaces, getDailySummary } from '../lib/api'
import AppDetailView from './AppDetailView'
import { SpaceIcon } from './SpacesView'

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
  const [tab, setTab] = createSignal<'apps' | 'projects'>('apps')
  const [spacesStore, setSpacesStore] = createStore<{ items: SpaceUsage[] }>({ items: [] })
  const [expandedProject, setExpandedProject] = createSignal<string | null>(null)
  const [ignoringProject, setIgnoringProject] = createSignal<string | null>(null)
  let lastFetchTime = 0

  const isToday = () => sameDay(selectedDate(), new Date())

  async function refresh() {
    try {
      const [data, spaces] = await Promise.all([
        getDailySummary(selectedDate()),
        getDailySpaces(selectedDate()),
      ])
      setState({
        activeSecs: data.total_active_secs,
        idleSecs: data.total_idle_secs,
        ready: true,
      })
      setState('apps', reconcile(data.apps, { key: 'bundle_id' }))
      setSpacesStore('items', reconcile(spaces, { key: 'space' }))
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
      const tag = (e.target as HTMLElement)?.tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return
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

        <Show when={state.ready}>
          <div class="today-stats">
            <div class="stat-card">
              <span class="stat-label">Active</span>
              <span class="stat-value mono">{formatTime(activeTime())}</span>
            </div>
            <div class="stat-card">
              <span class="stat-label">Idle</span>
              <span class="stat-value mono">{formatTime(idleTime())}</span>
            </div>
          </div>

          <div class="today-tabs">
            <div class="segment-control">
              <button classList={{ active: tab() === 'apps' }} onClick={() => setTab('apps')}>
                Apps
              </button>
              <button
                classList={{ active: tab() === 'projects' }}
                onClick={() => setTab('projects')}
              >
                Projects
              </button>
            </div>
          </div>

          <Show
            when={tab() === 'apps' ? visibleApps().length > 0 : spacesStore.items.length > 0}
            fallback={
              <div class="today-empty">
                {tab() === 'apps'
                  ? isToday()
                    ? 'No activity recorded yet. Keep working.'
                    : 'No activity recorded.'
                  : 'No projects detected.'}
              </div>
            }
          >
            <Show when={tab() === 'apps'}>
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

            <Show when={tab() === 'projects'}>
              <div class="app-list">
                <For each={spacesStore.items}>
                  {(group) => {
                    const topSecs = () => Math.max(spacesStore.items[0]?.total_secs ?? 1, 1)

                    const isSpaceGroup = () => group.space !== null
                    const groupKey = () => (group.space ? `space:${group.space.id}` : 'ungrouped')
                    const isGroupExpanded = () => expandedProject() === groupKey()

                    return (
                      <Show
                        when={isSpaceGroup()}
                        fallback={
                          <For each={group.projects}>
                            {(project) => {
                              const pct = () => Math.max(2, (project.total_secs / topSecs()) * 100)
                              const projKey = () => `proj:${project.project}`
                              const isExpanded = () => expandedProject() === projKey()
                              const hasDetails = () => project.details.length > 0
                              return (
                                <div class="project-row-wrap">
                                  <button
                                    type="button"
                                    class="app-row"
                                    classList={{ 'project-row-no-details': !hasDetails() }}
                                    onClick={() =>
                                      hasDetails() &&
                                      setExpandedProject(isExpanded() ? null : projKey())
                                    }
                                  >
                                    <div class="app-bar-accent" style={{ height: `${pct()}%` }} />
                                    <div class="project-icon-placeholder">
                                      <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
                                        <rect
                                          x="3"
                                          y="5"
                                          width="14"
                                          height="11"
                                          rx="2"
                                          stroke="currentColor"
                                          stroke-width="1.5"
                                          fill="none"
                                        />
                                        <path
                                          d="M7 5V4a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2v1"
                                          stroke="currentColor"
                                          stroke-width="1.5"
                                          fill="none"
                                        />
                                      </svg>
                                    </div>
                                    <div class="app-body">
                                      <div class="app-info">
                                        <span class="app-name">{project.project}</span>
                                        <span class="app-meta mono">
                                          {formatDuration(project.total_secs)}
                                          <span class="app-sessions">
                                            {project.session_count} session
                                            {project.session_count !== 1 ? 's' : ''}
                                          </span>
                                        </span>
                                      </div>
                                      <div class="app-bar-track">
                                        <div class="app-bar-fill" style={{ width: `${pct()}%` }} />
                                      </div>
                                    </div>
                                  </button>
                                  <button
                                    type="button"
                                    class="project-ignore-btn"
                                    title="Ignore this project"
                                    onClick={(e) => {
                                      e.stopPropagation()
                                      setIgnoringProject(project.project)
                                    }}
                                  >
                                    <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
                                      <circle
                                        cx="8"
                                        cy="8"
                                        r="6.5"
                                        stroke="currentColor"
                                        stroke-width="1.5"
                                      />
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
                                  <Show when={isExpanded()}>
                                    <div class="project-details">
                                      <For each={project.details}>
                                        {(detail) => (
                                          <div class="project-detail-row">
                                            <span class="project-detail-label">{detail.label}</span>
                                            <span class="project-detail-duration mono">
                                              {formatDuration(detail.total_secs)}
                                            </span>
                                          </div>
                                        )}
                                      </For>
                                    </div>
                                  </Show>
                                </div>
                              )
                            }}
                          </For>
                        }
                      >
                        <div>
                          <button
                            type="button"
                            class="app-row"
                            onClick={() =>
                              setExpandedProject(isGroupExpanded() ? null : groupKey())
                            }
                          >
                            <div
                              class="app-bar-accent"
                              style={{
                                height: `${Math.max(2, (group.total_secs / topSecs()) * 100)}%`,
                              }}
                            />
                            <SpaceIcon
                              color={group.space!.color}
                              initials={group.space!.initials}
                              emoji={group.space!.emoji}
                              size={20}
                            />
                            <div class="app-body">
                              <div class="app-info">
                                <span class="app-name">{group.space!.name}</span>
                                <span class="app-meta mono">
                                  {formatDuration(group.total_secs)}
                                  <span class="app-sessions">
                                    {group.projects.length} project
                                    {group.projects.length !== 1 ? 's' : ''}
                                  </span>
                                </span>
                              </div>
                              <div class="app-bar-track">
                                <div
                                  class="app-bar-fill"
                                  style={{
                                    width: `${Math.max(2, (group.total_secs / topSecs()) * 100)}%`,
                                  }}
                                />
                              </div>
                            </div>
                          </button>
                          <Show when={isGroupExpanded()}>
                            <div class="project-details">
                              <For each={group.projects}>
                                {(project) => (
                                  <div class="project-detail-row">
                                    <span class="project-detail-label">{project.project}</span>
                                    <span class="project-detail-duration mono">
                                      {formatDuration(project.total_secs)}
                                    </span>
                                  </div>
                                )}
                              </For>
                            </div>
                          </Show>
                        </div>
                      </Show>
                    )
                  }}
                </For>
              </div>
            </Show>
          </Show>
        </Show>

        <Show when={ignoringProject()}>
          {(project) => (
            <div
              class="modal-overlay"
              role="dialog"
              onClick={() => setIgnoringProject(null)}
              onKeyDown={() => setIgnoringProject(null)}
            >
              {/* biome-ignore lint/a11y/useKeyWithClickEvents: modal stop propagation */}
              <div class="modal" role="dialog" onClick={(e) => e.stopPropagation()}>
                <h3 class="modal-title">Ignore {project()}?</h3>
                <p class="modal-desc">This project won't appear in your activity tracking.</p>
                <div class="modal-actions">
                  <button
                    type="button"
                    class="modal-btn"
                    onClick={async () => {
                      const d = new Date()
                      d.setHours(d.getHours() + 1)
                      await addProjectExclusion(project(), d.toISOString())
                      setIgnoringProject(null)
                      refresh()
                    }}
                  >
                    For 1 hour
                  </button>
                  <button
                    type="button"
                    class="modal-btn"
                    onClick={async () => {
                      const d = new Date()
                      d.setHours(d.getHours() + 4)
                      await addProjectExclusion(project(), d.toISOString())
                      setIgnoringProject(null)
                      refresh()
                    }}
                  >
                    For 4 hours
                  </button>
                  <button
                    type="button"
                    class="modal-btn modal-btn-accent"
                    onClick={async () => {
                      await addProjectExclusion(project())
                      setIgnoringProject(null)
                      refresh()
                    }}
                  >
                    Always
                  </button>
                  <button
                    type="button"
                    class="modal-btn modal-btn-dim"
                    onClick={() => setIgnoringProject(null)}
                  >
                    Cancel
                  </button>
                </div>
              </div>
            </div>
          )}
        </Show>
      </div>
    </Show>
  )
}
