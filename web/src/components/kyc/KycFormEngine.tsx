'use client';

/**
 * KycFormEngine — task #481 step 5
 *
 * Multi-step KYC form orchestrator.
 *
 * Responsibilities:
 * - Drives multi-step navigation, validating only the current step's fields
 *   before advancing.
 * - Integrates react-hook-form + zodResolver for field-level validation.
 * - Renders dynamic field types: text / tel / number / date / select / file /
 *   webcam.
 * - Auto-saves drafts to localStorage on every change via formPersistenceService.
 * - Emits KycTelemetryEvent objects via the optional `onTelemetry` prop.
 * - Scrubs PII on successful submission (form.reset + clear document URLs).
 * - Shows KycProgressIndicator at top of form.
 */

import {
  useEffect,
  useRef,
  useState,
  useCallback,
  type FormEvent,
} from 'react';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';

import { KYC_FORM_CONFIGS } from '@/lib/kyc/form-config';
import { getSchemaForCountryAndTier } from '@/lib/kyc/schemas';
import { formPersistenceService } from '@/lib/kyc/persistence';
import { kycTelemetry } from '@/lib/kyc/telemetry';
import {
  KycFieldType,
  KycOnboardingStatus,
  type KycCountry,
  type KycTier,
  type KycFieldConfig,
  type KycFormValues,
  type KycTelemetryEvent,
  type KycSubmissionPayload,
  type KycDocumentUpload,
  KycDocumentType,
} from '@/lib/kyc/types';

import { KycProgressIndicator } from './KycProgressIndicator';
import { FileUploadComponent } from './FileUploadComponent';
import { useWebcamCapture } from '@/hooks/useWebcamCapture';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Field IDs that must never be auto-filled or pasted into (compliance). */
const SENSITIVE_FIELD_IDS = new Set(['bvn', 'nin', 'kraPin', 'ghanaCard']);

/** Accepted MIME types for document uploads. */
const DOC_ACCEPT_TYPES = ['image/jpeg', 'image/png', 'application/pdf'];

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface KycFormEngineProps {
  country: KycCountry;
  tier: KycTier;
  consumerId: string;
  onSubmit: (payload: KycSubmissionPayload) => Promise<void>;
  onStepChange?: (step: number, stepId: string) => void;
  onTelemetry?: (event: KycTelemetryEvent) => void;
  initialDraft?: Partial<KycFormValues>;
}

// ---------------------------------------------------------------------------
// Document data-URL side-channel state
// ---------------------------------------------------------------------------

interface DocumentEntry {
  fieldId: string;
  file: File;
  dataUrl: string;
}

// ---------------------------------------------------------------------------
// Helper: map fieldId to KycDocumentType enum member (best-effort)
// ---------------------------------------------------------------------------

function fieldIdToDocType(fieldId: string): KycDocumentType {
  const map: Record<string, KycDocumentType> = {
    bvn: KycDocumentType.BVN,
    nin: KycDocumentType.NIN,
    kraPin: KycDocumentType.KraPin,
    ghanaCard: KycDocumentType.GhanaCard,
    passport: KycDocumentType.Passport,
    driversLicense: KycDocumentType.DriversLicense,
    utilityBill: KycDocumentType.UtilityBill,
  };
  return map[fieldId] ?? KycDocumentType.UtilityBill;
}

// ---------------------------------------------------------------------------
// WebcamModal — inline modal for webcam capture
// ---------------------------------------------------------------------------

interface WebcamModalProps {
  fieldId: string;
  onCapture: (fieldId: string, dataUrl: string) => void;
  onClose: () => void;
}

