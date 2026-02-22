# Rates API Quick Start Guide

## Overview

The Rates API provides real-time exchange rate information for NGN/cNGN and other supported currency pairs. This is a public endpoint that requires no authentication.

## Base URL

```
http://localhost:8000/api/rates
```

## Endpoints

### 1. Get Single Rate

Get the exchange rate for a specific currency pair.

**Request:**
```bash
GET /api/rates?from=NGN&to=cNGN
```

**Response:**
```json
{
  "pair": "NGN/cNGN",
  "base_currency": "NGN",
  "quote_currency": "cNGN",
  "rate": "1.0",
  "inverse_rate": "1.0",
  "spread_percentage": "0.0",
  "last_updated": "2026-02-22T10:30:45Z",
  "source": "fixed_peg",
  "timestamp": "2026-02-22T10:31:00Z"
}
```

### 2. Get Multiple Rates

Get exchange rates for multiple currency pairs in a single request.

**Request:**
```bash
GET /api/rates?pairs=NGN/cNGN,cNGN/NGN
```

**Response:**
```json
{
  "rates": [
    {
      "pair": "NGN/cNGN",
      "rate": "1.0",
      "last_updated": "2026-02-22T10:30:45Z",
      "source": "fixed_peg"
    },
    {
      "pair": "cNGN/NGN",
      "rate": "1.0",
      "last_updated": "2026-02-22T10:30:45Z",
      "source": "fixed_peg"
    }
  ],
  "timestamp": "2026-02-22T10:31:00Z"
}
```

### 3. Get All Rates

Get all supported currency pairs.

**Request:**
```bash
GET /api/rates
```

**Response:**
```json
{
  "rates": {
    "NGN/cNGN": {
      "rate": "1.0",
      "inverse_rate": "1.0",
      "spread": "0.0",
      "last_updated": "2026-02-22T10:30:45Z",
      "source": "fixed_peg"
    },
    "cNGN/NGN": {
      "rate": "1.0",
      "inverse_rate": "1.0",
      "spread": "0.0",
      "last_updated": "2026-02-22T10:30:45Z",
      "source": "fixed_peg"
    }
  },
  "supported_currencies": ["NGN", "cNGN"],
  "timestamp": "2026-02-22T10:31:00Z"
}
```

## Supported Currency Pairs

Currently supported:
- `NGN/cNGN` - Nigerian Naira to crypto Naira (1:1 peg)
- `cNGN/NGN` - crypto Naira to Nigerian Naira (1:1 peg)

## Error Responses

### Invalid Currency

**Request:**
```bash
GET /api/rates?from=XYZ&to=cNGN
```

**Response (400 Bad Request):**
```json
{
  "error": {
    "code": "INVALID_CURRENCY",
    "message": "Unsupported currency: XYZ",
    "supported_currencies": ["NGN", "cNGN"]
  }
}
```

### Invalid Pair

**Request:**
```bash
GET /api/rates?from=NGN&to=BTC
```

**Response (400 Bad Request):**
```json
{
  "error": {
    "code": "INVALID_PAIR",
    "message": "Currency pair not supported: NGN/BTC",
    "supported_pairs": ["NGN/cNGN", "cNGN/NGN"]
  }
}
```

### Service Unavailable

**Response (503 Service Unavailable):**
```json
{
  "error": {
    "code": "RATE_SERVICE_UNAVAILABLE",
    "message": "Exchange rate service temporarily unavailable",
    "retry_after": 60
  }
}
```

## Caching

The API implements aggressive caching for optimal performance:

- **Cache Duration:** 30 seconds
- **Cache Headers:** `Cache-Control: public, max-age=30`
- **ETag Support:** Yes (for conditional requests)
- **Expected Response Time:** < 5ms (cached), < 50ms (uncached)

### Using ETags

**First Request:**
```bash
curl -i http://localhost:8000/api/rates?from=NGN&to=cNGN
```

**Response Headers:**
```
ETag: "rate-12345678"
Cache-Control: public, max-age=30
```

**Subsequent Request:**
```bash
curl -H 'If-None-Match: "rate-12345678"' \
  http://localhost:8000/api/rates?from=NGN&to=cNGN
```

**Response (if unchanged):**
```
304 Not Modified
```

## CORS Support

The API is fully CORS-enabled for frontend access:

