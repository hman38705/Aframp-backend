import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { InstitutionalLayout } from '@/components/layout/InstitutionalLayout';

vi.mock('next/navigation', () => ({
  usePathname: () => '/dashboard',
}));

vi.mock('next/link', () => ({
  default: ({ href, children, ...rest }: any) => (
    <a href={href} {...rest}>{children}</a>
  ),
}));

describe('InstitutionalLayout', () => {
  it('renders the side nav and the page content', () => {
    render(
      <InstitutionalLayout role="SuperAdmin" userId="u1" userName="Ada Signer">
        <p>Page content</p>
      </InstitutionalLayout>
    );

    expect(screen.getByText('◈ Aframp')).toBeInTheDocument();
    expect(screen.getByText('Page content')).toBeInTheDocument();
  });

  it('scopes RBAC to the provided role for descendants', () => {
    render(
      <InstitutionalLayout role="Signatory" userId="u1" userName="Ada Signer">
        <p>Page content</p>
      </InstitutionalLayout>
    );

    // Signatory lacks compliance:read / config:read
    expect(screen.queryByText('Compliance Trail')).not.toBeInTheDocument();
    expect(screen.queryByText('Configuration')).not.toBeInTheDocument();
  });

  it('renders the main content landmark with the expected id', () => {
    render(
      <InstitutionalLayout role="Operator" userId="u1" userName="Ada Signer">
        <p>Page content</p>
      </InstitutionalLayout>
    );

    const main = screen.getByText('Page content').closest('main');
    expect(main).toHaveAttribute('id', 'main-content');
  });
});
