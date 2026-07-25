import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ConfigManagerPanel } from '@/components/layout/ConfigManagerPanel';
import { RbacProvider } from '@/components/layout/RbacGate';
import type { InstitutionalUser } from '@/types';
import { getA11yViolations } from '../a11y';

function makeUser(overrides: Partial<InstitutionalUser> = {}): InstitutionalUser {
  return {
    id: 'user-1',
    name: 'Jane Signer',
    email: 'jane@aframp.io',
    role: 'Signatory',
    signerWeight: 2,
    ipWhitelist: [],
    isActive: true,
    createdAt: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

function renderPanel(
  role: 'SuperAdmin' | 'Operator' | 'ComplianceAuditor' | 'Signatory',
  users: InstitutionalUser[],
  handlers: Partial<{
    onRoleChange: (userId: string, role: string) => Promise<void>;
    onToggleActive: (userId: string, active: boolean) => Promise<void>;
    onUpdateIpWhitelist: (userId: string, ips: string[]) => Promise<void>;
  }> = {}
) {
  const onRoleChange = handlers.onRoleChange ?? vi.fn().mockResolvedValue(undefined);
  const onToggleActive = handlers.onToggleActive ?? vi.fn().mockResolvedValue(undefined);
  const onUpdateIpWhitelist = handlers.onUpdateIpWhitelist ?? vi.fn().mockResolvedValue(undefined);

  const utils = render(
    <RbacProvider role={role} userId="admin-1" userName="Admin">
      <ConfigManagerPanel
        users={users}
        onRoleChange={onRoleChange as any}
        onToggleActive={onToggleActive as any}
        onUpdateIpWhitelist={onUpdateIpWhitelist as any}
      />
    </RbacProvider>
  );

  return { ...utils, onRoleChange, onToggleActive, onUpdateIpWhitelist };
}

describe('ConfigManagerPanel', () => {
  it('renders a row per user with name, email, and signer weight', () => {
    renderPanel('SuperAdmin', [makeUser()]);

    expect(screen.getByTestId('user-row-user-1')).toBeInTheDocument();
    expect(screen.getByText('Jane Signer')).toBeInTheDocument();
    expect(screen.getByText('jane@aframp.io')).toBeInTheDocument();
    expect(screen.getByText('2')).toBeInTheDocument();
  });

  it('shows a role dropdown for a SuperAdmin (has users:write)', () => {
    renderPanel('SuperAdmin', [makeUser()]);

    expect(screen.getByLabelText('Role for Jane Signer')).toBeInTheDocument();
  });

  it('shows a read-only role badge for a role lacking users:write', () => {
    renderPanel('ComplianceAuditor', [makeUser()]);

    expect(screen.queryByLabelText('Role for Jane Signer')).not.toBeInTheDocument();
    expect(screen.getByText('Signatory')).toBeInTheDocument();
  });

  it('calls onRoleChange when a SuperAdmin changes a user role', async () => {
    const user = userEvent.setup();
    const { onRoleChange } = renderPanel('SuperAdmin', [makeUser()]);

    await user.selectOptions(screen.getByLabelText('Role for Jane Signer'), 'Operator');

    expect(onRoleChange).toHaveBeenCalledWith('user-1', 'Operator');
  });

  it('calls onToggleActive when the active/suspend button is clicked', async () => {
    const user = userEvent.setup();
    const { onToggleActive } = renderPanel('SuperAdmin', [makeUser({ isActive: true })]);

    await user.click(screen.getByLabelText('Suspend Jane Signer'));

    expect(onToggleActive).toHaveBeenCalledWith('user-1', false);
  });

  it('shows a read-only status badge for a role lacking users:write', () => {
    renderPanel('Operator', [makeUser({ isActive: false })]);

    expect(screen.queryByLabelText(/Activate Jane Signer/)).not.toBeInTheDocument();
    expect(screen.getByText('Suspended')).toBeInTheDocument();
  });

  it('edits and saves an IP whitelist for a user with config:write', async () => {
    const user = userEvent.setup();
    const { onUpdateIpWhitelist } = renderPanel('SuperAdmin', [makeUser({ ipWhitelist: [] })]);

    await user.click(screen.getByText('Any — edit'));

    const input = screen.getByLabelText('IP whitelist for Jane Signer');
    await user.type(input, '10.0.0.1, 10.0.0.2');
    await user.click(screen.getByText('Save'));

    expect(onUpdateIpWhitelist).toHaveBeenCalledWith('user-1', ['10.0.0.1', '10.0.0.2']);
  });

  it('cancels an in-progress IP whitelist edit without saving', async () => {
    const user = userEvent.setup();
    const { onUpdateIpWhitelist } = renderPanel('SuperAdmin', [makeUser({ ipWhitelist: ['1.1.1.1'] })]);

    await user.click(screen.getByText('1.1.1.1'));
    expect(screen.getByLabelText('IP whitelist for Jane Signer')).toBeInTheDocument();

    await user.click(screen.getByText('Cancel'));

    expect(screen.queryByLabelText('IP whitelist for Jane Signer')).not.toBeInTheDocument();
    expect(onUpdateIpWhitelist).not.toHaveBeenCalled();
  });

  it('shows a read-only IP whitelist for a role lacking config:write', () => {
    renderPanel('ComplianceAuditor', [makeUser({ ipWhitelist: ['1.1.1.1'] })]);

    expect(screen.getByText('1.1.1.1')).toBeInTheDocument();
    expect(screen.queryByLabelText('IP whitelist for Jane Signer')).not.toBeInTheDocument();
  });

  it('has no detectable accessibility violations', async () => {
    const { container } = renderPanel('SuperAdmin', [makeUser()]);
    expect(await getA11yViolations(container)).toHaveLength(0);
  });
});
