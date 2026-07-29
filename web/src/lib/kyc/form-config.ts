/**
 * KYC form configuration — JSON-driven step/field mappings per country/tier
 * Task #481 step 2
 *
 * This module exports a nested record mapping each (country, tier) pair to its
 * complete multi-step form configuration. Each step defines an ordered list of
 * fields, labels, types, masks, and client-side validation hints.
 */

import {
  KycCountry,
  KycTier,
  KycFieldType,
  type KycFormConfig,
  type KycStepConfig,
  type KycFieldConfig,
} from "./types";

// ---------------------------------------------------------------------------
// Reusable field builders
// ---------------------------------------------------------------------------

/** Personal information fields — used across all countries in step 1. */
const buildPersonalInfoFields = (phoneMask: string): KycFieldConfig[] => [
  {
    id: "fullName",
    label: "Full Legal Name",
    type: KycFieldType.text,
    placeholder: "Enter your full name as it appears on your ID",
    required: true,
    autocomplete: "name",
    validation: {
      min: 2,
      max: 100,
      message: "Full name must be between 2 and 100 characters",
    },
  },
  {
    id: "dateOfBirth",
    label: "Date of Birth",
    type: KycFieldType.date,
    placeholder: "YYYY-MM-DD",
    required: true,
    autocomplete: "bday",
    validation: {
      message: "You must be at least 18 years old",
    },
  },
  {
    id: "phone",
    label: "Phone Number",
    type: KycFieldType.tel,
    placeholder: phoneMask,
    required: true,
    mask: phoneMask,
    autocomplete: "tel",
    validation: {
      message: "Phone number must be in E.164 format",
    },
  },
];

/** Address fields — used in Standard+ tiers for Nigeria, Kenya, Ghana. */
const buildAddressFields = (): KycFieldConfig[] => [
  {
    id: "address",
    label: "Residential Address",
    type: KycFieldType.text,
    placeholder: "Enter your full street address",
    required: true,
    autocomplete: "street-address",
    validation: {
      min: 5,
      max: 200,
      message: "Address must be between 5 and 200 characters",
    },
  },
  {
    id: "postalCode",
    label: "Postal Code",
    type: KycFieldType.text,
    placeholder: "Enter your postal code",
    required: true,
    autocomplete: "postal-code",
    validation: {
      pattern: "^[A-Za-z0-9]{4,6}$",
      message: "Postal code must be 4–6 alphanumeric characters",
    },
  },
];

// ---------------------------------------------------------------------------
// Nigeria configurations
// ---------------------------------------------------------------------------

const nigeriaBasicSteps: KycStepConfig[] = [
  {
    id: "personal-info",
    title: "Personal Information",
    description:
      "Enter your basic personal details exactly as they appear on your government-issued ID.",
    fields: buildPersonalInfoFields("+234XXXXXXXXXX"),
  },
  {
    id: "identity-verification",
    title: "Identity Verification",
    description:
      "Provide your Bank Verification Number (BVN) to complete Basic tier verification.",
    fields: [
      {
        id: "bvn",
        label: "Bank Verification Number (BVN)",
        type: KycFieldType.text,
        placeholder: "Enter your 11-digit BVN",
        required: true,
        mask: "XXXXXXXXXXX",
        autocomplete: "off",
        validation: {
          pattern: "^\\d{11}$",
          message: "BVN must be exactly 11 digits",
        },
      },
    ],
  },
];

const nigeriaStandardSteps: KycStepConfig[] = [
  {
    id: "personal-info",
    title: "Personal Information",
    description:
      "Enter your basic personal details exactly as they appear on your government-issued ID.",
    fields: buildPersonalInfoFields("+234XXXXXXXXXX"),
  },
  {
    id: "identity-verification",
    title: "Identity Verification",
    description:
      "Provide your Bank Verification Number (BVN) and National Identification Number (NIN).",
    fields: [
      {
        id: "bvn",
        label: "Bank Verification Number (BVN)",
        type: KycFieldType.text,
        placeholder: "Enter your 11-digit BVN",
        required: true,
        mask: "XXXXXXXXXXX",
        autocomplete: "off",
        validation: {
          pattern: "^\\d{11}$",
          message: "BVN must be exactly 11 digits",
        },
      },
      {
        id: "nin",
        label: "National Identification Number (NIN)",
        type: KycFieldType.text,
        placeholder: "Enter your 11-digit NIN",
        required: true,
        mask: "XXXXXXXXXXX",
        autocomplete: "off",
        validation: {
          pattern: "^\\d{11}$",
          message: "NIN must be exactly 11 digits",
        },
      },
    ],
  },
  {
    id: "address",
    title: "Address Verification",
    description: "Provide your current residential address for Standard tier verification.",
    fields: buildAddressFields(),
  },
];

