/**
 * Unit tests for KYC Zod validation schemas — task #481 step 8
 *
 * Covers all primitive schemas, composite country/tier schemas,
 * the getSchemaForCountryAndTier selector, and the discriminatedFormErrorSchema.
 */

import { describe, it, expect } from 'vitest';
import {
  bvnSchema,
  ninSchema,
  kraPinSchema,
  ghanaCardSchema,
  phoneSchema,
  dateOfBirthSchema,
  fullNameSchema,
  nigeriaBasicSchema,
  nigeriaStandardSchema,
  kenyaBasicSchema,
  ghanaBasicSchema,
  getSchemaForCountryAndTier,
  discriminatedFormErrorSchema,
} from '@/lib/kyc/schemas';
import { KycCountry, KycTier } from '@/lib/kyc/types';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Returns a date string YYYY-MM-DD that is exactly `years` years ago today. */
function yearsAgo(years: number): string {
  const d = new Date();
  d.setFullYear(d.getFullYear() - years);
  return d.toISOString().slice(0, 10);
}

/** Returns a date string YYYY-MM-DD that is `years` years ago MINUS one day
 * (i.e. the birthday has not happened yet this year for the given age). */
function yearsAgoMinusOneDay(years: number): string {
  const d = new Date();
  d.setFullYear(d.getFullYear() - years);
  d.setDate(d.getDate() + 1); // one day in the future relative to birthday
  return d.toISOString().slice(0, 10);
}

// ---------------------------------------------------------------------------
// bvnSchema
// ---------------------------------------------------------------------------

