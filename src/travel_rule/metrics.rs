use lazy_static::lazy_static;
use prometheus::{register_counter_vec, register_counter, register_gauge, CounterVec, Counter, Gauge};

lazy_static! {
    pub static ref OUTBOUND_MESSAGES_TOTAL: CounterVec = register_counter_vec!(
        "aframp_travel_rule_outbound_messages_total",
        "Total outbound Travel Rule messages by status/protocol",
        &["status"]
    ).unwrap();

    pub static ref INBOUND_MESSAGES_TOTAL: CounterVec = register_counter_vec!(
        "aframp_travel_rule_inbound_messages_total",
        "Total inbound Travel Rule messages by protocol",
        &["protocol"]
    ).unwrap();

    pub static ref TRANSMISSION_FAILURES_TOTAL: CounterVec = register_counter_vec!(
        "aframp_travel_rule_transmission_failures_total",
        "Travel Rule outbound transmission failures by reason",
        &["reason"]
    ).unwrap();

    pub static ref SCREENING_FAILURES_TOTAL: Counter = register_counter!(
        "aframp_travel_rule_originator_screening_failures_total",
        "Inbound originator IVMS101 sanctions screening failures"
    ).unwrap();

    pub static ref UNHOSTED_WALLET_TRANSACTIONS_TOTAL: CounterVec = register_counter_vec!(
        "aframp_travel_rule_unhosted_wallet_transactions_total",
        "Transactions to unhosted wallets by policy outcome",
        &["policy_outcome"]
    ).unwrap();

    pub static ref SUNRISE_RULE_APPLIED_TOTAL: Counter = register_counter!(
        "aframp_travel_rule_sunrise_rule_applied_total",
        "Times the Travel Rule sunrise rule was applied (VASP known but no supported protocol)"
    ).unwrap();

    pub static ref UNACKNOWLEDGED_OUTBOUND_COUNT: Gauge = register_gauge!(
        "aframp_travel_rule_unacknowledged_outbound_count",
        "Current number of outbound Travel Rule messages awaiting acknowledgement"
    ).unwrap();

    pub static ref PENDING_INBOUND_SCREENING_COUNT: Gauge = register_gauge!(
        "aframp_travel_rule_pending_inbound_screening_count",
        "Current number of inbound messages pending originator screening"
    ).unwrap();

    pub static ref VASP_REGISTRY_SIZE: Gauge = register_gauge!(
        "aframp_travel_rule_vasp_registry_size",
        "Total number of VASPs in the registry"
    ).unwrap();
}

/// Register all Travel Rule metrics with the global Prometheus registry.
/// Called from `metrics::register_all()` in `src/metrics/mod.rs`.
pub fn register(r: &prometheus::Registry) {
    macro_rules! reg {
        ($m:expr) => {
            r.register(Box::new($m.clone())).ok();
        };
    }
    reg!(OUTBOUND_MESSAGES_TOTAL);
    reg!(INBOUND_MESSAGES_TOTAL);
    reg!(TRANSMISSION_FAILURES_TOTAL);
    reg!(SCREENING_FAILURES_TOTAL);
    reg!(UNHOSTED_WALLET_TRANSACTIONS_TOTAL);
    reg!(SUNRISE_RULE_APPLIED_TOTAL);
    reg!(UNACKNOWLEDGED_OUTBOUND_COUNT);
    reg!(PENDING_INBOUND_SCREENING_COUNT);
    reg!(VASP_REGISTRY_SIZE);
}
