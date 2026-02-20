# Webhook System - Quick Reference

## Architecture

```
Provider → Webhook Endpoint → Signature Verification → Event Parsing
                                        ↓
                              Idempotency Check (DB)
                                        ↓
                              Process Event (Orchestrator)
                                        ↓
                              Update Transaction State
                                        ↓
                              Return 200 OK
                                        
Failed Processing → Retry Queue → Background Worker (60s interval)
                                        ↓
                              Retry (max 5 attempts)
                                        ↓
                              Dead Letter Queue
```

## Key Components

### 1. WebhookProcessor (`src/services/webhook_processor.rs`)
- Main processing logic
- Signature verification
- Event parsing
- Idempotency handling
- Retry coordination

### 2. WebhookRetryWorker (`src/workers/webhook_retry.rs`)
- Background worker
- Runs every 60 seconds
- Processes pending webhooks
- Manages retry attempts

### 3. Webhook Endpoints (`src/api/webhooks.rs`)
- HTTP handlers
- Header extraction
- Response formatting

### 4. PaymentOrchestrator (`src/services/payment_orchestrator.rs`)
- Transaction state updates
- Event handlers:
  - `handle_payment_success()`
  - `handle_payment_failure()`
  - `handle_withdrawal_success()`
  - `handle_withdrawal_failure()`

## Event Type Mapping

| Provider Event | Internal Action |
|---------------|----------------|
| `charge.completed` | Payment Success |
| `charge.success` | Payment Success |
| `charge.failed` | Payment Failure |
| `transfer.completed` | Withdrawal Success |
| `transfer.success` | Withdrawal Success |
| `transfer.failed` | Withdrawal Failure |

## Database Operations

### Log Webhook Event
```rust
webhook_repo.log_event(
    event_id,
    provider,
    event_type,
    payload,
    signature,
    transaction_id
).await?
```

### Mark as Processed
```rust
webhook_repo.mark_processed(webhook_id).await?
```

### Record Failure
```rust
webhook_repo.record_failure(webhook_id, error_message).await?
```

### Get Pending Events
```rust
webhook_repo.get_pending_events(limit).await?
```

### Get Failed Events (DLQ)
```rust
webhook_repo.get_failed_events(limit).await?
```

## Configuration

### Required Environment Variables
```bash
FLUTTERWAVE_WEBHOOK_SECRET=your_secret
PAYSTACK_WEBHOOK_SECRET=your_secret
```

### Optional Environment Variables
```bash
WEBHOOK_RETRY_ENABLED=true  # Default: true
DATABASE_URL=postgresql://...
```

## Provider Setup

### Flutterwave
1. Go to Settings → Webhooks
2. Set URL: `https://your-domain.com/webhooks/flutterwave`
3. Copy webhook secret hash
4. Set `FLUTTERWAVE_WEBHOOK_SECRET` environment variable

### Paystack
1. Go to Settings → Webhooks
2. Set URL: `https://your-domain.com/webhooks/paystack`
3. Copy webhook secret key
4. Set `PAYSTACK_WEBHOOK_SECRET` environment variable

## Testing Commands

### Test Flutterwave Webhook
```bash
curl -X POST http://localhost:8000/webhooks/flutterwave \
  -H "verif-hash: your_secret" \
  -H "Content-Type: application/json" \
  -d '{
    "id": 12345,
    "event": "charge.completed",
    "data": {
      "tx_ref": "tx_123",
      "status": "successful",
      "amount": 5000,
      "currency": "NGN"
    }
  }'
```

### Test Paystack Webhook
```bash
# Generate signature: HMAC SHA512 of payload with secret
curl -X POST http://localhost:8000/webhooks/paystack \
  -H "x-paystack-signature: <computed_signature>" \
  -H "Content-Type: application/json" \
  -d '{
    "event": "charge.success",
    "data": {
      "reference": "tx_123",
      "status": "success",
      "amount": 500000,
      "currency": "NGN"
    }
  }'
```

## Monitoring Queries

