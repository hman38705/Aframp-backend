/**
 * FormPersistenceService — task #481 step 4
 *
 * Persists KYC multi-step form drafts to `localStorage` so users can resume
 * an interrupted flow without losing progress.
 *
 * ## Obfuscation
 * Draft data is XOR-obfuscated before being written to storage and
 * de-obfuscated on read.  This prevents casual inspection of sensitive form
 * fields (BVN, NIN, KRA PIN, etc.) in browser DevTools.
 *
 * **Security note**: XOR with a static key is NOT cryptographically strong.
 * It is obfuscation only — a determined observer with access to the storage
 * key (which ships in the client bundle) can trivially reverse it.  For
 * genuine at-rest encryption of sensitive PII, a Web Crypto API–based
 * solution with a user-derived key should be used instead.
 *
 * ## SSR safety
 * Every `localStorage` access is wrapped in `try/catch` so the service
 * degrades silently when running in a server-side rendering context where
 * `localStorage` is not defined.
 */

import type { KycFormValues } from './types';

// ---------------------------------------------------------------------------
// Internal storage shape
// ---------------------------------------------------------------------------

/** Envelope stored in localStorage (after XOR-obfuscation → base64). */
interface DraftEnvelope {
  /** ISO-8601 timestamp of when the draft was last saved. */
  savedAt: string;
  /** Zero-indexed step the user was on when the draft was saved. */
  step: number;
  /** Partial form field values. */
  data: Partial<KycFormValues>;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Drafts older than this are automatically expired on load. */
const DRAFT_MAX_AGE_MS = 24 * 60 * 60 * 1000; // 24 hours

// ---------------------------------------------------------------------------
// XOR obfuscation helpers
// ---------------------------------------------------------------------------

/**
 * XOR-obfuscates a plain-text string using a cycling key, then base64-encodes
 * the result for safe storage.
 *
 * @param plaintext - The UTF-16 string to obfuscate.
 * @param key - Cycling XOR key (static, ships in client bundle — see module
 *   JSDoc for the security caveat).
 * @returns Base64-encoded obfuscated string.
 */
function xorObfuscate(plaintext: string, key: string): string {
  if (!key) return btoa(plaintext);

  const keyLen = key.length;
  let result = '';

  for (let i = 0; i < plaintext.length; i++) {
    // XOR the char code of the plaintext character with the cycling key char.
    const obfuscatedCode = plaintext.charCodeAt(i) ^ key.charCodeAt(i % keyLen);
    result += String.fromCharCode(obfuscatedCode);
  }

  return btoa(result);
}

/**
 * Reverses `xorObfuscate`: base64-decodes, then XOR-deobfuscates.
 *
 * @param obfuscated - Base64-encoded obfuscated string (output of `xorObfuscate`).
 * @param key - The same cycling XOR key used during obfuscation.
 * @returns Original plain-text string.
 */
function xorDeobfuscate(obfuscated: string, key: string): string {
  const decoded = atob(obfuscated);

  if (!key) return decoded;

  const keyLen = key.length;
  let result = '';

  for (let i = 0; i < decoded.length; i++) {
    // XOR with the same key — XOR is its own inverse.
    const originalCode = decoded.charCodeAt(i) ^ key.charCodeAt(i % keyLen);
    result += String.fromCharCode(originalCode);
  }

  return result;
}

// ---------------------------------------------------------------------------
// FormPersistenceService
// ---------------------------------------------------------------------------

/**
 * Service responsible for persisting and restoring KYC form drafts using
 * `localStorage`.
 *
 * Draft data is XOR-obfuscated before storage to prevent casual reads.
 * **This is NOT cryptographic encryption** — see the module-level JSDoc.
 *
 * Drafts are automatically expired after 24 hours on `loadDraft()`.
 *
 * All public methods are SSR-safe: they silently return `null` / `false`
 * when `localStorage` is unavailable.
 *
 * @example
 * ```ts
 * // Save
 * formPersistenceService.saveDraft('consumer-123', formValues, 2);
 *
 * // Load
 * const draft = formPersistenceService.loadDraft('consumer-123');
 * if (draft) {
 *   restoreForm(draft.data, draft.step);
 * }
 *
 * // Clear on successful submission
 * formPersistenceService.clearDraft('consumer-123');
 * ```
 */
export class FormPersistenceService {
  /**
   * Prefix used when constructing the localStorage key.
   * The full key is `{storageKeyPrefix}_{consumerId}`.
   */
  private readonly storageKeyPrefix: string;

  /**
   * Cycling XOR key used for obfuscation/deobfuscation.
   *
   * @remarks
   * This key ships in the client bundle and is therefore NOT secret.
   * It provides obfuscation only — see the module-level JSDoc.
   */
  private readonly encryptionKey: string;

  /**
   * @param storageKeyPrefix - Prefix for the localStorage key
   *   (e.g. `"kyc_draft"`). The full key will be `{prefix}_{consumerId}`.
   * @param encryptionKey - Static cycling XOR key used for obfuscation.
   *   Ships in the client bundle — see module JSDoc for security caveats.
   */
  constructor(storageKeyPrefix: string, encryptionKey: string) {
    this.storageKeyPrefix = storageKeyPrefix;
    this.encryptionKey = encryptionKey;
  }

