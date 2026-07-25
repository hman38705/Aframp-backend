import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { XDRSigningPanel } from '@/components/multisig/XDRSigningPanel';
import type { MultisigEnvelope } from '@/types';
import type { ParsedXdrTransaction } from '@/lib/xdrParser';

const parseXdr = vi.fn<(xdrBase64: string) => Promise<ParsedXdrTransaction>>();
const xdrDigest = vi.fn<(xdrBase64: string) => Promise<string>>();

vi.mock('@/lib/xdrParser', () => ({
  parseXdr: (xdrBase64: string) => parseXdr(xdrBase64),
  xdrDigest: (xdrBase64: string) => xdrDigest(xdrBase64),
}));

function makeEnvelope(overrides: Partial<MultisigEnvelope> = {}): MultisigEnvelope {
  return {
    id: 'env-1',
    opType: 'mint',
    description: 'Mint 10,000 cNGN',
    unsignedXdr: 'BASE64XDR==',
    signedXdr: null,
    stellarTxHash: null,
    requiredSignatures: 3,
    totalSigners: 5,
    signoffMatrix: {
      proposalId: 'env-1',
      requiredWeight: 6,
      accumulatedWeight: 3,
      entries: [],
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

const parsedTx: ParsedXdrTransaction = {
  sourceAccount: 'GSOURCE123',
  fee: 100,
  seqNum: '42',
  memo: 'none',
  operations: [{ type: 'payment', body: { amount: '10000' } }],
};

beforeEach(() => {
  parseXdr.mockReset().mockResolvedValue(parsedTx);
  xdrDigest.mockReset().mockResolvedValue('deadbeef'.repeat(8));
  delete (window as any).freighter;
  if (!navigator.clipboard) {
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: async () => undefined },
      configurable: true,
    });
  }
});

describe('XDRSigningPanel', () => {
  it('shows a loading state while the XDR is being decoded', () => {
    parseXdr.mockReturnValue(new Promise(() => {})); // never resolves
    xdrDigest.mockReturnValue(new Promise(() => {})); // never resolves
    render(<XDRSigningPanel envelope={makeEnvelope()} onSign={vi.fn()} canSign={false} />);

    expect(screen.getByText('Decoding XDR…')).toBeInTheDocument();
  });

  it('renders parsed transaction details once decoding resolves', async () => {
    render(<XDRSigningPanel envelope={makeEnvelope()} onSign={vi.fn()} canSign={false} />);

    expect(await screen.findByText('GSOURCE123')).toBeInTheDocument();
    expect(screen.getByText('100')).toBeInTheDocument();
    expect(screen.getByText('42')).toBeInTheDocument();
    expect(screen.getByText(/deadbeef/)).toBeInTheDocument();
  });

  it('renders each operation as an expandable section', async () => {
    render(<XDRSigningPanel envelope={makeEnvelope()} onSign={vi.fn()} canSign={false} />);

    await screen.findByText('GSOURCE123');
    expect(screen.getByText('Operations (1)')).toBeInTheDocument();
    expect(screen.getByText('payment')).toBeInTheDocument();
  });

  it('copies the raw XDR to the clipboard', async () => {
    const user = userEvent.setup();
    const writeText = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue(undefined);
    render(<XDRSigningPanel envelope={makeEnvelope()} onSign={vi.fn()} canSign={false} />);

    await screen.findByText('GSOURCE123');
    await user.click(screen.getByLabelText('Copy raw XDR to clipboard'));

    expect(writeText).toHaveBeenCalledWith('BASE64XDR==');
  });

  it('does not render the signing footer when canSign is false', async () => {
    render(<XDRSigningPanel envelope={makeEnvelope()} onSign={vi.fn()} canSign={false} />);

    await screen.findByText('GSOURCE123');
    expect(screen.queryByText(/Freighter/)).not.toBeInTheDocument();
  });

  it('shows a wallet-not-detected warning when canSign is true but Freighter is absent', async () => {
    render(<XDRSigningPanel envelope={makeEnvelope()} onSign={vi.fn()} canSign={true} />);

    expect(await screen.findByText(/Freighter wallet not detected/)).toBeInTheDocument();
  });

  it('shows the connected wallet key and a sign button once Freighter is connected', async () => {
    (window as any).freighter = {
      isConnected: vi.fn().mockResolvedValue(true),
      getPublicKey: vi.fn().mockResolvedValue('GWALLETKEY123'),
      signTransaction: vi.fn(),
    };

    render(<XDRSigningPanel envelope={makeEnvelope()} onSign={vi.fn()} canSign={true} />);

    expect(await screen.findByText('GWALLETKEY123')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Sign with Freighter/ })).toBeInTheDocument();
  });

  it('signs the transaction and calls onSign with the signed XDR and signer key', async () => {
    const user = userEvent.setup();
    const signTransaction = vi.fn().mockResolvedValue('SIGNEDXDR==');
    (window as any).freighter = {
      isConnected: vi.fn().mockResolvedValue(true),
      getPublicKey: vi.fn().mockResolvedValue('GWALLETKEY123'),
      signTransaction,
    };
    const onSign = vi.fn().mockResolvedValue(undefined);

    render(<XDRSigningPanel envelope={makeEnvelope()} onSign={onSign} canSign={true} />);

    await user.click(await screen.findByRole('button', { name: /Sign with Freighter/ }));

    await waitFor(() => expect(onSign).toHaveBeenCalledWith('env-1', 'SIGNEDXDR==', 'GWALLETKEY123'));
    expect(signTransaction).toHaveBeenCalledWith('BASE64XDR==', {
      networkPassphrase: 'Public Global Stellar Network ; September 2015',
    });
  });

  it('shows an error message when signing fails', async () => {
    const user = userEvent.setup();
    (window as any).freighter = {
      isConnected: vi.fn().mockResolvedValue(true),
      getPublicKey: vi.fn().mockResolvedValue('GWALLETKEY123'),
      signTransaction: vi.fn().mockRejectedValue(new Error('User rejected the request')),
    };

    render(<XDRSigningPanel envelope={makeEnvelope()} onSign={vi.fn()} canSign={true} />);

    await user.click(await screen.findByRole('button', { name: /Sign with Freighter/ }));

    expect(await screen.findByRole('alert')).toHaveTextContent('User rejected the request');
  });
});
