'use client';

/**
 * KycProgressIndicator — task #481 step 5
 *
 * Horizontal multi-step progress bar for the KYC onboarding flow.
 * Fully accessible (ARIA) and styled with pure Tailwind CSS.
 */

import { KycOnboardingStatus } from '@/lib/kyc/types';

// ---------------------------------------------------------------------------
// Status label mapping
// ---------------------------------------------------------------------------

const STATUS_LABELS: Record<KycOnboardingStatus, string> = {
  [KycOnboardingStatus.Idle]: '',
  [KycOnboardingStatus.InProgress]: 'In Progress…',
  [KycOnboardingStatus.SubmittedPendingReview]: 'Uploading Documents…',
  [KycOnboardingStatus.Approved]: 'Approved',
  [KycOnboardingStatus.Rejected]: 'Rejected',
  [KycOnboardingStatus.ManualReview]: 'Under Compliance Review',
};

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface KycProgressIndicatorProps {
  /** Ordered step definitions to render. */
  steps: Array<{ id: string; title: string }>;
  /** Zero-based index of the currently active step. */
  currentStep: number;
  /** Current lifecycle status of the KYC onboarding flow. */
  status: KycOnboardingStatus;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/**
 * Renders a horizontal step-progress indicator.
 *
 * - Completed steps (index < currentStep) show a ✓ checkmark.
 * - The active step is highlighted in brand blue and marked aria-current="step".
 * - Upcoming steps are muted.
 * - A status label is rendered in an aria-live="polite" region below the steps
 *   so screen readers announce status changes without interrupting the user.
 */
export function KycProgressIndicator({
  steps,
  currentStep,
  status,
}: KycProgressIndicatorProps) {
  const statusLabel = STATUS_LABELS[status] ?? '';

  return (
    <div className="w-full">
      {/* Step bubbles + connector lines */}
      <nav
        role="navigation"
        aria-label="Onboarding progress"
        className="flex items-center justify-between"
      >
        {steps.map((step, index) => {
          const isCompleted = index < currentStep;
          const isActive = index === currentStep;
          const isUpcoming = index > currentStep;

          // Connector line between steps (not before the first step)
          const showConnector = index > 0;

          return (
            <div key={step.id} className="flex flex-1 items-center">
              {/* Connector line */}
              {showConnector && (
                <div
                  className={[
                    'flex-1 h-0.5 mx-1',
                    isCompleted ? 'bg-blue-600' : 'bg-gray-300',
                  ].join(' ')}
                  aria-hidden="true"
                />
              )}

              {/* Step bubble + label */}
              <div className="flex flex-col items-center gap-1 min-w-0">
                {/* Bubble */}
                <div
                  role="listitem"
                  aria-current={isActive ? 'step' : undefined}
                  aria-label={
                    isCompleted
                      ? `${step.title} — completed`
                      : isActive
                        ? `${step.title} — current step`
                        : `${step.title} — not started`
                  }
                  className={[
                    'flex items-center justify-center w-8 h-8 rounded-full text-sm font-semibold',
                    'transition-colors duration-200 select-none shrink-0',
                    isCompleted
                      ? 'bg-blue-600 text-white'
                      : isActive
                        ? 'bg-blue-600 text-white ring-2 ring-blue-300 ring-offset-1'
                        : 'bg-gray-200 text-gray-500',
                  ].join(' ')}
                >
                  {isCompleted ? (
                    /* Checkmark SVG — no external lib required */
                    <svg
                      xmlns="http://www.w3.org/2000/svg"
                      className="w-4 h-4"
                      viewBox="0 0 20 20"
                      fill="currentColor"
                      aria-hidden="true"
                    >
                      <path
                        fillRule="evenodd"
                        d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
                        clipRule="evenodd"
                      />
                    </svg>
                  ) : (
                    <span>{index + 1}</span>
                  )}
                </div>

                {/* Step title below bubble */}
                <span
                  className={[
                    'text-xs text-center leading-tight max-w-[64px] break-words',
                    isActive
                      ? 'text-blue-700 font-semibold'
                      : isCompleted
                        ? 'text-blue-600'
                        : isUpcoming
                          ? 'text-gray-400'
                          : 'text-gray-500',
                  ].join(' ')}
                >
                  {step.title}
                </span>
              </div>
            </div>
          );
        })}
      </nav>

      {/* Status label — aria-live so assistive tech announces changes */}
      {statusLabel && (
        <div
          aria-live="polite"
          aria-atomic="true"
          className="mt-3 text-center text-sm"
        >
          <span
            className={[
              'inline-block font-medium px-3 py-1 rounded-full text-xs',
              status === KycOnboardingStatus.Approved
                ? 'bg-green-100 text-green-800'
                : status === KycOnboardingStatus.Rejected
                  ? 'bg-red-100 text-red-800'
                  : status === KycOnboardingStatus.ManualReview
                    ? 'bg-yellow-100 text-yellow-800'
                    : 'bg-blue-50 text-blue-700',
            ].join(' ')}
          >
            {statusLabel}
          </span>
        </div>
      )}
    </div>
  );
}
