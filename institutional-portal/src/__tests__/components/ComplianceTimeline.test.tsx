import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ComplianceTimeline } from '@/components/compliance/ComplianceTimeline';
import type { AuditEntry } from '@/types';
import { getA11yViolations } from '../a11y';

function makeEntry(overrides: Partial<AuditEntry> = {}): AuditEntry {
  return {
    id: 'entry-1',
    proposalId: 'proposal-1',
    eventType: 'proposal_created',
    actorKey: 'GABC123',
    actorId: 'user-1',
    actorName: 'Jane Signer',
    payload: {},
    currentHash: 'a'.repeat(64),
    previousHash: null,
    createdAt: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

describe('ComplianceTimeline', () => {
  it('renders the empty state when there are no entries', () => {
    render(<ComplianceTimeline entries={[]} />);

    expect(screen.getByText('No audit entries found.')).toBeInTheDocument();
    expect(screen.getByText('0 events')).toBeInTheDocument();
  });

  it('renders entries sorted chronologically', () => {
    const later = makeEntry({ id: 'later', eventType: 'threshold_met', createdAt: '2026-01-02T00:00:00Z' });
    const earlier = makeEntry({ id: 'earlier', eventType: 'proposal_created', createdAt: '2026-01-01T00:00:00Z' });

    render(<ComplianceTimeline entries={[later, earlier]} />);

    const items = screen.getAllByRole('listitem');
    expect(items).toHaveLength(2);
    expect(items[0]).toHaveAttribute('data-testid', 'audit-event-0');
    expect(items[0]).toHaveTextContent('Proposal Created');
    expect(items[1]).toHaveTextContent('Threshold Met');
  });

  it('filters entries by proposalId when provided', () => {
    const entries = [
      makeEntry({ id: 'a', proposalId: 'proposal-1' }),
      makeEntry({ id: 'b', proposalId: 'proposal-2' }),
    ];

    render(<ComplianceTimeline entries={entries} proposalId="proposal-2" />);

    expect(screen.getByText('1 events')).toBeInTheDocument();
  });

  it('renders the actor name and key when present', () => {
    render(
      <ComplianceTimeline
        entries={[makeEntry({ actorName: 'Jane Signer', actorKey: 'GABC123XYZ' })]}
      />
    );

    expect(screen.getByText('Jane Signer')).toBeInTheDocument();
    expect(screen.getByText('GABC123XYZ')).toBeInTheDocument();
  });

  it('renders a Stellar Explorer link once a transaction is confirmed', () => {
    const confirmed = makeEntry({
      id: 'confirmed',
      eventType: 'transaction_confirmed',
      payload: { tx_hash: 'deadbeef' },
    });

    render(<ComplianceTimeline entries={[confirmed]} />);

    const link = screen.getByRole('link', { name: /View on Stellar Expert/i });
    expect(link).toHaveAttribute('href', 'https://stellar.expert/explorer/public/tx/deadbeef');
    expect(link).toHaveAttribute('target', '_blank');
    expect(link).toHaveAttribute('rel', 'noopener noreferrer');
  });

  it('omits the Stellar Explorer link when nothing is confirmed', () => {
    render(<ComplianceTimeline entries={[makeEntry()]} />);

    expect(screen.queryByRole('link', { name: /View on Stellar Expert/i })).not.toBeInTheDocument();
  });

  it('has no detectable accessibility violations', async () => {
    const { container } = render(
      <ComplianceTimeline entries={[makeEntry(), makeEntry({ id: 'entry-2', eventType: 'signature_submitted' })]} />
    );

    expect(await getA11yViolations(container)).toHaveLength(0);
  });
});
