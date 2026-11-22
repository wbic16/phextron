#!/usr/bin/env python3
"""Phextron web UI (WASM) stage-1 browser verification.

Loads /web in a real Chromium, in light and dark, and asserts the things
stage 1 exists to prove:
  - the WASM module instantiated and rendered the panel (the glue marks
    <html data-renderer="wasm"> and <main data-rendered-by> only after the
    module returns HTML),
  - the rendered table's row count equals the live /api/panels/nodes row
    count the desktop shell reads (same data, same server),
  - the distilled banner from state.phext is on the page,
  - theme follows the emulated OS scheme by default, and the toggle's
    explicit choice persists to localStorage,
  - zero console errors, page errors, or failed requests.

Run the console first with live sources + PHEXTRON_WEB_DIR set, e.g.:
  PHEXTRON_PORT=8917 PHEXTRON_NODES_JSON=... PHEXTRON_LAST_PULL=... \
  PHEXTRON_STATE_PHEXT=... PHEXTRON_WEB_DIR=crates/phextron-web/dist phextron
Then:  python3 verify_browser.py [base-url] [chromium-path]
Screenshots land in docs/ beside this script.
"""
import json
import os
import sys
import urllib.request

from playwright.sync_api import sync_playwright

BASE = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:8917"
# Optional: a Chromium executable path. Default is Playwright's bundled
# Chromium — a stock headless shell, which is what stage 1 verified with.
CHROME = sys.argv[2] if len(sys.argv) > 2 else None
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "docs")
os.makedirs(OUT, exist_ok=True)

with urllib.request.urlopen(BASE + "/api/panels/nodes") as r:
    api = json.load(r)
api_rows = len(api["body"]["rows"])
api_state = api["state"]
print(f"api: state={api_state} rows={api_rows}")

failures = []
for scheme in ("light", "dark"):
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path=CHROME) if CHROME else p.chromium.launch()
        ctx = browser.new_context(color_scheme=scheme, viewport={"width": 1280, "height": 900})
        page = ctx.new_page()
        errors = []
        page.on("console", lambda m: errors.append(f"console.{m.type}: {m.text}") if m.type in ("error", "warning") else None)
        page.on("pageerror", lambda e: errors.append(f"pageerror: {e}"))
        page.on("requestfailed", lambda r: errors.append(f"requestfailed: {r.url} {r.failure}"))
        page.goto(BASE + "/web", wait_until="networkidle")
        page.wait_for_selector("main table tbody tr", timeout=15000)

        renderer = page.evaluate("document.documentElement.dataset.renderer")
        rendered_by = page.evaluate("document.getElementById('panel').dataset.renderedBy")
        row_count = page.locator("main table tbody tr").count()
        body_text = page.inner_text("main")
        theme = page.evaluate("document.documentElement.dataset.theme")
        page.screenshot(path=f"{OUT}/web-nodes-{scheme}.png", full_page=True)

        # Theme toggle: explicit choice persists and applies.
        page.click("#themeBtn")
        stored = page.evaluate("localStorage.getItem('phextron.theme')")
        theme_after = page.evaluate("document.documentElement.dataset.theme")
        browser.close()

    print(f"[{scheme}] renderer={renderer} by={rendered_by} rows={row_count} theme={theme} "
          f"stored={stored} theme_after_toggle={theme_after} errors={errors}")
    if renderer != "wasm" or rendered_by != "phextron-web-wasm":
        failures.append(f"{scheme}: panel was not rendered by the WASM module")
    if row_count != api_rows:
        failures.append(f"{scheme}: expected {api_rows} node rows (live API), got {row_count}")
    if "nodes up" not in body_text:
        failures.append(f"{scheme}: distilled summary missing from panel text")
    if theme != scheme:
        failures.append(f"{scheme}: default theme {theme} does not follow OS scheme {scheme}")
    if not stored or theme_after not in ("light", "dark"):
        failures.append(f"{scheme}: theme toggle did not persist/apply (stored={stored}, after={theme_after})")
    real_errors = [e for e in errors if "favicon" not in e.lower()]
    if real_errors:
        failures.append(f"{scheme}: console/page errors: {real_errors}")

if failures:
    print("FAILURES:")
    for f in failures:
        print(" -", f)
    sys.exit(1)
print("BROWSER VERIFICATION OK")
