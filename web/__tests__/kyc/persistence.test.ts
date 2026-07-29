/**
 * Unit tests for FormPersistenceService — task #481 step 8
 *
 * Tests cover:
 * - saveDraft + loadDraft round-trip
 * - clearDraft removes the stored item
 * - hasDraft returns correct boolean
 * - getDraftAge returns approximate age
 * - Auto-expiry: drafts older than 24 h are discarded on load
 * - Graceful degradation when localStorage throws (e.g. SSR / quota exceeded)
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { FormPersistenceService, formPersistenceService } from '@/lib/kyc/persistence';

// ---------------------------------------------------------------------------
// localStorage mock helpers
// ---------------------------------------------------------------------------

function createLocalStorageMock() {
  const store: Record<string, string> = {};

  return {
    getItem: vi.fn((key: string) => store[key] ?? null),
    setItem: vi.fn((key: string, value: string) => {
      store[key] = value;
    }),
    removeItem: vi.fn((key: string) => {
      delete store[key];
    }),
    clear: vi.fn(() => {
      for (const k of Object.keys(store)) {
        delete store[k];
      }
    }),
    get length() {
      return Object.keys(store).length;
    },
    key: vi.fn((index: number) => Object.keys(store)[index] ?? null),
    _store: store,
  };
}

// ---------------------------------------------------------------------------
// Test suite
// ---------------------------------------------------------------------------

describe('FormPersistenceService', () => {
  let mockLocalStorage: ReturnType<typeof createLocalStorageMock>;

  beforeEach(() => {
    mockLocalStorage = createLocalStorageMock();
    vi.stubGlobal('localStorage', mockLocalStorage);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  // ── Construction ────────────────────────────────────────────────────────

  it('is constructed with a storageKeyPrefix and encryptionKey', () => {
    const svc = new FormPersistenceService('test_prefix', 'test_key');
    expect(svc).toBeDefined();
  });

  // ── saveDraft + loadDraft round-trip ────────────────────────────────────

  describe('saveDraft + loadDraft', () => {
    it('round-trips a draft with all scalar field types', () => {
      const svc = new FormPersistenceService('kyc_test', 'roundtrip_key');
      const data = {
        fullName: 'Ada Obi',
        phone: '+2348012345678',
        bvn: '12345678901',
        dateOfBirth: '1990-01-15',
        someNumber: 42,
        someBoolean: true,
      };

      svc.saveDraft('consumer-001', data, 1);
      const loaded = svc.loadDraft('consumer-001');

      expect(loaded).not.toBeNull();
      expect(loaded?.data).toEqual(data);
      expect(loaded?.step).toBe(1);
    });

    it('preserves step 0 correctly', () => {
      const svc = new FormPersistenceService('kyc_test', 'step_key');
      svc.saveDraft('consumer-002', { fullName: 'Kofi' }, 0);
      const loaded = svc.loadDraft('consumer-002');
      expect(loaded?.step).toBe(0);
    });

    it('overwrites an existing draft when called again', () => {
      const svc = new FormPersistenceService('kyc_test', 'overwrite_key');
      svc.saveDraft('consumer-003', { fullName: 'First Save' }, 0);
      svc.saveDraft('consumer-003', { fullName: 'Second Save' }, 2);

      const loaded = svc.loadDraft('consumer-003');
      expect(loaded?.data.fullName).toBe('Second Save');
      expect(loaded?.step).toBe(2);
    });

    it('returns null when no draft exists for the consumer', () => {
      const svc = new FormPersistenceService('kyc_test', 'missing_key');
      expect(svc.loadDraft('no-such-consumer')).toBeNull();
    });

    it('different consumers have independent drafts', () => {
      const svc = new FormPersistenceService('kyc_test', 'isolated_key');
      svc.saveDraft('consumer-A', { fullName: 'Alice' }, 0);
      svc.saveDraft('consumer-B', { fullName: 'Bob' }, 1);

      expect(svc.loadDraft('consumer-A')?.data.fullName).toBe('Alice');
      expect(svc.loadDraft('consumer-B')?.data.fullName).toBe('Bob');
    });

    it('data integrity is preserved (no field truncation or corruption)', () => {
      const svc = new FormPersistenceService('kyc_test', 'integrity_key');
      const longAddress = 'A'.repeat(200);
      svc.saveDraft('consumer-005', { address: longAddress }, 3);
      const loaded = svc.loadDraft('consumer-005');
      expect(loaded?.data.address).toBe(longAddress);
    });
  });

  // ── clearDraft ──────────────────────────────────────────────────────────

  describe('clearDraft', () => {
    it('removes the draft from localStorage so loadDraft returns null', () => {
      const svc = new FormPersistenceService('kyc_test', 'clear_key');
      svc.saveDraft('consumer-del', { fullName: 'Delete Me' }, 0);

      svc.clearDraft('consumer-del');

      expect(svc.loadDraft('consumer-del')).toBeNull();
    });

    it('calls localStorage.removeItem with the correct key', () => {
      const svc = new FormPersistenceService('kyc_test', 'clear_key2');
      svc.saveDraft('consumer-del2', {}, 0);
      svc.clearDraft('consumer-del2');

      expect(mockLocalStorage.removeItem).toHaveBeenCalledWith('kyc_test_consumer-del2');
    });

    it('does not throw when the draft does not exist', () => {
      const svc = new FormPersistenceService('kyc_test', 'clear_noexist');
      expect(() => svc.clearDraft('nonexistent')).not.toThrow();
    });
  });

  // ── hasDraft ────────────────────────────────────────────────────────────

  describe('hasDraft', () => {
    it('returns false when no draft has been saved', () => {
      const svc = new FormPersistenceService('kyc_test', 'hasdraft_key');
      expect(svc.hasDraft('consumer-hdA')).toBe(false);
    });

    it('returns true after saving a draft', () => {
      const svc = new FormPersistenceService('kyc_test', 'hasdraft_key');
      svc.saveDraft('consumer-hdB', { fullName: 'Has Draft' }, 0);
      expect(svc.hasDraft('consumer-hdB')).toBe(true);
    });

    it('returns false after clearing an existing draft', () => {
      const svc = new FormPersistenceService('kyc_test', 'hasdraft_key');
      svc.saveDraft('consumer-hdC', {}, 0);
      svc.clearDraft('consumer-hdC');
      expect(svc.hasDraft('consumer-hdC')).toBe(false);
    });
  });

  // ── getDraftAge ─────────────────────────────────────────────────────────

  describe('getDraftAge', () => {
    it('returns null when no draft exists', () => {
      const svc = new FormPersistenceService('kyc_test', 'age_key');
      expect(svc.getDraftAge('no-draft-consumer')).toBeNull();
    });

    it('returns a non-negative age in milliseconds immediately after saving', () => {
      const svc = new FormPersistenceService('kyc_test', 'age_key');
      svc.saveDraft('consumer-age1', { fullName: 'Fresh' }, 0);
      const age = svc.getDraftAge('consumer-age1');
      expect(age).not.toBeNull();
      // Saved just now — should be < 1 second (generous upper bound for slow CI)
      expect(age!).toBeGreaterThanOrEqual(0);
      expect(age!).toBeLessThan(5000);
    });

    it('returns an approximate age when the clock advances', () => {
      const svc = new FormPersistenceService('kyc_test', 'age_key2');

      // Freeze time at a known past moment
      const pastTime = Date.now() - 60_000; // 60 seconds ago
      const realNow = Date.now;

      // Trick: save with a timestamp 60 s in the past by temporarily mocking Date
      const pastDate = new Date(pastTime);
      vi.spyOn(global, 'Date').mockImplementation(
        (...args: ConstructorParameters<DateConstructor>) =>
          args.length === 0
            ? pastDate
            : new (Function.prototype.bind.apply(
                globalThis.Date,
                [null, ...args],
              ) as typeof Date)(),
      );
      // Also mock Date.now for the saveDraft timestamp
      vi.spyOn(Date, 'now').mockReturnValue(pastTime);

      svc.saveDraft('consumer-age2', { fullName: 'Old' }, 0);

      // Restore Date so getDraftAge uses real current time
      vi.restoreAllMocks();

      const age = svc.getDraftAge('consumer-age2');
      // Age should be around 60 000 ms — allow 10 s leeway for CI slowness
      expect(age).not.toBeNull();
      expect(age!).toBeGreaterThanOrEqual(55_000);
    });
  });

  // ── Auto-expiry ─────────────────────────────────────────────────────────

  describe('auto-expiry on loadDraft', () => {
    it('returns null for a draft saved more than 24 hours ago', () => {
      const svc = new FormPersistenceService('kyc_test', 'expiry_key');
      const TWENTY_FIVE_HOURS_AGO = Date.now() - 25 * 60 * 60 * 1000;

      vi.spyOn(Date, 'now').mockReturnValue(TWENTY_FIVE_HOURS_AGO);
      svc.saveDraft('consumer-old', { fullName: 'Ancient Draft' }, 1);
      vi.restoreAllMocks(); // restore Date.now so loadDraft sees current time

      const loaded = svc.loadDraft('consumer-old');
      expect(loaded).toBeNull();
    });

    it('removes the expired draft from storage (calls removeItem)', () => {
      const svc = new FormPersistenceService('kyc_test', 'expiry_key2');
      const TWENTY_FIVE_HOURS_AGO = Date.now() - 25 * 60 * 60 * 1000;

      vi.spyOn(Date, 'now').mockReturnValue(TWENTY_FIVE_HOURS_AGO);
      svc.saveDraft('consumer-old2', { fullName: 'Stale' }, 0);
      vi.restoreAllMocks();

      svc.loadDraft('consumer-old2');

      // After expiry removal, hasDraft should return false
      expect(svc.hasDraft('consumer-old2')).toBe(false);
    });

    it('returns the draft when it is exactly 23 hours old (not expired)', () => {
      const svc = new FormPersistenceService('kyc_test', 'expiry_key3');
      const TWENTY_THREE_HOURS_AGO = Date.now() - 23 * 60 * 60 * 1000;

      vi.spyOn(Date, 'now').mockReturnValue(TWENTY_THREE_HOURS_AGO);
      svc.saveDraft('consumer-fresh', { fullName: 'Recent' }, 0);
      vi.restoreAllMocks();

      const loaded = svc.loadDraft('consumer-fresh');
      expect(loaded).not.toBeNull();
      expect(loaded?.data.fullName).toBe('Recent');
    });
  });

  // ── Graceful degradation when localStorage throws ────────────────────────

  describe('graceful degradation when localStorage is unavailable', () => {
    it('loadDraft returns null without throwing when getItem throws', () => {
      const svc = new FormPersistenceService('kyc_test', 'error_key');
      mockLocalStorage.getItem.mockImplementation(() => {
        throw new Error('localStorage not available');
      });

      expect(() => svc.loadDraft('consumer-err')).not.toThrow();
      expect(svc.loadDraft('consumer-err')).toBeNull();
    });

    it('saveDraft does not throw when setItem throws', () => {
      const svc = new FormPersistenceService('kyc_test', 'error_key2');
      mockLocalStorage.setItem.mockImplementation(() => {
        throw new DOMException('QuotaExceededError');
      });

      expect(() =>
        svc.saveDraft('consumer-quota', { fullName: 'Quota' }, 0),
      ).not.toThrow();
    });

    it('clearDraft does not throw when removeItem throws', () => {
      const svc = new FormPersistenceService('kyc_test', 'error_key3');
      mockLocalStorage.removeItem.mockImplementation(() => {
        throw new Error('Storage error');
      });

      expect(() => svc.clearDraft('consumer-err3')).not.toThrow();
    });

    it('hasDraft returns false without throwing when getItem throws', () => {
      const svc = new FormPersistenceService('kyc_test', 'error_key4');
      mockLocalStorage.getItem.mockImplementation(() => {
        throw new Error('Unavailable');
      });

      expect(() => svc.hasDraft('consumer-err4')).not.toThrow();
      expect(svc.hasDraft('consumer-err4')).toBe(false);
    });

    it('getDraftAge returns null without throwing when getItem throws', () => {
      const svc = new FormPersistenceService('kyc_test', 'error_key5');
      mockLocalStorage.getItem.mockImplementation(() => {
        throw new Error('Unavailable');
      });

      expect(() => svc.getDraftAge('consumer-err5')).not.toThrow();
      expect(svc.getDraftAge('consumer-err5')).toBeNull();
    });

    it('loadDraft returns null without throwing on corrupted stored data', () => {
      const svc = new FormPersistenceService('kyc_test', 'corrupt_key');
      // Force a corrupt value that will fail base64 decode or JSON parse
      mockLocalStorage.getItem.mockReturnValue('!!!not-valid-base64!!!');

      expect(() => svc.loadDraft('consumer-corrupt')).not.toThrow();
      expect(svc.loadDraft('consumer-corrupt')).toBeNull();
    });
  });

  // ── Singleton export ────────────────────────────────────────────────────

  describe('formPersistenceService singleton', () => {
    it('is exported as a FormPersistenceService instance', () => {
      expect(formPersistenceService).toBeInstanceOf(FormPersistenceService);
    });

    it('singleton saves and loads a draft correctly', () => {
      formPersistenceService.saveDraft('singleton-user', { fullName: 'Singleton' }, 0);
      const loaded = formPersistenceService.loadDraft('singleton-user');
      expect(loaded?.data.fullName).toBe('Singleton');
    });
  });
});
