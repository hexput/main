#!/usr/bin/env python3
"""Assert the AD-enforcing edges of the workspace crate graph.

The Architecture Spine states several rules of the shape "only X may reach Y
directly". A crate boundary only enforces such a rule once some code actually
imports the crate: a forbidden edge present in a manifest but not yet used
compiles and lints clean. Every crate in this workspace is currently a stub, so
nothing would catch a forbidden edge added today — it would surface much later,
in an unrelated change, reading as pre-existing.

This script closes that gap by asserting the edges directly against the
resolved dependency graph from `cargo metadata`, so the Spine's rules fail CI
rather than a future code review.

Run: python3 scripts/check-crate-graph.py
"""

from __future__ import annotations

import json
import subprocess
import sys

# (crate, forbidden dependency, architecture decision, why it matters)
FORBIDDEN_EDGES = [
    ("hexput-check", "hexput-interpreter", "AD-8", "the check pass must not be able to execute a script"),
    ("hexput-check", "hexput-rpc", "AD-8", "the check pass must not be able to reach the host"),
    ("hexput-check", "hexput-enforce", "AD-8", "the check pass must not perform capability/budget checks"),
    ("hexput-globalvar", "hexput-plugin", "AD-4", "the Global Variable store must outlive the Plugin actor"),
    ("hexput-globalvar", "hexput-rpc", "AD-4", "the Global Variable store must not be reachable through RPC"),
    ("hexput-session", "hexput-config", "AD-5", "per-backend Config and System Config must stay separate surfaces"),
    ("hexput-config", "hexput-session", "AD-5", "per-backend Config and System Config must stay separate surfaces"),
]

# crate -> the exact set of workspace crates allowed to depend on it
SOLE_DEPENDENTS = [
    ("hexput-enforce", {"hexput-exec"}, "AD-3",
     "capability and budget enforcement must be reachable only through the one shared Executor"),
    ("hexput-transport", {"hexput-daemon"}, "AD-1",
     "no crate but the wiring root may branch on a transport type"),
]


def workspace_graph() -> dict[str, set[str]]:
    """Map each workspace crate to its workspace-internal dependencies."""
    raw = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--no-deps"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    meta = json.loads(raw)

    members = {pkg["name"] for pkg in meta["packages"]}
    return {
        pkg["name"]: {d["name"] for d in pkg["dependencies"] if d["name"] in members}
        for pkg in meta["packages"]
    }


def main() -> int:
    graph = workspace_graph()
    failures: list[str] = []

    for crate, forbidden, ad, why in FORBIDDEN_EDGES:
        if crate not in graph:
            failures.append(f"{ad}: crate `{crate}` is missing from the workspace")
        elif forbidden in graph[crate]:
            failures.append(
                f"{ad}: `{crate}` must not depend on `{forbidden}` — {why}"
            )

    for crate, allowed, ad, why in SOLE_DEPENDENTS:
        actual = {c for c, deps in graph.items() if crate in deps}
        if actual != allowed:
            unexpected = actual - allowed
            missing = allowed - actual
            detail = []
            if unexpected:
                detail.append(f"unexpected dependents {sorted(unexpected)}")
            if missing:
                detail.append(f"expected dependents {sorted(missing)} absent")
            failures.append(
                f"{ad}: `{crate}` must be depended on by exactly {sorted(allowed)} — "
                f"{'; '.join(detail)} — {why}"
            )

    if failures:
        print("Crate graph violates the Architecture Spine:\n", file=sys.stderr)
        for f in failures:
            print(f"  - {f}", file=sys.stderr)
        print(
            "\nThese edges are architectural invariants, not style preferences.\n"
            "See the Spine's 'Crate dependency graph' section before changing one.",
            file=sys.stderr,
        )
        return 1

    checked = len(FORBIDDEN_EDGES) + len(SOLE_DEPENDENTS)
    print(f"Crate graph OK — {checked} Architecture Decision edges asserted.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
