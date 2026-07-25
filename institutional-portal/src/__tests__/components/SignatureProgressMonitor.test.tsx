import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { SignatureProgressMonitor } from '@/components/multisig/SignatureProgressMonitor';
import type { MultisigEnvelope, SignoffEntry } from '@/types';
import { getA11yViolations } from '../a11y';

function makeSignoffEntry(overrides: Partial<SignoffEntry> = {}): SignoffEntry {
  return {
    signerId: 's1',
    signerKey: 'GABC'.padEnd(56, '0'),
    signerName: 'Jane Signer',
    signerWeight: 2,
    signedAt: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

function makeEnvelope(overrides: Partial<MultisigEnvelope> = {}): MultisigEnvelope {
  return {
    id: 'env-1',
    opType: 'mint',
    description: 'Mint 10,000 cNGN',
    unsignedXdr: 'AAAA',
    signedXdr: null,
    stellarTxHash: null,
    requiredSignatures: 3,
    totalSigners: 5,
    signoffMatrix: {
      proposalId: 'env-1',
      requiredWeight: 6,
      accumulatedWeight: 2,
      entries: [makeSignoffEntry()],
      thresholdMet: false,
    },
    timeLockUntil: null,
    timeLockRemainingSeconds: null,
    status: 'PENDING_SIGNATURES',
    failureReason: null,
    proposedBy: 'user-1',
    proposedByKey: 'GXYZ',
    expiresAt: '2026-01-02T00:00:00Z',
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

describe('SignatureProgressMonitor', () => {
  it('renders the accumulated/required weight and progress bar', () => {
    render(<SignatureProgressMonitor envelope={makeEnvelope()} />);

    const bar = screen.getByRole('progressbar');
    expect(bar).toHaveAttribute('aria-valuenow', '33');
    expect(screen.getByText('2 / 6')).toBeInTheDocument();
  });

  it('shows how much weight is still needed when the threshold is unmet', () => {
    render(<SignatureProgressMonitor envelope={makeEnvelope()} />);

    expect(screen.getByText(/Need/)).toBeInTheDocument();
    expect(screen.getByText('4')).toBeInTheDocument();
  });

  it('shows the threshold-met badge and hides the remaining-weight copy once met', () => {
    const envelope = makeEnvelope({
      signoffMatrix: {
        proposalId: 'env-1',
        requiredWeight: 2,
        accumulatedWeight: 2,
        entries: [makeSignoffEntry()],
        thresholdMet: true,
      },
    });

    render(<SignatureProgressMonitor envelope={envelope} />);

    expect(screen.getByText('✓ Threshold Met')).toBeInTheDocument();
    expect(screen.queryByText(/Need/)).not.toBeInTheDocument();
  });

  it('caps the progress bar at 100% when weight exceeds the requirement', () => {
    const envelope = makeEnvelope({
      signoffMatrix: {
        proposalId: 'env-1',
        requiredWeight: 2,
        accumulatedWeight: 5,
        entries: [makeSignoffEntry()],
        thresholdMet: true,
      },
    });

    render(<SignatureProgressMonitor envelope={envelope} />);

    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '100');
  });

  it('renders a signed entry with signer name, key, and weight', () => {
    render(<SignatureProgressMonitor envelope={makeEnvelope()} />);

    expect(screen.getByTestId('signature-0')).toHaveTextContent('Jane Signer');
    expect(screen.getByTestId('signature-0')).toHaveTextContent('Weight: 2');
  });

  it('renders placeholder slots for signatories that have not yet signed', () => {
    render(<SignatureProgressMonitor envelope={makeEnvelope()} />);

    // requiredSignatures: 3, entries: 1 signed → 2 pending placeholders
    expect(screen.getAllByText('Awaiting signatory')).toHaveLength(2);
  });

  it('renders no placeholder slots once all signatories have signed', () => {
    const envelope = makeEnvelope({
      requiredSignatures: 1,
      signoffMatrix: {
        proposalId: 'env-1',
        requiredWeight: 2,
        accumulatedWeight: 2,
        entries: [makeSignoffEntry()],
        thresholdMet: true,
      },
    });

    render(<SignatureProgressMonitor envelope={envelope} />);

    expect(screen.queryByText('Awaiting signatory')).not.toBeInTheDocument();
  });

  it('has no detectable accessibility violations', async () => {
    // Pre-existing gap: the `role="progressbar"` div has no accessible name
    // (no aria-label/aria-labelledby). Tracked separately from this test task.
    const { container } = render(<SignatureProgressMonitor envelope={makeEnvelope()} />);
    const violations = await getA11yViolations(container);
    expect(violations.map(v => v.id)).toEqual(['aria-progressbar-name']);
  });
});
