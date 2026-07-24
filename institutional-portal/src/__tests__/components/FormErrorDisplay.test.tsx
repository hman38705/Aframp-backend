import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { FormErrorDisplay } from '@/components/ui/FormErrorDisplay';
import { getA11yViolations } from '../a11y';

describe('FormErrorDisplay', () => {
  it('renders the error code and message', () => {
    render(
      <FormErrorDisplay error={{ code: 'INSUFFICIENT_WEIGHT', message: 'Not enough weight.' }} />
    );

    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(screen.getByText('[INSUFFICIENT_WEIGHT]')).toBeInTheDocument();
    expect(screen.getByText('Not enough weight.')).toBeInTheDocument();
  });

  it('renders the hint when provided', () => {
    render(
      <FormErrorDisplay
        error={{ code: 'XDR_MISMATCH', message: 'Mismatch detected.', hint: 'Do not proceed.' }}
      />
    );

    expect(screen.getByText('Do not proceed.')).toBeInTheDocument();
  });

  it('omits the hint paragraph when absent', () => {
    render(<FormErrorDisplay error={{ code: 'UNKNOWN', message: 'Something broke.' }} />);

    expect(screen.queryByText(/hint/i)).not.toBeInTheDocument();
  });

  it('announces the error assertively for screen readers', () => {
    render(<FormErrorDisplay error={{ code: 'BAD_SEQUENCE', message: 'Bad sequence.' }} />);

    const alert = screen.getByRole('alert');
    expect(alert).toHaveAttribute('aria-live', 'assertive');
  });

  it('has no detectable accessibility violations', async () => {
    const { container } = render(
      <FormErrorDisplay error={{ code: 'UNKNOWN', message: 'Something broke.', hint: 'Try again.' }} />
    );

    expect(await getA11yViolations(container)).toHaveLength(0);
  });
});
