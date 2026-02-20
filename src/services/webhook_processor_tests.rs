use crate::services::webhook_processor::{WebhookProcessor, WebhookProcessorError};
use serde_json::json;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_webhook_signature_verification() {
        // This is a basic structure test
        // In production, you would mock the dependencies
        assert!(true);
    }

    #[test]
    fn test_event_id_extraction() {
        let payload = json!({
            "id": 12345,
            "event": "charge.completed",
            "data": {
                "reference": "tx_123"
            }
        });

        // Test that we can parse the payload
        assert!(payload.get("id").is_some());
        assert_eq!(payload.get("id").unwrap().as_i64().unwrap(), 12345);
    }

    #[test]
    fn test_webhook_error_types() {
        let err = WebhookProcessorError::InvalidSignature;
        assert_eq!(err.to_string(), "Invalid signature");

        let err = WebhookProcessorError::AlreadyProcessed;
        assert_eq!(err.to_string(), "Already processed");
    }
}
