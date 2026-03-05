import { createSignal, lazy } from 'solid-js'
import { Dynamic } from 'solid-js/web'
import './index.css'

const TodayView = lazy(() => import('./components/TodayView'))
const MonthlyView = lazy(() => import('./components/MonthlyView'))
const SettingsView = lazy(() => import('./components/SettingsView'))

type View = 'today' | 'monthly' | 'settings'

const views = {
  today: TodayView,
  monthly: MonthlyView,
  settings: SettingsView,
} as const

function App() {
  const [view, setView] = createSignal<View>('today')

  return (
    <div class="app">
      <nav class="sidebar">
        <div class="logo">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
            <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="2" />
            <circle cx="12" cy="12" r="4" fill="var(--accent)" />
            <circle cx="12" cy="12" r="7" stroke="var(--accent)" stroke-width="1.5" opacity="0.3" />
          </svg>
          Record
        </div>
        <button classList={{ active: view() === 'today' }} onClick={() => setView('today')}>
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
        <button classList={{ active: view() === 'monthly' }} onClick={() => setView('monthly')}>
          <svg width="15" height="15" viewBox="0 0 16 16" fill="none">
            <rect x="2" y="5" width="3" height="9" rx="1" fill="currentColor" opacity="0.4" />
            <rect x="6.5" y="2" width="3" height="12" rx="1" fill="currentColor" opacity="0.6" />
            <rect x="11" y="7" width="3" height="7" rx="1" fill="currentColor" opacity="0.8" />
          </svg>
          Monthly
        </button>
        <div class="sidebar-spacer" />
        <button classList={{ active: view() === 'settings' }} onClick={() => setView('settings')}>
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
        <Dynamic component={views[view()]} />
      </main>
    </div>
  )
}

export default App