```
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: GET, OPTIONS
Access-Control-Allow-Headers: Content-Type
Access-Control-Max-Age: 86400
```

## Frontend Integration

### JavaScript/TypeScript

```javascript
// Fetch current rate
async function getCurrentRate(from, to) {
  const response = await fetch(
    `http://localhost:8000/api/rates?from=${from}&to=${to}`
  );
  
  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }
  
  const data = await response.json();
  return data;
}

// Usage
const rate = await getCurrentRate('NGN', 'cNGN');
console.log(`1 ${rate.base_currency} = ${rate.rate} ${rate.quote_currency}`);
```

### With ETag Caching

```javascript
let cachedETag = null;

async function getRateWithCache(from, to) {
  const headers = {};
  if (cachedETag) {
    headers['If-None-Match'] = cachedETag;
  }
  
  const response = await fetch(
    `http://localhost:8000/api/rates?from=${from}&to=${to}`,
    { headers }
  );
  
  if (response.status === 304) {
    // Use cached data
    return getCachedRate();
  }
  
  // Update cache
  cachedETag = response.headers.get('ETag');
  const data = await response.json();
  cacheRate(data);
  return data;
}
```

### React Hook

```typescript
import { useState, useEffect } from 'react';

interface Rate {
  pair: string;
  rate: string;
  last_updated: string;
}

function useExchangeRate(from: string, to: string) {
  const [rate, setRate] = useState<Rate | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  useEffect(() => {
    let mounted = true;

    async function fetchRate() {
      try {
        const response = await fetch(
          `http://localhost:8000/api/rates?from=${from}&to=${to}`
        );
        
        if (!response.ok) {
          throw new Error('Failed to fetch rate');
        }
        
        const data = await response.json();
        
        if (mounted) {
          setRate(data);
          setLoading(false);
        }
      } catch (err) {
        if (mounted) {
          setError(err as Error);
          setLoading(false);
        }
      }
    }

    fetchRate();
    
    // Refresh every 30 seconds
    const interval = setInterval(fetchRate, 30000);

    return () => {
      mounted = false;
      clearInterval(interval);
    };
  }, [from, to]);

  return { rate, loading, error };
}

// Usage in component
function RateDisplay() {
  const { rate, loading, error } = useExchangeRate('NGN', 'cNGN');

  if (loading) return <div>Loading...</div>;
  if (error) return <div>Error: {error.message}</div>;
  if (!rate) return null;

  return (
    <div>
      <p>1 {rate.base_currency} = {rate.rate} {rate.quote_currency}</p>
      <small>Last updated: {new Date(rate.last_updated).toLocaleString()}</small>
    </div>
  );
}
```

## Testing

### Using curl

```bash
# Single pair
curl http://localhost:8000/api/rates?from=NGN&to=cNGN

# Multiple pairs
curl http://localhost:8000/api/rates?pairs=NGN/cNGN,cNGN/NGN

# All pairs
curl http://localhost:8000/api/rates

# With headers
curl -i http://localhost:8000/api/rates?from=NGN&to=cNGN

# OPTIONS preflight
curl -X OPTIONS http://localhost:8000/api/rates
```

### Using the Demo Application

Run the standalone demo:

```bash
cargo run --example rates_api_demo
```

Then visit:
- http://localhost:3000/api/rates?from=NGN&to=cNGN
- http://localhost:3000/api/rates?pairs=NGN/cNGN,cNGN/NGN
- http://localhost:3000/api/rates

## Performance Tips

1. **Use caching**: Respect the `Cache-Control` headers
2. **Batch requests**: Use the `pairs` parameter for multiple rates
3. **Implement ETags**: Reduce bandwidth with conditional requests
4. **Poll wisely**: Don't poll more frequently than the 30s cache TTL
5. **Handle errors**: Implement retry logic with exponential backoff

## Rate Limiting

While not currently enforced, consider implementing client-side rate limiting:
- Recommended: 100 requests per minute per client
- Use caching to minimize requests
- Batch multiple pairs in single requests

## Support

For issues or questions:
- Check the full API documentation: `docs/RATES_API.md`
- Review integration guide: `docs/RATES_API_INTEGRATION.md`
- Run the demo: `cargo run --example rates_api_demo`

## Next Steps

1. Integrate the API into your frontend
2. Implement proper error handling
3. Add rate display to transaction flows
4. Monitor API usage and performance
5. Consider WebSocket integration for real-time updates (future)
