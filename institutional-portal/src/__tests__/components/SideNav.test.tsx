import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { SideNav } from '@/components/layout/SideNav';
import { RbacProvider } from '@/components/layout/RbacGate';
import { getA11yViolations } from '../a11y';

let mockPathname = '/dashboard';

vi.mock('next/navigation', () => ({
  usePathname: () => mockPathname,
}));

vi.mock('next/link', () => ({
  default: ({ href, children, ...rest }: any) => (
    <a href={href} {...rest}>{children}</a>
  ),
}));

function renderSideNav(role: 'SuperAdmin' | 'Operator' | 'ComplianceAuditor' | 'Signatory') {
  return render(
    <RbacProvider role={role} userId="u1" userName="Ada Signer">
      <SideNav />
    </RbacProvider>
  );
}

describe('SideNav', () => {
  it('shows every nav item for a SuperAdmin (has all permissions)', () => {
    renderSideNav('SuperAdmin');

    expect(screen.getByText('Overview')).toBeInTheDocument();
    expect(screen.getByText('Pending Actions')).toBeInTheDocument();
    expect(screen.getByText('Compliance Trail')).toBeInTheDocument();
    expect(screen.getByText('Configuration')).toBeInTheDocument();
  });

  it('hides items the role lacks permission for (Signatory has no compliance:read or config:read)', () => {
    renderSideNav('Signatory');

    expect(screen.getByText('Overview')).toBeInTheDocument();
    expect(screen.getByText('Pending Actions')).toBeInTheDocument();
    expect(screen.queryByText('Compliance Trail')).not.toBeInTheDocument();
    expect(screen.queryByText('Configuration')).not.toBeInTheDocument();
  });

  it('marks the link matching the current pathname as active', () => {
    mockPathname = '/compliance';
    renderSideNav('ComplianceAuditor');

    const complianceLink = screen.getByText('Compliance Trail').closest('a');
    expect(complianceLink).toHaveAttribute('aria-current', 'page');

    const overviewLink = screen.getByText('Overview').closest('a');
    expect(overviewLink).not.toHaveAttribute('aria-current');

    mockPathname = '/dashboard';
  });

  it('renders the user name and role badge in the footer', () => {
    renderSideNav('Operator');

    expect(screen.getByText('Ada Signer')).toBeInTheDocument();
    expect(screen.getByText('Operator')).toBeInTheDocument();
  });

  it('has no detectable accessibility violations', async () => {
    const { container } = renderSideNav('SuperAdmin');
    expect(await getA11yViolations(container)).toHaveLength(0);
  });
});
