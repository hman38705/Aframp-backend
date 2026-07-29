/**
 * KYC/KYB form types — task #481 step 1
 *
 * Shared enums and interfaces consumed by the KYC multi-step form,
 * Zod schemas, server actions, and telemetry layers.
 */

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/** Supported countries for KYC onboarding. */
export enum KycCountry {
  Nigeria = "Nigeria",
  Kenya = "Kenya",
  Ghana = "Ghana",
}

/**
 * KYC verification tier.
 * Names must match the backend `KycTier` enum exactly.
 */
export enum KycTier {
  Basic = "Basic",
  Standard = "Standard",
  Premium = "Premium",
}

/** Supported field input types rendered by the dynamic form engine. */
export enum KycFieldType {
  text = "text",
  number = "number",
  select = "select",
  file = "file",
  webcam = "webcam",
  tel = "tel",
  date = "date",
}

/** Document types accepted during KYC verification. */
export enum KycDocumentType {
  BVN = "BVN",
  NIN = "NIN",
  KraPin = "KraPin",
  GhanaCard = "GhanaCard",
  Passport = "Passport",
  DriversLicense = "DriversLicense",
  UtilityBill = "UtilityBill",
}

/** Lifecycle states of the KYC onboarding flow. */
export enum KycOnboardingStatus {
  Idle = "Idle",
  InProgress = "InProgress",
  SubmittedPendingReview = "SubmittedPendingReview",
  Approved = "Approved",
  Rejected = "Rejected",
  ManualReview = "ManualReview",
}

// ---------------------------------------------------------------------------
// Field & step configuration
// ---------------------------------------------------------------------------

/** Optional per-field client-side validation hint (augments Zod schema). */
export interface KycFieldValidation {
  /** Minimum string length or numeric value. */
  min?: number;
  /** Maximum string length or numeric value. */
  max?: number;
  /** Regex pattern the raw input must satisfy. */
  pattern?: string;
  /** Human-readable message shown when validation fails. */
  message?: string;
}

/** Configuration for a single field within a KYC step. */
export interface KycFieldConfig {
  /** Unique field identifier — used as the form state key. */
  id: string;
  /** Human-readable label shown above the input. */
  label: string;
  /** Input type rendered by the form engine. */
  type: KycFieldType;
  /** Placeholder text shown inside the input when empty. */
  placeholder?: string;
  /** Whether the field must be completed before advancing. */
  required: boolean;
  /** Optional client-side validation constraints. */
  validation?: KycFieldValidation;
  /** Input mask pattern, e.g. "99999999999" for BVN. */
  mask?: string;
  /** HTML autocomplete attribute value, e.g. "tel", "name", "bday". */
  autocomplete: string;
  /** Renders the field as read-only when true. */
  disabled?: boolean;
  /** Allowed options for fields of type `KycFieldType.select`. */
  options?: Array<{ value: string; label: string }>;
}

/** Configuration for a single step in the multi-step KYC flow. */
export interface KycStepConfig {
  /** Unique step identifier used for routing and telemetry. */
  id: string;
  /** Short title displayed in the step header. */
  title: string;
  /** Longer descriptive copy shown beneath the title. */
  description: string;
  /** Ordered list of fields rendered in this step. */
  fields: KycFieldConfig[];
  /** If set, this step is shown only for the specified country. */
  country?: KycCountry;
  /** If set, this step is shown only for the specified tier. */
  tier?: KycTier;
}

/** Top-level configuration object for a complete KYC form flow. */
export interface KycFormConfig {
  /** Country for which this configuration applies. */
  country: KycCountry;
  /** Verification tier for which this configuration applies. */
  tier: KycTier;
  /** Ordered list of steps that make up the form. */
  steps: KycStepConfig[];
}

// ---------------------------------------------------------------------------
// Form values
// ---------------------------------------------------------------------------

/**
 * Loosely-typed record for in-progress form field values.
 * Narrowed at validation time by the country/tier Zod schema.
 */
export type KycFormValues = Record<string, string | number | boolean | File | null | undefined>;

// ---------------------------------------------------------------------------
// Submission payload
// ---------------------------------------------------------------------------

/** Metadata describing a document file attached to a KYC submission. */
export interface KycDocumentUpload {
  /** Classification of the uploaded document. */
  type: KycDocumentType;
  /** Original file name as provided by the user's OS. */
  fileName: string;
  /** MIME type, e.g. "image/jpeg" or "application/pdf". */
  mimeType: string;
  /** File size in bytes. */
  sizeBytes: number;
  /**
   * Base-64 data URL of the file content.
   * Present for small previews; omitted when the file is uploaded separately.
   */
  dataUrl?: string;
}

/** Payload sent to the backend when a KYC form is submitted. */
export interface KycSubmissionPayload {
  /** Country context of this submission. */
  country: KycCountry;
  /** Verification tier being applied for. */
  tier: KycTier;
  /** Platform consumer / account ID, if known at submission time. */
  consumerId?: string;
  /** Flat map of validated field values keyed by field ID. */
  fields: Record<string, unknown>;
  /** Attached document metadata records. */
  documents: KycDocumentUpload[];
}

// ---------------------------------------------------------------------------
// Validation errors & form state
// ---------------------------------------------------------------------------

/** A single field-level validation error surfaced to the UI. */
export interface KycValidationError {
  /** The `KycFieldConfig.id` that failed validation. */
  field: string;
  /** Human-readable error message to display next to the field. */
  message: string;
  /**
   * Machine-readable error code for programmatic handling.
   * Matches the `discriminatedFormErrorSchema` code union.
   */
  code: string;
}

/** Snapshot of the multi-step form's runtime state. */
export interface KycFormState {
  /** Zero-indexed step the user is currently on. */
  currentStep: number;
  /** Total number of steps in the active flow. */
  totalSteps: number;
  /** True while an async submission is in flight. */
  isSubmitting: boolean;
  /** True once the user has modified at least one field. */
  isDirty: boolean;
  /** Current set of validation errors across all steps. */
  errors: KycValidationError[];
}

// ---------------------------------------------------------------------------
// Telemetry
// ---------------------------------------------------------------------------

/** Event emitted to the analytics / telemetry layer during the KYC flow. */
export interface KycTelemetryEvent {
  /** Descriptive event name, e.g. "kyc_step_completed" or "kyc_field_blur". */
  eventName: string;
  /** ID of the step where the event occurred. */
  stepId: string;
  /** ID of the field involved, if applicable. */
  fieldId?: string;
  /** Time elapsed in milliseconds, e.g. time-on-step. */
  durationMs?: number;
  /** Country context at the time of the event. */
  country: KycCountry;
  /** Tier context at the time of the event. */
  tier: KycTier;
}