### Check Recent Webhooks
```sql
SELECT provider, event_type, status, retry_count, created_at
FROM webhook_events
ORDER BY created_at DESC
LIMIT 20;
```

### Check Failed Webhooks (DLQ)
```sql
SELECT id, provider, event_type, error_message, retry_count, created_at
FROM webhook_events
WHERE status = 'failed'
ORDER BY created_at DESC;
```

### Check Pending Retries
```sql
SELECT provider, event_type, retry_count, error_message, created_at
FROM webhook_events
WHERE status = 'pending' AND retry_count > 0
ORDER BY created_at DESC;
```

### Webhook Success Rate
```sql
SELECT 
    provider,
    COUNT(*) as total,
    SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) as successful,
    ROUND(100.0 * SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) / COUNT(*), 2) as success_rate
FROM webhook_events
WHERE created_at > NOW() - INTERVAL '24 hours'
GROUP BY provider;
```

## Common Issues

### Issue: Invalid Signature
**Cause**: Webhook secret mismatch  
**Solution**: Verify secret in provider dashboard matches environment variable

### Issue: Webhook Not Processed
**Cause**: Transaction not found  
**Solution**: Ensure transaction created before webhook arrives, or wait for retry

### Issue: Duplicate Processing
**Cause**: Idempotency check failed  
**Solution**: Check UNIQUE constraint on (provider, event_id) exists

### Issue: Stuck in Retry Queue
**Cause**: Non-retryable error or max retries exceeded  
**Solution**: Check error_message, manually investigate and reprocess if needed

## Manual Reprocessing

To manually reprocess a failed webhook:

```sql
-- Reset webhook status
UPDATE webhook_events
SET status = 'pending', retry_count = 0, error_message = NULL
WHERE id = '<webhook_id>';

-- Worker will pick it up on next run (60s)
```

## Performance Tips

1. **Index Optimization**: Ensure indexes exist on (provider, event_id) and (status, retry_count)
2. **Batch Size**: Adjust retry worker batch size if needed (default: 50)
3. **Worker Interval**: Adjust retry interval based on load (default: 60s)
4. **Connection Pool**: Ensure adequate database connections for webhook load

## Security Checklist

- ✅ Webhook secrets configured
- ✅ HTTPS enabled in production
- ✅ Signature verification active
- ✅ Constant-time comparison used
- ✅ Invalid signatures rejected
- ✅ Payload validation enabled
- ⬜ IP whitelisting (optional)
- ⬜ Rate limiting (optional)

## Logs to Monitor

```bash
# Successful processing
INFO Webhook processed successfully event_id=12345

# Duplicate webhook
INFO Webhook already processed event_id=12345

# Invalid signature
WARN Invalid webhook signature provider=paystack

# Processing failure
WARN Webhook processing failed event_id=12345 error="..."

# Retry worker
INFO Retried pending webhooks processed=5
```

## Quick Debugging

1. Check webhook received: `grep "Received webhook" logs/app.log`
2. Check signature issues: `grep "Invalid webhook signature" logs/app.log`
3. Check processing errors: `grep "Webhook processing failed" logs/app.log`
4. Check retry activity: `grep "Retried pending webhooks" logs/app.log`
5. Query database: `SELECT * FROM webhook_events WHERE status = 'failed';`

## API Response Codes

| Code | Meaning | Action |
|------|---------|--------|
| 200 | Success | Webhook processed or duplicate |
| 400 | Bad Request | Invalid JSON payload |
| 401 | Unauthorized | Invalid signature |
| 500 | Server Error | Internal error (provider will retry) |

## Integration Checklist

- ✅ Webhook endpoints created
- ✅ Signature verification implemented
- ✅ Event parsing working
- ✅ Idempotency handling active
- ✅ Retry worker running
- ✅ Dead letter queue configured
- ✅ Orchestrator integration complete
- ✅ Logging enabled
- ⬜ Provider webhooks configured
- ⬜ Production testing complete

## Support

For issues or questions:
1. Check logs for error messages
2. Query webhook_events table for details
3. Review provider webhook logs
4. Check WEBHOOK_IMPLEMENTATION.md for detailed documentation
