# Rates API Documentation

## Overview

The Rates API provides real-time exchange rate information for NGN/cNGN and other supported currency pairs. This is a public endpoint designed for high performance with aggressive caching.

## Endpoint

```
GET /api/rates
```

## Features

- ✅ Real-time NGN/cNGN exchange rates (1:1 fixed peg)
- ✅ Support for single pair, multiple pairs, and all pairs queries
- ✅ 30-second caching with Redis
- ✅ ETag support for conditional requests
- ✅ CORS enabled for frontend access
- ✅ < 5ms response time (cached)
- ✅ < 50ms response time (uncached)
- ✅ No authentication required

## Query Parameters

### Option 1: Single Pair

Query a specific currency pair:

```
GET /api/rates?from=NGN&to=cNGN
```

**Parameters:**
- `from` (string, required) - Base currency (e.g., "NGN")
- `to` (string, required) - Quote currency (e.g., "cNGN")

### Option 2: Multiple Pairs

Query multiple currency pairs at once:

```
GET /api/rates?pairs=NGN/cNGN,cNGN/NGN
```

**Parameters:**
- `pairs` (string, required) - Comma-separated list of pairs in format "FROM/TO"

### Option 3: All Pairs

Get all supported currency pairs:

```
GET /api/rates
```

**Parameters:** None

## Response Formats

### Single Pair Response

```json
{
  "pair": "NGN/cNGN",
  "base_currency": "NGN",
  "quote_currency": "cNGN",
  "rate": "1.0",
  "inverse_rate": "1.0",
  "spread_percentage": "0.0",
  "last_updated": "2026-02-20T10:30:45Z",
  "source": "fixed_peg",
  "timestamp": "2026-02-20T10:31:00Z"
}
```

**Fields:**
- `pair` - Currency pair in format "FROM/TO"
- `base_currency` - Base currency code
- `quote_currency` - Quote currency code
- `rate` - Exchange rate (how much quote currency per base currency)
- `inverse_rate` - Inverse rate (for convenience)
- `spread_percentage` - Bid/ask spread (0.0 for fixed peg)
- `last_updated` - When rate was last updated
- `source` - Rate source ("fixed_peg" or "external_api")
- `timestamp` - Response generation time

### Multiple Pairs Response

```json
{
  "rates": [
    {
      "pair": "NGN/cNGN",
      "rate": "1.0",
      "last_updated": "2026-02-20T10:30:45Z",
      "source": "fixed_peg"
    },
    {
      "pair": "cNGN/NGN",
      "rate": "1.0",
      "last_updated": "2026-02-20T10:30:45Z",
      "source": "fixed_peg"
    }
  ],
  "timestamp": "2026-02-20T10:31:00Z"
}
```

### All Pairs Response

```json
{
  "rates": {
    "NGN/cNGN": {
      "rate": "1.0",
      "inverse_rate": "1.0",
      "spread": "0.0",
      "last_updated": "2026-02-20T10:30:45Z",
      "source": "fixed_peg"
    },
    "cNGN/NGN": {
      "rate": "1.0",
      "inverse_rate": "1.0",
      "spread": "0.0",
      "last_updated": "2026-02-20T10:30:45Z",
      "source": "fixed_peg"
    }
  },
  "supported_currencies": ["NGN", "cNGN"],
  "timestamp": "2026-02-20T10:31:00Z"
}
```

## Error Responses

### 400 Bad Request - Invalid Currency

```json
{
  "error": {
    "code": "INVALID_CURRENCY",
    "message": "Unsupported currency: XYZ",
    "supported_currencies": ["NGN", "cNGN"]
  }
}
```

### 400 Bad Request - Invalid Pair

```json
{
  "error": {
    "code": "INVALID_PAIR",
    "message": "Currency pair not supported: ABC/XYZ",
    "supported_pairs": ["NGN/cNGN", "cNGN/NGN"]
  }
}
```

