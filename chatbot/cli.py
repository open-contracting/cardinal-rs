#!/usr/bin/env python3
"""Tiny CLI for the OCDS chatbot POC.

Ask procurement questions in natural language; the Claude Sonnet agent writes
guarded DuckDB SQL over the sample Parquet and answers, refuses, or asks a
clarifying question. Interactive REPL by default, or one-shot with a question
on the command line.

Needs ANTHROPIC_API_KEY. Examples:
  uv run python chatbot/cli.py
  uv run python chatbot/cli.py "single-bid rate in Rwanda vs the Dominican Republic"
"""

from __future__ import annotations

import os
import sys

import anthropic

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from agent import solve
from query_core import QueryEngine, build_system_prompt

MAX_ROWS = 20
MAX_COL = 40


def _fmt_table(cols, rows):
    if not rows:
        return "  (no rows)"

    def cell(x):
        s = "" if x is None else str(x)
        return s if len(s) <= MAX_COL else s[: MAX_COL - 1] + "…"

    widths = [len(c) for c in cols]
    shown = rows[:MAX_ROWS]
    for r in shown:
        widths = [max(w, len(cell(v))) for w, v in zip(widths, r, strict=True)]
    line = lambda vals: "  " + " | ".join(cell(v).ljust(w) for v, w in zip(vals, widths, strict=True))  # noqa: E731
    out = [line(cols), "  " + "-+-".join("-" * w for w in widths)]
    out += [line(r) for r in shown]
    if len(rows) > MAX_ROWS:
        out.append(f"  ... {len(rows) - MAX_ROWS} more row(s)")
    return "\n".join(out)


def render(result):
    action = result["action"]
    if action == "sql":
        print(f"\n\033[2m{result['sql']}\033[0m")
        print(_fmt_table(result["cols"], result["rows"]))
        note = result.get("message", "")
        if note:
            print(f"\n{note}")
    elif action == "clarify":
        print(f"\n❓ {result['message']}")
    elif action == "blocked":
        print(f"\n⛔ Guardrail blocked the query: {result['message']}")
    else:  # refuse
        print(f"\n⚠️  {result['message']}")
    if timing := result.get("timing"):
        attempts = result.get("attempts", 1)
        print(
            f"\033[2m({timing['total_s']:.1f} s: model {timing['model_s']:.1f} s, SQL {timing['sql_s']:.2f} s; "
            f"{attempts} attempt{'s' if attempts > 1 else ''})\033[0m"
        )
    if usage := result.get("usage"):
        print(
            f"\033[2m(tokens: {usage['input_tokens']:,} in, {usage['output_tokens']:,} out incl. thinking, "
            f"{usage['cache_read_input_tokens']:,} cache read, {usage['cache_creation_input_tokens']:,} cache write)\033[0m"
        )


def ask(question, eng, prompt, client):
    try:
        render(solve(question, eng, prompt, client))
    except anthropic.APIStatusError as e:  # surface API/billing errors without a traceback
        print(f"\n[API error {e.status_code}] {e.message}")


def main():
    if not os.environ.get("ANTHROPIC_API_KEY"):
        sys.exit("ANTHROPIC_API_KEY is not set.")
    eng = QueryEngine()
    prompt = build_system_prompt(eng)
    client = anthropic.Anthropic()

    oneshot = " ".join(sys.argv[1:]).strip()
    if oneshot:
        ask(oneshot, eng, prompt, client)
        return

    print(f"OCDS procurement chatbot (stopgap POC) — datasets: {', '.join(eng.datasets)}.")
    print("Ask a question, or Ctrl-D / 'exit' to quit.\n")
    while True:
        try:
            q = input("> ").strip()
        except EOFError:
            print()
            break
        if q.lower() in ("exit", "quit"):
            break
        if not q:
            continue
        ask(q, eng, prompt, client)
        print()


if __name__ == "__main__":
    main()
