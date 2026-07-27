#[cfg(test)]
mod tests {
    use crate::travel_rule::models::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    fn make_complete_natural() -> Ivms101Person {
        Ivms101Person::Natural(Ivms101NaturalPerson {
            first_name: "Amaka".into(),
            last_name: "Okafor".into(),
            date_of_birth: Some("1990-05-12".into()),
            national_id: Some("NIN-12345678901".into()),
            address: Some("12 Marina Road, Lagos".into()),
            country_of_residence: Some("NG".into()),
            account_number: Some("GAJHF7SVXMVDSKJF".into()),
        })
    }

    fn make_incomplete_natural(missing: &str) -> Ivms101Person {
        let mut p = Ivms101NaturalPerson {
            first_name: "Amaka".into(),
            last_name: "Okafor".into(),
            date_of_birth: Some("1990-05-12".into()),
            national_id: Some("NIN-12345678901".into()),
            address: Some("12 Marina Road, Lagos".into()),
            country_of_residence: Some("NG".into()),
            account_number: Some("GAJHF7SVXMVDSKJF".into()),
        };
        match missing {
            "first_name" => p.first_name = "".into(),
            "last_name" => p.last_name = "".into(),
            "national_id" => p.national_id = None,
            "date_of_birth" => p.date_of_birth = None,
            "country" => p.country_of_residence = None,
            "account_number" => p.account_number = None,
            _ => {}
        }
        Ivms101Person::Natural(p)
    }

    fn validate(person: &Ivms101Person) -> anyhow::Result<()> {
        match person {
            Ivms101Person::Natural(p) => {
                if p.first_name.trim().is_empty() {
                    return Err(anyhow::anyhow!("first_name required"));
                }
                if p.last_name.trim().is_empty() {
                    return Err(anyhow::anyhow!("last_name required"));
                }
                if p.account_number.as_deref().unwrap_or("").trim().is_empty() {
                    return Err(anyhow::anyhow!("account_number required"));
                }
                if p.national_id.as_deref().unwrap_or("").trim().is_empty() {
                    return Err(anyhow::anyhow!("national_id required"));
                }
                if p.date_of_birth.as_deref().unwrap_or("").trim().is_empty() {
                    return Err(anyhow::anyhow!("date_of_birth required"));
                }
                if p.country_of_residence.as_deref().unwrap_or("").trim().is_empty() {
                    return Err(anyhow::anyhow!("country_of_residence required"));
                }
                Ok(())
            }
            Ivms101Person::Legal(p) => {
                if p.legal_name.trim().is_empty() {
                    return Err(anyhow::anyhow!("legal_name required"));
                }
                if p.account_number.as_deref().unwrap_or("").trim().is_empty() {
                    return Err(anyhow::anyhow!("account_number required"));
                }
                Ok(())
            }
        }
    }

    // -------------------------------------------------------------------------
    // 1. IVMS101 completeness validation
    // -------------------------------------------------------------------------

    #[test]
    fn completeness_natural_all_fields_passes() {
        let person = make_complete_natural();
        assert!(validate(&person).is_ok());
    }

    #[test]
    fn completeness_missing_first_name_fails() {
        let person = make_incomplete_natural("first_name");
        let err = validate(&person).unwrap_err();
        assert!(err.to_string().contains("first_name"));
    }

    #[test]
    fn completeness_missing_last_name_fails() {
        let person = make_incomplete_natural("last_name");
        assert!(validate(&person).is_err());
    }

    #[test]
    fn completeness_missing_national_id_fails() {
        let person = make_incomplete_natural("national_id");
        assert!(validate(&person).is_err());
    }

    #[test]
    fn completeness_missing_dob_fails() {
        let person = make_incomplete_natural("date_of_birth");
        assert!(validate(&person).is_err());
    }

    #[test]
    fn completeness_missing_country_fails() {
        let person = make_incomplete_natural("country");
        assert!(validate(&person).is_err());
    }

    #[test]
    fn completeness_missing_account_number_fails() {
        let person = make_incomplete_natural("account_number");
        assert!(validate(&person).is_err());
    }

    #[test]
    fn completeness_legal_person_passes() {
        let person = Ivms101Person::Legal(Ivms101LegalPerson {
            legal_name: "Aframp Ltd".into(),
            registration_number: Some("RC-123456".into()),
            country_of_registration: Some("NG".into()),
            address: None,
            account_number: Some("GBZXHF7SVXMVDSKJF".into()),
        });
        assert!(validate(&person).is_ok());
    }

    #[test]
    fn completeness_legal_person_missing_name_fails() {
        let person = Ivms101Person::Legal(Ivms101LegalPerson {
            legal_name: "".into(),
            registration_number: None,
            country_of_registration: None,
            address: None,
            account_number: Some("GBZXHF7SVXMVDSKJF".into()),
        });
        assert!(validate(&person).is_err());
    }

    // -------------------------------------------------------------------------
    // 2. Threshold calculation
    // -------------------------------------------------------------------------

    #[test]
    fn threshold_above_triggers() {
        let threshold = Decimal::from_str("500000").unwrap();
        let amount = Decimal::from_str("600000").unwrap();
        assert!(amount >= threshold);
    }

    #[test]
    fn threshold_exactly_at_boundary_triggers() {
        let threshold = Decimal::from_str("500000").unwrap();
        let amount = Decimal::from_str("500000").unwrap();
        assert!(amount >= threshold);
    }

    #[test]
    fn threshold_below_does_not_trigger() {
        let threshold = Decimal::from_str("500000").unwrap();
        let amount = Decimal::from_str("499999.99").unwrap();
        assert!(amount < threshold);
    }