### 503 Service Unavailable

```json
{
  "error": {
    "code": "RATE_SERVICE_UNAVAILABLE",
    "message": "Exchange rate service temporarily unavailable",
    "retry_after": 60
  }
}
```

## Response Headers

### Caching Headers

```
Cache-Control: public, max-age=30
ETag: "rate-12345678"
Last-Modified: Thu, 20 Feb 2026 10:30:45 GMT
```

### CORS Headers

```
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: GET, OPTIONS
Access-Control-Allow-Headers: Content-Type
Access-Control-Max-Age: 86400
```

## Caching Strategy

### Client-Side Caching

The API returns `Cache-Control` headers instructing clients to cache responses for 30 seconds:

```
Cache-Control: public, max-age=30
```

### Conditional Requests

Use ETags to avoid unnecessary data transfer:

```bash
# First request
curl -i http://localhost:3000/api/rates?from=NGN&to=cNGN
# Returns: ETag: "rate-12345678"

# Subsequent request with ETag
curl -H 'If-None-Match: "rate-12345678"' \
     http://localhost:3000/api/rates?from=NGN&to=cNGN
# Returns: 304 Not Modified (if rate unchanged)
```

### Server-Side Caching

- Redis cache with 30-second TTL
- Cache key format: `api:rates:{params}`
- Automatic cache invalidation on rate updates

## Usage Examples

### JavaScript/TypeScript

```javascript
// Fetch single rate
async function getCurrentRate() {
  const response = await fetch('/api/rates?from=NGN&to=cNGN');
  const data = await response.json();
  console.log(`1 NGN = ${data.rate} cNGN`);
  return data;
}

// With ETag caching
let cachedETag = null;

async function getRateWithCache() {
  const headers = {};
  if (cachedETag) {
    headers['If-None-Match'] = cachedETag;
  }

  const response = await fetch('/api/rates?from=NGN&to=cNGN', { headers });
  
  if (response.status === 304) {
    console.log('Using cached rate');
    return null; // Use cached data
  }

  cachedETag = response.headers.get('ETag');
  return await response.json();
}

// Fetch all rates
async function getAllRates() {
  const response = await fetch('/api/rates');
  const data = await response.json();
  return data.rates;
}
```

### cURL

```bash
# Single pair
curl "http://localhost:3000/api/rates?from=NGN&to=cNGN"

# Multiple pairs
curl "http://localhost:3000/api/rates?pairs=NGN/cNGN,cNGN/NGN"

# All pairs
curl "http://localhost:3000/api/rates"

# With conditional request
curl -H "If-None-Match: \"rate-12345678\"" \
     "http://localhost:3000/api/rates?from=NGN&to=cNGN"
```

### Python

```python
import requests

# Fetch single rate
def get_rate(from_currency, to_currency):
    response = requests.get(
        'http://localhost:3000/api/rates',
        params={'from': from_currency, 'to': to_currency}
    )
    response.raise_for_status()
    return response.json()

# With caching
class RateClient:
    def __init__(self):
        self.etag = None
    
    def get_rate(self, from_currency, to_currency):
        headers = {}
        if self.etag:
            headers['If-None-Match'] = self.etag
        
        response = requests.get(
            'http://localhost:3000/api/rates',
            params={'from': from_currency, 'to': to_currency},
            headers=headers
        )
        
        if response.status_code == 304:
            return None  # Use cached data
        
        response.raise_for_status()
        self.etag = response.headers.get('ETag')
        return response.json()
```

## Performance

### Response Times

- **Cached (Redis hit):** < 5ms
- **Uncached (service fetch):** < 50ms
- **95th percentile:** < 100ms

### Caching Metrics

- **Target cache hit rate:** > 90%
- **Cache TTL:** 30 seconds
- **Cache invalidation:** Automatic on rate updates

## Supported Currency Pairs

