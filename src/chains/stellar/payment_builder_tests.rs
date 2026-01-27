#[cfg(test)]
mod payment_builder_tests {
    use crate::chains::stellar::{
        client::StellarClient,
        config::{StellarConfig, StellarNetwork},
        errors::StellarError,
        payment_builder::{MemoType, PaymentBuilder, PaymentOperation},
    };
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn create_test_client() -> StellarClient {
        let config = StellarConfig {
            network: StellarNetwork::Testnet,
            request_timeout: std::time::Duration::from_secs(10),
            max_retries: 3,
            health_check_interval: std::time::Duration::from_secs(30),
        };
        StellarClient::new(config).expect("Failed to create test client")
    }

    #[test]
    fn test_payment_builder_creation() {
        let client = create_test_client();
        let config = StellarConfig::default();
        let builder = PaymentBuilder::new(client, config);

        assert_eq!(builder.operations.len(), 0);
        assert!(builder.memo.is_none());
        assert!(builder.source_account.is_none());
    }

    #[test]
    fn test_with_source_account() {
        let client = create_test_client();
        let config = StellarConfig::default();
        let source = "GCCVPYFOHY7ZB7557JKENAX62LUAPLMGIWNZJAFV2MITK6T32V37KEJU";

        let builder = PaymentBuilder::new(client, config).with_source_account(source);

        assert_eq!(builder.source_account, Some(source.to_string()));
    }