const nigeriaPremiumSteps: KycStepConfig[] = [
  {
    id: "personal-info",
    title: "Personal Information",
    description:
      "Enter your basic personal details exactly as they appear on your government-issued ID.",
    fields: buildPersonalInfoFields("+234XXXXXXXXXX"),
  },
  {
    id: "identity-verification",
    title: "Identity Verification",
    description:
      "Provide your Bank Verification Number (BVN) and National Identification Number (NIN).",
    fields: [
      {
        id: "bvn",
        label: "Bank Verification Number (BVN)",
        type: KycFieldType.text,
        placeholder: "Enter your 11-digit BVN",
        required: true,
        mask: "XXXXXXXXXXX",
        autocomplete: "off",
        validation: {
          pattern: "^\\d{11}$",
          message: "BVN must be exactly 11 digits",
        },
      },
      {
        id: "nin",
        label: "National Identification Number (NIN)",
        type: KycFieldType.text,
        placeholder: "Enter your 11-digit NIN",
        required: true,
        mask: "XXXXXXXXXXX",
        autocomplete: "off",
        validation: {
          pattern: "^\\d{11}$",
          message: "NIN must be exactly 11 digits",
        },
      },
    ],
  },
  {
    id: "address",
    title: "Address Verification",
    description: "Provide your current residential address.",
    fields: buildAddressFields(),
  },
  {
    id: "documents",
    title: "Document Upload",
    description:
      "Upload a recent utility bill (electricity, water, or internet) dated within the last 3 months for Premium tier verification.",
    fields: [
      {
        id: "utilityBill",
        label: "Utility Bill",
        type: KycFieldType.file,
        placeholder: "Upload a utility bill (PDF, JPEG, or PNG)",
        required: true,
        autocomplete: "off",
        validation: {
          max: 5,
          message: "File size must not exceed 5 MB",
        },
      },
    ],
  },
];

// ---------------------------------------------------------------------------
// Kenya configurations
// ---------------------------------------------------------------------------

const kenyaBasicSteps: KycStepConfig[] = [
  {
    id: "personal-info",
    title: "Personal Information",
    description:
      "Enter your basic personal details exactly as they appear on your government-issued ID.",
    fields: buildPersonalInfoFields("+254XXXXXXXXX"),
  },
  {
    id: "identity-verification",
    title: "Identity Verification",
    description:
      "Provide your Kenya Revenue Authority (KRA) Personal Identification Number (PIN).",
    fields: [
      {
        id: "kraPin",
        label: "KRA PIN",
        type: KycFieldType.text,
        placeholder: "A999999999Z",
        required: true,
        mask: "A999999999Z",
        autocomplete: "off",
        validation: {
          pattern: "^[A-Z]\\d{9}[A-Z]$",
          message: "KRA PIN must follow the format A123456789Z",
        },
      },
    ],
  },
];

const kenyaStandardSteps: KycStepConfig[] = [
  {
    id: "personal-info",
    title: "Personal Information",
    description:
      "Enter your basic personal details exactly as they appear on your government-issued ID.",
    fields: buildPersonalInfoFields("+254XXXXXXXXX"),
  },
  {
    id: "identity-verification",
    title: "Identity Verification",
    description:
      "Provide your Kenya Revenue Authority (KRA) Personal Identification Number (PIN).",
    fields: [
      {
        id: "kraPin",
        label: "KRA PIN",
        type: KycFieldType.text,
        placeholder: "A999999999Z",
        required: true,
        mask: "A999999999Z",
        autocomplete: "off",
        validation: {
          pattern: "^[A-Z]\\d{9}[A-Z]$",
          message: "KRA PIN must follow the format A123456789Z",
        },
      },
    ],
  },
  {
    id: "address",
    title: "Address Verification",
    description: "Provide your current residential address for Standard tier verification.",
    fields: buildAddressFields(),
  },
];

// ---------------------------------------------------------------------------
// Ghana configurations
// ---------------------------------------------------------------------------

