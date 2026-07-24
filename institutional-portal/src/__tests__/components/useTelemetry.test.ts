import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook } from '@testing-library/react';
import { useTelemetry } from '@/hooks/useTelemetry';

describe('useTelemetry', () => {
  beforeEach(() => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true }));
  });

  it('posts multisig_approval_latency with latency converted to seconds', () => {
    const { result } = renderHook(() => useTelemetry());

    result.current.recordApprovalLatency('proposal-1', 2500);

    expect(fetch).toHaveBeenCalledWith('/api/telemetry', expect.objectContaining({
      method: 'POST',
      keepalive: true,
    }));
    const body = JSON.parse((fetch as any).mock.calls[0][1].body);
    expect(body).toMatchObject({
      name: 'multisig_approval_latency',
      value: 2.5,
      labels: { proposal_id: 'proposal-1' },
    });
    expect(typeof body.ts).toBe('number');
  });

  it('posts portal_access_denied with permission and role labels', () => {
    const { result } = renderHook(() => useTelemetry());

    result.current.recordAccessDenied('config:write', 'Operator');

    const body = JSON.parse((fetch as any).mock.calls[0][1].body);
    expect(body).toMatchObject({
      name: 'portal_access_denied',
      labels: { permission: 'config:write', role: 'Operator' },
    });
    expect(body.value).toBeUndefined();
  });

  it('posts partial_signature_submitted with a truncated signer key', () => {
    const { result } = renderHook(() => useTelemetry());

    result.current.recordPartialSignature('proposal-1', 'GABCDEFGHIJKLMNOP');

    const body = JSON.parse((fetch as any).mock.calls[0][1].body);
    expect(body).toMatchObject({
      name: 'partial_signature_submitted',
      labels: { proposal_id: 'proposal-1', signer_key: 'GABCDEFG' },
    });
  });

  it('swallows telemetry fetch failures without throwing', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('network down')));
    const { result } = renderHook(() => useTelemetry());

    expect(() => result.current.recordAccessDenied('users:write', 'Signatory')).not.toThrow();
    // Allow the rejected promise's .catch handler to run before the test ends
    await Promise.resolve();
    await Promise.resolve();
  });
});
