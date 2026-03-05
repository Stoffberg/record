import { createSignal, onCleanup, Show } from 'solid-js'
import { requestAccessibility } from '../lib/api'
import { useApp } from '../lib/context'

export default function OnboardingView() {
  const { accessibilityGranted, recheckAccessibility, completeOnboarding } = useApp()
  const [checking, setChecking] = createSignal(false)
  const [prompted, setPrompted] = createSignal(false)

  let pollId: ReturnType<typeof setInterval> | undefined

  const handleGrant = async () => {
    setChecking(true)
    await requestAccessibility()
    setPrompted(true)

    pollId = setInterval(async () => {
      const granted = await recheckAccessibility()
      if (granted) {
        clearInterval(pollId)
      }
    }, 1500)
  }

  onCleanup(() => {
    if (pollId) clearInterval(pollId)
  })

  return (
    <div class="onboarding">
      <div class="onboarding-content">
        <div class="onboarding-icon">
          <svg width="48" height="48" viewBox="0 0 24 24" fill="none">
            <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="1.5" />
            <circle cx="12" cy="12" r="4" fill="var(--accent)" />
            <circle cx="12" cy="12" r="7" stroke="var(--accent)" stroke-width="1.5" opacity="0.3" />
          </svg>
        </div>

        <h1 class="onboarding-title">Welcome to Record</h1>
        <p class="onboarding-desc">
          Record needs Accessibility permission to see which app you're using. Everything stays
          local on your machine.
        </p>

        <Show
          when={!accessibilityGranted()}
          fallback={
            <div class="onboarding-success">
              <div class="onboarding-granted">
                <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
                  <circle cx="10" cy="10" r="9" stroke="var(--accent)" stroke-width="1.5" />
                  <path
                    d="M6 10l3 3 5-5"
                    stroke="var(--accent)"
                    stroke-width="1.5"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                </svg>
                Permission granted
              </div>
              <button class="onboarding-btn" onClick={completeOnboarding}>
                Get Started
              </button>
            </div>
          }
        >
          <Show
            when={!prompted()}
            fallback={
              <div class="onboarding-waiting">
                <span class="onboarding-spinner" />
                Waiting for permission...
              </div>
            }
          >
            <button class="onboarding-btn" onClick={handleGrant} disabled={checking()}>
              Grant Accessibility Access
            </button>
          </Show>
        </Show>

        <p class="onboarding-hint">
          You can change this later in System Settings &gt; Privacy &amp; Security &gt;
          Accessibility.
        </p>
      </div>
    </div>
  )
}