Currently supported:
- `NGN/cNGN` - Nigerian Naira to crypto Naira (1:1 fixed peg)
- `cNGN/NGN` - crypto Naira to Nigerian Naira (1:1 fixed peg)

Future support planned:
- `USD/NGN` - US Dollar to Naira
- `GBP/NGN` - British Pound to Naira
- `EUR/NGN` - Euro to Naira
- `USD/cNGN` - Dollar to crypto Naira (calculated)

## Rate Limiting

Currently no rate limiting is enforced, but consider implementing:
- 100 requests per minute per IP
- Burst allowance: 20 requests
- Use `X-RateLimit-*` headers to communicate limits

## Monitoring

### Key Metrics

Track these metrics for operational health:

1. **Request Rate**
   - Requests per minute
   - Peak request times

2. **Cache Performance**
   - Cache hit rate (target: > 90%)
   - Cache miss rate
   - Average cache lookup time

3. **Response Times**
   - P50, P95, P99 latencies
   - Cached vs uncached response times

4. **Error Rates**
   - 4xx errors (client errors)
   - 5xx errors (server errors)
   - Error breakdown by type

5. **Availability**
   - Uptime percentage
   - Service health checks

### Alerts

Set up alerts for:
- Cache hit rate < 85%
- P95 response time > 100ms
- Error rate > 1%
- Rate service unavailable > 2 minutes

## Integration with Frontend

### Display Current Rate

```javascript
function displayRate(rateData) {
  document.getElementById('rate').textContent = 
    `1 ${rateData.base_currency} = ${rateData.rate} ${rateData.quote_currency}`;
  
  const lastUpdated = new Date(rateData.last_updated);
  document.getElementById('last-updated').textContent = 
    `Updated: ${formatTimeAgo(lastUpdated)}`;
}

function formatTimeAgo(date) {
  const seconds = Math.floor((new Date() - date) / 1000);
  if (seconds < 60) return `${seconds} seconds ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} minutes ago`;
  const hours = Math.floor(minutes / 60);
  return `${hours} hours ago`;
}
```

### Auto-Refresh

```javascript
// Refresh rate every 30 seconds
setInterval(async () => {
  const rate = await getCurrentRate();
  displayRate(rate);
}, 30000);
```

## Testing

Run the test suite:

```bash
cargo test api_rates_test
```

Test coverage includes:
- Single pair queries
- Multiple pairs queries
- All pairs queries
- Invalid currency handling
- Invalid pair handling
- Cache behavior
- ETag support
- CORS headers
- Response format validation

## Troubleshooting

### Rate Service Unavailable

If you receive 503 errors:

1. Check database connectivity
2. Verify exchange rate service is running
3. Check rate provider health
4. Review service logs for errors

### Slow Response Times

If responses are slower than expected:

1. Check Redis connectivity
2. Verify cache hit rate
3. Monitor database query performance
4. Check network latency

### Cache Not Working

If cache hit rate is low:

1. Verify Redis is running
2. Check Redis connection pool
3. Review cache key generation
4. Monitor cache TTL settings

## Security Considerations

1. **Public Endpoint:** No authentication required, but consider rate limiting
2. **Input Validation:** All currency codes are validated against whitelist
3. **CORS:** Enabled for all origins (public API)
4. **No Sensitive Data:** Rates are public information

## Future Enhancements

1. **WebSocket Support:** Real-time rate updates
2. **Historical Data:** `/api/rates/history` endpoint
3. **Rate Alerts:** Subscribe to rate threshold notifications
4. **More Currency Pairs:** USD, GBP, EUR support
5. **Rate Limiting:** Per-IP request limits
6. **API Versioning:** `/api/v1/rates` for stability

## Related Documentation

- [Exchange Rate Service](./EXCHANGE_RATE_SERVICE.md)
- [Caching Strategy](./CACHING.md)
- [API Architecture](./API_ARCHITECTURE.md)
