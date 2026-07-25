import axe from 'axe-core';

/**
 * Run axe-core against a rendered container and return any violations found.
 * `color-contrast` is disabled — jsdom has no real layout engine or canvas
 * support, so that check can only produce noise, never a meaningful result.
 */
export async function getA11yViolations(container: Element): Promise<axe.Result[]> {
  const results = await axe.run(container, {
    rules: { 'color-contrast': { enabled: false } },
  });
  return results.violations;
}
