import { createSignal, lazy, onCleanup, onMount, Show, Suspense } from 'solid-js'
import { Dynamic } from 'solid-js/web'
import OnboardingView from './components/OnboardingView'
import { useApp } from './lib/context'
import './index.css'

const TodayView = lazy(() => import('./components/TodayView'))
const WeeklyView = lazy(() => import('./components/WeeklyView'))
const MonthlyView = lazy(() => import('./components/MonthlyView'))
const SettingsView = lazy(() => import('./components/SettingsView'))

type View = 'today' | 'weekly' | 'monthly' | 'settings'

const viewOrder: View[] = ['today', 'weekly', 'monthly', 'settings']

const views = {
  today: TodayView,
  weekly: WeeklyView,
  monthly: MonthlyView,
  settings: SettingsView,
} as const

function App() {
  const { onboardingDone } = useApp()
  const [view, setView] = createSignal<View>('today')
  const [transitioning, setTransitioning] = createSignal(false)

  function switchView(next: View) {
    if (next === view()) return
    setTransitioning(true)
    requestAnimationFrame(() => {
      setTimeout(() => {
        setView(next)
        requestAnimationFrame(() => {
          setTransitioning(false)
        })
      }, 100)
    })
  }

  function preloadView(v: View) {
    views[v].preload()
  }

  onMount(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.metaKey && e.key >= '1' && e.key <= '4') {
        e.preventDefault()
        const target = viewOrder[Number.parseInt(e.key, 10) - 1]
        if (target) switchView(target)
      }
    }
    window.addEventListener('keydown', handler)
    onCleanup(() => window.removeEventListener('keydown', handler))
  })

  return (
    <Show when={onboardingDone()} fallback={<OnboardingView />}>
      <div class="app">
        <nav class="sidebar">
          <div class="logo">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
              <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="2" />
              <circle cx="12" cy="12" r="4" fill="var(--accent)" />
              <circle
                cx="12"
                cy="12"
                r="7"
                stroke="var(--accent)"
                stroke-width="1.5"
                opacity="0.3"
              />
            </svg>
            Record
          </div>
          <button
            classList={{ active: view() === 'today' }}
            onClick={() => switchView('today')}
            onMouseEnter={() => preloadView('today')}
            onFocus={() => preloadView('today')}
          >
            <svg width="15" height="15" viewBox="0 0 16 16" fill="none">
              <rect
                x="1.5"
                y="1.5"
                width="13"
                height="13"
                rx="2"
                stroke="currentColor"
                stroke-width="1.5"
              />
              <line x1="1.5" y1="5.5" x2="14.5" y2="5.5" stroke="currentColor" stroke-width="1.5" />
              <circle cx="8" cy="10" r="1.5" fill="currentColor" />
            </svg>
            Today
          </button>
          <button
            classList={{ active: view() === 'weekly' }}
            onClick={() => switchView('weekly')}
            onMouseEnter={() => preloadView('weekly')}
            onFocus={() => preloadView('weekly')}
          >
            <svg width="15" height="15" viewBox="0 0 16 16" fill="none">
              <rect x="1" y="9" width="2" height="5" rx="0.75" fill="currentColor" opacity="0.4" />
              <rect x="4" y="6" width="2" height="8" rx="0.75" fill="currentColor" opacity="0.5" />
              <rect x="7" y="3" width="2" height="11" rx="0.75" fill="currentColor" opacity="0.7" />
              <rect x="10" y="7" width="2" height="7" rx="0.75" fill="currentColor" opacity="0.6" />
              <rect x="13" y="5" width="2" height="9" rx="0.75" fill="currentColor" opacity="0.8" />
            </svg>
            Weekly
          </button>
          <button
            classList={{ active: view() === 'monthly' }}
            onClick={() => switchView('monthly')}
            onMouseEnter={() => preloadView('monthly')}
            onFocus={() => preloadView('monthly')}
          >
            <svg width="15" height="15" viewBox="0 0 16 16" fill="none">
              <rect x="2" y="5" width="3" height="9" rx="1" fill="currentColor" opacity="0.4" />
              <rect x="6.5" y="2" width="3" height="12" rx="1" fill="currentColor" opacity="0.6" />
              <rect x="11" y="7" width="3" height="7" rx="1" fill="currentColor" opacity="0.8" />
            </svg>
            Monthly
          </button>
          <div class="sidebar-spacer" />
          <button
            classList={{ active: view() === 'settings' }}
            onClick={() => switchView('settings')}
            onMouseEnter={() => preloadView('settings')}
            onFocus={() => preloadView('settings')}
          >
            <svg width="15" height="15" viewBox="0 0 16 16" fill="none">
              <circle cx="8" cy="8" r="2.5" stroke="currentColor" stroke-width="1.5" />
              <path
                d="M8 1.5v1.5M8 13v1.5M1.5 8H3M13 8h1.5M3.4 3.4l1.1 1.1M11.5 11.5l1.1 1.1M3.4 12.6l1.1-1.1M11.5 4.5l1.1-1.1"
                stroke="currentColor"
                stroke-width="1.2"
                stroke-linecap="round"
              />
            </svg>
            Settings
          </button>
        </nav>
        <main class="content">
          <Suspense>
            <div classList={{ 'view-container': true, 'view-fade-out': transitioning() }}>
              <Dynamic component={views[view()]} />
            </div>
          </Suspense>
        </main>
      </div>
    </Show>
  )
}

export default App
