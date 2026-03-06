import type { SpaceWithProjects } from '@record/types'
import { createMemo, createSignal, For, Show } from 'solid-js'
import { createStore, reconcile } from 'solid-js/store'
import {
  addProjectToSpace,
  createSpace,
  deleteSpace,
  getAllProjects,
  getSpaces,
  removeProjectFromSpace,
  updateSpace,
} from '../lib/api'

const COLORS = [
  '#5ba5f5',
  '#f56565',
  '#48bb78',
  '#ed8936',
  '#9f7aea',
  '#ed64a6',
  '#38b2ac',
  '#ecc94b',
]

function deriveInitials(name: string): string {
  const words = name.trim().split(/\s+/)
  if (words.length >= 2) return (words[0][0] + words[1][0]).toUpperCase()
  return name.slice(0, 2).toUpperCase()
}

function formatDuration(secs: number): string {
  if (secs < 60) return `${secs}s`
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  if (h > 0) return `${h}h ${m}m`
  return `${m}m`
}

export function SpaceIcon(props: {
  color: string
  initials: string
  emoji: string | null
  size?: number
}) {
  const s = () => props.size ?? 28
  return (
    <div
      class="space-icon"
      style={{
        width: `${s()}px`,
        height: `${s()}px`,
        'border-radius': '6px',
        background: props.emoji ? 'var(--bg-hover)' : props.color,
        display: 'flex',
        'align-items': 'center',
        'justify-content': 'center',
        'font-size': props.emoji ? `${s() * 0.55}px` : `${s() * 0.38}px`,
        'font-weight': '600',
        color: props.emoji ? 'inherit' : '#fff',
        'flex-shrink': '0',
        'line-height': '1',
        'letter-spacing': '-0.02em',
      }}
    >
      {props.emoji ?? props.initials}
    </div>
  )
}

interface ProjectInfo {
  name: string
  totalSecs: number
}

