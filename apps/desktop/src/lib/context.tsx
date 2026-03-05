import type { JSX } from 'solid-js'
import { createContext, createEffect, createSignal, onMount, useContext } from 'solid-js'
import { checkAccessibility, getAutoStartEnabled, setAutoStart as setAutoStartApi } from './api'

export type Theme = 'system' | 'light' | 'dark'

interface AppState {
  autoStart: () => boolean
  toggleAutoStart: () => Promise<void>
  theme: () => Theme
  setTheme: (theme: Theme) => void
  accessibilityGranted: () => boolean
  recheckAccessibility: () => Promise<boolean>
  onboardingDone: () => boolean
  completeOnboarding: () => void
}

const THEME_KEY = 'record-theme'
const ONBOARDING_KEY = 'record-onboarding-done'

function applyTheme(theme: Theme) {
  document.documentElement.setAttribute('data-theme', theme)
}

function loadTheme(): Theme {
  const stored = localStorage.getItem(THEME_KEY)
  if (stored === 'light' || stored === 'dark' || stored === 'system') return stored
  return 'system'
}

const AppContext = createContext<AppState>()

export function AppProvider(props: { children: JSX.Element; fallback?: JSX.Element }) {
  const [ready, setReady] = createSignal(false)
  const [autoStart, setAutoStart] = createSignal(false)
  const [theme, setThemeSignal] = createSignal<Theme>(loadTheme())
  const [accessibilityGranted, setAccessibilityGranted] = createSignal(false)
  const [onboardingDone, setOnboardingDone] = createSignal(
    localStorage.getItem(ONBOARDING_KEY) === 'true',
  )

  applyTheme(theme())

  const setTheme = (next: Theme) => {
    setThemeSignal(next)
    localStorage.setItem(THEME_KEY, next)
  }

  createEffect(() => {
    applyTheme(theme())
  })

  const recheckAccessibility = async () => {
    const granted = await checkAccessibility()
    setAccessibilityGranted(granted)
    return granted
  }

  const completeOnboarding = () => {
    setOnboardingDone(true)
    localStorage.setItem(ONBOARDING_KEY, 'true')
  }

  onMount(async () => {
    try {
      const [autoStartEnabled, accessible] = await Promise.all([
        getAutoStartEnabled(),
        checkAccessibility(),
      ])
      setAutoStart(autoStartEnabled)
      setAccessibilityGranted(accessible)
      if (!accessible) {
        setOnboardingDone(false)
        localStorage.removeItem(ONBOARDING_KEY)
      }
    } finally {
      setReady(true)
    }
  })

  const toggleAutoStart = async () => {
    const next = !autoStart()
    setAutoStart(next)
    try {
      await setAutoStartApi(next)
    } catch {
      setAutoStart(!next)
    }
  }

  const state: AppState = {
    autoStart,
    toggleAutoStart,
    theme,
    setTheme,
    accessibilityGranted,
    recheckAccessibility,
    onboardingDone,
    completeOnboarding,
  }

  return (
    <AppContext.Provider value={state}>
      {ready() ? props.children : (props.fallback ?? null)}
    </AppContext.Provider>
  )
}

export function useApp(): AppState {
  const ctx = useContext(AppContext)
  if (!ctx) throw new Error('useApp must be used within AppProvider')
  return ctx
}
