#!/usr/bin/env bash
#
# Fail when a direct dependency is pinned below the newest version its own
# requirement allows.
#
# This exists because of a specific failure mode that no ordinary CI job sees.
# When the manifest asks for a feature a newer version has dropped, cargo does
# not error — it quietly backtracks to the newest version that still has the
# feature. Everything builds, every test passes, and the lockfile records the
# old version forever. The damage lands on consumers: anyone who needs the
# newer version gets an unresolvable conflict from a crate that looked healthy.
#
# `polymarket-us` 0.5.0 shipped exactly that. It asked reqwest for
# `webpki-roots`, removed in 0.13.4, so the resolver held it at 0.13.1 and no
# build ever complained.
#
# A held-back direct dependency is not always a bug — a deliberate ceiling in
# the manifest is legitimate — but it is always worth a human look, which is
# why this fails rather than warns.

set -euo pipefail

if ! command -v jq >/dev/null 2>&1; then
    echo "error: jq is required" >&2
    exit 2
fi

# `--no-deps` restricts this to the workspace's own manifest, so a transitive
# dependency held back by somebody else's constraint is not our problem.
direct_deps="$(cargo metadata --no-deps --format-version 1 \
    | jq -r '.packages[].dependencies[].name' \
    | sort -u)"

# Reports lines like: `Unchanged reqwest v0.13.1 (available: v0.13.4)`
held_back="$(cargo update --dry-run --verbose 2>&1 \
    | grep -E 'Unchanged .+ \(available: ' || true)"

if [ -z "$held_back" ]; then
    echo "All direct dependencies resolve to the newest version they allow."
    exit 0
fi

findings=""
while IFS= read -r dep; do
    [ -n "$dep" ] || continue
    match="$(printf '%s\n' "$held_back" | grep -E "Unchanged ${dep} v" || true)"
    if [ -n "$match" ]; then
        findings="${findings}${match}"$'\n'
    fi
done <<< "$direct_deps"

if [ -z "$findings" ]; then
    echo "All direct dependencies resolve to the newest version they allow."
    echo "(Some transitive dependencies are held back, which is normal.)"
    exit 0
fi

echo "error: a direct dependency is held back below the newest version it allows:" >&2
printf '%s' "$findings" >&2
cat >&2 <<'MSG'

Two things cause this. Either the manifest requests a feature the newer version
no longer has, or -- since this crate uses the MSRV-aware resolver v3 -- the
newer version needs a rustc above the `rust-version` floor. Cargo backtracks
silently in both cases, so the build stays green while consumers of this crate
get a version conflict.

`cargo update --verbose` names the reason for each held-back package; a line
ending "requires Rust <version>" is the MSRV case, and the fix there is to raise
`rust-version` deliberately, not to chase the dependency.

Reproduce the real error with:

    cargo update -p <name> --precise <newest-version>

If the pin is deliberate, record the ceiling in Cargo.toml (e.g. ">=1.2, <1.5")
with a comment saying why, so the constraint is visible instead of emergent.
MSG
exit 1
