//! Maps raw payment-provider error codes/messages to user-friendly, localized
//! messages surfaced via [`PaymentError::user_message`] /
//! [`PaymentError::user_message_localized`].
//!
//! Providers return opaque, provider-specific codes and messages (e.g.
//! Flutterwave's `"do_not_honor"`, M-Pesa's numeric `ResultCode`s). Surfacing
//! those directly to end users is confusing and leaks internal detail. This
//! module centralizes the translation into actionable, localized messages
//! while always logging the raw code/message internally for debugging — the
//! raw values never reach the client.

use super::error::PaymentError;
use tracing::warn;

/// Supported client-facing locales.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    En,
    Fr,
}

impl Locale {
    /// Parses an `Accept-Language`-style tag (e.g. `"fr-CI"`, `"en-US"`) into
    /// a supported locale, defaulting to English.
    pub fn from_lang_tag(tag: &str) -> Self {
        if tag.trim().to_lowercase().starts_with("fr") {
            Locale::Fr
        } else {
            Locale::En
        }
    }
}

#[derive(Clone, Copy)]
enum Kind {
    InsufficientFunds,
    Declined,
    Validation,
    RateLimit,
    Provider,
}

struct CodeEntry {
    /// Matched case-insensitively against the provider's error code first,
    /// falling back to a substring match against the raw message when no
    /// code is available.
    matches: &'static [&'static str],
    kind: Kind,
    en: &'static str,
    fr: &'static str,
}

const FLUTTERWAVE: &[CodeEntry] = &[
    CodeEntry {
        matches: &["insufficient", "low balance"],
        kind: Kind::InsufficientFunds,
        en: "Your account has insufficient funds for this payment.",
        fr: "Votre compte ne dispose pas de fonds suffisants pour ce paiement.",
    },
    CodeEntry {
        matches: &["do_not_honor", "do not honor", "declined"],
        kind: Kind::Declined,
        en: "Your bank declined this card. Please try a different payment method.",
        fr: "Votre banque a refusé cette carte. Veuillez essayer un autre moyen de paiement.",
    },
    CodeEntry {
        matches: &["expired_card", "expired card"],
        kind: Kind::Declined,
        en: "This card has expired. Please use a different card.",
        fr: "Cette carte a expiré. Veuillez utiliser une autre carte.",
    },
    CodeEntry {
        matches: &["restricted_card", "pickup_card", "stolen_card"],
        kind: Kind::Declined,
        en: "This card cannot be used for this payment. Please try a different card.",
        fr: "Cette carte ne peut pas être utilisée pour ce paiement. Veuillez essayer une autre carte.",
    },
    CodeEntry {
        matches: &["invalid", "not found", "missing", "unsupported"],
        kind: Kind::Validation,
        en: "Some of the payment details provided are invalid.",
        fr: "Certaines informations de paiement fournies sont invalides.",
    },
    CodeEntry {
        matches: &["too many requests", "rate limit"],
        kind: Kind::RateLimit,
        en: "Too many payment attempts. Please wait a moment and try again.",
        fr: "Trop de tentatives de paiement. Veuillez patienter puis réessayer.",
    },
];

const PAYSTACK: &[CodeEntry] = &[
    CodeEntry {
        matches: &["insufficient"],
        kind: Kind::InsufficientFunds,
        en: "Your account has insufficient funds for this payment.",
        fr: "Votre compte ne dispose pas de fonds suffisants pour ce paiement.",
    },
    CodeEntry {
        matches: &["declined", "do not honor"],
        kind: Kind::Declined,
        en: "Your bank declined this card. Please try a different payment method.",
        fr: "Votre banque a refusé cette carte. Veuillez essayer un autre moyen de paiement.",
    },
    CodeEntry {
        matches: &["invalid", "not found", "missing"],
        kind: Kind::Validation,
        en: "Some of the payment details provided are invalid.",
        fr: "Certaines informations de paiement fournies sont invalides.",
    },
    CodeEntry {
        matches: &["timeout"],
        kind: Kind::Provider,
        en: "The payment provider timed out. Please try again shortly.",
        fr: "Le fournisseur de paiement n'a pas répondu à temps. Veuillez réessayer sous peu.",
    },
];

