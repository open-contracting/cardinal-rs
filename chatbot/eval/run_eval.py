#!/usr/bin/env python3
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

import datetime
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from query_core import GuardrailError, QueryEngine, build_system_prompt

GOLD = os.path.join(os.path.dirname(os.path.abspath(__file__)), "gold.json")
RUN_LOG = os.path.join(os.path.dirname(os.path.abspath(__file__)), "runs.jsonl")

# List prices in USD per million tokens, from platform.claude.com/docs/en/about-claude/pricing (2026-10-01).
PRICES_PER_MTOK = {
    "claude-sonnet-5": {
        "input_tokens": 2.00,
        "output_tokens": 10.00,
        "cache_read_input_tokens": 0.20,
        "cache_creation_input_tokens": 2.50,
    },
    "claude-sonnet-5-5": {
        "input_tokens": 2.00,
        "output_tokens": 10.00,
        "cache_read_input_tokens": 0.20,
        "cache_creation_input_tokens": 2.50,
    },
}
USAGE_FIELDS = tuple(PRICES_PER_MTOK["claude-sonnet-5"])


def cost_usd(usage, model):
    """Return the list-price cost, or None for a model without a price entry."""
    prices = PRICES_PER_MTOK.get(model)
    return None if prices is None else sum(usage.get(field, 0) * price for field, price in prices.items()) / 1e6


def fmt_cost(cost):
    return "$?" if cost is None else f"${cost:.4f}"


# --- assertions on a (cols, rows) result ------------------------------------
# Column-name- and order-agnostic: the model writes free-form SQL and picks its own
# aliases/orderings, so we assert on VALUES appearing in the result, not on named columns.
def _num(x):
    try:
        return float(x)
    except (TypeError, ValueError):
        return None


def _cell_matches(cell, spec):
    if "approx" in spec:
        n = _num(cell)
        return n is not None and abs(n - spec["approx"]) <= spec.get("tol", 0)
    return str(cell) == str(spec["value"])


def check_assert(spec, rows):
    kind = spec["type"]
    if kind == "empty":
        return (len(rows) == 0, f"expected no rows, got {len(rows)}")
    if kind == "nonempty":
        return (len(rows) > 0, "expected >=1 row, got 0")
    if kind == "row_count":
        return (len(rows) == spec["value"], f"expected {spec['value']} rows, got {len(rows)}")
    if kind == "scalar":
        got = rows[0][0] if rows and rows[0] else None
        n = _num(got)
        ok = n is not None and abs(n - spec["approx"]) <= spec.get("tol", 0)
        return (ok, f"expected {spec['approx']}±{spec.get('tol', 0)}, got {got}")
    if kind == "first_row_has_value":
        if not rows:
            return (False, "expected a first row, got none")
        ok = any(_cell_matches(c, spec) for c in rows[0])
        want = spec.get("value", spec.get("approx"))
        return (ok, f"first row {'has' if ok else 'MISSING'} {want!r}; row={rows[0]}")
    if kind == "values_present":
        flat = [c for r in rows for c in r]
        missing = [
            s.get("value", s.get("approx")) for s in spec["checks"] if not any(_cell_matches(c, s) for c in flat)
        ]
        return (not missing, "all expected values present" if not missing else f"missing {missing}")
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
    if probe["check"] == "scope_keyword":
        hay = (m.get("exclusions") or "").lower()
        hits = [k for k in probe["keywords"] if k.lower() in hay]
        return (bool(hits), f"dataset_meta.exclusions grounds refusal via {hits or 'NO KEYWORDS FOUND'}")
    if probe["check"] == "coverage_absent":
        _, rows, _ = eng.run_sql(
            f"SELECT field_path FROM field_coverage WHERE dataset_id='{probe['dataset_id']}' "
            f"AND field_path LIKE '{probe['field_path_like']}'"
        )
        absent = len(rows) == 0
        return (
            absent,
            f"field_coverage {probe['field_path_like']} in {probe['dataset_id']}: "
            f"{'absent — refusal grounded' if absent else 'PRESENT — not grounded'}",
        )
    if probe["check"] == "coverage_present":
        _, rows, _ = eng.run_sql(
            f"SELECT coverage FROM field_coverage WHERE dataset_id='{probe['dataset_id']}' "
            f"AND field_path = '{probe['field_path']}'"
        )
        present = len(rows) > 0
        return (
            present,
            f"field_coverage {probe['field_path']} in {probe['dataset_id']}: "
            f"{'present — not in the tables' if present else 'ABSENT — nothing to point to'}",
        )
    return (False, f"unknown probe {probe['check']!r}")


