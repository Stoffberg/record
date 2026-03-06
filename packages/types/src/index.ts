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
  project: string | null
  detail: string | null
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

export interface ProjectUsage {
  project: string
  total_secs: number
  active_secs: number
  agent_secs: number
  session_count: number
  details: ProjectDetail[]
}

export interface ProjectDetail {
  label: string
  total_secs: number
}

export interface Space {
  id: number
  name: string
  color: string
  initials: string
  emoji: string | null
}

export interface SpaceWithProjects {
  space: Space
  projects: string[]
}

export interface SpaceUsage {
  space: Space | null
  projects: ProjectUsage[]
  total_secs: number
  session_count: number
}

export interface AgentSession {
  id: number
  agent: string
  project: string
  session_ref: string
  started_at: string
  ended_at: string
  duration_secs: number
}

export interface AgentProjectUsage {
  project: string
  total_secs: number
  session_count: number
  agents: AgentUsage[]
}

export interface AgentUsage {
  agent: string
  total_secs: number
  session_count: number
}

export interface AgentSummary {
  date: string
  total_agent_secs: number
  projects: AgentProjectUsage[]
}