// Safaricom Daraja API ResultCode values — see M-Pesa API docs.
const MPESA: &[CodeEntry] = &[
    CodeEntry {
        matches: &["1"],
        kind: Kind::InsufficientFunds,
        en: "Insufficient balance in your M-Pesa account.",
        fr: "Solde insuffisant sur votre compte M-Pesa.",
    },
    CodeEntry {
        matches: &["1032"],
        kind: Kind::Declined,
        en: "The payment request was cancelled.",
        fr: "La demande de paiement a été annulée.",
    },
    CodeEntry {
        matches: &["1037"],
        kind: Kind::Provider,
        en: "We couldn't reach your phone. Please check it is on and try again.",
        fr: "Impossible de joindre votre téléphone. Veuillez vérifier qu'il est allumé et réessayer.",
    },
    CodeEntry {
        matches: &["2001"],
        kind: Kind::Validation,
        en: "The PIN entered was incorrect.",
        fr: "Le code PIN saisi est incorrect.",
    },
];

const GHANA: &[CodeEntry] = &[
    CodeEntry {
        matches: &["insufficient"],
        kind: Kind::InsufficientFunds,
        en: "Your mobile money account has insufficient funds.",
        fr: "Votre compte mobile money ne dispose pas de fonds suffisants.",
    },
    CodeEntry {
        matches: &["declined", "failed"],
        kind: Kind::Declined,
        en: "The payment was declined. Please try a different payment method.",
        fr: "Le paiement a été refusé. Veuillez essayer un autre moyen de paiement.",
    },
    CodeEntry {
        matches: &["invalid", "not found"],
        kind: Kind::Validation,
        en: "Some of the payment details provided are invalid.",
        fr: "Certaines informations de paiement fournies sont invalides.",
    },
];

pub struct PaymentProviderErrorMapping;

impl PaymentProviderErrorMapping {
    /// Classifies a provider's raw error (code and/or message) into the
    /// matching [`PaymentError`] variant, preserving the raw message for
    /// internal logging/Display and attaching `provider_code` where the
    /// variant supports it. Always logs the raw code/message internally —
    /// callers should surface [`PaymentError::user_message`] /
    /// [`PaymentError::user_message_localized`] to the client, never the raw
    /// message.
    pub fn classify(provider: &str, code: Option<&str>, message: &str) -> PaymentError {
        warn!(
            provider,
            provider_error_code = code.unwrap_or("none"),
            provider_message = message,
            "payment provider returned an error"
        );

        let haystack = code.unwrap_or(message).to_lowercase();
        let entry = Self::table(provider)
            .iter()
            .find(|e| e.matches.iter().any(|needle| haystack.contains(needle)));

        match entry {
            Some(entry) => Self::build(entry.kind, message.to_string(), provider, code),
            None => PaymentError::ProviderError {
                provider: provider.to_string(),
                message: message.to_string(),
                provider_code: code.map(str::to_string),
                retryable: false,
            },
        }
    }

    /// Looks up the localized, user-friendly message for a known
    /// `provider_code`, searching across all providers' tables. Returns
    /// `None` when the code isn't recognized, in which case callers should
    /// fall back to a generic message.
    pub fn friendly_text_for_code(code: &str, locale: Locale) -> Option<String> {
        let needle = code.to_lowercase();
        [FLUTTERWAVE, PAYSTACK, MPESA, GHANA]
            .iter()
            .flat_map(|table| table.iter())
            .find(|e| e.matches.iter().any(|m| needle.contains(m)))
            .map(|entry| {
                match locale {
                    Locale::En => entry.en,
                    Locale::Fr => entry.fr,
                }
                .to_string()
            })
    }

    fn table(provider: &str) -> &'static [CodeEntry] {
        match provider.to_lowercase().as_str() {
            "flutterwave" => FLUTTERWAVE,
            "paystack" => PAYSTACK,
            "mpesa" | "mpesa_kenya" => MPESA,
            "ghana" | "ghana_mobile_money" => GHANA,
            _ => &[],
        }
    }

    fn build(kind: Kind, message: String, provider: &str, code: Option<&str>) -> PaymentError {
        match kind {
            Kind::InsufficientFunds => PaymentError::InsufficientFundsError { message },
            Kind::Declined => PaymentError::PaymentDeclinedError {
                message,
                provider_code: code.map(str::to_string),
            },
            Kind::Validation => PaymentError::ValidationError {
                message,
                field: None,
            },
            Kind::RateLimit => PaymentError::RateLimitError {
                message,
                retry_after_seconds: None,
            },
            Kind::Provider => PaymentError::ProviderError {
                provider: provider.to_string(),
                message,
                provider_code: code.map(str::to_string),
                retryable: false,
            },
        }
    }
}
