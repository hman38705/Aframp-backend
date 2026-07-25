// Issue #482 — Unit Tests: domain parsing, hex validation, theme fallbacks

// ── Domain / host parsing ─────────────────────────────────────────────────────

function extractSubdomain(host: string): string {
  const clean = host.split(':')[0];
  const parts = clean.split('.');
  if (parts.length >= 3) return parts[0];
  return 'default';
}

describe('extractSubdomain', () => {
  it('extracts subdomain from three-part host', () => {
    expect(extractSubdomain('zenith.aframp.io')).toBe('zenith');
  });

  it('returns default for two-part host', () => {
    expect(extractSubdomain('aframp.io')).toBe('default');
  });

  it('strips port before parsing', () => {
    expect(extractSubdomain('uba.aframp.io:3000')).toBe('uba');
  });
});

// ── Hex color validation ──────────────────────────────────────────────────────

function isValidHex(color: string): boolean {
  return /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/.test(color);
}

describe('isValidHex', () => {
  it('accepts 6-digit hex', () => {
    expect(isValidHex('#3fb950')).toBe(true);
  });

  it('accepts 3-digit hex', () => {
    expect(isValidHex('#fff')).toBe(true);
  });

  it('accepts 8-digit hex with alpha', () => {
    expect(isValidHex('#3fb95080')).toBe(true);
  });

  it('rejects invalid hex', () => {
    expect(isValidHex('not-a-color')).toBe(false);
    expect(isValidHex('#gg0000')).toBe(false);
    expect(isValidHex('')).toBe(false);
  });
});

// ── Theme fallback ────────────────────────────────────────────────────────────

interface Theme { primaryColor: string; fontFamily: string }

function safeColor(val: string, fallback: string): string {
  return isValidHex(val) || val.startsWith('rgba(') || val.startsWith('rgb(') ? val : fallback;
}

describe('safeColor', () => {
  it('returns valid hex as-is', () => {
    expect(safeColor('#3fb950', '#000')).toBe('#3fb950');
  });

  it('returns fallback for invalid color', () => {
    expect(safeColor('not-a-color', '#000')).toBe('#000');
  });

  it('accepts rgba values', () => {
    expect(safeColor('rgba(63,185,80,0.5)', '#000')).toBe('rgba(63,185,80,0.5)');
  });
});

// ── Feature flag isolation ────────────────────────────────────────────────────

interface FeatureFlags { enableStellarSettlement: boolean; enableFiatDeposit: boolean }

function isFeatureEnabled(flags: FeatureFlags, key: keyof FeatureFlags): boolean {
  return flags[key] === true;
}

describe('isFeatureEnabled', () => {
  const flags: FeatureFlags = { enableStellarSettlement: true, enableFiatDeposit: false };

  it('returns true for enabled feature', () => {
    expect(isFeatureEnabled(flags, 'enableStellarSettlement')).toBe(true);
  });

  it('returns false for disabled feature', () => {
    expect(isFeatureEnabled(flags, 'enableFiatDeposit')).toBe(false);
  });
});

// ── Custom copy substitution ──────────────────────────────────────────────────

function substituteText(template: string, copy: Record<string, string>): string {
  return template.replace(/\{\{(\w+)\}\}/g, (_, key) => copy[key] ?? `{{${key}}}`);
}

describe('substituteText', () => {
  it('replaces known keys', () => {
    expect(substituteText('Welcome to {{platformName}}', { platformName: 'ZenithPay' })).toBe('Welcome to ZenithPay');
  });

  it('leaves unknown keys as-is', () => {
    expect(substituteText('Hello {{unknown}}', {})).toBe('Hello {{unknown}}');
  });

  it('handles multiple substitutions', () => {
    const result = substituteText('{{a}} and {{b}}', { a: 'foo', b: 'bar' });
    expect(result).toBe('foo and bar');
  });
});

// ── Issue #808 — Unknown tenant resolution + config fallback (integration) ────
//
// Mirrors frontend/middleware/tenant-resolver.ts (host → tenant id resolution)
// and frontend/hooks/useTenantConfig.ts (fetch-failure fallback selection),
// exercised together end-to-end for the "unknown tenant" path.

interface ResolvedTenant { tenantId: string; isUnknown: boolean }

const TENANT_MAP: Record<string, string> = {
  'app.aframp.io': 'aframp',
  'pay.zenithbank.com': 'zenith',
  'remit.uba.africa': 'uba',
};

function resolveTenantId(host: string, partnerDomain?: string): ResolvedTenant {
  if (partnerDomain && TENANT_MAP[partnerDomain]) return { tenantId: TENANT_MAP[partnerDomain], isUnknown: false };

  const cleanHost = host.split(':')[0];
  if (TENANT_MAP[cleanHost]) return { tenantId: TENANT_MAP[cleanHost], isUnknown: false };

  const subdomain = cleanHost.split('.')[0];
  if (subdomain && subdomain !== 'www' && subdomain !== 'app') return { tenantId: subdomain, isUnknown: false };

  return { tenantId: 'default', isUnknown: true };
}

interface DefaultTenantConfig { theme: { tenantId: string } }

const DEFAULT_TENANT_CONFIG: DefaultTenantConfig = { theme: { tenantId: 'default' } };

/** Mirrors useTenantConfig's error-path fallback: prefer last-known-good cache over hard-coded defaults. */
function selectFallbackConfig(lastKnownGood: DefaultTenantConfig | null): { config: DefaultTenantConfig; isStale: boolean } {
  return { config: lastKnownGood ?? DEFAULT_TENANT_CONFIG, isStale: true };
}

describe('unknown tenant resolution + config fallback (integration)', () => {
  it('flags a bare apex host as an unknown tenant', () => {
    const resolved = resolveTenantId('aframp.io');
    expect(resolved.tenantId).toBe('default');
    expect(resolved.isUnknown).toBe(true);
  });

  it('flags localhost as an unknown tenant', () => {
    const resolved = resolveTenantId('localhost:3000');
    expect(resolved.tenantId).toBe('default');
    expect(resolved.isUnknown).toBe(true);
  });

  it('does not flag a known mapped host as unknown', () => {
    const resolved = resolveTenantId('app.aframp.io');
    expect(resolved.tenantId).toBe('aframp');
    expect(resolved.isUnknown).toBe(false);
  });

  it('falls back to the cached last-known-good config when the tenant is unknown and the refetch fails', () => {
    const { tenantId, isUnknown } = resolveTenantId('aframp.io');
    expect(isUnknown).toBe(true);

    const cachedFromEarlierSession: DefaultTenantConfig = { theme: { tenantId: 'zenith' } };
    const { config, isStale } = selectFallbackConfig(cachedFromEarlierSession);

    expect(config.theme.tenantId).toBe('zenith');
    expect(isStale).toBe(true);
    expect(tenantId).toBe('default');
  });

  it('falls back to the hard-coded default config when there is no cache at all', () => {
    const { config, isStale } = selectFallbackConfig(null);
    expect(config).toBe(DEFAULT_TENANT_CONFIG);
    expect(isStale).toBe(true);
  });
});
