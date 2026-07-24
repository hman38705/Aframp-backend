# Test snapshots

This directory holds serialized ledger/environment state captured by the
integration test suite (`test_snapshots/tests/*.json`). They are fixtures,
not hand-written expectations — regenerate them rather than editing by hand.

## Updating stale snapshots

If a test's output format changes, its snapshot file(s) will no longer match
and CI will fail the "Check for stale snapshots" step in
`.github/workflows/ci-cd.yml`.

To regenerate snapshots locally:

```bash
UPDATE_SNAPSHOTS=true cargo test --tests --all-features
```

Then review the diff under `test_snapshots/` carefully before committing —
an unexpected diff usually means a regression, not an intentional format
change. Only commit the updated files once you've confirmed the new values
are correct.

## CI enforcement

The `integration-tests` job runs the full test suite and then fails the
build if `test_snapshots/` has any uncommitted changes afterward
(`git diff --exit-code -- test_snapshots/`). This catches snapshots that
drifted from the checked-in fixtures without being intentionally updated
and committed.