const ghanaBasicSteps: KycStepConfig[] = [
  {
    id: "personal-info",
    title: "Personal Information",
    description:
      "Enter your basic personal details exactly as they appear on your government-issued ID.",
    fields: buildPersonalInfoFields("+233XXXXXXXXX"),
  },
  {
    id: "identity-verification",
    title: "Identity Verification",
    description:
      "Provide your Ghana Card number issued by the National Identification Authority.",
    fields: [
      {
        id: "ghanaCard",
        label: "Ghana Card Number",
        type: KycFieldType.text,
        placeholder: "GHA-XXXXXXXXX-X",
        required: true,
        mask: "GHA-XXXXXXXXX-X",
        autocomplete: "off",
        validation: {
          pattern: "^GHA-\\d{9}-\\d$",
          message: "Ghana Card number must follow the format GHA-123456789-1",
        },
      },
    ],
  },
];

const ghanaStandardSteps: KycStepConfig[] = [
  {
    id: "personal-info",
    title: "Personal Information",
    description:
      "Enter your basic personal details exactly as they appear on your government-issued ID.",
    fields: buildPersonalInfoFields("+233XXXXXXXXX"),
  },
  {
    id: "identity-verification",
    title: "Identity Verification",
    description:
      "Provide your Ghana Card number issued by the National Identification Authority.",
    fields: [
      {
        id: "ghanaCard",
        label: "Ghana Card Number",
        type: KycFieldType.text,
        placeholder: "GHA-XXXXXXXXX-X",
        required: true,
        mask: "GHA-XXXXXXXXX-X",
        autocomplete: "off",
        validation: {
          pattern: "^GHA-\\d{9}-\\d$",
          message: "Ghana Card number must follow the format GHA-123456789-1",
        },
      },
    ],
  },
  {
    id: "address",
    title: "Address Verification",
    description: "Provide your current residential address for Standard tier verification.",
    fields: buildAddressFields(),
  },
];

// ---------------------------------------------------------------------------
// Top-level nested configuration map
// ---------------------------------------------------------------------------

/**
 * Complete KYC form configuration lookup:
 *   KYC_FORM_CONFIGS[country][tier] → KycFormConfig
 *
 * Each country/tier combination maps to a fully-formed multi-step form with
 * field schemas, masks, validation rules, and labels ready to drive the
 * dynamic form renderer.
 */
export const KYC_FORM_CONFIGS: Record<
  KycCountry,
  Record<KycTier, KycFormConfig>
> = {
  [KycCountry.Nigeria]: {
    [KycTier.Basic]: {
      country: KycCountry.Nigeria,
      tier: KycTier.Basic,
      steps: nigeriaBasicSteps,
    },
    [KycTier.Standard]: {
      country: KycCountry.Nigeria,
      tier: KycTier.Standard,
      steps: nigeriaStandardSteps,
    },
    [KycTier.Premium]: {
      country: KycCountry.Nigeria,
      tier: KycTier.Premium,
      steps: nigeriaPremiumSteps,
    },
  },
  [KycCountry.Kenya]: {
    [KycTier.Basic]: {
      country: KycCountry.Kenya,
      tier: KycTier.Basic,
      steps: kenyaBasicSteps,
    },
    [KycTier.Standard]: {
      country: KycCountry.Kenya,
      tier: KycTier.Standard,
      steps: kenyaStandardSteps,
    },
    // Premium tier for Kenya not yet defined — reuse Standard config
    [KycTier.Premium]: {
      country: KycCountry.Kenya,
      tier: KycTier.Premium,
      steps: kenyaStandardSteps,
    },
  },
  [KycCountry.Ghana]: {
    [KycTier.Basic]: {
      country: KycCountry.Ghana,
      tier: KycTier.Basic,
      steps: ghanaBasicSteps,
    },
    [KycTier.Standard]: {
      country: KycCountry.Ghana,
      tier: KycTier.Standard,
      steps: ghanaStandardSteps,
    },
    // Premium tier for Ghana not yet defined — reuse Standard config
    [KycTier.Premium]: {
      country: KycCountry.Ghana,
      tier: KycTier.Premium,
      steps: ghanaStandardSteps,
    },
  },
};

/**
 * Helper to retrieve the form configuration for a given country and tier.
 * Throws if the combination is not defined (better to fail loudly at dev time).
 *
 * @param country - The KYC country
 * @param tier - The KYC verification tier
 * @returns The complete form configuration object
 */
export function getFormConfig(
  country: KycCountry,
  tier: KycTier,
): KycFormConfig {
  const config = KYC_FORM_CONFIGS[country]?.[tier];
  if (!config) {
    throw new Error(
      `No KYC form configuration defined for country="${country}" tier="${tier}". ` +
        `Check KYC_FORM_CONFIGS in form-config.ts.`,
    );
  }
  return config;
}
