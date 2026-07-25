import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ToastProvider, useMultisigToast } from '@/components/ui/Toast';
import { getA11yViolations } from '../a11y';

function TestHarness() {
  const { notifySignatureRequired, notifyThresholdMet, notifyConfirmed, notifyError } = useMultisigToast();
  return (
    <div>
      <button onClick={() => notifySignatureRequired('proposal-abcdef123456', 'mint')}>
        Fire Signature Required
      </button>
      <button onClick={() => notifyThresholdMet('proposal-abcdef123456')}>Fire Threshold Met</button>
      <button onClick={() => notifyConfirmed('deadbeefcafebabe1234567890')}>Fire Confirmed</button>
      <button onClick={() => notifyError('Broadcast Failed', 'Ledger rejected the transaction')}>
        Fire Error
      </button>
    </div>
  );
}

describe('Toast', () => {
  it('throws when useToast is used outside a ToastProvider', () => {
    function Bare() {
      useMultisigToast();
      return null;
    }
    // Suppress the expected React error boundary console noise
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() => render(<Bare />)).toThrow('useToast must be within ToastProvider');
    spy.mockRestore();
  });

  it('renders nothing when there are no toasts', () => {
    render(<ToastProvider><TestHarness /></ToastProvider>);
    expect(screen.queryByRole('region', { name: 'Notifications' })).not.toBeInTheDocument();
  });

  it('adds a sticky (duration 0) toast that is not auto-dismissed', async () => {
    const user = userEvent.setup();
    render(<ToastProvider><TestHarness /></ToastProvider>);

    await user.click(screen.getByText('Fire Signature Required'));

    expect(screen.getByText('Signature Required')).toBeInTheDocument();
    expect(screen.getByText(/Your key weight is needed for a mint operation/)).toBeInTheDocument();
    expect(screen.getByText(/…123456\)/)).toBeInTheDocument();
  });

  it('dismisses a toast when the dismiss button is clicked', async () => {
    const user = userEvent.setup();
    render(<ToastProvider><TestHarness /></ToastProvider>);

    await user.click(screen.getByText('Fire Threshold Met'));
    expect(screen.getByText('Threshold Met')).toBeInTheDocument();

    await user.click(screen.getByLabelText('Dismiss notification'));
    expect(screen.queryByText('Threshold Met')).not.toBeInTheDocument();
  });

  it('shows the most recently added toast first', async () => {
    const user = userEvent.setup();
    render(<ToastProvider><TestHarness /></ToastProvider>);

    await user.click(screen.getByText('Fire Threshold Met'));
    await user.click(screen.getByText('Fire Confirmed'));

    const titles = screen.getAllByRole('alert').map(el => el.querySelector('.toast__title')?.textContent);
    expect(titles).toEqual(['Transaction Confirmed', 'Threshold Met']);
  });

  it('renders a danger variant toast with its message', async () => {
    const user = userEvent.setup();
    render(<ToastProvider><TestHarness /></ToastProvider>);

    await user.click(screen.getByText('Fire Error'));

    expect(screen.getByText('Broadcast Failed')).toBeInTheDocument();
    expect(screen.getByText('Ledger rejected the transaction')).toBeInTheDocument();
  });

  describe('auto-dismiss timing', () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it('auto-dismisses a default-duration toast after 5000ms', async () => {
      render(<ToastProvider><TestHarness /></ToastProvider>);

      act(() => {
        screen.getByText('Fire Threshold Met').click();
      });
      expect(screen.getByText('Threshold Met')).toBeInTheDocument();

      act(() => {
        vi.advanceTimersByTime(5000);
      });

      expect(screen.queryByText('Threshold Met')).not.toBeInTheDocument();
    });

    it('does not auto-dismiss a sticky toast even after a long delay', () => {
      render(<ToastProvider><TestHarness /></ToastProvider>);

      act(() => {
        screen.getByText('Fire Signature Required').click();
      });

      act(() => {
        vi.advanceTimersByTime(60_000);
      });

      expect(screen.getByText('Signature Required')).toBeInTheDocument();
    });
  });

  it('has no detectable accessibility violations', async () => {
    const user = userEvent.setup();
    const { container } = render(<ToastProvider><TestHarness /></ToastProvider>);

    await user.click(screen.getByText('Fire Confirmed'));

    expect(await getA11yViolations(container)).toHaveLength(0);
  });
});
