import type { AppCategory, AppSession, DailySummary } from '@record/types'
import { getVersion as tauriGetVersion } from '@tauri-apps/api/app'
import { invoke } from '@tauri-apps/api/core'
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as isAutostartEnabled,
} from '@tauri-apps/plugin-autostart'

function localDateStr(date: Date): string {
  const y = date.getFullYear()
  const m = String(date.getMonth() + 1).padStart(2, '0')
  const d = String(date.getDate()).padStart(2, '0')
  return `${y}-${m}-${d}`
}

function tzOffset(): number {
  return -new Date().getTimezoneOffset()
}

export { localDateStr }

export async function getSessions(start: Date, end: Date): Promise<AppSession[]> {
  return invoke<AppSession[]>('get_sessions', {
    start: start.toISOString(),
    end: end.toISOString(),
  })
}

export async function getDailySummary(date: Date): Promise<DailySummary> {
  return invoke<DailySummary>('get_daily_summary', {
    date: localDateStr(date),
    tzOffsetMinutes: tzOffset(),
  })
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

export async function getAppSessions(date: Date, bundleId: string): Promise<AppSession[]> {
  return invoke<AppSession[]>('get_app_sessions', {
    date: localDateStr(date),
    bundleId,
    tzOffsetMinutes: tzOffset(),
  })
}

export async function getAppAverages(date: Date, bundleId: string): Promise<[number, number]> {
  return invoke<[number, number]>('get_app_averages', {
    date: localDateStr(date),
    bundleId,
    tzOffsetMinutes: tzOffset(),
  })
}

export async function addExclusion(
  bundleId: string,
  appName: string,
  expiresAt?: string,
): Promise<void> {
  await invoke('add_exclusion', { bundleId, appName, expiresAt: expiresAt ?? null })
}

export async function removeExclusion(bundleId: string): Promise<void> {
  await invoke('remove_exclusion', { bundleId })
}

export async function getExclusions(): Promise<[string, string, string | null][]> {
  return invoke('get_exclusions')
}

export async function setAppCategory(bundleId: string, category: AppCategory): Promise<void> {
  await invoke('set_app_category', { bundleId, category })
}

export async function getAppCategory(bundleId: string): Promise<AppCategory> {
  return invoke<AppCategory>('get_app_category', { bundleId })
}

export async function getAllCategories(): Promise<[string, AppCategory][]> {
  return invoke<[string, AppCategory][]>('get_all_categories')
}

export async function checkAccessibility(): Promise<boolean> {
  return invoke<boolean>('check_accessibility')
}

export async function requestAccessibility(): Promise<boolean> {
  return invoke<boolean>('request_accessibility')
}

export async function getVersion(): Promise<string> {
  return tauriGetVersion()
}
