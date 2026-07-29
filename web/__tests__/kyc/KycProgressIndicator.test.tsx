/**
 * Unit tests for KycProgressIndicator — task #481 step 8
 *
 * Tests cover:
 * - Renders all step titles
 * - Current step bubble has aria-current="step"
 * - Status chip text for each KycOnboardingStatus variant
 * - Completed steps show checkmark (SVG accessible aria-label or text)
 * - Upcoming steps are not marked current
 * - Role and aria-label of the nav wrapper
 */

import { describe, it, expect } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import { KycProgressIndicator } from '@/components/kyc/KycProgressIndicator';
import { KycOnboardingStatus } from '@/lib/kyc/types';

// ---------------------------------------------------------------------------
// Shared test data
// ---------------------------------------------------------------------------

const THREE_STEPS = [
  { id: 'personal-info', title: 'Personal Information' },
  { id: 'identity-verification', title: 'Identity Verification' },
  { id: 'documents', title: 'Document Upload' },
];

const TWO_STEPS = [
  { id: 'step-1', title: 'Step One' },
  { id: 'step-2', title: 'Step Two' },
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('KycProgressIndicator', () => {
  // ── Rendering / step titles ──────────────────────────────────────────

  describe('renders all step titles', () => {
    it('renders all three step titles', () => {
      render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={0}
          status={KycOnboardingStatus.InProgress}
        />,
      );

      expect(screen.getByText('Personal Information')).toBeInTheDocument();
      expect(screen.getByText('Identity Verification')).toBeInTheDocument();
      expect(screen.getByText('Document Upload')).toBeInTheDocument();
    });

    it('renders two step titles when only two steps are provided', () => {
      render(
        <KycProgressIndicator
          steps={TWO_STEPS}
          currentStep={0}
          status={KycOnboardingStatus.Idle}
        />,
      );

      expect(screen.getByText('Step One')).toBeInTheDocument();
      expect(screen.getByText('Step Two')).toBeInTheDocument();
    });
  });

  // ── Navigation role & aria-label ─────────────────────────────────────

  describe('nav wrapper accessibility', () => {
    it('renders a nav element with role="navigation"', () => {
      render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={0}
          status={KycOnboardingStatus.Idle}
        />,
      );

      expect(screen.getByRole('navigation')).toBeInTheDocument();
    });

    it('nav has aria-label="Onboarding progress"', () => {
      render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={0}
          status={KycOnboardingStatus.Idle}
        />,
      );

      const nav = screen.getByRole('navigation');
      expect(nav.getAttribute('aria-label')).toBe('Onboarding progress');
    });
  });

  // ── aria-current="step" on active bubble ─────────────────────────────

  describe('aria-current="step" on active step', () => {
    it('marks step 0 as current when currentStep=0', () => {
      render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={0}
          status={KycOnboardingStatus.InProgress}
        />,
      );

      const currentBubble = screen.getByRole('listitem', {
        // aria-label includes "current step"
        name: /personal information — current step/i,
      });
      expect(currentBubble.getAttribute('aria-current')).toBe('step');
    });

    it('marks step 1 as current when currentStep=1', () => {
      render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={1}
          status={KycOnboardingStatus.InProgress}
        />,
      );

      const currentBubble = screen.getByRole('listitem', {
        name: /identity verification — current step/i,
      });
      expect(currentBubble.getAttribute('aria-current')).toBe('step');
    });

    it('marks step 2 as current when currentStep=2 (last step)', () => {
      render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={2}
          status={KycOnboardingStatus.InProgress}
        />,
      );

      const currentBubble = screen.getByRole('listitem', {
        name: /document upload — current step/i,
      });
      expect(currentBubble.getAttribute('aria-current')).toBe('step');
    });

    it('does NOT mark non-active steps as aria-current', () => {
      render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={1}
          status={KycOnboardingStatus.InProgress}
        />,
      );

      const completedBubble = screen.getByRole('listitem', {
        name: /personal information — completed/i,
      });
      expect(completedBubble.getAttribute('aria-current')).toBeNull();

      const upcomingBubble = screen.getByRole('listitem', {
        name: /document upload — not started/i,
      });
      expect(upcomingBubble.getAttribute('aria-current')).toBeNull();
    });
  });

  // ── Completed steps show checkmark ───────────────────────────────────

  describe('completed steps show checkmark', () => {
    it('completed step bubble has aria-label containing "completed"', () => {
      render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={2}
          status={KycOnboardingStatus.InProgress}
        />,
      );

      // Both step 0 and step 1 are completed
      const completedStep0 = screen.getByRole('listitem', {
        name: /personal information — completed/i,
      });
      expect(completedStep0).toBeInTheDocument();

      const completedStep1 = screen.getByRole('listitem', {
        name: /identity verification — completed/i,
      });
      expect(completedStep1).toBeInTheDocument();
    });

    it('completed step bubble contains an SVG (checkmark icon)', () => {
      render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={1}
          status={KycOnboardingStatus.InProgress}
        />,
      );

      const completedBubble = screen.getByRole('listitem', {
        name: /personal information — completed/i,
      });

      // The SVG checkmark should be inside the completed bubble
      const svg = completedBubble.querySelector('svg');
      expect(svg).not.toBeNull();
    });

    it('active step bubble does NOT contain an SVG (shows number instead)', () => {
      render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={0}
          status={KycOnboardingStatus.InProgress}
        />,
      );

      const activeBubble = screen.getByRole('listitem', {
        name: /personal information — current step/i,
      });

      const svg = activeBubble.querySelector('svg');
      expect(svg).toBeNull();

      // Should show step number "1" instead
      expect(activeBubble).toHaveTextContent('1');
    });
  });

  // ── Status label chip ─────────────────────────────────────────────────

  describe('status label rendering', () => {
    it('renders no status chip when status is Idle', () => {
      const { container } = render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={0}
          status={KycOnboardingStatus.Idle}
        />,
      );

      // STATUS_LABELS[Idle] = '' — so no chip element should be rendered
      const liveRegion = container.querySelector('[aria-live="polite"]');
      expect(liveRegion).toBeNull();
    });

    it('renders "In Progress…" chip for InProgress status', () => {
      render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={0}
          status={KycOnboardingStatus.InProgress}
        />,
      );

      expect(screen.getByText('In Progress…')).toBeInTheDocument();
    });

    it('renders "Uploading Documents…" chip for SubmittedPendingReview status', () => {
      render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={2}
          status={KycOnboardingStatus.SubmittedPendingReview}
        />,
      );

      expect(screen.getByText('Uploading Documents…')).toBeInTheDocument();
    });

    it('renders "Under Compliance Review" chip for ManualReview status', () => {
      render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={2}
          status={KycOnboardingStatus.ManualReview}
        />,
      );

      expect(screen.getByText('Under Compliance Review')).toBeInTheDocument();
    });

    it('renders "Approved" chip for Approved status', () => {
      render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={2}
          status={KycOnboardingStatus.Approved}
        />,
      );

      expect(screen.getByText('Approved')).toBeInTheDocument();
    });

    it('renders "Rejected" chip for Rejected status', () => {
      render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={2}
          status={KycOnboardingStatus.Rejected}
        />,
      );

      expect(screen.getByText('Rejected')).toBeInTheDocument();
    });

    it('status chip is inside an aria-live="polite" region', () => {
      const { container } = render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={0}
          status={KycOnboardingStatus.InProgress}
        />,
      );

      const liveRegion = container.querySelector('[aria-live="polite"]');
      expect(liveRegion).not.toBeNull();
      expect(within(liveRegion as HTMLElement).getByText('In Progress…')).toBeInTheDocument();
    });
  });

  // ── Connector lines ───────────────────────────────────────────────────

  describe('connector lines', () => {
    it('renders connector lines between steps (step count - 1 connectors)', () => {
      const { container } = render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={0}
          status={KycOnboardingStatus.Idle}
        />,
      );

      // Connector lines have aria-hidden="true" and class containing "h-0.5"
      const connectors = container.querySelectorAll('[aria-hidden="true"].h-0\\.5, [aria-hidden][class*="h-0.5"]');
      // 3 steps → 2 connectors
      // We use a more flexible selector since jsdom handles class escaping differently
      const allAriaHidden = Array.from(container.querySelectorAll('[aria-hidden="true"]')).filter(
        (el) => (el as HTMLElement).className.includes('h-0.5'),
      );
      expect(allAriaHidden.length).toBe(TWO_STEPS.length - 1 + 1); // 2 connectors for 3-step config
    });
  });

  // ── Step number rendering ─────────────────────────────────────────────

  describe('step number rendering', () => {
    it('shows step number 1 in the first bubble when step 0 is active', () => {
      render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={0}
          status={KycOnboardingStatus.InProgress}
        />,
      );

      const activeBubble = screen.getByRole('listitem', {
        name: /personal information — current step/i,
      });
      expect(activeBubble).toHaveTextContent('1');
    });

    it('shows step number 2 in the second bubble when step 1 is active', () => {
      render(
        <KycProgressIndicator
          steps={THREE_STEPS}
          currentStep={1}
          status={KycOnboardingStatus.InProgress}
        />,
      );

      const activeBubble = screen.getByRole('listitem', {
        name: /identity verification — current step/i,
      });
      expect(activeBubble).toHaveTextContent('2');
    });
  });
});