function WebcamModal({ fieldId, onCapture, onClose }: WebcamModalProps) {
  const { videoRef, isStreaming, error, startCamera, stopCamera, captureSnapshot, previewUrl, isProcessing } =
    useWebcamCapture();

  // Start camera on mount, stop on unmount
  useEffect(() => {
    void startCamera();
    return () => stopCamera();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleCapture = useCallback(async () => {
    const dataUrl = await captureSnapshot();
    if (dataUrl) {
      onCapture(fieldId, dataUrl);
    }
  }, [captureSnapshot, fieldId, onCapture]);

  const handleConfirm = useCallback(() => {
    if (previewUrl) {
      onCapture(fieldId, previewUrl);
      stopCamera();
      onClose();
    }
  }, [previewUrl, fieldId, onCapture, stopCamera, onClose]);

  return (
    /* Backdrop */
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Capture identity photo"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
    >
      <div className="bg-white rounded-xl shadow-xl w-full max-w-md overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200">
          <h2 className="text-base font-semibold text-gray-800">Capture Photo</h2>
          <button
            type="button"
            onClick={() => { stopCamera(); onClose(); }}
            className="text-gray-400 hover:text-gray-600 transition-colors"
            aria-label="Close camera"
          >
            <svg xmlns="http://www.w3.org/2000/svg" className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>

        {/* Camera feed / preview */}
        <div className="relative bg-black aspect-square overflow-hidden">
          {previewUrl ? (
            <img src={previewUrl} alt="Captured snapshot" className="w-full h-full object-cover" />
          ) : (
            // eslint-disable-next-line jsx-a11y/media-has-caption
            <video
              ref={videoRef}
              autoPlay
              playsInline
              muted
              className="w-full h-full object-cover"
            />
          )}

          {/* Overlaid status */}
          {!isStreaming && !error && (
            <div className="absolute inset-0 flex items-center justify-center">
              <span className="text-white text-sm animate-pulse">Starting camera…</span>
            </div>
          )}
        </div>

        {/* Error */}
        {error && (
          <p role="alert" className="px-4 py-2 text-sm text-red-600 bg-red-50">
            {error}
          </p>
        )}

        {/* Actions */}
        <div className="flex gap-2 px-4 py-3 border-t border-gray-200">
          {!previewUrl ? (
            <button
              type="button"
              onClick={handleCapture}
              disabled={!isStreaming || isProcessing}
              className="flex-1 rounded-lg bg-blue-600 py-2 px-4 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {isProcessing ? 'Processing…' : 'Capture'}
            </button>
          ) : (
            <>
              <button
                type="button"
                onClick={handleCapture}
                disabled={isProcessing}
                className="flex-1 rounded-lg border border-gray-300 py-2 px-4 text-sm font-medium text-gray-700 hover:bg-gray-50 disabled:opacity-50 transition-colors"
              >
                Retake
              </button>
              <button
                type="button"
                onClick={handleConfirm}
                className="flex-1 rounded-lg bg-blue-600 py-2 px-4 text-sm font-medium text-white hover:bg-blue-700 transition-colors"
              >
                Use Photo
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// KycFormEngine
// ---------------------------------------------------------------------------

export function KycFormEngine({
  country,
  tier,
  consumerId,
  onSubmit,
  onStepChange,
  onTelemetry,
  initialDraft,
}: KycFormEngineProps) {
  // ── Config & schema ───────────────────────────────────────────────────────
  const config = KYC_FORM_CONFIGS[country][tier];
  const schema = getSchemaForCountryAndTier(country, tier);
  const steps = config.steps;

  // ── Form state ────────────────────────────────────────────────────────────
  const form = useForm<KycFormValues>({
    resolver: zodResolver(schema),
    mode: 'onTouched',
    defaultValues: initialDraft ?? {},
  });
  const { register, handleSubmit, watch, trigger, formState, setValue, reset } = form;

  // ── Step navigation ───────────────────────────────────────────────────────
  const [currentStep, setCurrentStep] = useState(0);
  const [onboardingStatus, setOnboardingStatus] = useState<KycOnboardingStatus>(
    KycOnboardingStatus.Idle,
  );
  const stepStartedAtRef = useRef<number>(Date.now());

  // ── Document uploads (side-channel, not in RHF) ───────────────────────────
  const [documents, setDocuments] = useState<DocumentEntry[]>([]);

  // ── Webcam modal ──────────────────────────────────────────────────────────
  const [webcamFieldId, setWebcamFieldId] = useState<string | null>(null);

  // ── Field-level upload errors ─────────────────────────────────────────────
  const [uploadErrors, setUploadErrors] = useState<Record<string, string>>({});

  // ── Submission error ──────────────────────────────────────────────────────
  const [submitError, setSubmitError] = useState<string | null>(null);

  // ── Step shapes for progress indicator ───────────────────────────────────
  const progressSteps = steps.map((s) => ({ id: s.id, title: s.title }));

  // ─────────────────────────────────────────────────────────────────────────
  // Draft persistence — load on mount
  // ─────────────────────────────────────────────────────────────────────────
  useEffect(() => {
    const saved = formPersistenceService.loadDraft(consumerId);
    if (saved) {
      const { data, step } = saved;
      // Hydrate form values from draft
      Object.entries(data).forEach(([key, value]) => {
        if (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') {
          setValue(key, value, { shouldDirty: false });
        }
      });
      if (step < steps.length) setCurrentStep(step);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [consumerId]);

  // Draft persistence — auto-save on every change (debounced by useEffect dep array)
  const watchedValues = watch();
  useEffect(() => {
    // Only persist primitive values (omit File objects — not serialisable)
    const serialisable: Partial<KycFormValues> = {};
    for (const [k, v] of Object.entries(watchedValues)) {
      if (v instanceof File) continue;
      serialisable[k] = v as string | number | boolean | null | undefined;
    }
    formPersistenceService.saveDraft(consumerId, serialisable, currentStep);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [watchedValues, currentStep, consumerId]);

  // Telemetry: start tracking time on mount / step change
  useEffect(() => {
    kycTelemetry.trackStepStart(country, tier, steps[currentStep].id);
    stepStartedAtRef.current = Date.now();
    setOnboardingStatus(KycOnboardingStatus.InProgress);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentStep]);

  // ─────────────────────────────────────────────────────────────────────────
  // Navigation
  // ─────────────────────────────────────────────────────────────────────────

  const handleNext = useCallback(async () => {
    const stepFields = steps[currentStep].fields.map((f) => f.id);
    const valid = await trigger(stepFields as Array<keyof KycFormValues>);
    if (!valid) return;

    // Telemetry: step complete
    const event = kycTelemetry.trackStepComplete(country, tier, steps[currentStep].id);
    onTelemetry?.(event);

    const next = currentStep + 1;
    setCurrentStep(next);
    onStepChange?.(next, steps[next].id);
  }, [country, currentStep, onStepChange, onTelemetry, steps, tier, trigger]);

  const handleBack = useCallback(() => {
    if (currentStep === 0) return;
    const prev = currentStep - 1;
    setCurrentStep(prev);
    onStepChange?.(prev, steps[prev].id);
  }, [currentStep, onStepChange, steps]);

  // ─────────────────────────────────────────────────────────────────────────
  // Submit
  // ─────────────────────────────────────────────────────────────────────────

  const handleFormSubmit = useCallback(
    async (values: KycFormValues) => {
      setSubmitError(null);
      setOnboardingStatus(KycOnboardingStatus.SubmittedPendingReview);

      try {
        // Build document list from side-channel
        const docUploads: KycDocumentUpload[] = documents.map((d) => ({
          type: fieldIdToDocType(d.fieldId),
          fileName: d.file.name,
          mimeType: d.file.type,
          sizeBytes: d.file.size,
          dataUrl: d.dataUrl,
        }));

        // Add webcam captures (stored as synthetic entries with empty file name)
        // Webcam-captured images are stored directly in documents[] already via handleWebcamCapture

        // Strip File objects from field values — only keep primitives
        const fields: Record<string, unknown> = {};
        for (const [k, v] of Object.entries(values)) {
          if (!(v instanceof File)) fields[k] = v;
        }

        const payload: KycSubmissionPayload = {
          country,
          tier,
          consumerId,
          fields,
          documents: docUploads,
        };

        await onSubmit(payload);

        // Telemetry: final step complete
        const event = kycTelemetry.trackStepComplete(country, tier, steps[currentStep].id);
        onTelemetry?.(event);

        // PII scrub
        reset();
        setDocuments([]);
        formPersistenceService.clearDraft(consumerId);
        setOnboardingStatus(KycOnboardingStatus.Approved);
      } catch (err) {
        const msg = err instanceof Error ? err.message : 'Submission failed. Please try again.';
        setSubmitError(msg);
        setOnboardingStatus(KycOnboardingStatus.InProgress);
      }
    },
    [country, currentStep, consumerId, documents, onSubmit, onTelemetry, reset, steps, tier],
  );

  const isLastStep = currentStep === steps.length - 1;

  // ─────────────────────────────────────────────────────────────────────────
  // File upload handlers
  // ─────────────────────────────────────────────────────────────────────────

  const handleFileSelected = useCallback(
    (fieldId: string, file: File, dataUrl: string) => {
      setDocuments((prev) => {
        const rest = prev.filter((d) => d.fieldId !== fieldId);
        return [...rest, { fieldId, file, dataUrl }];
      });
      // Clear any previous error for this field
      setUploadErrors((prev) => {
        const next = { ...prev };
        delete next[fieldId];
        return next;
      });
      // Mark field as touched so RHF knows it has a value
      setValue(fieldId, file.name);
    },
    [setValue],
  );

  const handleFileError = useCallback(
    (fieldId: string, errorMsg: string) => {
      setUploadErrors((prev) => ({ ...prev, [fieldId]: errorMsg }));
      const event = kycTelemetry.trackDocumentUploadFailure(country, tier, errorMsg);
      onTelemetry?.(event);
    },
    [country, onTelemetry, tier],
  );

  // ─────────────────────────────────────────────────────────────────────────
  // Webcam capture handler
  // ─────────────────────────────────────────────────────────────────────────

  const handleWebcamCapture = useCallback(
    (fieldId: string, dataUrl: string) => {
      // Store as a synthetic document entry (no real File object for webcam captures)
      const syntheticFile = new File(
        [Uint8Array.from(atob(dataUrl.split(',')[1] ?? ''), (c) => c.charCodeAt(0))],
        `${fieldId}_capture.jpg`,
        { type: 'image/jpeg' },
      );
      setDocuments((prev) => {
        const rest = prev.filter((d) => d.fieldId !== fieldId);
        return [...rest, { fieldId, file: syntheticFile, dataUrl }];
      });
      setValue(fieldId, `${fieldId}_capture.jpg`);
      setWebcamFieldId(null);
    },
    [setValue],
  );

  // ─────────────────────────────────────────────────────────────────────────
  // Field renderer
  // ─────────────────────────────────────────────────────────────────────────

  function renderField(field: KycFieldConfig) {
    const isSensitive = SENSITIVE_FIELD_IDS.has(field.id);
    const error = formState.errors[field.id];
    const errorMsg =
      typeof error?.message === 'string' ? error.message : undefined;

    const baseInputClass = [
      'mt-1 block w-full rounded-md border px-3 py-2 text-sm',
      'focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500',
      'disabled:opacity-50 disabled:cursor-not-allowed',
      errorMsg ? 'border-red-400 bg-red-50' : 'border-gray-300 bg-white',
    ].join(' ');

    const commonProps = {
      id: field.id,
      disabled: field.disabled ?? false,
      placeholder: field.placeholder,
      className: baseInputClass,
      autoComplete: isSensitive ? ('off' as const) : (field.autocomplete as string),
      ...(isSensitive && {
        onPaste: (e: React.ClipboardEvent) => e.preventDefault(),
      }),
      ...register(field.id),
    };

    switch (field.type) {
      case KycFieldType.text:
      case KycFieldType.tel:
      case KycFieldType.number:
      case KycFieldType.date:
        return (
          <input
            type={field.type}
            {...commonProps}
          />
        );

      case KycFieldType.select:
        return (
          <select {...commonProps} className={baseInputClass}>
            <option value="">{field.placeholder ?? 'Select…'}</option>
            {field.options?.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
        );

      case KycFieldType.file: {
        const currentDoc = documents.find((d) => d.fieldId === field.id);
        return (
          <FileUploadComponent
            fieldId={field.id}
            accept={DOC_ACCEPT_TYPES}
            maxSizeMb={field.validation?.max ?? 5}
            currentFileName={currentDoc?.file.name}
            onFileSelected={(file, dataUrl) => handleFileSelected(field.id, file, dataUrl)}
            onError={(err) => handleFileError(field.id, err)}
            disabled={field.disabled}
          />
        );
      }

      case KycFieldType.webcam: {
        const currentDoc = documents.find((d) => d.fieldId === field.id);
        return (
          <div className="mt-1">
            {currentDoc ? (
              <div className="flex items-center gap-3">
                <img
                  src={currentDoc.dataUrl}
                  alt="Captured photo"
                  className="w-16 h-16 rounded-md object-cover border border-gray-200"
                />
                <button
                  type="button"
                  onClick={() => setWebcamFieldId(field.id)}
                  className="text-sm text-blue-600 hover:underline"
                >
                  Retake photo
                </button>
              </div>
            ) : (
              <button
                type="button"
                onClick={() => setWebcamFieldId(field.id)}
                className="flex items-center gap-2 rounded-md border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 transition-colors"
              >
                <svg xmlns="http://www.w3.org/2000/svg" className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <path d="M23 19a2 2 0 01-2 2H3a2 2 0 01-2-2V8a2 2 0 012-2h4l2-3h6l2 3h4a2 2 0 012 2z" />
                  <circle cx="12" cy="13" r="4" />
                </svg>
                Open Camera
              </button>
            )}
          </div>
        );
      }

      default:
        return null;
    }
  }

  // ─────────────────────────────────────────────────────────────────────────
  // Render
  // ─────────────────────────────────────────────────────────────────────────

  const activeStep = steps[currentStep];

  return (
    <div className="w-full max-w-2xl mx-auto">
      {/* Progress indicator */}
      <div className="mb-8">
        <KycProgressIndicator
          steps={progressSteps}
          currentStep={currentStep}
          status={onboardingStatus}
        />
      </div>

      {/* Webcam modal */}
      {webcamFieldId !== null && (
        <WebcamModal
          fieldId={webcamFieldId}
          onCapture={handleWebcamCapture}
          onClose={() => setWebcamFieldId(null)}
        />
      )}

      {/* Form */}
      <form
        onSubmit={(e: FormEvent<HTMLFormElement>) => {
          e.preventDefault();
          if (isLastStep) {
            void handleSubmit(handleFormSubmit)(e);
          } else {
            void handleNext();
          }
        }}
        noValidate
        className="rounded-xl border border-gray-200 bg-white p-6 shadow-sm"
      >
        {/* Step header */}
        <div className="mb-6">
          <h2 className="text-lg font-semibold text-gray-900">{activeStep.title}</h2>
          <p className="mt-1 text-sm text-gray-500">{activeStep.description}</p>
        </div>

        {/* Fields */}
        <div className="space-y-5">
          {activeStep.fields.map((field) => {
            const rhfError = formState.errors[field.id];
            const rhfMsg =
              typeof rhfError?.message === 'string' ? rhfError.message : undefined;
            const uploadErr = uploadErrors[field.id];
            const displayError = rhfMsg ?? uploadErr;

            return (
              <div key={field.id}>
                <label
                  htmlFor={field.id}
                  className="block text-sm font-medium text-gray-700"
                >
                  {field.label}
                  {field.required && (
                    <span className="ml-1 text-red-500" aria-hidden="true">
                      *
                    </span>
                  )}
                </label>

                {renderField(field)}

                {displayError && (
                  <p
                    id={`${field.id}-error`}
                    role="alert"
                    className="mt-1 text-xs text-red-600"
                  >
                    {displayError}
                  </p>
                )}
              </div>
            );
          })}
        </div>

        {/* Submission error */}
        {submitError && (
          <div
            role="alert"
            className="mt-4 rounded-md bg-red-50 border border-red-200 px-4 py-3 text-sm text-red-700"
          >
            {submitError}
          </div>
        )}

        {/* Navigation buttons */}
        <div className="mt-8 flex items-center justify-between gap-3">
          <button
            type="button"
            onClick={handleBack}
            disabled={currentStep === 0}
            className="rounded-lg border border-gray-300 px-5 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
          >
            Back
          </button>

          <button
            type="submit"
            disabled={formState.isSubmitting}
            className="rounded-lg bg-blue-600 px-6 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {formState.isSubmitting
              ? 'Submitting…'
              : isLastStep
                ? 'Submit'
                : 'Continue'}
          </button>
        </div>
      </form>
    </div>
  );
}
