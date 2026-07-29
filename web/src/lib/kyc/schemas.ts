/**
 * KYC/KYB Zod validation schemas — task #481 step 1
 *
 * All schemas are exported.  Country+tier composite schemas are built
 * progressively (Basic → Standard → Premium) using `.extend()` so that
 * each level is a strict superset of the previous one.
 */

import { z } from "zod";
import { KycCountry, KycTier } from "./types";

// ---------------------------------------------------------------------------
// Primitive / reusable field schemas
// ---------------------------------------------------------------------------

/**
 * Nigerian Bank Verification Number.
 * Must be exactly 11 decimal digits.
 */
export const bvnSchema = z
  .string()
  .trim()
  .regex(/^\d{11}$/, "BVN must be exactly 11 digits")
  .describe("BVN — 11-digit Bank Verification Number issued by NIBSS");

/**
 * Nigerian National Identification Number.
 * Must be exactly 11 decimal digits.
 */
export const ninSchema = z
  .string()
  .trim()
  .regex(/^\d{11}$/, "NIN must be exactly 11 digits")
  .describe("NIN — 11-digit National Identification Number (Nigeria)");

/**
 * Kenyan KRA PIN.
 * Format: one uppercase/lowercase letter, 9 digits, one uppercase/lowercase letter
 * e.g. A123456789Z  (case-insensitive).
 */
export const kraPinSchema = z
  .string()
  .trim()
  .toUpperCase()
  .regex(/^[A-Z]\d{9}[A-Z]$/, "KRA PIN must follow the format A123456789Z")
  .describe("KRA PIN — Kenyan Revenue Authority Personal Identification Number (format: A123456789Z)");

/**
 * Ghanaian National ID card number.
 * Format: GHA-XXXXXXXXX-X where X is a digit.
 * e.g. GHA-123456789-1
 */
export const ghanaCardSchema = z
  .string()
  .trim()
  .regex(/^GHA-\d{9}-\d$/, "Ghana Card number must follow the format GHA-123456789-1")
  .describe("Ghana Card — National Identification Authority card number (format: GHA-XXXXXXXXX-X)");

/**
 * Phone number in E.164 format.
 * Starts with '+', followed by 7–15 digits.
 */
export const phoneSchema = z
  .string()
  .trim()
  .regex(/^\+\d{7,15}$/, "Phone number must be in E.164 format (e.g. +2348012345678)")
  .describe("Phone number in E.164 international format starting with '+'");

/**
 * Date of birth as an ISO 8601 date string (YYYY-MM-DD).
 * The applicant must be at least 18 years old as of today.
 */
export const dateOfBirthSchema = z
  .string()
  .trim()
  .regex(/^\d{4}-\d{2}-\d{2}$/, "Date of birth must be in YYYY-MM-DD format")
  .refine(
    (value) => {
      const dob = new Date(value);
      if (isNaN(dob.getTime())) return false;
      const today = new Date();
      // Calculate age by comparing full calendar years
      const age = today.getFullYear() - dob.getFullYear();
      const hasHadBirthdayThisYear =
        today.getMonth() > dob.getMonth() ||
        (today.getMonth() === dob.getMonth() && today.getDate() >= dob.getDate());
      return hasHadBirthdayThisYear ? age >= 18 : age - 1 >= 18;
    },
    { message: "Applicant must be at least 18 years old" },
  )
  .describe("Date of birth (ISO 8601 YYYY-MM-DD). Applicant must be 18 or older.");

/**
 * Full legal name.
 * Trimmed, minimum 2 characters, maximum 100 characters.
 */
export const fullNameSchema = z
  .string()
  .trim()
  .min(2, "Full name must be at least 2 characters")
  .max(100, "Full name must be at most 100 characters")
  .describe("Full legal name as it appears on a government-issued ID");

/**
 * Street / residential address.
 * Trimmed, minimum 5 characters, maximum 200 characters.
 */
export const addressSchema = z
  .string()
  .trim()
  .min(5, "Address must be at least 5 characters")
  .max(200, "Address must be at most 200 characters")
  .describe("Full residential or business address");

/**
 * Postal / ZIP code.
 * 4–6 alphanumeric characters (covers NG, KE, GH formats).
 */
export const postalCodeSchema = z
  .string()
  .trim()
  .regex(/^[A-Za-z0-9]{4,6}$/, "Postal code must be 4–6 alphanumeric characters")
  .describe("Postal or ZIP code (4–6 alphanumeric characters)");

