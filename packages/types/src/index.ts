export interface AppSession {
  id: number
  app_name: string
  bundle_id: string
  window_title: string
  started_at: string
  ended_at: string
  duration_secs: number
  is_idle: boolean
  metadata: string
}

export interface DailySummary {
  date: string
  total_active_secs: number
  total_idle_secs: number
  apps: AppUsage[]
}

export interface AppUsage {
  app_name: string
  bundle_id: string
  total_secs: number
  session_count: number
}
