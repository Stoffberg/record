import type { JSX } from 'solid-js'
import { createContext, createSignal, onMount, useContext } from 'solid-js'
import { getAutoStartEnabled, setAutoStart as setAutoStartApi } from './api'

interface AppState {
  autoStart: () => boolean
  toggleAutoStart: () => Promise<void>
}

const AppContext = createContext<AppState>()

export function AppProvider(props: { children: JSX.Element; fallback?: JSX.Element }) {
  const [ready, setReady] = createSignal(false)
  const [autoStart, setAutoStart] = createSignal(false)

  onMount(async () => {
    try {
      setAutoStart(await getAutoStartEnabled())
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
