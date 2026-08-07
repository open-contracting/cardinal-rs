#!/usr/bin/env python3
"""Claude text-to-SQL agent for the OCDS chatbot POC.

Turns a natural-language question into either DuckDB SQL (run through the query
core's guardrail), a refusal, or a clarifying question — using Claude Sonnet via
the Anthropic SDK. The generated system prompt (query_core.build_system_prompt)
is "the model's whole world" and is prompt-cached across calls; the model is
forced to answer in a small structured protocol, and a guardrail error is fed
back once so the model can repair its SQL.

Needs ANTHROPIC_API_KEY in the environment. Run `uv run python chatbot/agent.py`
for a quick interactive-style smoke test.
"""

from __future__ import annotations

import json
import os
import sys

import anthropic

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from query_core import GuardrailError, QueryEngine, build_system_prompt

MODEL = "claude-sonnet-5"
MAX_TOKENS = 2048
MAX_ATTEMPTS = 2  # one initial try + one guardrail-repair retry

# The model answers ONLY in this shape (structured outputs guarantees valid JSON).
RESPONSE_SCHEMA = {
    "type": "object",
    "properties": {
        "action": {
            "type": "string",
            "enum": ["sql", "refuse", "clarify"],
            "description": "sql = you can answer with a query; refuse = out of scope/unanswerable; "
            "clarify = ambiguous, ask a question",
        },
        "sql": {"type": "string", "description": 'A single read-only DuckDB SELECT when action=sql, else ""'},
        "message": {
            "type": "string",
            "description": "When action=refuse, the reason (cite the dataset scope/coverage). When "
            "action=clarify, the question to ask. When action=sql, a one-line note. Never empty.",
        },
    },
    "required": ["action", "sql", "message"],
    "additionalProperties": False,
}

PROTOCOL = """

## How to answer
Reply in the structured format. Choose exactly one action:
- "sql": the question is answerable from the tables under the rules above — return one read-only
  DuckDB SELECT in `sql`. Scope by dataset_id. A LIMIT is added automatically.
- "refuse": the question's scope/time/geography is outside a dataset (dataset_meta), the needed
  field is absent (field_coverage), or it asks for something the rules forbid (e.g. a cross-dataset
  monetary total). Put the concrete reason in `message`.
- "clarify": the question is ambiguous (e.g. no country/dataset named and several fit). Ask for the
  missing detail in `message`.
If a query you propose is rejected by the guardrail, you'll get the error and one chance to fix it."""


def _system_blocks(system_prompt: str):
    # One cached block — identical across every question, so it is a stable cache prefix.
    return [{"type": "text", "text": system_prompt + PROTOCOL, "cache_control": {"type": "ephemeral"}}]


def _parse(response):
    if response.stop_reason == "refusal":
        return {"action": "refuse", "sql": "", "message": "model safety refusal"}
    text = next((b.text for b in response.content if b.type == "text"), None)
    if text is None:
        return {"action": "refuse", "sql": "", "message": "no text block in response"}
    return json.loads(text)


def solve(question: str, engine: QueryEngine, system_prompt: str, client: anthropic.Anthropic | None = None):
    """Return a dict: {action, message, sql?, cols?, rows?, attempts, blocked_reason?}."""
    client = client or anthropic.Anthropic()
    system = _system_blocks(system_prompt)
    messages = [{"role": "user", "content": question}]

    for attempt in range(1, MAX_ATTEMPTS + 1):
        response = client.messages.create(
            model=MODEL,
            max_tokens=MAX_TOKENS,
            thinking={"type": "adaptive"},
            output_config={"effort": "medium", "format": {"type": "json_schema", "schema": RESPONSE_SCHEMA}},
            system=system,
            messages=messages,
        )
        out = _parse(response)
        if out["action"] != "sql":
            return {**out, "attempts": attempt}

        try:
            cols, rows, safe = engine.run_sql(out["sql"])
            return {
                "action": "sql",
                "message": out.get("message", ""),
                "sql": safe,
                "cols": cols,
                "rows": rows,
                "attempts": attempt,
            }
        except GuardrailError as e:
            if attempt == MAX_ATTEMPTS:
                return {
                    "action": "blocked",
                    "message": str(e),
                    "sql": out["sql"],
                    "blocked_reason": str(e),
                    "attempts": attempt,
                }
            # Feed the guardrail error back and let the model repair once.
            messages.append({"role": "assistant", "content": response.content})
            messages.append(
                {
                    "role": "user",
                    "content": f"The guardrail rejected that SQL: {e}\nFix it or switch to refuse/clarify.",
                }
            )
    return {"action": "refuse", "message": "exhausted attempts", "attempts": MAX_ATTEMPTS}


def _smoke():
    eng = QueryEngine()
    prompt = build_system_prompt(eng)
    client = anthropic.Anthropic()
    for q in [
        "What is the single-bid rate in Rwanda versus the Dominican Republic?",
        "How much did Rwanda spend on classified defence procurement?",
        "What is the single-bid rate?",
    ]:
        r = solve(q, eng, prompt, client)
        print(f"\nQ: {q}\n -> [{r['action']}] {r.get('message', '')[:120]}")
        if r["action"] == "sql":
            print(f"    SQL: {r['sql'][:160]}")
            print(f"    rows[:3]: {r['rows'][:3]}")


if __name__ == "__main__":
    _smoke()
