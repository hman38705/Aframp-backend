/**
 * KYC telemetry service — task #481 step 5
 *
 * Tracks timing and drop-off events during the KYC flow.
 * All methods produce a `KycTelemetryEvent` and log via console.debug;
 * swap `console.debug` calls for a real analytics sink when ready.
 */

import type { KycCountry, KycTier, KycTelemetryEvent } from "./types";

// ---------------------------------------------------------------------------
// Internal timestamp store
// ---------------------------------------------------------------------------

/**
 * Key format: `${country}:${tier}:${stepId}`
 * Stores the Unix timestamp (ms) when a step was started.
 */
const stepStartTimestamps = new Map<string, number>();

function makeStepKey(country: KycCountry, tier: KycTier, stepId: string): string {
  return `${country}:${tier}:${stepId}`;
}

// ---------------------------------------------------------------------------
// KycTelemetry implementation
// ---------------------------------------------------------------------------

export const kycTelemetry = {
  /**
   * Record that the user entered `stepId`.
   * Call this at the beginning of each step render.
   */
  trackStepStart(country: KycCountry, tier: KycTier, stepId: string): void {
    const key = makeStepKey(country, tier, stepId);
    stepStartTimestamps.set(key, Date.now());

    const event: KycTelemetryEvent = {
      eventName: "kyc_step_started",
      stepId,
      country,
      tier,
    };

    console.debug("[kycTelemetry] kyc_step_started", event);
  },

  /**
   * Record that the user successfully completed `stepId`.
   * Computes `durationMs` from the matching `trackStepStart` call.
   * Returns the emitted event (so the caller can forward it via `onTelemetry`).
   */
  trackStepComplete(country: KycCountry, tier: KycTier, stepId: string): KycTelemetryEvent {
    const key = makeStepKey(country, tier, stepId);
    const startedAt = stepStartTimestamps.get(key);
    const durationMs = startedAt !== undefined ? Date.now() - startedAt : undefined;

    // Clean up to avoid stale entries accumulating
    stepStartTimestamps.delete(key);

    const event: KycTelemetryEvent = {
      eventName: "kyc_step_completed",
      stepId,
      country,
      tier,
      ...(durationMs !== undefined && { durationMs }),
    };

    console.debug("[kycTelemetry] kyc_step_completed", event);
    return event;
  },

  /**
   * Record that the user abandoned the flow on `stepId` while focused on `fieldId`.
   * Computes time-on-step if `trackStepStart` was called for this step.
   */
  trackDropOff(
    country: KycCountry,
    tier: KycTier,
    stepId: string,
    fieldId: string,
  ): KycTelemetryEvent {
    const key = makeStepKey(country, tier, stepId);
    const startedAt = stepStartTimestamps.get(key);
    const durationMs = startedAt !== undefined ? Date.now() - startedAt : undefined;

    const event: KycTelemetryEvent = {
      eventName: "kyc_drop_off",
      stepId,
      fieldId,
      country,
      tier,
      ...(durationMs !== undefined && { durationMs }),
    };

    console.debug("[kycTelemetry] kyc_drop_off", event);
    return event;
  },

  /**
   * Record that the user triggered a validation error on `fieldId` within `stepId`.
   */
  trackValidationError(
    country: KycCountry,
    tier: KycTier,
    stepId: string,
    fieldId: string,
  ): KycTelemetryEvent {
    const event: KycTelemetryEvent = {
      eventName: "kyc_validation_error",
      stepId,
      fieldId,
      country,
      tier,
    };

    console.debug("[kycTelemetry] kyc_validation_error", event);
    return event;
  },

  /**
   * Record that a document upload attempt failed for `reason`.
   * `stepId` is set to "documents" by convention; callers can override.
   */
  trackDocumentUploadFailure(
    country: KycCountry,
    tier: KycTier,
    reason: string,
  ): KycTelemetryEvent {
    const event: KycTelemetryEvent = {
      eventName: "kyc_document_upload_failure",
      stepId: "documents",
      fieldId: reason,
      country,
      tier,
    };

    console.debug("[kycTelemetry] kyc_document_upload_failure", event);
    return event;
  },
} as const;
