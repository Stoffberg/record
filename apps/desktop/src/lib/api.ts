import type { AppSession, DailySummary } from '@record/types'
import { invoke } from '@tauri-apps/api/core'
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as isAutostartEnabled,
} from '@tauri-apps/plugin-autostart'

export async function getSessions(start: Date, end: Date): Promise<AppSession[]> {
  return invoke<AppSession[]>('get_sessions', {
    start: start.toISOString(),
    end: end.toISOString(),
  })
}

export async function getDailySummary(date: Date): Promise<DailySummary> {
  const dateStr = date.toISOString().split('T')[0]
  return invoke<DailySummary>('get_daily_summary', { date: dateStr })
}

const iconCache = new Map<string, string | null>()

export async function getAppIcon(bundleId: string): Promise<string | null> {
  if (iconCache.has(bundleId)) return iconCache.get(bundleId)!
  const result = await invoke<string | null>('get_app_icon', { bundleId })
  iconCache.set(bundleId, result)
  return result
}

export async function getAutoStartEnabled(): Promise<boolean> {
  return isAutostartEnabled()
}

export async function setAutoStart(enabled: boolean): Promise<void> {
  if (enabled) {
    await enableAutostart()
  } else {
    await disableAutostart()
  }
}

export async function checkAccessibility(): Promise<boolean> {
  return invoke<boolean>('check_accessibility')
}

export async function requestAccessibility(): Promise<boolean> {
  return invoke<boolean>('request_accessibility')
}
