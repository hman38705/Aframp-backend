/**
 * Integration tests for the KYC onboarding lifecycle — task #481 step 9
 *
 * Uses fireEvent from @testing-library/react (no @testing-library/user-event needed).
 *
 * Covers:
 * 1. Nigeria Basic happy path — fill step 1, advance, fill step 2, submit
 * 2. Validation blocks step progression — empty required field, error shown, no advance
 * 3. Draft persistence — saveDraft called after advancing step 1
 * 4. Country change via useKycCountryRouter — resets form and loads new config
 * 5. Submission error handling
 *
 * Mocks:
 * - formPersistenceService  (vi.mock)
 * - kycTelemetry            (vi.mock)
 * - useWebcamCapture        (vi.mock — webcam not exercised here)
 * - next/navigation         (global stub in vitest.setup.ts)
 */

import React, { useState } from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';

// ---------------------------------------------------------------------------
// Module mocks — must appear before any imports that depend on them so that
// vitest can hoist them during transformation.
// ---------------------------------------------------------------------------

vi.mock('@/lib/kyc/persistence', () => ({
  formPersistenceService: {
    loadDraft: vi.fn().mockReturnValue(null),
    saveDraft: vi.fn(),
    clearDraft: vi.fn(),
    hasDraft: vi.fn().mockReturnValue(false),
    getDraftAge: vi.fn().mockReturnValue(null),
  },
  FormPersistenceService: vi.fn(),
}));

vi.mock('@/lib/kyc/telemetry', () => ({
  kycTelemetry: {
    trackStepStart: vi.fn(),
    trackStepComplete: vi.fn().mockReturnValue({
      eventName: 'kyc_step_completed',
      stepId: 'personal-info',
      country: 'Nigeria',
      tier: 'Basic',
    }),
    trackDropOff: vi.fn(),
    trackValidationError: vi.fn(),
    trackDocumentUploadFailure: vi.fn(),
  },
}));

vi.mock('@/hooks/useWebcamCapture', () => ({
  useWebcamCapture: vi.fn().mockReturnValue({
    videoRef: { current: null },
    isStreaming: false,
    hasPermission: false,
    error: null,
    startCamera: vi.fn(),
    stopCamera: vi.fn(),
    captureSnapshot: vi.fn().mockResolvedValue(null),
    previewUrl: null,
    isProcessing: false,
  }),
}));

// ---------------------------------------------------------------------------
// Imports that depend on the mocked modules
// ---------------------------------------------------------------------------

import { KycFormEngine } from '@/components/kyc/KycFormEngine';
import { formPersistenceService } from '@/lib/kyc/persistence';
import { kycTelemetry } from '@/lib/kyc/telemetry';
import { KycCountry, KycTier } from '@/lib/kyc/types';
import type { KycSubmissionPayload, KycFormConfig } from '@/lib/kyc/types';
import { useKycCountryRouter } from '@/hooks/useKycCountryRouter';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Valid adult date of birth, well past the 18-year minimum. */
const VALID_DOB = '1990-06-15';

/** Shared base props for KycFormEngine. Re-assigned in each beforeEach. */
const makeBaseProps = () => ({
  country: KycCountry.Nigeria,
  tier: KycTier.Basic,
  consumerId: 'test-consumer-001',
  onSubmit: vi.fn<[KycSubmissionPayload], Promise<void>>().mockResolvedValue(undefined),
  onTelemetry: vi.fn(),
});

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Flush all pending React state updates and microtasks. */
async function flushAsync(): Promise<void> {
  await act(async () => {
    await new Promise<void>((r) => setTimeout(r, 0));
  });
}

/** Type into an input via fireEvent, simulating real character-by-character entry. */
function typeInto(element: HTMLElement, value: string): void {
  fireEvent.focus(element);
  fireEvent.change(element, { target: { value } });
  fireEvent.blur(element);
}

/** Fill the Nigeria Basic step-1 (personal-info) fields. */
function fillStep1Fields(): void {
  typeInto(screen.getByLabelText(/full legal name/i), 'Emeka Okafor');
  typeInto(screen.getByLabelText(/date of birth/i), VALID_DOB);
  typeInto(screen.getByLabelText(/phone number/i), '+2348012345678');
}

/** Click Continue and wait for the next step heading to appear. */
async function clickContinue(): Promise<void> {
  fireEvent.click(screen.getByRole('button', { name: /continue/i }));
  // Allow async validation + state update to settle
  await flushAsync();
}

/** Click Submit and wait for async work. */
async function clickSubmit(): Promise<void> {
  fireEvent.click(screen.getByRole('button', { name: /submit/i }));
  await flushAsync();
}

