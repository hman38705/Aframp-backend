import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { RbacGate, RbacProvider, useRbac } from '@/components/layout/RbacGate';
import { getA11yViolations } from '../a11y';

describe('RbacGate', () => {
  it('renders children when the role has the required permission', () => {
    render(
      <RbacProvider role="SuperAdmin" userId="u1" userName="Ada">
        <RbacGate permission="users:write">
          <button>Edit Role</button>
        </RbacGate>
      </RbacProvider>
    );

    expect(screen.getByText('Edit Role')).toBeInTheDocument();
  });

  it('renders nothing by default when the role lacks the permission', () => {
    render(
      <RbacProvider role="Signatory" userId="u1" userName="Ada">
        <RbacGate permission="users:write">
          <button>Edit Role</button>
        </RbacGate>
      </RbacProvider>
    );

    expect(screen.queryByText('Edit Role')).not.toBeInTheDocument();
  });

  it('renders the fallback when the role lacks the permission and a fallback is given', () => {
    render(
      <RbacProvider role="ComplianceAuditor" userId="u1" userName="Ada">
        <RbacGate permission="users:write" fallback={<span>Read-only</span>}>
          <button>Edit Role</button>
        </RbacGate>
      </RbacProvider>
    );

    expect(screen.getByText('Read-only')).toBeInTheDocument();
    expect(screen.queryByText('Edit Role')).not.toBeInTheDocument();
  });

  it('throws when useRbac is used outside an RbacProvider', () => {
    function Bare() {
      useRbac();
      return null;
    }
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() => render(<Bare />)).toThrow('useRbac must be used within RbacProvider');
    spy.mockRestore();
  });

  it('exposes the current role and userName via useRbac', () => {
    function Consumer() {
      const { role, userName } = useRbac();
      return <span>{role} / {userName}</span>;
    }

    render(
      <RbacProvider role="Operator" userId="u2" userName="Bosun">
        <Consumer />
      </RbacProvider>
    );

    expect(screen.getByText('Operator / Bosun')).toBeInTheDocument();
  });

  it('has no detectable accessibility violations', async () => {
    const { container } = render(
      <RbacProvider role="SuperAdmin" userId="u1" userName="Ada">
        <RbacGate permission="users:write">
          <button>Edit Role</button>
        </RbacGate>
      </RbacProvider>
    );

    expect(await getA11yViolations(container)).toHaveLength(0);
  });
});