// ---------------------------------------------------------------------------
// Factory schemas (parameterised)
// ---------------------------------------------------------------------------

/**
 * Validates that a file size (in bytes) does not exceed `maxMb` megabytes.
 *
 * @param maxMb - Upper bound in megabytes (e.g. 5 for 5 MB).
 */
export const fileSizeSchema = (maxMb: number): z.ZodNumber =>
  z
    .number()
    .int("File size must be a whole number of bytes")
    .nonnegative("File size must be non-negative")
    .max(maxMb * 1024 * 1024, `File must not exceed ${maxMb} MB`)
    .describe(`File size in bytes (maximum ${maxMb} MB)`);

/**
 * Validates that a MIME type string is within an allowed list.
 *
 * @param allowed - Array of accepted MIME type strings, e.g. ["image/jpeg", "image/png"].
 */
export const mimeTypeSchema = (allowed: string[]): z.ZodString =>
  z
    .string()
    .trim()
    .refine((value) => allowed.includes(value), {
      message: `MIME type must be one of: ${allowed.join(", ")}`,
    })
    .describe(`Allowed MIME types: ${allowed.join(", ")}`);

// ---------------------------------------------------------------------------
// Utility bill file-metadata sub-schema
// ---------------------------------------------------------------------------

/**
 * Inline file-metadata object for an uploaded utility bill.
 * Does not contain the raw binary — just enough info for the submission payload.
 */
const utilityBillMetaSchema = z
  .object({
    fileName: z.string().trim().min(1, "File name is required").describe("Original file name"),
    mimeType: mimeTypeSchema(["image/jpeg", "image/png", "application/pdf"]).describe(
      "MIME type of the utility bill document",
    ),
    sizeBytes: fileSizeSchema(5).describe("Size of the utility bill file in bytes"),
  })
  .describe("Utility bill document metadata");

// ---------------------------------------------------------------------------
// Nigeria schemas
// ---------------------------------------------------------------------------

/** Nigeria — Basic tier: personal details + BVN. */
export const nigeriaBasicSchema = z
  .object({
    fullName: fullNameSchema,
    phone: phoneSchema,
    dateOfBirth: dateOfBirthSchema,
    bvn: bvnSchema,
  })
  .describe("Nigeria Basic KYC — personal details and BVN");

/** Nigeria — Standard tier: Basic + NIN + address. */
export const nigeriaStandardSchema = nigeriaBasicSchema
  .extend({
    nin: ninSchema,
    address: addressSchema,
  })
  .describe("Nigeria Standard KYC — Basic fields plus NIN and address");

/** Nigeria — Premium tier: Standard + optional utility bill. */
export const nigeriaPremiumSchema = nigeriaStandardSchema
  .extend({
    utilityBill: utilityBillMetaSchema.optional().describe(
      "Optional utility bill document for address verification",
    ),
  })
  .describe("Nigeria Premium KYC — Standard fields plus optional utility bill upload");

// ---------------------------------------------------------------------------
// Kenya schemas
// ---------------------------------------------------------------------------

/** Kenya — Basic tier: personal details + KRA PIN. */
export const kenyaBasicSchema = z
  .object({
    fullName: fullNameSchema,
    phone: phoneSchema,
    dateOfBirth: dateOfBirthSchema,
    kraPin: kraPinSchema,
  })
  .describe("Kenya Basic KYC — personal details and KRA PIN");

/** Kenya — Standard tier: Basic + address. */
export const kenyaStandardSchema = kenyaBasicSchema
  .extend({
    address: addressSchema,
  })
  .describe("Kenya Standard KYC — Basic fields plus address");

// ---------------------------------------------------------------------------
// Ghana schemas
// ---------------------------------------------------------------------------

/** Ghana — Basic tier: personal details + Ghana Card number. */
export const ghanaBasicSchema = z
  .object({
    fullName: fullNameSchema,
    phone: phoneSchema,
    dateOfBirth: dateOfBirthSchema,
    ghanaCard: ghanaCardSchema,
  })
  .describe("Ghana Basic KYC — personal details and Ghana Card number");

/** Ghana — Standard tier: Basic + address. */
export const ghanaStandardSchema = ghanaBasicSchema
  .extend({
    address: addressSchema,
  })
  .describe("Ghana Standard KYC — Basic fields plus address");

// ---------------------------------------------------------------------------
// Lookup map & selector
// ---------------------------------------------------------------------------

/**
 * Nested lookup map: country → tier → Zod schema.
 * Using `z.ZodTypeAny` as the common type so the map compiles without
 * complex generic gymnastics while still returning the real type via the
 * overloaded function below.
 */