describe('bvnSchema', () => {
  it('accepts a valid 11-digit BVN', () => {
    expect(bvnSchema.safeParse('12345678901').success).toBe(true);
  });

  it('accepts another valid 11-digit BVN', () => {
    expect(bvnSchema.safeParse('00000000000').success).toBe(true);
  });

  it('rejects a 10-digit BVN (too short)', () => {
    const result = bvnSchema.safeParse('1234567890');
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.error.issues[0].message).toContain('11 digits');
    }
  });

  it('rejects a 12-digit BVN (too long)', () => {
    expect(bvnSchema.safeParse('123456789012').success).toBe(false);
  });

  it('rejects a BVN containing letters', () => {
    expect(bvnSchema.safeParse('1234567890A').success).toBe(false);
  });

  it('rejects an empty string', () => {
    expect(bvnSchema.safeParse('').success).toBe(false);
  });

  it('trims whitespace before validating', () => {
    // After trim, '12345678901' is valid
    expect(bvnSchema.safeParse('  12345678901  ').success).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// ninSchema
// ---------------------------------------------------------------------------

describe('ninSchema', () => {
  it('accepts a valid 11-digit NIN', () => {
    expect(ninSchema.safeParse('98765432109').success).toBe(true);
  });

  it('rejects a 10-digit NIN', () => {
    const result = ninSchema.safeParse('9876543210');
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.error.issues[0].message).toContain('11 digits');
    }
  });

  it('rejects a NIN with letters', () => {
    expect(ninSchema.safeParse('9876543210A').success).toBe(false);
  });

  it('rejects a NIN with special characters', () => {
    expect(ninSchema.safeParse('9876543-101').success).toBe(false);
  });

  it('rejects an empty string', () => {
    expect(ninSchema.safeParse('').success).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// kraPinSchema
// ---------------------------------------------------------------------------

describe('kraPinSchema', () => {
  it('accepts a valid KRA PIN (A123456789Z)', () => {
    expect(kraPinSchema.safeParse('A123456789Z').success).toBe(true);
  });

  it('accepts lowercase input and upcases it', () => {
    const result = kraPinSchema.safeParse('a123456789z');
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data).toBe('A123456789Z');
    }
  });

  it('rejects a PIN that is too short (A12345Z — only 5 digits)', () => {
    expect(kraPinSchema.safeParse('A12345Z').success).toBe(false);
  });

  it('rejects a PIN with all digits (no leading/trailing letter)', () => {
    expect(kraPinSchema.safeParse('12345678901').success).toBe(false);
  });

  it('rejects a PIN with two trailing letters', () => {
    expect(kraPinSchema.safeParse('A12345678ZZ').success).toBe(false);
  });

  it('rejects a PIN missing the trailing letter', () => {
    expect(kraPinSchema.safeParse('A123456789').success).toBe(false);
  });

  it('rejects an empty string', () => {
    expect(kraPinSchema.safeParse('').success).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// ghanaCardSchema
// ---------------------------------------------------------------------------

describe('ghanaCardSchema', () => {
  it('accepts a valid Ghana Card number (GHA-123456789-1)', () => {
    expect(ghanaCardSchema.safeParse('GHA-123456789-1').success).toBe(true);
  });

  it('accepts GHA-000000000-0', () => {
    expect(ghanaCardSchema.safeParse('GHA-000000000-0').success).toBe(true);
  });

  it('rejects a card number missing the GHA prefix', () => {
    expect(ghanaCardSchema.safeParse('123456789-1').success).toBe(false);
  });

  it('rejects a card number with only 8 digits in the middle', () => {
    expect(ghanaCardSchema.safeParse('GHA-12345678-1').success).toBe(false);
  });

  it('rejects a card number with two digits in the trailing segment', () => {
    expect(ghanaCardSchema.safeParse('GHA-123456789-12').success).toBe(false);
  });

  it('rejects a card number with letters in the digit segments', () => {
    expect(ghanaCardSchema.safeParse('GHA-12345678A-1').success).toBe(false);
  });

  it('rejects a card number with lowercase gha prefix', () => {
    // ghanaCardSchema does NOT call .toUpperCase(), so lowercase should fail
    expect(ghanaCardSchema.safeParse('gha-123456789-1').success).toBe(false);
  });

  it('rejects an empty string', () => {
    expect(ghanaCardSchema.safeParse('').success).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// phoneSchema
// ---------------------------------------------------------------------------

describe('phoneSchema', () => {
  it('accepts a valid Nigerian E.164 phone number (+2348012345678)', () => {
    expect(phoneSchema.safeParse('+2348012345678').success).toBe(true);
  });

  it('accepts a valid Kenyan E.164 phone number (+254712345678)', () => {
    expect(phoneSchema.safeParse('+254712345678').success).toBe(true);
  });

  it('accepts a minimum-length E.164 number (+ followed by 7 digits)', () => {
    expect(phoneSchema.safeParse('+1234567').success).toBe(true);
  });

  it('accepts a maximum-length E.164 number (+ followed by 15 digits)', () => {
    expect(phoneSchema.safeParse('+123456789012345').success).toBe(true);
  });

  it('rejects a number without the leading + (08012345678)', () => {
    const result = phoneSchema.safeParse('08012345678');
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.error.issues[0].message).toContain('E.164');
    }
  });

  it('rejects a number that is too short (fewer than 7 digits after +)', () => {
    expect(phoneSchema.safeParse('+12345').success).toBe(false);
  });

  it('rejects a number that is too long (more than 15 digits after +)', () => {
    expect(phoneSchema.safeParse('+1234567890123456').success).toBe(false);
  });

  it('rejects a number with non-digit characters after +', () => {
    expect(phoneSchema.safeParse('+234ABC45678').success).toBe(false);
  });

  it('rejects an empty string', () => {
    expect(phoneSchema.safeParse('').success).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// dateOfBirthSchema
// ---------------------------------------------------------------------------

describe('dateOfBirthSchema', () => {
  it('accepts a valid adult birth date (1990-01-01)', () => {
    expect(dateOfBirthSchema.safeParse('1990-01-01').success).toBe(true);
  });

  it('accepts a birth date that makes the applicant exactly 18 today', () => {
    expect(dateOfBirthSchema.safeParse(yearsAgo(18)).success).toBe(true);
  });

  it('rejects a birth date that makes the applicant under 18 (birthday tomorrow)', () => {
    const result = dateOfBirthSchema.safeParse(yearsAgoMinusOneDay(18));
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.error.issues[0].message).toContain('18 years old');
    }
  });

  it('rejects a birth date that makes the applicant 17 years old', () => {
    expect(dateOfBirthSchema.safeParse(yearsAgo(17)).success).toBe(false);
  });

  it('rejects a non-date string', () => {
    expect(dateOfBirthSchema.safeParse('not-a-date').success).toBe(false);
  });

  it('rejects a date in MM/DD/YYYY format (must be YYYY-MM-DD)', () => {
    expect(dateOfBirthSchema.safeParse('01/01/1990').success).toBe(false);
  });

  it('rejects an invalid calendar date', () => {
    expect(dateOfBirthSchema.safeParse('1990-13-01').success).toBe(false);
  });

  it('rejects an empty string', () => {
    expect(dateOfBirthSchema.safeParse('').success).toBe(false);
  });

  it('accepts a 50-year-old applicant', () => {
    expect(dateOfBirthSchema.safeParse(yearsAgo(50)).success).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// getSchemaForCountryAndTier
// ---------------------------------------------------------------------------

describe('getSchemaForCountryAndTier', () => {
  it('returns the Nigeria Basic schema for Nigeria/Basic', () => {
    const schema = getSchemaForCountryAndTier(KycCountry.Nigeria, KycTier.Basic);
    // Nigeria Basic requires bvn — a successful parse with bvn present confirms
    const result = schema.safeParse({
      fullName: 'Ada Obi',
      phone: '+2348012345678',
      dateOfBirth: '1990-06-15',
      bvn: '12345678901',
    });
    expect(result.success).toBe(true);
  });

  it('returns a schema that does NOT accept kraPin for Nigeria/Basic', () => {
    const schema = getSchemaForCountryAndTier(KycCountry.Nigeria, KycTier.Basic);
    // Strip required fields — kraPin is not a recognised key for this schema
    const result = schema.safeParse({
      fullName: 'Ada Obi',
      phone: '+2348012345678',
      dateOfBirth: '1990-06-15',
      kraPin: 'A123456789Z', // wrong country field
    });
    // bvn is missing so parse should fail
    expect(result.success).toBe(false);
  });

  it('returns the Kenya Basic schema for Kenya/Basic', () => {
    const schema = getSchemaForCountryAndTier(KycCountry.Kenya, KycTier.Basic);
    const result = schema.safeParse({
      fullName: 'Amani Kamau',
      phone: '+254712345678',
      dateOfBirth: '1985-03-20',
      kraPin: 'A123456789Z',
    });
    expect(result.success).toBe(true);
  });

  it('returns the Ghana Basic schema for Ghana/Basic', () => {
    const schema = getSchemaForCountryAndTier(KycCountry.Ghana, KycTier.Basic);
    const result = schema.safeParse({
      fullName: 'Kofi Mensah',
      phone: '+233244123456',
      dateOfBirth: '1992-11-05',
      ghanaCard: 'GHA-123456789-1',
    });
    expect(result.success).toBe(true);
  });

  it('throws for an unsupported country/tier combination', () => {
    // TypeScript cast needed to test runtime guard
    expect(() =>
      getSchemaForCountryAndTier('Wakanda' as KycCountry, KycTier.Basic),
    ).toThrow();
  });
});

// ---------------------------------------------------------------------------
// nigeriaBasicSchema — happy path & missing field
// ---------------------------------------------------------------------------

describe('nigeriaBasicSchema', () => {
  const validPayload = {
    fullName: 'Emeka Okafor',
    phone: '+2348012345678',
    dateOfBirth: '1988-07-22',
    bvn: '22222222222',
  };

  it('parses a complete valid payload successfully', () => {
    const result = nigeriaBasicSchema.safeParse(validPayload);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.fullName).toBe('Emeka Okafor');
      expect(result.data.bvn).toBe('22222222222');
    }
  });

  it('fails when fullName is missing', () => {
    const { fullName: _omit, ...rest } = validPayload;
    const result = nigeriaBasicSchema.safeParse(rest);
    expect(result.success).toBe(false);
  });

  it('fails when phone is missing', () => {
    const { phone: _omit, ...rest } = validPayload;
    const result = nigeriaBasicSchema.safeParse(rest);
    expect(result.success).toBe(false);
  });

  it('fails when bvn is missing', () => {
    const { bvn: _omit, ...rest } = validPayload;
    const result = nigeriaBasicSchema.safeParse(rest);
    expect(result.success).toBe(false);
  });

  it('fails when dateOfBirth is missing', () => {
    const { dateOfBirth: _omit, ...rest } = validPayload;
    const result = nigeriaBasicSchema.safeParse(rest);
    expect(result.success).toBe(false);
  });

  it('fails when fullName is a single character (below minimum length)', () => {
    const result = nigeriaBasicSchema.safeParse({ ...validPayload, fullName: 'A' });
    expect(result.success).toBe(false);
  });

  it('fails when bvn has only 10 digits', () => {
    const result = nigeriaBasicSchema.safeParse({ ...validPayload, bvn: '1234567890' });
    expect(result.success).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// nigeriaStandardSchema — superset of basic
// ---------------------------------------------------------------------------

describe('nigeriaStandardSchema', () => {
  const validPayload = {
    fullName: 'Ngozi Adeyemi',
    phone: '+2348098765432',
    dateOfBirth: '1990-02-14',
    bvn: '11111111111',
    nin: '99999999999',
    address: '14 Adeola Odeku Street, Victoria Island',
  };

  it('parses a complete Standard payload', () => {
    const result = nigeriaStandardSchema.safeParse(validPayload);
    expect(result.success).toBe(true);
  });

  it('fails without nin', () => {
    const { nin: _omit, ...rest } = validPayload;
    const result = nigeriaStandardSchema.safeParse(rest);
    expect(result.success).toBe(false);
  });

  it('fails without address', () => {
    const { address: _omit, ...rest } = validPayload;
    const result = nigeriaStandardSchema.safeParse(rest);
    expect(result.success).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// kenyaBasicSchema
// ---------------------------------------------------------------------------

describe('kenyaBasicSchema', () => {
  it('parses a valid Kenya Basic payload', () => {
    const result = kenyaBasicSchema.safeParse({
      fullName: 'Wanjiru Mwangi',
      phone: '+254722987654',
      dateOfBirth: '1995-09-10',
      kraPin: 'B987654321C',
    });
    expect(result.success).toBe(true);
  });

  it('fails when kraPin is absent', () => {
    const result = kenyaBasicSchema.safeParse({
      fullName: 'Wanjiru Mwangi',
      phone: '+254722987654',
      dateOfBirth: '1995-09-10',
    });
    expect(result.success).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// ghanaBasicSchema
// ---------------------------------------------------------------------------

describe('ghanaBasicSchema', () => {
  it('parses a valid Ghana Basic payload', () => {
    const result = ghanaBasicSchema.safeParse({
      fullName: 'Ama Asante',
      phone: '+233507654321',
      dateOfBirth: '1991-04-25',
      ghanaCard: 'GHA-987654321-5',
    });
    expect(result.success).toBe(true);
  });

  it('fails when ghanaCard is absent', () => {
    const result = ghanaBasicSchema.safeParse({
      fullName: 'Ama Asante',
      phone: '+233507654321',
      dateOfBirth: '1991-04-25',
    });
    expect(result.success).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// fullNameSchema (spot-checks)
// ---------------------------------------------------------------------------

describe('fullNameSchema', () => {
  it('accepts a standard two-word name', () => {
    expect(fullNameSchema.safeParse('John Doe').success).toBe(true);
  });

  it('rejects a single-character name', () => {
    expect(fullNameSchema.safeParse('A').success).toBe(false);
  });

  it('rejects a name longer than 100 characters', () => {
    expect(fullNameSchema.safeParse('A'.repeat(101)).success).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// discriminatedFormErrorSchema
// ---------------------------------------------------------------------------

describe('discriminatedFormErrorSchema', () => {
  it('parses a VALIDATION_ERROR correctly', () => {
    const result = discriminatedFormErrorSchema.safeParse({
      code: 'VALIDATION_ERROR',
      message: 'BVN is invalid',
      field: 'bvn',
    });
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.code).toBe('VALIDATION_ERROR');
      expect(result.data.message).toBe('BVN is invalid');
    }
  });

  it('parses a VALIDATION_ERROR without the optional field property', () => {
    const result = discriminatedFormErrorSchema.safeParse({
      code: 'VALIDATION_ERROR',
      message: 'Form-level error',
    });
    expect(result.success).toBe(true);
  });

  it('parses an API_TIMEOUT correctly', () => {
    const result = discriminatedFormErrorSchema.safeParse({
      code: 'API_TIMEOUT',
      message: 'Request timed out',
      retryAfterMs: 3000,
    });
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.code).toBe('API_TIMEOUT');
    }
  });

  it('parses a DOCUMENT_REJECTED correctly', () => {
    const result = discriminatedFormErrorSchema.safeParse({
      code: 'DOCUMENT_REJECTED',
      message: 'Document was blurry',
      documentType: 'BVN',
    });
    expect(result.success).toBe(true);
  });

  it('parses a NAME_MISMATCH correctly', () => {
    const result = discriminatedFormErrorSchema.safeParse({
      code: 'NAME_MISMATCH',
      message: 'Name does not match',
      providedName: 'John Doe',
      idName: 'Jonathan Doe',
    });
    expect(result.success).toBe(true);
  });

  it('parses a DUPLICATE_IDENTITY correctly', () => {
    const result = discriminatedFormErrorSchema.safeParse({
      code: 'DUPLICATE_IDENTITY',
      message: 'Identity already in use',
    });
    expect(result.success).toBe(true);
  });

  it('rejects an unknown error code', () => {
    const result = discriminatedFormErrorSchema.safeParse({
      code: 'UNKNOWN_CODE',
      message: 'Some error',
    });
    expect(result.success).toBe(false);
  });

  it('rejects a VALIDATION_ERROR missing the required message field', () => {
    const result = discriminatedFormErrorSchema.safeParse({
      code: 'VALIDATION_ERROR',
    });
    expect(result.success).toBe(false);
  });
});
