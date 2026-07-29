'use client';

/**
 * useKycCountryRouter — task #481 step 3
 *
 * Country-routing interceptor hook for the KYC multi-step form.
 *
 * Responsibilities:
 * - Tracks the currently-selected country and tier as local state.
 * - Looks up the matching `KycFormConfig` from `KYC_FORM_CONFIGS` whenever
 *   the country or tier changes.
 * - Calls `onCountryChange` with the new config so the parent can reset
 *   form state, re-validate, etc.
 * - Simulates an async schema-pointer update via `Promise.resolve()`;
 *   the pattern is intentionally left open to swap in a real remote-config
 *   fetch without changing callers.
 * - Wraps handlers in `useCallback` to keep referential stability and
 *   prevent unnecessary child re-renders.
 */

import { useState, useCallback, useMemo } from 'react';
import { KYC_FORM_CONFIGS } from '@/lib/kyc/form-config';
import type { KycFormConfig } from '@/lib/kyc/types';
import { KycCountry, KycTier } from '@/lib/kyc/types';

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

export interface UseKycCountryRouterOptions {
  /** Country the form was initialised with, or null if not yet selected. */
  currentCountry: KycCountry | null;
  /** Tier the form was initialised with. */
  currentTier: KycTier;
  /**
   * Callback fired whenever the effective (country, tier) combination changes.
   * Receives the resolved `KycFormConfig` so the caller can reset form state.
   */
  onCountryChange: (country: KycCountry, config: KycFormConfig) => void;
}

export interface UseKycCountryRouterReturn {
  /** All countries supported by the platform. */
  availableCountries: KycCountry[];
  /** All verification tiers supported by the platform. */
  availableTiers: KycTier[];
  /**
   * Call when the user picks a new country.
   * Resolves asynchronously (simulated; extensible to remote-config fetch).
   */
  handleCountryChange: (country: KycCountry) => Promise<void>;
  /**
   * Call when the user selects a different verification tier.
   * Resolves asynchronously (simulated; extensible to remote-config fetch).
   */
  handleTierChange: (tier: KycTier) => Promise<void>;
  /** The form configuration for the currently-selected country + tier, or null. */
  currentConfig: KycFormConfig | null;
}

// ---------------------------------------------------------------------------
// Derived constants — computed once at module level
// ---------------------------------------------------------------------------

/** Ordered list of all supported countries, derived from the enum. */
const AVAILABLE_COUNTRIES: KycCountry[] = Object.values(KycCountry);

/** Ordered list of all supported tiers, derived from the enum. */
const AVAILABLE_TIERS: KycTier[] = Object.values(KycTier);

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

/**
 * Country-routing interceptor for the KYC multi-step form.
 *
 * @example
 * ```tsx
 * const { availableCountries, handleCountryChange, currentConfig } =
 *   useKycCountryRouter({
 *     currentCountry: null,
 *     currentTier: KycTier.Basic,
 *     onCountryChange: (country, config) => {
 *       setCountry(country);
 *       resetForm(config);
 *     },
 *   });
 * ```
 */
export function useKycCountryRouter({
  currentCountry,
  currentTier,
  onCountryChange,
}: UseKycCountryRouterOptions): UseKycCountryRouterReturn {
  // Internal selected-country state.  Initialised from the prop so the hook
  // owns a copy; callers that need to keep the source of truth outside the
  // hook should pass the prop through their own state.
  const [selectedCountry, setSelectedCountry] = useState<KycCountry | null>(currentCountry);
  const [selectedTier, setSelectedTier] = useState<KycTier>(currentTier);

  // ---------------------------------------------------------------------------
  // Derived config — recomputed only when country or tier changes
  // ---------------------------------------------------------------------------

  const currentConfig = useMemo<KycFormConfig | null>(() => {
    if (!selectedCountry) return null;
    return KYC_FORM_CONFIGS[selectedCountry]?.[selectedTier] ?? null;
  }, [selectedCountry, selectedTier]);

  // ---------------------------------------------------------------------------
  // Handlers
  // ---------------------------------------------------------------------------

  /**
   * Handle a country-selection change.
   *
   * The async wrapper simulates a remote schema-pointer fetch.  Replace the
   * `Promise.resolve()` with an actual `fetch()` call once remote configs are
   * available (e.g. `await fetchRemoteKycConfig(country, selectedTier)`).
   */
  const handleCountryChange = useCallback(
    async (country: KycCountry): Promise<void> => {
      // Simulate an async operation (e.g. fetching a remote schema pointer).
      await Promise.resolve();

      const config = KYC_FORM_CONFIGS[country]?.[selectedTier];
      if (!config) {
        // No config for this combo — do not update state.
        console.warn(
          `[useKycCountryRouter] No config found for country="${country}" tier="${selectedTier}". ` +
            'Skipping country change.',
        );
        return;
      }

      setSelectedCountry(country);
      onCountryChange(country, config);
    },
    [selectedTier, onCountryChange],
  );

  /**
   * Handle a tier-selection change.
   *
   * Keeps the current country intact and looks up the config for the new tier.
   * Also simulates async schema-pointer resolution.
   */
  const handleTierChange = useCallback(
    async (tier: KycTier): Promise<void> => {
      // Simulate an async operation.
      await Promise.resolve();

      if (!selectedCountry) {
        // No country selected yet — update tier state but don't fire the
        // callback because we don't have a complete (country, tier) pair.
        setSelectedTier(tier);
        return;
      }

      const config = KYC_FORM_CONFIGS[selectedCountry]?.[tier];
      if (!config) {
        console.warn(
          `[useKycCountryRouter] No config found for country="${selectedCountry}" tier="${tier}". ` +
            'Skipping tier change.',
        );
        return;
      }

      setSelectedTier(tier);
      onCountryChange(selectedCountry, config);
    },
    [selectedCountry, onCountryChange],
  );

  // ---------------------------------------------------------------------------
  // Return
  // ---------------------------------------------------------------------------

  return {
    availableCountries: AVAILABLE_COUNTRIES,
    availableTiers: AVAILABLE_TIERS,
    handleCountryChange,
    handleTierChange,
    currentConfig,
  };
}
