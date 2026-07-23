#!/usr/bin/env python
"""Eval harness for the OCDS chatbot POC.

Scores the gold set (gold.json) against the query core. Runs in two modes:

* reference (default, no LLM): validates that the DATA + GUARDRAIL support each
  gold expectation — answerable items' reference_sql returns the expected result,
  guardrail-refusal items are actually blocked, and scope/temporal-refusal items
  are grounded in dataset_meta (the metadata the model will see). This is the
  "de-risk answer quality first" check: it proves the substrate is correct before
  a model is wired.

* model (--model, needs ANTHROPIC_API_KEY): plug the Claude Sonnet text-to-SQL
  loop in as `solve()` and compare its answer/refusal to the gold. Not yet wired
  — see solve_with_model() below. The reference checks still run as a baseline.

Run: uv run python chatbot/eval/run_eval.py
"""

from __future__ import annotations

import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from query_core import GuardrailError, QueryEngine, build_system_prompt

GOLD = os.path.join(os.path.dirname(os.path.abspath(__file__)), "gold.json")


# --- assertions on a (cols, rows) result ------------------------------------
def check_assert(spec, cols, rows):
    kind = spec["type"]
    if kind == "empty":
        return (len(rows) == 0, f"expected no rows, got {len(rows)}")
    if kind == "nonempty":
        return (len(rows) > 0, "expected >=1 row, got 0")
    if kind == "row_count":
        return (len(rows) == spec["value"], f"expected {spec['value']} rows, got {len(rows)}")
    if kind == "scalar":
        got = rows[0][0]
        ok = abs(got - spec["approx"]) <= spec["tol"]
        return (ok, f"expected {spec['approx']}±{spec['tol']}, got {got}")
    if kind == "first_row":
        idx = cols.index(spec["col"])
        got = rows[0][idx]
        return (got == spec["value"], f"first_row.{spec['col']}: expected {spec['value']!r}, got {got!r}")
    if kind == "cells":
        for c in spec["checks"]:
            idx = cols.index(c["col"])
            got = rows[c["row"]][idx]
            if "approx" in c:
                if abs(got - c["approx"]) > c["tol"]:
                    return (False, f"row{c['row']}.{c['col']}: expected {c['approx']}±{c['tol']}, got {got}")
            elif got != c["value"]:
                return (False, f"row{c['row']}.{c['col']}: expected {c['value']!r}, got {got!r}")
        return (True, "all cells ok")
    return (False, f"unknown assertion type {kind!r}")


# --- dataset_meta grounding for model-judged refusals -----------------------
def meta_row(eng, did):
    cols, rows, _ = eng.run_sql(f"SELECT * FROM dataset_meta WHERE dataset_id='{did}'")
    return dict(zip(cols, rows[0], strict=True))


def check_probe(eng, probe):
    """Confirm the refusal is grounded in dataset_meta the model will see."""
    m = meta_row(eng, probe["dataset_id"])
    if probe["check"] == "temporal":
        year = probe["year"]
        lo = int(m["date_from"][:4])
        hi = int(m["date_to"][:4])
        outside = year < lo or year > hi
        return (outside, f"year {year} vs range {lo}..{hi} -> {'out of range' if outside else 'IN range'}")
    if probe["check"] in ("scope_keyword", "threshold_keyword"):
        field = "exclusions" if probe["check"] == "scope_keyword" else "threshold"
        hay = (m.get(field) or "").lower() + " " + (m.get("exclusions") or "").lower()
        hits = [k for k in probe["keywords"] if k.lower() in hay]
        return (bool(hits), f"dataset_meta.{field} grounds refusal via {hits or 'NO KEYWORDS FOUND'}")
    return (False, f"unknown probe {probe['check']!r}")