// ---------------------------------------------------------------------------
// Suite 1 — Nigeria Basic happy-path flow
// ---------------------------------------------------------------------------

describe('Nigeria Basic — happy-path onboarding flow', () => {
  let props: ReturnType<typeof makeBaseProps>;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(formPersistenceService.loadDraft).mockReturnValue(null);
    props = makeBaseProps();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('renders step 1 (Personal Information) on initial mount', async () => {
    render(<KycFormEngine {...props} />);
    await flushAsync();

    expect(screen.getByText('Personal Information')).toBeInTheDocument();
    expect(screen.getByLabelText(/full legal name/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/date of birth/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/phone number/i)).toBeInTheDocument();
  });

  it('shows the KycProgressIndicator nav', async () => {
    render(<KycFormEngine {...props} />);
    await flushAsync();

    expect(
      screen.getByRole('navigation', { name: /onboarding progress/i }),
    ).toBeInTheDocument();
  });

  it('advances to step 2 after valid step-1 input', async () => {
    render(<KycFormEngine {...props} />);
    await flushAsync();

    fillStep1Fields();
    await clickContinue();

    await waitFor(() => {
      expect(screen.getByText('Identity Verification')).toBeInTheDocument();
    });
    expect(screen.getByLabelText(/bank verification number/i)).toBeInTheDocument();
  });

  it('calls onSubmit with a correctly-shaped payload after completing both steps', async () => {
    render(<KycFormEngine {...props} />);
    await flushAsync();

    // Step 1
    fillStep1Fields();
    await clickContinue();

    await waitFor(() =>
      expect(screen.getByLabelText(/bank verification number/i)).toBeInTheDocument(),
    );

    // Step 2
    typeInto(screen.getByLabelText(/bank verification number/i), '22222222222');
    await clickSubmit();

    await waitFor(() => {
      expect(props.onSubmit).toHaveBeenCalledOnce();
    });

    const [payload] = props.onSubmit.mock.calls[0] as [KycSubmissionPayload];
    expect(payload.country).toBe(KycCountry.Nigeria);
    expect(payload.tier).toBe(KycTier.Basic);
    expect(payload.consumerId).toBe('test-consumer-001');
    expect(payload.fields.fullName).toBe('Emeka Okafor');
    expect(payload.fields.phone).toBe('+2348012345678');
    expect(payload.fields.dateOfBirth).toBe(VALID_DOB);
    expect(payload.fields.bvn).toBe('22222222222');
    expect(Array.isArray(payload.documents)).toBe(true);
  });

  it('clears the draft after successful submission', async () => {
    render(<KycFormEngine {...props} />);
    await flushAsync();

    fillStep1Fields();
    await clickContinue();

    await waitFor(() =>
      expect(screen.getByLabelText(/bank verification number/i)).toBeInTheDocument(),
    );

    typeInto(screen.getByLabelText(/bank verification number/i), '33333333333');
    await clickSubmit();

    await waitFor(() => {
      expect(formPersistenceService.clearDraft).toHaveBeenCalledWith('test-consumer-001');
    });
  });

  it('fires trackStepStart for personal-info on mount', async () => {
    render(<KycFormEngine {...props} />);
    await flushAsync();

    expect(kycTelemetry.trackStepStart).toHaveBeenCalledWith(
      KycCountry.Nigeria,
      KycTier.Basic,
      'personal-info',
    );
  });

  it('fires trackStepComplete for personal-info when advancing', async () => {
    render(<KycFormEngine {...props} />);
    await flushAsync();

    fillStep1Fields();
    await clickContinue();

    await waitFor(() => {
      expect(kycTelemetry.trackStepComplete).toHaveBeenCalledWith(
        KycCountry.Nigeria,
        KycTier.Basic,
        'personal-info',
      );
    });
  });

  it('Back button is disabled on step 1', async () => {
    render(<KycFormEngine {...props} />);
    await flushAsync();

    expect(screen.getByRole('button', { name: /back/i })).toBeDisabled();
  });

  it('Back button navigates from step 2 to step 1', async () => {
    render(<KycFormEngine {...props} />);
    await flushAsync();

    fillStep1Fields();
    await clickContinue();

    await waitFor(() =>
      expect(screen.getByLabelText(/bank verification number/i)).toBeInTheDocument(),
    );

    fireEvent.click(screen.getByRole('button', { name: /back/i }));
    await flushAsync();

    await waitFor(() => {
      expect(screen.getByLabelText(/full legal name/i)).toBeInTheDocument();
    });
  });
});

// ---------------------------------------------------------------------------
// Suite 2 — Validation blocks step progression
// ---------------------------------------------------------------------------