  // -------------------------------------------------------------------------
  // Private helpers
  // -------------------------------------------------------------------------

  /** Builds the localStorage key for a given consumer ID. */
  private buildKey(consumerId: string): string {
    return `${this.storageKeyPrefix}_${consumerId}`;
  }

  // -------------------------------------------------------------------------
  // Public API
  // -------------------------------------------------------------------------

  /**
   * Persists a partial draft to `localStorage`.
   *
   * The envelope includes a `savedAt` timestamp (ISO-8601) used for
   * auto-expiry.  Data is XOR-obfuscated before writing.
   *
   * No-ops silently if `localStorage` is unavailable (SSR context).
   *
   * @param consumerId - Unique identifier for the KYC consumer / account.
   * @param data - Partial form values to persist.
   * @param step - Zero-indexed step the user is currently on.
   */
  saveDraft(consumerId: string, data: Partial<KycFormValues>, step: number): void {
    try {
      const envelope: DraftEnvelope = {
        savedAt: new Date().toISOString(),
        step,
        data,
      };

      const json = JSON.stringify(envelope);
      const obfuscated = xorObfuscate(json, this.encryptionKey);

      localStorage.setItem(this.buildKey(consumerId), obfuscated);
    } catch {
      // localStorage unavailable (SSR, private browsing quota exceeded, etc.)
    }
  }

  /**
   * Loads a previously-saved draft from `localStorage`.
   *
   * Automatically clears and returns `null` for drafts older than 24 hours.
   *
   * @param consumerId - Unique identifier for the KYC consumer / account.
   * @returns The saved `{ data, step }` pair, or `null` if no valid draft exists.
   */
  loadDraft(consumerId: string): { data: Partial<KycFormValues>; step: number } | null {
    try {
      const key = this.buildKey(consumerId);
      const stored = localStorage.getItem(key);

      if (!stored) return null;

      const json = xorDeobfuscate(stored, this.encryptionKey);
      const envelope = JSON.parse(json) as DraftEnvelope;

      // Auto-expire drafts older than DRAFT_MAX_AGE_MS.
      const savedAt = new Date(envelope.savedAt).getTime();
      if (isNaN(savedAt) || Date.now() - savedAt > DRAFT_MAX_AGE_MS) {
        localStorage.removeItem(key);
        return null;
      }

      return {
        data: envelope.data,
        step: envelope.step,
      };
    } catch {
      // Corrupted data, JSON parse error, localStorage unavailable, etc.
      return null;
    }
  }

  /**
   * Removes the draft for the given consumer from `localStorage`.
   *
   * Call this after a successful KYC submission to clean up.
   *
   * @param consumerId - Unique identifier for the KYC consumer / account.
   */
  clearDraft(consumerId: string): void {
    try {
      localStorage.removeItem(this.buildKey(consumerId));
    } catch {
      // localStorage unavailable.
    }
  }

  /**
   * Returns `true` if a draft exists for the given consumer, `false` otherwise.
   *
   * Does NOT check whether the draft has expired — call `loadDraft()` to get
   * a valid draft (it auto-expires stale entries).
   *
   * @param consumerId - Unique identifier for the KYC consumer / account.
   */
  hasDraft(consumerId: string): boolean {
    try {
      return localStorage.getItem(this.buildKey(consumerId)) !== null;
    } catch {
      return false;
    }
  }

  /**
   * Returns the age of the stored draft in milliseconds, or `null` if no
   * draft exists or the timestamp cannot be parsed.
   *
   * Useful for showing the user a "You have a draft saved X minutes ago"
   * message before deciding whether to restore it.
   *
   * @param consumerId - Unique identifier for the KYC consumer / account.
   * @returns Age in milliseconds, or `null`.
   */
  getDraftAge(consumerId: string): number | null {
    try {
      const key = this.buildKey(consumerId);
      const stored = localStorage.getItem(key);

      if (!stored) return null;

      const json = xorDeobfuscate(stored, this.encryptionKey);
      const envelope = JSON.parse(json) as DraftEnvelope;

      const savedAt = new Date(envelope.savedAt).getTime();
      if (isNaN(savedAt)) return null;

      return Date.now() - savedAt;
    } catch {
      return null;
    }
  }
}

// ---------------------------------------------------------------------------
// Singleton
// ---------------------------------------------------------------------------

/**
 * Shared `FormPersistenceService` instance.
 *
 * Import this singleton in any component or hook that needs to save/load
 * KYC draft state rather than constructing a new instance.
 *
 * @example
 * ```ts
 * import { formPersistenceService } from '@/lib/kyc/persistence';
 *
 * formPersistenceService.saveDraft(consumerId, values, currentStep);
 * ```
 */
export const formPersistenceService = new FormPersistenceService(
  'kyc_draft',
  'aframp_kyc_v1',
);
