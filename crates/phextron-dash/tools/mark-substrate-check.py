#!/usr/bin/env python3
"""Record a substrate token-budget check in substrate-checks.phext.

The Phextron Substrates panel (§1.10, task 31.8.11/9.7.13/8.7.10) reads
its budget-checked signal from a recorded event, never a guess: this
script is the write path. The console itself is read-only today (no
console actions exist yet), so a steering pass — or Will — records a
check by running this script; the panel picks the scroll up on its
next read.

The register lives with the other phext registers the console reads:
~/workspace/phext/substrate-checks.phext on the Muse VM, synced to
/home/wbic16/phextron-data/phext-vm/substrate-checks.phext on
aletheia-core, where the panel's default path points.

Layout (house register pattern — Home scroll first, one scroll per
event after it, coordinates positional by scroll order):

  Home      1.1.1/1.1.1/1.1.1  what this register is
  checks    1.1.1/1.1.1/1.1.N  one per recorded budget check

A check scroll:

  # Budget check — Claude

  Substrate: claude
  Checked at (unix): 1795324800
  Checked at: 2026-11-21 23:20 CST
  Recorded by: emise — steering pass
  Note: <optional free text>

The panel keys on the `Substrate:` + `Checked at (unix):` field lines;
the Home scroll carries neither, so it never parses as a check.

Usage:
  mark-substrate-check.py <substrate> [--by WHO] [--note TEXT]
                          [--file PATH] [--at UNIX]
  mark-substrate-check.py --init [--file PATH]

Substrates: claude, grok, muse, chatgpt, google, local.
--at exists for tests and backfills of checks that were recorded
elsewhere at a known time; a normal mark uses the current time.
"""

import argparse
import sys
import time
from datetime import datetime
from pathlib import Path
from zoneinfo import ZoneInfo

DELIM = "\x17"  # phext scroll delimiter (ETB), as in every house register
CHICAGO = ZoneInfo("America/Chicago")
DEFAULT_FILE = Path.home() / "workspace" / "phext" / "substrate-checks.phext"

SUBSTRATES = {
    "claude": "Claude",
    "grok": "Grok",
    "muse": "Muse",
    "chatgpt": "ChatGPT",
    "google": "Google/Antigravity",
    "local": "Local",
}

HOME = """# substrate-checks.phext — the substrate budget-check register

One scroll per recorded token-budget check, appended by
mark-substrate-check.py (crates/phextron-dash/tools/ in wbic16/phextron)
whenever a steering pass — or Will — checks a substrate's token budget.
The Phextron Substrates panel (task 31.8.11/9.7.13/8.7.10, W4 plan
section 1.10) reads this register live with libphext and derives each
substrate's budget state from the checks that fall inside its current
reset window; a check recorded in a previous window never carries
over. This Home scroll is documentation, not a check — a check scroll
is the one carrying the two field lines the panel keys on (the
substrate slug line and the unix timestamp line).

Reset windows (America/Chicago, standing constants from the
substrate-steering record): Claude Wednesdays 22:00, Grok Thursdays
20:23, Muse Fridays 16:10; Google/Antigravity rolls; ChatGPT is a
context reservoir; Local is unmetered."""


def append_scroll(path: Path, scroll: str) -> int:
    """Append one scroll; returns its 1-based scroll index (so the
    coordinate is 1.1.1/1.1.1/1.1.<index>)."""
    existing = path.read_bytes().decode("utf-8") if path.exists() else ""
    if not existing:
        path.write_text(scroll, encoding="utf-8")
        return 1
    count = existing.count(DELIM) + 1
    with path.open("a", encoding="utf-8") as f:
        if not existing.endswith(DELIM):
            f.write(DELIM)
        f.write(scroll)
    return count + 1


def init_register(path: Path) -> None:
    if path.exists() and path.read_bytes():
        print(f"register already present: {path}")
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(HOME, encoding="utf-8")
    print(f"initialized {path} (Home scroll at 1.1.1/1.1.1/1.1.1)")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("substrate", nargs="?", choices=sorted(SUBSTRATES),
                    help="substrate whose budget was checked")
    ap.add_argument("--by", default="emise", help="who/what recorded the check")
    ap.add_argument("--note", default="", help="optional free-text note")
    ap.add_argument("--file", type=Path, default=DEFAULT_FILE,
                    help=f"register file (default {DEFAULT_FILE})")
    ap.add_argument("--at", type=int, default=None,
                    help="unix timestamp of the check (default: now)")
    ap.add_argument("--init", action="store_true",
                    help="create the register with its Home scroll if missing")
    args = ap.parse_args()

    if args.init:
        init_register(args.file)
        return 0
    if not args.substrate:
        ap.error("a substrate is required (or use --init)")

    if not args.file.exists() or not args.file.read_bytes():
        init_register(args.file)

    ts = args.at if args.at is not None else int(time.time())
    local = datetime.fromtimestamp(ts, CHICAGO)
    human = local.strftime("%Y-%m-%d %H:%M %Z")
    lines = [
        f"# Budget check — {SUBSTRATES[args.substrate]}",
        "",
        f"Substrate: {args.substrate}",
        f"Checked at (unix): {ts}",
        f"Checked at: {human}",
        f"Recorded by: {args.by}",
    ]
    if args.note:
        lines.append(f"Note: {args.note}")
    index = append_scroll(args.file, "\n".join(lines))
    print(f"recorded {args.substrate} budget check at 1.1.1/1.1.1/1.1.{index} "
          f"({human}) in {args.file}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