describe('Validation blocks step progression', () => {
  let props: ReturnType<typeof makeBaseProps>;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(formPersistenceService.loadDraft).mockReturnValue(null);
    props = makeBaseProps();
  });

  it('stays on step 1 and shows an error when fullName is empty', async () => {
    render(<KycFormEngine {...props} />);
    await flushAsync();

    // Fill only date + phone; leave fullName blank
    typeInto(screen.getByLabelText(/date of birth/i), VALID_DOB);
    typeInto(screen.getByLabelText(/phone number/i), '+2348012345678');

    await clickContinue();

    await waitFor(() => {
      expect(screen.getByText('Personal Information')).toBeInTheDocument();
    });
    expect(screen.getAllByRole('alert').length).toBeGreaterThan(0);
  });

  it('stays on step 1 and shows an error when phone is empty', async () => {
    render(<KycFormEngine {...props} />);
    await flushAsync();

    typeInto(screen.getByLabelText(/full legal name/i), 'Emeka Okafor');
    typeInto(screen.getByLabelText(/date of birth/i), VALID_DOB);
    // phone left blank

    await clickContinue();

    await waitFor(() => {
      expect(screen.getByText('Personal Information')).toBeInTheDocument();
    });
    expect(screen.getAllByRole('alert').length).toBeGreaterThan(0);
  });

  it('shows a phone-format error when phone has no leading +', async () => {
    render(<KycFormEngine {...props} />);
    await flushAsync();

    typeInto(screen.getByLabelText(/full legal name/i), 'Emeka Okafor');
    typeInto(screen.getByLabelText(/date of birth/i), VALID_DOB);
    typeInto(screen.getByLabelText(/phone number/i), '08012345678'); // missing +

    await clickContinue();

    await waitFor(() => {
      expect(screen.getByText('Personal Information')).toBeInTheDocument();
    });

    const errorText = screen
      .getAllByRole('alert')
      .map((a) => a.textContent)
      .join(' ');
    expect(errorText).toMatch(/E\.164|format/i);
  });

  it('stays on step 2 and shows an error when BVN has only 10 digits', async () => {
    render(<KycFormEngine {...props} />);
    await flushAsync();

    fillStep1Fields();
    await clickContinue();

    await waitFor(() =>
      expect(screen.getByLabelText(/bank verification number/i)).toBeInTheDocument(),
    );

    typeInto(screen.getByLabelText(/bank verification number/i), '1234567890'); // 10 digits
    await clickSubmit();

    await waitFor(() => {
      expect(screen.getByText('Identity Verification')).toBeInTheDocument();
    });

    const errorText = screen
      .getAllByRole('alert')
      .map((a) => a.textContent)
      .join(' ');
    expect(errorText).toMatch(/11 digits/i);
  });

  it('does not call onSubmit when BVN is invalid', async () => {
    render(<KycFormEngine {...props} />);
    await flushAsync();

    fillStep1Fields();
    await clickContinue();

    await waitFor(() =>
      expect(screen.getByLabelText(/bank verification number/i)).toBeInTheDocument(),
    );

    typeInto(screen.getByLabelText(/bank verification number/i), 'BADVALUE');
    await clickSubmit();

    expect(props.onSubmit).not.toHaveBeenCalled();
  });

  it('advances after correcting a previously-invalid field', async () => {
    render(<KycFormEngine {...props} />);
    await flushAsync();

    // First attempt — fullName blank
    typeInto(screen.getByLabelText(/date of birth/i), VALID_DOB);
    typeInto(screen.getByLabelText(/phone number/i), '+2348012345678');
    await clickContinue();

    await waitFor(() =>
      expect(screen.getByText('Personal Information')).toBeInTheDocument(),
    );

    // Fix fullName and retry
    typeInto(screen.getByLabelText(/full legal name/i), 'Emeka Okafor');
    await clickContinue();

    await waitFor(() => {
      expect(screen.getByText('Identity Verification')).toBeInTheDocument();
    });
  });
});

// ---------------------------------------------------------------------------
// Suite 3 — Draft persistence
// ---------------------------------------------------------------------------

