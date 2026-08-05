#!/usr/bin/env python3
"""
Enumerate every array path defined by OCDS 1.1 core + all registered extensions,
in Cardinal `coverage` path format (e.g. /awards[]/items[]/additionalClassifications[]).

Reads data/core-release-schema.json + data/ext_cache/*.json (run download.py, then
fetch_ext_schemas.py, first). Writes data/schema_arrays.json: {path: {"source": "core"|"extension"}}.
"""

import glob
import json
import os
from pathlib import Path

DATA = Path(__file__).parent / "data"


def merge_patch(target, patch):
    """RFC 7396 JSON Merge Patch (OCDS extension release-schema.json semantics)."""
    if not isinstance(patch, dict):
        return patch
    if not isinstance(target, dict):
        target = {}
    for k, v in patch.items():
        if v is None:
            target.pop(k, None)
        elif isinstance(v, dict):
            target[k] = merge_patch(target.get(k), v)
        else:
            target[k] = v
    return target


def load_ext_patches():
    patches = {}
    for f in sorted(glob.glob(str(DATA / "ext_cache" / "*.json"))):
        eid = os.path.splitext(os.path.basename(f))[0]
        patches[eid] = json.load(open(f))
    return patches


def types_of(node):
    t = node.get("type")
    if t is None:
        return set()
    return set(t) if isinstance(t, list) else {t}


def enumerate_arrays(schema):
    """
    Return set of array paths (coverage format). Resolves local $refs; guards recursion
    by the set of definition names already on the current branch.
    """
    defs = schema.get("definitions", {})
    arrays = set()

    def resolve(node):
        if isinstance(node, dict) and "$ref" in node:
            ref = node["$ref"]
            if ref.startswith("#/definitions/"):
                name = ref.split("/")[-1]
                return defs.get(name, {}), name
            return {}, None  # external ref: treat as opaque
        return node, None

    def walk(node, prefix, stack):
        if len(prefix) > 200:  # runaway guard
            return
        node, _ = resolve(node)
        if not isinstance(node, dict):
            return
        branches = [node] + [s for kw in ("oneOf", "anyOf", "allOf") for s in node.get(kw, [])]
        for b in branches:
            b, _ = resolve(b)
            if not isinstance(b, dict):
                continue
            ts = types_of(b)
            if "array" in ts and "items" in b:
                apath = prefix + "[]"
                arrays.add(apath)
                items = b["items"]
                _, iref = resolve(items)
                if iref and iref in stack:
                    continue
                walk(items, apath, stack | ({iref} if iref else set()))
            if "object" in ts or "properties" in b:
                for pname, pschema in b.get("properties", {}).items():
                    _, pref = resolve(pschema)
                    if pref and pref in stack:
                        continue
                    walk(pschema, prefix + "/" + pname, stack | ({pref} if pref else set()))

    walk(schema, "", set())
    return arrays


def main():
    core = json.load(open(DATA / "core-release-schema.json"))
    core_arrays = enumerate_arrays(core)

    merged = json.loads(json.dumps(core))
    for patch in load_ext_patches().values():
        merged = merge_patch(merged, patch)
    all_arrays = enumerate_arrays(merged)

    result = {p: {"source": "core" if p in core_arrays else "extension"} for p in sorted(all_arrays)}
    json.dump(result, open(DATA / "schema_arrays.json", "w"), indent=0)

    print(f"total schema array paths (core+ext): {len(result)}")
    print(f"  core:           {len(core_arrays)}")
    print(f"  extension-only: {len(all_arrays - core_arrays)}")


if __name__ == "__main__":
    main()