const SCHEMA_MAP: Record<KycCountry, Partial<Record<KycTier, z.ZodTypeAny>>> = {
  [KycCountry.Nigeria]: {
    [KycTier.Basic]: nigeriaBasicSchema,
    [KycTier.Standard]: nigeriaStandardSchema,
    [KycTier.Premium]: nigeriaPremiumSchema,
  },
  [KycCountry.Kenya]: {
    [KycTier.Basic]: kenyaBasicSchema,
    [KycTier.Standard]: kenyaStandardSchema,
    // Premium not yet defined for Kenya — falls through to Standard
    [KycTier.Premium]: kenyaStandardSchema,
  },
  [KycCountry.Ghana]: {
    [KycTier.Basic]: ghanaBasicSchema,
    [KycTier.Standard]: ghanaStandardSchema,
    // Premium not yet defined for Ghana — falls through to Standard
    [KycTier.Premium]: ghanaStandardSchema,
  },
};

/**
 * Returns the Zod validation schema for the given country + tier combination.
 *
 * Throws if an unsupported combination is requested so callers surface
 * configuration errors at dev time rather than silently skipping validation.
 */
export function getSchemaForCountryAndTier(
  country: KycCountry,
  tier: KycTier,
): z.ZodTypeAny {
  const schema = SCHEMA_MAP[country]?.[tier];
  if (!schema) {
    throw new Error(
      `No KYC schema defined for country="${country}" tier="${tier}". ` +
        `Add it to the SCHEMA_MAP in schemas.ts.`,
    );
  }
  return schema;
}

// ---------------------------------------------------------------------------
// Union type of all schema outputs
// ---------------------------------------------------------------------------

/**
 * Union of every concrete schema's inferred output type.
 * Use this as the type parameter for react-hook-form when the
 * country/tier is not statically known.
 */
export type KycFormValuesFromSchema =
  | z.infer<typeof nigeriaBasicSchema>
  | z.infer<typeof nigeriaStandardSchema>
  | z.infer<typeof nigeriaPremiumSchema>
  | z.infer<typeof kenyaBasicSchema>
  | z.infer<typeof kenyaStandardSchema>
  | z.infer<typeof ghanaBasicSchema>
  | z.infer<typeof ghanaStandardSchema>;

// ---------------------------------------------------------------------------
// Discriminated form error schema
// ---------------------------------------------------------------------------

/**
 * Discriminated union of structured form-level errors returned by the API
 * or produced by client-side validation.  The `code` field drives UI branching
 * (e.g. different copy / retry behaviour per error type).
 */
export const discriminatedFormErrorSchema = z
  .discriminatedUnion("code", [
    z
      .object({
        code: z.literal("VALIDATION_ERROR"),
        message: z.string().describe("Human-readable summary of what failed validation"),
        field: z.string().optional().describe("Field ID that caused the error, if field-level"),
      })
      .describe("One or more form fields failed validation"),

    z
      .object({
        code: z.literal("API_TIMEOUT"),
        message: z.string().describe("Timeout description"),
        retryAfterMs: z.number().optional().describe("Suggested retry delay in milliseconds"),
      })
      .describe("The submission request timed out — safe to retry"),

    z
      .object({
        code: z.literal("DOCUMENT_REJECTED"),
        message: z.string().describe("Reason the document was rejected"),
        documentType: z.string().optional().describe("The document type that was rejected"),
      })
      .describe("An uploaded document was rejected during automated checks"),

    z
      .object({
        code: z.literal("NAME_MISMATCH"),
        message: z.string().describe("Description of the name discrepancy"),
        providedName: z.string().optional().describe("Name as entered by the applicant"),
        idName: z.string().optional().describe("Name as it appears on the identity document"),
      })
      .describe("The name on the submitted document does not match the provided full name"),

    z
      .object({
        code: z.literal("DUPLICATE_IDENTITY"),
        message: z.string().describe("Details about the duplicate identity detection"),
        existingAccountId: z
          .string()
          .optional()
          .describe("Obfuscated ID of the existing account with the same identity"),
      })
      .describe(
        "The submitted identity credentials are already associated with another account",
      ),
  ])
  .describe(
    "Structured KYC form error returned by the API or produced by client-side validation",
  );

/** TypeScript type inferred from `discriminatedFormErrorSchema`. */
export type DiscriminatedFormError = z.infer<typeof discriminatedFormErrorSchema>;