describe('Draft persistence', () => {
  let props: ReturnType<typeof makeBaseProps>;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(formPersistenceService.loadDraft).mockReturnValue(null);
    props = makeBaseProps();
  });

  it('calls saveDraft when a field value changes', async () => {
    render(<KycFormEngine {...props} />);
    await flushAsync();

    typeInto(screen.getByLabelText(/full legal name/i), 'Draft Test');

    await waitFor(() => {
      expect(formPersistenceService.saveDraft).toHaveBeenCalled();
    });

    const firstCall = vi.mocked(formPersistenceService.saveDraft).mock
      .calls[0] as [string, unknown, number];
    expect(firstCall[0]).toBe('test-consumer-001');
  });

  it('calls saveDraft with step index 1 after advancing from step 1', async () => {
    render(<KycFormEngine {...props} />);
    await flushAsync();

    fillStep1Fields();
    await clickContinue();

    await waitFor(() =>
      expect(screen.getByText('Identity Verification')).toBeInTheDocument(),
    );

    const calls = vi.mocked(formPersistenceService.saveDraft).mock.calls;
    const lastCall = calls[calls.length - 1] as [string, unknown, number];
    expect(lastCall[2]).toBe(1);
  });

  it('restores fullName from a saved draft on mount', async () => {
    vi.mocked(formPersistenceService.loadDraft).mockReturnValue({
      data: {
        fullName: 'Restored Name',
        phone: '+2348012345678',
        dateOfBirth: VALID_DOB,
      },
      step: 0,
    });

    render(<KycFormEngine {...props} />);
    await flushAsync();

    await waitFor(() => {
      const input = screen.getByLabelText(/full legal name/i) as HTMLInputElement;
      expect(input.value).toBe('Restored Name');
    });
  });

  it('does not persist File objects in the draft data', async () => {
    render(<KycFormEngine {...props} />);
    await flushAsync();

    typeInto(screen.getByLabelText(/full legal name/i), 'No Files Here');

    await waitFor(() =>
      expect(formPersistenceService.saveDraft).toHaveBeenCalled(),
    );

    const calls = vi.mocked(formPersistenceService.saveDraft).mock.calls;
    for (const [, data] of calls as Array<[string, Record<string, unknown>, number]>) {
      for (const value of Object.values(data)) {
        expect(value).not.toBeInstanceOf(File);
      }
    }
  });
});

// ---------------------------------------------------------------------------
// Suite 4 — Country change via useKycCountryRouter
// ---------------------------------------------------------------------------

/**
 * Test harness that combines useKycCountryRouter with KycFormEngine.
 * Selecting a country from the dropdown calls handleCountryChange, which
 * updates the country state and re-mounts the engine via `key={country}`.
 */
function CountrySwitcherHarness({
  initialCountry,
  onSubmit,
}: {
  initialCountry: KycCountry;
  onSubmit: (p: KycSubmissionPayload) => Promise<void>;
}) {
  const [country, setCountry] = useState<KycCountry>(initialCountry);
  const [activeConfig, setActiveConfig] = useState<KycFormConfig | null>(null);

  const { availableCountries, handleCountryChange, currentConfig } =
    useKycCountryRouter({
      currentCountry: country,
      currentTier: KycTier.Basic,
      onCountryChange: (newCountry, newConfig) => {
        setCountry(newCountry);
        setActiveConfig(newConfig);
      },
    });

  const displayConfig = activeConfig ?? currentConfig;

  return (
    <div>
      <select
        aria-label="Select country"
        value={country}
        onChange={(e) => {
          void handleCountryChange(e.target.value as KycCountry);
        }}
      >
        {availableCountries.map((c) => (
          <option key={c} value={c}>
            {c}
          </option>
        ))}
      </select>

      {displayConfig && (
        <span data-testid="config-country">{displayConfig.country}</span>
      )}

      <KycFormEngine
        key={country}
        country={country}
        tier={KycTier.Basic}
        consumerId="harness-consumer"
        onSubmit={onSubmit}
      />
    </div>
  );
}