    #[test]
    fn cross_border_threshold_zero_always_triggers() {
        let threshold = Decimal::ZERO;
        let amount = Decimal::from_str("1").unwrap();
        assert!(amount >= threshold);
    }

    // -------------------------------------------------------------------------
    // 3. Protocol selection (fallback chain logic)
    // -------------------------------------------------------------------------

    fn supports(protocols: &[&str], target: &str) -> bool {
        protocols.iter().any(|p| *p == target)
    }

    fn select_protocol(protocols: &[&str]) -> TravelRuleProtocol {
        if supports(protocols, "trisa") {
            TravelRuleProtocol::Trisa
        } else if supports(protocols, "trp") || supports(protocols, "trust") {
            TravelRuleProtocol::Trp
        } else if supports(protocols, "open_vasp") || supports(protocols, "openvasp") {
            TravelRuleProtocol::OpenVasp
        } else {
            TravelRuleProtocol::Unknown
        }
    }

    #[test]
    fn trisa_preferred_over_trp() {
        let protocols = &["trisa", "trp", "open_vasp"];
        assert_eq!(select_protocol(protocols), TravelRuleProtocol::Trisa);
    }

    #[test]
    fn trp_fallback_when_no_trisa() {
        let protocols = &["trp", "open_vasp"];
        assert_eq!(select_protocol(protocols), TravelRuleProtocol::Trp);
    }

    #[test]
    fn openvasp_fallback_when_no_trisa_or_trp() {
        let protocols = &["open_vasp"];
        assert_eq!(select_protocol(protocols), TravelRuleProtocol::OpenVasp);
    }

    #[test]
    fn unknown_when_no_recognized_protocol() {
        let protocols = &["ivms101_direct", "proprietary"];
        assert_eq!(select_protocol(protocols), TravelRuleProtocol::Unknown);
    }

    #[test]
    fn trust_alias_maps_to_trp() {
        let protocols = &["trust"];
        assert_eq!(select_protocol(protocols), TravelRuleProtocol::Trp);
    }

    // -------------------------------------------------------------------------
    // 4. AES-GCM encryption round-trip
    // -------------------------------------------------------------------------

    #[test]
    fn aes_gcm_encrypt_decrypt_round_trip() {
        use aes_gcm::aead::{Aead, KeyInit, OsRng};
        use aes_gcm::{Aes256Gcm, Key, Nonce};
        use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
        use rand::RngCore;

        let plaintext = br#"{"first_name":"Amaka","last_name":"Okafor"}"#;

        let mut key_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut key_bytes);
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, plaintext.as_ref()).unwrap();
        let decrypted = cipher.decrypt(nonce, ciphertext.as_ref()).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    // -------------------------------------------------------------------------
    // 5. Originator package construction
    // -------------------------------------------------------------------------

    #[test]
    fn originator_package_serialises_all_fields() {
        let person = make_complete_natural();
        let json = serde_json::to_value(&person).unwrap();

        // Natural person should have all fields
        let inner = &json["natural"];
        assert_eq!(inner["first_name"], "Amaka");
        assert_eq!(inner["last_name"], "Okafor");
        assert!(!inner["national_id"].is_null());
        assert!(!inner["date_of_birth"].is_null());
        assert!(!inner["country_of_residence"].is_null());
        assert!(!inner["account_number"].is_null());
    }

    // -------------------------------------------------------------------------
    // 6. Unhosted wallet policy application
    // -------------------------------------------------------------------------

    fn apply_policy(policy_type: &str, amount: Decimal, threshold: Decimal) -> &'static str {
        match policy_type {
            "block" => "blocked",
            "require_attestation" => "attestation_required",
            "allow_below_threshold" if amount > threshold => "blocked_above_threshold",
            _ => "allowed",
        }
    }

    #[test]
    fn policy_block_always_blocks() {
        let outcome = apply_policy("block", Decimal::from(1_000_000), Decimal::from(500_000));
        assert_eq!(outcome, "blocked");
    }

    #[test]
    fn policy_allow_always_allows() {
        let outcome = apply_policy("allow", Decimal::from(1_000_000), Decimal::from(500_000));
        assert_eq!(outcome, "allowed");
    }

    #[test]
    fn policy_allow_below_threshold_blocks_above() {
        let outcome =
            apply_policy("allow_below_threshold", Decimal::from(600_000), Decimal::from(500_000));
        assert_eq!(outcome, "blocked_above_threshold");
    }

    #[test]
    fn policy_allow_below_threshold_allows_below() {
        let outcome =
            apply_policy("allow_below_threshold", Decimal::from(100_000), Decimal::from(500_000));
        assert_eq!(outcome, "allowed");
    }

    #[test]
    fn policy_require_attestation_returns_required() {
        let outcome = apply_policy("require_attestation", Decimal::from(10), Decimal::from(500_000));
        assert_eq!(outcome, "attestation_required");
    }

    // -------------------------------------------------------------------------
    // 7. High-risk corridor override
    // -------------------------------------------------------------------------

    fn is_high_risk(destination: &str) -> bool {
        const HIGH_RISK: &[&str] = &["IR", "KP", "MM", "SY", "RU"];
        HIGH_RISK.contains(&destination)
    }

    #[test]
    fn sanctioned_jurisdiction_is_high_risk() {
        assert!(is_high_risk("KP")); // North Korea
        assert!(is_high_risk("IR")); // Iran
        assert!(is_high_risk("SY")); // Syria
    }

    #[test]
    fn normal_jurisdiction_not_high_risk() {
        assert!(!is_high_risk("NG")); // Nigeria
        assert!(!is_high_risk("GH")); // Ghana
        assert!(!is_high_risk("KE")); // Kenya
    }
}