export default function SpacesView() {
  const [store, setStore] = createStore<{ items: SpaceWithProjects[] }>({ items: [] })
  const [expanded, setExpanded] = createSignal<number | null>(null)
  const [showCreate, setShowCreate] = createSignal(false)
  const [editing, setEditing] = createSignal<number | null>(null)

  const [formName, setFormName] = createSignal('')
  const [formColor, setFormColor] = createSignal(COLORS[0])
  const [formEmoji, setFormEmoji] = createSignal('')

  const [knownProjects, setKnownProjects] = createSignal<ProjectInfo[]>([])
  const [search, setSearch] = createSignal('')

  async function refresh() {
    const spaces = await getSpaces()
    setStore('items', reconcile(spaces, { key: 'space' }))
  }

  async function refreshProjects() {
    try {
      const rows = await getAllProjects()
      setKnownProjects(rows.map(([name, totalSecs]) => ({ name, totalSecs })))
    } catch {
      setKnownProjects([])
    }
  }

  refresh()
  refreshProjects()

  const linkedSet = () => {
    const linked = new Set<string>()
    for (const sw of store.items) {
      for (const p of sw.projects) linked.add(p)
    }
    return linked
  }

  const filteredProjects = (spaceId: number) => {
    const sw = store.items.find((s) => s.space.id === spaceId)
    if (!sw) return []
    const linked = linkedSet()
    const mine = new Set(sw.projects)
    const q = search().toLowerCase()
    return knownProjects().filter((p) => {
      if (linked.has(p.name) && !mine.has(p.name)) return false
      if (q && !p.name.toLowerCase().includes(q)) return false
      return true
    })
  }

  const projectDuration = createMemo(() => {
    const map = new Map<string, number>()
    for (const p of knownProjects()) map.set(p.name, p.totalSecs)
    return map
  })

  function openCreate() {
    setFormName('')
    setFormColor(COLORS[Math.floor(Math.random() * COLORS.length)])
    setFormEmoji('')
    setShowCreate(true)
    setExpanded(null)
    setEditing(null)
  }

  async function handleCreate() {
    const name = formName().trim()
    if (!name) return
    const emoji = formEmoji().trim() || undefined
    const space = await createSpace(name, formColor(), deriveInitials(name), emoji)
    setShowCreate(false)
    await refresh()
    setExpanded(space.id)
  }

  function startEdit(sw: SpaceWithProjects) {
    setFormName(sw.space.name)
    setFormColor(sw.space.color)
    setFormEmoji(sw.space.emoji ?? '')
    setEditing(sw.space.id)
  }

  async function handleSave(id: number) {
    const name = formName().trim()
    if (!name) return
    const emoji = formEmoji().trim() || undefined
    await updateSpace(id, name, formColor(), deriveInitials(name), emoji)
    setEditing(null)
    await refresh()
  }

  async function handleDelete(id: number) {
    await deleteSpace(id)
    setExpanded(null)
    setEditing(null)
    await refresh()
  }

  async function handleLink(spaceId: number, project: string) {
    await addProjectToSpace(spaceId, project)
    await refresh()
  }

  async function handleUnlink(spaceId: number, project: string) {
    await removeProjectFromSpace(spaceId, project)
    await refresh()
  }

  function toggleExpand(id: number) {
    if (expanded() === id) {
      setExpanded(null)
      setEditing(null)
    } else {
      setExpanded(id)
      setEditing(null)
      setSearch('')
      refreshProjects()
    }
    setShowCreate(false)
  }

  return (
    <div class="spaces-view">
      <header class="spaces-view-header">
        <h1>Spaces</h1>
        <button type="button" class="spaces-new-btn" onClick={openCreate}>
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
            <path
              d="M8 3v10M3 8h10"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
            />
          </svg>
          New space
        </button>
      </header>

      <Show when={showCreate()}>
        <div class="space-create-form">
          <div class="space-form-top">
            <SpaceIcon
              color={formColor()}
              initials={deriveInitials(formName() || 'AB')}
              emoji={formEmoji().trim() || null}
              size={36}
            />
            <input
              type="text"
              class="space-name-input"
              placeholder="Space name"
              value={formName()}
              onInput={(e) => setFormName(e.currentTarget.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleCreate()
                if (e.key === 'Escape') setShowCreate(false)
              }}
              autofocus
            />
          </div>
          <div class="space-form-options">
            <input
              type="text"
              class="space-emoji-input"
              placeholder="Emoji"
              value={formEmoji()}
              onInput={(e) => setFormEmoji(e.currentTarget.value)}
              maxLength={2}
            />
            <div class="space-color-dots">
              <For each={COLORS}>
                {(c) => (
                  <button
                    type="button"
                    class="space-color-dot"
                    classList={{ active: formColor() === c }}
                    style={{ background: c }}
                    onClick={() => setFormColor(c)}
                  />
                )}
              </For>
            </div>
            <div style={{ flex: '1' }} />
            <button type="button" class="space-btn-ghost" onClick={() => setShowCreate(false)}>
              Cancel
            </button>
            <button type="button" class="space-btn-primary" onClick={handleCreate}>
              Create
            </button>
          </div>
        </div>
      </Show>

      <div class="spaces-list">
        <For each={store.items}>
          {(sw) => {
            const isExpanded = () => expanded() === sw.space.id
            const isEditing = () => editing() === sw.space.id
            return (
              <div class="space-item" classList={{ 'space-item-expanded': isExpanded() }}>
                <button
                  type="button"
                  class="space-item-row"
                  onClick={() => toggleExpand(sw.space.id)}
                >
                  <SpaceIcon
                    color={sw.space.color}
                    initials={sw.space.initials}
                    emoji={sw.space.emoji}
                    size={28}
                  />
                  <div class="space-item-body">
                    <span class="space-item-name">{sw.space.name}</span>
                    <span class="space-item-meta">
                      {sw.projects.length} project{sw.projects.length !== 1 ? 's' : ''}
                    </span>
                  </div>
                  <svg
                    class="space-item-chevron"
                    classList={{ 'space-item-chevron-open': isExpanded() }}
                    width="14"
                    height="14"
                    viewBox="0 0 16 16"
                    fill="none"
                  >
                    <path
                      d="M4 6l4 4 4-4"
                      stroke="currentColor"
                      stroke-width="1.5"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    />
                  </svg>
                </button>

                <Show when={isExpanded()}>
                  <div class="space-item-detail">
                    <Show
                      when={!isEditing()}
                      fallback={
                        <div class="space-edit-form">
                          <div class="space-form-top">
                            <SpaceIcon
                              color={formColor()}
                              initials={deriveInitials(formName() || 'AB')}
                              emoji={formEmoji().trim() || null}
                              size={28}
                            />
                            <input
                              type="text"
                              class="space-name-input"
                              value={formName()}
                              onInput={(e) => setFormName(e.currentTarget.value)}
                              onKeyDown={(e) => {
                                if (e.key === 'Enter') handleSave(sw.space.id)
                                if (e.key === 'Escape') setEditing(null)
                              }}
                              autofocus
                            />
                          </div>
                          <div class="space-form-options">
                            <input
                              type="text"
                              class="space-emoji-input"
                              placeholder="Emoji"
                              value={formEmoji()}
                              onInput={(e) => setFormEmoji(e.currentTarget.value)}
                              maxLength={2}
                            />
                            <div class="space-color-dots">
                              <For each={COLORS}>
                                {(c) => (
                                  <button
                                    type="button"
                                    class="space-color-dot"
                                    classList={{ active: formColor() === c }}
                                    style={{ background: c }}
                                    onClick={() => setFormColor(c)}
                                  />
                                )}
                              </For>
                            </div>
                            <div style={{ flex: '1' }} />
                            <button
                              type="button"
                              class="space-btn-danger"
                              onClick={() => handleDelete(sw.space.id)}
                            >
                              Delete
                            </button>
                            <button
                              type="button"
                              class="space-btn-ghost"
                              onClick={() => setEditing(null)}
                            >
                              Cancel
                            </button>
                            <button
                              type="button"
                              class="space-btn-primary"
                              onClick={() => handleSave(sw.space.id)}
                            >
                              Save
                            </button>
                          </div>
                        </div>
                      }
                    >
                      <div class="space-detail-actions">
                        <button type="button" class="space-btn-ghost" onClick={() => startEdit(sw)}>
                          Edit
                        </button>
                      </div>
                    </Show>

                    <Show when={!isEditing()}>
                      <Show when={sw.projects.length > 0}>
                        <div class="space-linked-projects">
                          <For each={sw.projects}>
                            {(project) => (
                              <div class="space-project-chip">
                                <span>{project}</span>
                                <span class="space-project-chip-duration mono">
                                  {formatDuration(projectDuration().get(project) ?? 0)}
                                </span>
                                <button
                                  type="button"
                                  class="space-project-chip-x"
                                  onClick={() => handleUnlink(sw.space.id, project)}
                                >
                                  <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
                                    <path
                                      d="M2.5 2.5l5 5M7.5 2.5l-5 5"
                                      stroke="currentColor"
                                      stroke-width="1.5"
                                      stroke-linecap="round"
                                    />
                                  </svg>
                                </button>
                              </div>
                            )}
                          </For>
                        </div>
                      </Show>

                      <div class="space-available-section">
                        <span class="space-available-label">Link a project</span>
                        <input
                          type="text"
                          class="space-search-input"
                          placeholder="Search projects..."
                          value={search()}
                          onInput={(e) => setSearch(e.currentTarget.value)}
                        />
                        <div class="space-available-list">
                          <For each={filteredProjects(sw.space.id)}>
                            {(project) => {
                              const isLinked = () => sw.projects.includes(project.name)
                              return (
                                <button
                                  type="button"
                                  class="space-available-item"
                                  classList={{ linked: isLinked() }}
                                  onClick={() =>
                                    isLinked()
                                      ? handleUnlink(sw.space.id, project.name)
                                      : handleLink(sw.space.id, project.name)
                                  }
                                >
                                  <span class="space-available-check">
                                    <Show when={isLinked()}>
                                      <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                                        <path
                                          d="M2.5 6l2.5 2.5 5-5"
                                          stroke="currentColor"
                                          stroke-width="1.5"
                                          stroke-linecap="round"
                                          stroke-linejoin="round"
                                        />
                                      </svg>
                                    </Show>
                                  </span>
                                  <span class="space-available-name">{project.name}</span>
                                  <span class="space-available-duration mono">
                                    {formatDuration(project.totalSecs)}
                                  </span>
                                </button>
                              )
                            }}
                          </For>
                          <Show when={filteredProjects(sw.space.id).length === 0}>
                            <span class="spaces-detail-empty">
                              {search()
                                ? 'No projects match your search.'
                                : 'No projects detected yet. Use the app and run a backfill.'}
                            </span>
                          </Show>
                        </div>
                      </div>
                    </Show>
                  </div>
                </Show>
              </div>
            )
          }}
        </For>
      </div>

      <Show when={store.items.length === 0 && !showCreate()}>
        <div class="spaces-empty">
          Spaces group related projects together for tracking and reporting. Create one to get
          started.
        </div>
      </Show>
    </div>
  )
}
