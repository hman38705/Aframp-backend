// Issue #482 — Next.js Edge Middleware: Tenant Resolution
// Extracts tenant_id from host header or X-Partner-Domain and injects it
// into request headers for downstream consumption. Runs at the edge — zero FOUC.

import { NextRequest, NextResponse, type NextFetchEvent } from 'next/server';

const TENANT_MAP: Record<string, string> = {
  'app.aframp.io': 'aframp',
  'pay.zenithbank.com': 'zenith',
  'remit.uba.africa': 'uba',
  // Additional tenants loaded from KV store in production
};

interface ResolvedTenant {
  tenantId: string;
  isUnknown: boolean;
}

function resolveTenantId(req: NextRequest): ResolvedTenant {
  // 1. Explicit partner header (B2B API calls)
  const partnerDomain = req.headers.get('x-partner-domain');
  if (partnerDomain && TENANT_MAP[partnerDomain]) {
    return { tenantId: TENANT_MAP[partnerDomain], isUnknown: false };
  }

  // 2. Host-based resolution
  const host = req.headers.get('host') ?? '';
  const cleanHost = host.split(':')[0]; // strip port
  if (TENANT_MAP[cleanHost]) return { tenantId: TENANT_MAP[cleanHost], isUnknown: false };

  // 3. Subdomain extraction: zenith.aframp.io → zenith
  const subdomain = cleanHost.split('.')[0];
  if (subdomain && subdomain !== 'www' && subdomain !== 'app') {
    return { tenantId: subdomain, isUnknown: false };
  }

  return { tenantId: 'default', isUnknown: true };
}

/** Fire-and-forget visibility log — never blocks or fails the request. */
function reportUnknownTenant(req: NextRequest): Promise<void> {
  return fetch(`${req.nextUrl.origin}/api/v1/tenant/unknown`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      host: req.headers.get('host') ?? '',
      path: req.nextUrl.pathname,
      timestamp: new Date().toISOString(),
    }),
  })
    .then(() => undefined)
    .catch(() => undefined);
}

export function middleware(req: NextRequest, event: NextFetchEvent) {
  const { tenantId, isUnknown } = resolveTenantId(req);

  if (isUnknown) {
    event.waitUntil(reportUnknownTenant(req));
  }

  const res = NextResponse.next();
  // Inject tenant_id for server components and API routes
  res.headers.set('x-tenant-id', tenantId);
  return res;
}

export const config = {
  matcher: ['/((?!_next/static|_next/image|favicon.ico).*)'],
};
