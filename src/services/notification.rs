use crate::database::transaction_repository::Transaction;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationType {
    OfframpCompleted,
    OfframpFailed,
    OfframpRefunded,
    CngnReceived,
}

pub struct NotificationService;

impl NotificationService {
    pub fn new() -> Self {
        Self
    }

    pub async fn send_notification(
        &self,
        tx: &Transaction,
        notification_type: NotificationType,
        message: &str,
    ) {
        // Placeholder for real notification logic (email, SMS, push, webhook)
        // For now, we just log it with a structured format.
        match notification_type {
            NotificationType::OfframpCompleted => {
                info!(
                    transaction_id = %tx.transaction_id,
                    wallet = %tx.wallet_address,
                    amount = %tx.to_amount,
                    currency = %tx.to_currency,
                    "🔔 NOTIFICATION: Offramp Completed - {}", message
                );
            }
            NotificationType::OfframpFailed => {
                error!(
                    transaction_id = %tx.transaction_id,
                    wallet = %tx.wallet_address,
                    "🔔 NOTIFICATION: Offramp Failed - {}", message
                );
            }
            NotificationType::OfframpRefunded => {
                info!(
                    transaction_id = %tx.transaction_id,
                    wallet = %tx.wallet_address,
                    "🔔 NOTIFICATION: Offramp Refunded - {}", message
                );
            }
            NotificationType::CngnReceived => {
                info!(
                    transaction_id = %tx.transaction_id,
                    wallet = %tx.wallet_address,
                    amount = %tx.cngn_amount,
                    "🔔 NOTIFICATION: cNGN Received - {}", message
                );
            }
        }
    }
}