# --- model mode: score the Claude Sonnet text-to-SQL agent ------------------
def run_model(eng, items):
    import anthropic  # noqa: PLC0415
    from agent import solve  # noqa: PLC0415

    client = anthropic.Anthropic()
    prompt = build_system_prompt(eng)
    results = []
    for it in items:
        cat = it["category"]
        try:
            r = solve(it["question"], eng, prompt, client)
            action = r["action"]
            if cat == "answerable":
                if action != "sql":
                    status, detail = (
                        "FAIL",
                        f"model chose {action!r} ({r.get('message', '')[:60]}) — expected an answer",
                    )
                else:
                    ok, why = check_assert(it["assert"], r["cols"], r["rows"])
                    status = "PASS" if ok else "FAIL"
                    detail = f"answered ({r['attempts']} attempt/s); {why}"
            elif cat in ("refuse_guardrail", "refuse_scope", "refuse_temporal"):
                # A model refusal OR a guardrail block both mean "did not return a wrong answer".
                ok = action in ("refuse", "blocked")
                status = "PASS" if ok else "FAIL"
                detail = f"{action}: {r.get('message', '')[:80]}"
            elif cat == "clarify":
                status = "PASS" if action == "clarify" else "FAIL"
                detail = f"{action}: {r.get('message', '')[:80]}"
            else:
                status, detail = "FAIL", f"unknown category {cat!r}"
        except Exception as e:  # noqa: BLE001
            status, detail = "ERROR", f"{type(e).__name__}: {e}"
        results.append((it["id"], cat, status, detail))
    return results


# --- runner -----------------------------------------------------------------
def run_reference(eng, items):
    results = []
    for it in items:
        cat = it["category"]
        try:
            if cat == "answerable":
                cols, rows, _ = eng.run_sql(it["reference_sql"])
                ok, detail = check_assert(it["assert"], cols, rows)
                status = "PASS" if ok else "FAIL"
            elif cat == "refuse_guardrail":
                try:
                    eng.run_sql(it["bad_sql"])
                    status, detail = "FAIL", "guardrail did NOT block the unsafe query"
                except GuardrailError as e:
                    want = it.get("expected_reason_contains", "")
                    ok = want.lower() in str(e).lower()
                    status = "PASS" if ok else "FAIL"
                    detail = f"blocked; reason {'matches' if ok else 'MISMATCH (want ' + want + ')'}"
            elif cat in ("refuse_scope", "refuse_temporal"):
                ok, detail = check_probe(eng, it["probe"])
                status = "PASS" if ok else "FAIL"
                detail = "grounded: " + detail
            elif cat == "clarify":
                status, detail = "PENDING_MODEL", "needs the LLM to ask a clarifying question"
            else:
                status, detail = "FAIL", f"unknown category {cat!r}"
        except Exception as e:  # noqa: BLE001
            status, detail = "ERROR", f"{type(e).__name__}: {e}"
        results.append((it["id"], cat, status, detail))
    return results


def main():
    import argparse  # noqa: PLC0415

    ap = argparse.ArgumentParser(description="Score the gold set against the query core.")
    ap.add_argument(
        "--model",
        action="store_true",
        help="run the Claude Sonnet text-to-SQL agent (needs ANTHROPIC_API_KEY) instead of reference mode",
    )
    args = ap.parse_args()

    eng = QueryEngine()
    gold = json.load(open(GOLD))
    items = gold["items"]
    prompt = build_system_prompt(eng)
    mode = "model (claude-sonnet-5)" if args.model else "reference"
    print(
        f"Mode: {mode}. System prompt: {len(prompt)} chars (~{len(prompt) // 4} tokens). "
        f"Gold items: {len(items)}. Datasets: {', '.join(eng.datasets)}.\n"
    )

    results = run_model(eng, items) if args.model else run_reference(eng, items)
    width = max(len(r[0]) for r in results)
    counts = {}
    for rid, cat, status, detail in results:
        counts[status] = counts.get(status, 0) + 1
        mark = {"PASS": "✓", "FAIL": "✗", "ERROR": "✗", "PENDING_MODEL": "·"}.get(status, "?")
        print(f"  {mark} {rid:<{width}}  [{cat:<16}] {status:<13} {detail}")

    print("\nsummary:", ", ".join(f"{k}={v}" for k, v in sorted(counts.items())))
    hard_fail = counts.get("FAIL", 0) + counts.get("ERROR", 0)
    if hard_fail:
        print(f"\n{hard_fail} hard failure(s).")
        sys.exit(1)
    if args.model:
        print("\nModel mode passed the gold set.")
    else:
        pending = counts.get("PENDING_MODEL", 0)
        print(
            f"\nReference checks passed. {pending} item(s) await the LLM loop "
            f"(clarify + model-judged refusals are scored in --model mode)."
        )


if __name__ == "__main__":
    main()