    #[tokio::test]
    async fn test_invalid_destination_address() {
        let client = create_test_client();
        let config = StellarConfig::default();
        let mut builder = PaymentBuilder::new(client, config);

        let result = builder
            .add_payment_op(
                "invalid_address",
                Decimal::from_str("100.50").unwrap(),
                "AFRI",
                "GCCVPYFOHY7ZB7557JKENAX62LUAPLMGIWNZJAFV2MITK6T32V37KEJU",
            )
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StellarError::InvalidAddress { .. }
        ));
    }

    #[tokio::test]
    async fn test_invalid_issuer_address() {
        let client = create_test_client();
        let config = StellarConfig::default();
        let mut builder = PaymentBuilder::new(client, config);

        let result = builder
            .add_payment_op(
                "GCCVPYFOHY7ZB7557JKENAX62LUAPLMGIWNZJAFV2MITK6T32V37KEJU",
                Decimal::from_str("100.50").unwrap(),
                "AFRI",
                "invalid_issuer",
            )
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StellarError::InvalidAddress { .. }
        ));
    }

    #[tokio::test]
    async fn test_zero_amount() {
        let client = create_test_client();
        let config = StellarConfig::default();
        let mut builder = PaymentBuilder::new(client, config);

        let result = builder
            .add_payment_op(
                "GCCVPYFOHY7ZB7557JKENAX62LUAPLMGIWNZJAFV2MITK6T32V37KEJU",
                Decimal::ZERO,
                "AFRI",
                "GCCVPYFOHY7ZB7557JKENAX62LUAPLMGIWNZJAFV2MITK6T32V37KEJU",
            )
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StellarError::InvalidAmount { .. }
        ));
    }

    #[tokio::test]
    async fn test_negative_amount() {
        let client = create_test_client();
        let config = StellarConfig::default();
        let mut builder = PaymentBuilder::new(client, config);

        let result = builder
            .add_payment_op(
                "GCCVPYFOHY7ZB7557JKENAX62LUAPLMGIWNZJAFV2MITK6T32V37KEJU",
                Decimal::from_str("-10.50").unwrap(),
                "AFRI",
                "GCCVPYFOHY7ZB7557JKENAX62LUAPLMGIWNZJAFV2MITK6T32V37KEJU",
            )
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StellarError::InvalidAmount { .. }
        ));
    }

    #[tokio::test]
    async fn test_invalid_asset_code_empty() {
        let client = create_test_client();
        let config = StellarConfig::default();
        let mut builder = PaymentBuilder::new(client, config);

        let result = builder
            .add_payment_op(
                "GCCVPYFOHY7ZB7557JKENAX62LUAPLMGIWNZJAFV2MITK6T32V37KEJU",
                Decimal::from_str("100.50").unwrap(),
                "",
                "GCCVPYFOHY7ZB7557JKENAX62LUAPLMGIWNZJAFV2MITK6T32V37KEJU",
            )
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StellarError::InvalidAmount { .. }
        ));
    }

    #[tokio::test]
    async fn test_invalid_asset_code_too_long() {
        let client = create_test_client();
        let config = StellarConfig::default();
        let mut builder = PaymentBuilder::new(client, config);

        let result = builder
            .add_payment_op(
                "GCCVPYFOHY7ZB7557JKENAX62LUAPLMGIWNZJAFV2MITK6T32V37KEJU",
                Decimal::from_str("100.50").unwrap(),
                "TOOLONGASSETCODE",
                "GCCVPYFOHY7ZB7557JKENAX62LUAPLMGIWNZJAFV2MITK6T32V37KEJU",
            )
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StellarError::InvalidAmount { .. }
        ));
    }

    #[test]
    fn test_add_text_memo_valid() {
        let client = create_test_client();
        let config = StellarConfig::default();
        let mut builder = PaymentBuilder::new(client, config);

        let result = builder.add_memo(MemoType::Text, "test memo");
        assert!(result.is_ok());
        assert!(builder.memo.is_some());
    }

    #[test]
    fn test_add_text_memo_too_long() {
        let client = create_test_client();
        let config = StellarConfig::default();
        let mut builder = PaymentBuilder::new(client, config);

        let result = builder.add_memo(
            MemoType::Text,
            "this is a very long memo that exceeds the 28 byte limit",
        );
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StellarError::InvalidMemo { .. }
        ));
    }

    #[test]
    fn test_add_id_memo_valid() {
        let client = create_test_client();
        let config = StellarConfig::default();
        let mut builder = PaymentBuilder::new(client, config);

        let result = builder.add_memo(MemoType::Id, "123456789");
        assert!(result.is_ok());
        assert!(builder.memo.is_some());
    }

    #[test]
    fn test_add_id_memo_invalid() {
        let client = create_test_client();
        let config = StellarConfig::default();
        let mut builder = PaymentBuilder::new(client, config);

        let result = builder.add_memo(MemoType::Id, "not_a_number");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StellarError::InvalidMemo { .. }
        ));
    }

    #[test]
    fn test_add_hash_memo_invalid_length() {
        let client = create_test_client();
        let config = StellarConfig::default();
        let mut builder = PaymentBuilder::new(client, config);

        let result = builder.add_memo(MemoType::Hash, "tooshort");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StellarError::InvalidMemo { .. }
        ));
    }

    #[tokio::test]
    async fn test_estimate_fee_no_operations() {
        let client = create_test_client();
        let config = StellarConfig::default();
        let builder = PaymentBuilder::new(client, config);

        let result = builder.estimate_fee().await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StellarError::BuildFailed { .. }
        ));
    }

    #[tokio::test]
    async fn test_sign_tx_no_operations() {
        let client = create_test_client();
        let config = StellarConfig::default();
        let builder = PaymentBuilder::new(client, config);

        let result = builder
            .sign_tx("SCCVPYFOHY7ZB7557JKENAX62LUAPLMGIWNZJAFV2MITK6T32V37KEJU")
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StellarError::BuildFailed { .. }
        ));
    }

    #[tokio::test]
    async fn test_sign_tx_invalid_secret_key() {
        let client = create_test_client();
        let config = StellarConfig::default();
        let mut builder = PaymentBuilder::new(client, config);
        builder.source_account =
            Some("GCCVPYFOHY7ZB7557JKENAX62LUAPLMGIWNZJAFV2MITK6T32V37KEJU".to_string());

        let operation = PaymentOperation {
            destination: "GCCVPYFOHY7ZB7557JKENAX62LUAPLMGIWNZJAFV2MITK6T32V37KEJU".to_string(),
            amount: Decimal::from_str("100").unwrap(),
            asset_code: "AFRI".to_string(),
            asset_issuer: "GCCVPYFOHY7ZB7557JKENAX62LUAPLMGIWNZJAFV2MITK6T32V37KEJU".to_string(),
            source_account: None,
        };
        builder.operations.push(operation);

        let result = builder.sign_tx("invalid_key").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StellarError::SigningFailed { .. }
        ));
    }

    #[tokio::test]
    async fn test_sign_tx_no_source_account() {
        let client = create_test_client();
        let config = StellarConfig::default();
        let builder = PaymentBuilder::new(client, config);

        let result = builder
            .sign_tx("SCCVPYFOHY7ZB7557JKENAX62LUAPLMGIWNZJAFV2MITK6T32V37KEJU")
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StellarError::BuildFailed { .. }
        ));
    }
}