describe('Country change via useKycCountryRouter', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(formPersistenceService.loadDraft).mockReturnValue(null);
  });

  it('initially renders the Nigeria Basic personal-info step', async () => {
    render(
      <CountrySwitcherHarness
        initialCountry={KycCountry.Nigeria}
        onSubmit={vi.fn().mockResolvedValue(undefined)}
      />,
    );
    await flushAsync();

    expect(screen.getByText('Personal Information')).toBeInTheDocument();
  });

  it('switching to Kenya updates the config-country indicator', async () => {
    render(
      <CountrySwitcherHarness
        initialCountry={KycCountry.Nigeria}
        onSubmit={vi.fn().mockResolvedValue(undefined)}
      />,
    );
    await flushAsync();

    fireEvent.change(screen.getByRole('combobox', { name: /select country/i }), {
      target: { value: KycCountry.Kenya },
    });

    await waitFor(() => {
      expect(screen.getByTestId('config-country')).toHaveTextContent(KycCountry.Kenya);
    });
  });

  it('switching country resets the form to step 1 (Personal Information)', async () => {
    render(
      <CountrySwitcherHarness
        initialCountry={KycCountry.Nigeria}
        onSubmit={vi.fn().mockResolvedValue(undefined)}
      />,
    );
    await flushAsync();

    // Advance to step 2 on Nigeria
    fillStep1Fields();
    await clickContinue();
    await waitFor(() =>
      expect(screen.getByText('Identity Verification')).toBeInTheDocument(),
    );

    // Switch to Ghana — key-based re-mount resets the engine
    fireEvent.change(screen.getByRole('combobox', { name: /select country/i }), {
      target: { value: KycCountry.Ghana },
    });
    await flushAsync();

    await waitFor(() => {
      expect(screen.getByText('Personal Information')).toBeInTheDocument();
    });
  });

  it('step 2 for Ghana shows Ghana Card field, not BVN', async () => {
    render(
      <CountrySwitcherHarness
        initialCountry={KycCountry.Ghana}
        onSubmit={vi.fn().mockResolvedValue(undefined)}
      />,
    );
    await flushAsync();

    fillStep1Fields();
    await clickContinue();

    await waitFor(() => {
      expect(screen.getByLabelText(/ghana card number/i)).toBeInTheDocument();
    });
    expect(screen.queryByLabelText(/bank verification number/i)).not.toBeInTheDocument();
  });

  it('step 2 for Kenya shows KRA PIN field, not BVN', async () => {
    render(
      <CountrySwitcherHarness
        initialCountry={KycCountry.Kenya}
        onSubmit={vi.fn().mockResolvedValue(undefined)}
      />,
    );
    await flushAsync();

    fillStep1Fields();
    await clickContinue();

    await waitFor(() => {
      expect(screen.getByLabelText(/kra pin/i)).toBeInTheDocument();
    });
    expect(screen.queryByLabelText(/bank verification number/i)).not.toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Suite 5 — Submission error handling
// ---------------------------------------------------------------------------

describe('Submission error handling', () => {
  let props: ReturnType<typeof makeBaseProps>;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(formPersistenceService.loadDraft).mockReturnValue(null);
    props = makeBaseProps();
  });

  it('displays a submission error message when onSubmit rejects', async () => {
    props.onSubmit = vi
      .fn<[KycSubmissionPayload], Promise<void>>()
      .mockRejectedValue(new Error('Server unavailable'));

    render(<KycFormEngine {...props} />);
    await flushAsync();

    fillStep1Fields();
    await clickContinue();

    await waitFor(() =>
      expect(screen.getByLabelText(/bank verification number/i)).toBeInTheDocument(),
    );

    typeInto(screen.getByLabelText(/bank verification number/i), '44444444444');
    await clickSubmit();

    await waitFor(() => {
      const alerts = screen.getAllByRole('alert');
      const text = alerts.map((a) => a.textContent).join(' ');
      expect(text).toContain('Server unavailable');
    });
  });

  it('does not clear the draft when submission fails', async () => {
    props.onSubmit = vi
      .fn<[KycSubmissionPayload], Promise<void>>()
      .mockRejectedValue(new Error('Network error'));

    render(<KycFormEngine {...props} />);
    await flushAsync();

    fillStep1Fields();
    await clickContinue();

    await waitFor(() =>
      expect(screen.getByLabelText(/bank verification number/i)).toBeInTheDocument(),
    );

    typeInto(screen.getByLabelText(/bank verification number/i), '55555555555');
    await clickSubmit();

    await waitFor(() => {
      const text = screen.getAllByRole('alert').map((a) => a.textContent).join(' ');
      expect(text).toContain('Network error');
    });

    expect(formPersistenceService.clearDraft).not.toHaveBeenCalled();
  });

  it('disables the Submit button while a submission is in flight', async () => {
    let resolveSubmit!: () => void;
    props.onSubmit = vi.fn<[KycSubmissionPayload], Promise<void>>(
      () => new Promise<void>((resolve) => { resolveSubmit = resolve; }),
    );

    render(<KycFormEngine {...props} />);
    await flushAsync();

    fillStep1Fields();
    await clickContinue();

    await waitFor(() =>
      expect(screen.getByLabelText(/bank verification number/i)).toBeInTheDocument(),
    );

    typeInto(screen.getByLabelText(/bank verification number/i), '66666666666');

    fireEvent.click(screen.getByRole('button', { name: /submit/i }));

    await waitFor(() => {
      expect(
        screen.getByRole('button', { name: /submitting/i }),
      ).toBeDisabled();
    });

    // Resolve to avoid leaking async state into other tests
    act(() => { resolveSubmit(); });
  });
});