# --- model mode: score the Claude Sonnet text-to-SQL agent ------------------
def run_model(eng, items, stats, model):
    """Score each item with the agent, appending its timing and token usage to ``stats``."""
    import anthropic  # noqa: PLC0415
    from agent import solve  # noqa: PLC0415

    client = anthropic.Anthropic()
    prompt = build_system_prompt(eng)
    results = []
    for it in items:
        cat = it["category"]
        try:
            r = solve(it["question"], eng, prompt, client, model=model)
            stats[it["id"]] = {"timing": r["timing"], "usage": r["usage"]}
            action = r["action"]
            if cat == "answerable":
                if action == "sql":
                    ok, why = check_assert(it["assert"], r["rows"])
                    # An answer that needs a caveat must mention at least one of these terms.
                    if terms := it.get("expected_message_any"):
                        message = (r.get("message") or "").lower()
                        if not any(t.lower() in message for t in terms):
                            ok, why = False, f"{why}; message lacks any of {terms}"
                    status = "PASS" if ok else "FAIL"
                    detail = f"answered ({r['attempts']} attempt/s); {why}"
                elif it.get("accept_refusal") and action in ("refuse", "blocked"):
                    # e.g. "does dataset X publish Y?" — answering "no, not published" is correct.
                    status, detail = "PASS", f"acceptable {action}: {r.get('message', '')[:70]}"
                else:
                    status, detail = (
                        "FAIL",
                        f"model chose {action!r} ({r.get('message', '')[:60]}) — expected an answer",
                    )
            elif cat in ("refuse_guardrail", "refuse_scope", "refuse_temporal"):
                # A model refusal OR a guardrail block both mean "did not return a wrong answer".
                ok = action in ("refuse", "blocked")
                status = "PASS" if ok else "FAIL"
                detail = f"{action}: {r.get('message', '')[:80]}"
            elif cat == "refuse_in_source":
                # The refusal must also point to the source publication.
                message = r.get("message") or ""
                missing = [w for w in it["expected_message_contains"] if w.lower() not in message.lower()]
                ok = action == "refuse" and not missing
                status = "PASS" if ok else "FAIL"
                detail = f"{action}{f' (missing {missing})' if missing else ''}: {message[:80]}"
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
                _, rows, _ = eng.run_sql(it["reference_sql"])
                ok, detail = check_assert(it["assert"], rows)
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
            elif cat in ("refuse_scope", "refuse_temporal", "refuse_in_source"):
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


def report_cost(stats, counts, failed, model):
    """Print run totals for time, tokens and cost, and append them to RUN_LOG."""
    usage = {field: sum(s["usage"][field] for s in stats.values()) for field in USAGE_FIELDS}
    total_s = sum(s["timing"]["total_s"] for s in stats.values())
    model_s = sum(s["timing"]["model_s"] for s in stats.values())
    cost = cost_usd(usage, model)
    print(f"time: {total_s:.0f}s total ({model_s:.0f}s in the model), {total_s / len(stats):.1f}s per question")
    print(
        f"tokens: {usage['input_tokens']:,} in, {usage['output_tokens']:,} out incl. thinking, "
        f"{usage['cache_read_input_tokens']:,} cache read, {usage['cache_creation_input_tokens']:,} cache write"
    )
    if cost is None:
        print(f"cost: no price entry for {model}")
    else:
        print(f"cost: ${cost:.3f} total, ${cost / len(stats):.4f} per question ({model} list prices)")
    with open(RUN_LOG, "a") as f:
        f.write(
            json.dumps(
                {
                    "at": datetime.datetime.now(datetime.UTC).isoformat(timespec="seconds"),
                    "items": len(stats),
                    "counts": counts,
                    "failed": failed,
                    "total_s": round(total_s, 1),
                    "usage": usage,
                    "model": model,
                    "cost_usd": None if cost is None else round(cost, 4),
                }
            )
            + "\n"
        )
    print(f"(appended to {os.path.relpath(RUN_LOG)})")


def agent_model_default():
    from agent import MODEL  # noqa: PLC0415

    return MODEL


def main():
    import argparse  # noqa: PLC0415

    ap = argparse.ArgumentParser(description="Score the gold set against the query core.")
    ap.add_argument(
        "--model",
        action="store_true",
        help="run the Claude Sonnet text-to-SQL agent (needs ANTHROPIC_API_KEY) instead of reference mode",
    )
    ap.add_argument("--agent-model", default=agent_model_default(), help="model for --model (default: %(default)s)")
    args = ap.parse_args()

    eng = QueryEngine()
    gold = json.load(open(GOLD))
    items = gold["items"]
    prompt = build_system_prompt(eng)
    mode = f"model ({args.agent_model})" if args.model else "reference"
    print(
        f"Mode: {mode}. System prompt: {len(prompt)} chars (~{len(prompt) // 4} tokens). "
        f"Gold items: {len(items)}. Datasets: {', '.join(eng.datasets)}.\n"
    )

    stats = {}
    results = run_model(eng, items, stats, args.agent_model) if args.model else run_reference(eng, items)
    width = max(len(r[0]) for r in results)
    counts = {}
    for rid, cat, status, detail in results:
        counts[status] = counts.get(status, 0) + 1
        mark = {"PASS": "✓", "FAIL": "✗", "ERROR": "✗", "PENDING_MODEL": "·"}.get(status, "?")
        cost = ""
        if rid in stats:
            s = stats[rid]
            cost = f"{s['timing']['total_s']:5.1f}s {fmt_cost(cost_usd(s['usage'], args.agent_model))}  "
        print(f"  {mark} {rid:<{width}}  [{cat:<16}] {status:<13} {cost}{detail}")

    print("\nsummary:", ", ".join(f"{k}={v}" for k, v in sorted(counts.items())))
    if stats:
        report_cost(
            stats, counts, [rid for rid, _, status, _ in results if status in ("FAIL", "ERROR")], args.agent_model
        )
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
