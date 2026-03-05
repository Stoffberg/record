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

export interface Heartbeat {
  app_name: string
  bundle_id: string
  window_title: string
  is_idle: boolean
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

export interface MonthlySummary {
  year: number
  month: number
  total_active_secs: number
  total_idle_secs: number
  days_tracked: number
  apps: AppUsage[]
}

export interface DateRange {
  start: string
  end: string
}

export interface TrackerConfig {
  poll_interval_secs: number
  idle_threshold_secs: number
  merge_gap_secs: number
}

export const DEFAULT_CONFIG: TrackerConfig = {
  poll_interval_secs: 5,
  idle_threshold_secs: 300,
  merge_gap_secs: 10,
}
