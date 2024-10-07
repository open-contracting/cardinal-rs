#!/usr/bin/env python3
import argparse
import csv
import datetime
import json
import re
from collections import defaultdict
from pathlib import Path

directory = Path(__file__).resolve().parent


def json_to_csv(args):
    fieldnames = ["ocid", "subject", "code", "result", "buyer_id", "procuring_entity_id", "tenderer_id", "created_at"]
    subject_code_to_map_id = {
        "Buyer": {"R038": "ocid_buyer_r038"},
        "ProcuringEntity": {"R038": "ocid_procuringentity_r038"},
        "Tenderer": defaultdict(
            lambda: "ocid_tenderer",  # R025 R038 R048
            R024="ocid_tenderer_r024",
            R028="ocid_tenderer_r028",
            R030="ocid_tenderer_r030",
            R035="ocid_tenderer_r035",
            R058="ocid_tenderer_r058",
        ),
    }
    created_at = datetime.datetime.now(tz=datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    infile = Path(args.infile)
    outfile = Path(args.outfile)
    exists = outfile.exists()

    seen = set()
    if exists:
        with outfile.open() as f:
            for row in csv.DictReader(f, fieldnames=fieldnames):
                seen.add((row["ocid"], row["code"], row["buyer_id"], row["procuring_entity_id"], row["tenderer_id"]))

    with infile.open() as f:
        data = json.load(f)

    identifier_to_ocid = defaultdict(lambda: defaultdict(list))
    # Looks like: {"Maps": {"ocid_tenderer": {"an-ocid": ["a-tenderer-id"]}}}
    for map_id, mapping in data["Maps"].items():
        for ocid, identifiers in mapping.items():
            # ocid_buyer* and ocid_procuringentity* are `str`.
            for identifier in (identifiers if isinstance(identifiers, list) else [identifiers]):
                identifier_to_ocid[map_id][identifier].append(ocid)

    rows = []
    for ocid, results in data.get("OCID", {}).items():
        for code, result in results.items():
            if (ocid, code, "", "", "") not in seen:
                rows.append(
                    {
                        "ocid": ocid,
                        "subject": "OCID",
                        "code": code,
                        "result": result,
                        "created_at": created_at,
                    }
                )

    for subject, index, column in (
        ("Buyer", 2, "buyer_id"),
        ("ProcuringEntity", 3, "procuring_entity_id"),
        ("Tenderer", 4, "tenderer_id"),
    ):
        # Looks like: {"Tenderer": {"a-tenderer-id": {"R038": 0.1}}}
        for identifier, results in data.get(subject, {}).items():
            for code, result in results.items():
                map_id = subject_code_to_map_id[subject][code]
                for ocid in identifier_to_ocid.get(map_id, {}).get(identifier, []):
                    key = [ocid, code, "", "", ""]
                    key[index] = identifier
                    if tuple(key) not in seen:
                        rows.append(
                            {
                                "ocid": ocid,
                                "subject": subject,
                                "code": code,
                                column: identifier,
                                "result": result,
                                "created_at": created_at,
                            }
                        )

    if not args.quiet:
        print(f"Writing {len(rows)} rows")
    with outfile.open("a") as f:
        writer = csv.DictWriter(f, fieldnames=fieldnames, lineterminator="\n")
        if not exists:
            writer.writeheader()
        writer.writerows(rows)


def add_indicator(args):
    """
    Add boilerplate for a new indicator.
    """

    lower = args.code.lower()
    upper = args.code.upper()
    letter, number = upper[0], upper[1:]
    templates = directory / "docs" / "contributing" / "indicators" / "templates"

    for path in (
        directory / "tests" / "fixtures" / "indicators" / f"{upper}.jsonl",
        directory / "tests" / "fixtures" / "indicators" / f"{upper}.expected",
        directory / "src" / "indicators" / f"{lower}.rs",
        directory / "docs" / "cli" / "indicators" / letter / f"{number}.md",
        directory / "docs" / "examples" / letter / f"{number}.jsonl",
    ):
        with (templates / path.suffix[1:]).open() as f:
            content = f.read()
        with path.open("w") as f:
            f.write(content.replace("R999", upper).replace("R/999", f"{letter}/{number}"))

    for path, instructions in (
        (
            directory / "src" / "indicators" / "mod.rs",
            [
                (r"mod [a-z]\d{3}", r"", lower, f"pub mod {lower};\n"),
                (r"struct Settings {", r"^}\n", upper, f"    pub {upper}: Option<Empty>,\n"),
                (r"enum Indicator {", r"^}\n", upper, f"    {upper},\n"),
            ],
        ),
        (
            directory / "src" / "lib.rs",
            [
                (r"^use crate::indicators::[a-z]\d{3}", r"", lower, f"use crate::indicators::{lower}::{upper};\n"),
                (r"add_indicators!", r"\)", upper, f"            {upper},\n"),
            ],
        ),
        (
            directory / "benches" / "main.rs",
            [
                (r"\[[A-Z]\d{3}", r"", upper, f"                    {upper}: Some(Default::default()),\n"),
            ],
        ),
        (
            directory / "docs" / "examples" / "settings.ini",
            [
                (r"\[[A-Z]\d{3}", r"", upper, f"[{upper}]\n"),
            ],
        ),
    ):
        instructions.append(("🦀", r"", upper, ""))

        lines = []
        start, end, word, content = instructions.pop(0)
        started = add = False

        with path.open() as f:
            for line in f:
                if re.search(start, line):
                    started = True

                if started:
                    if match := re.search(r"[A-Za-z]\d{3}", line):
                        add = match.group(0) > word
                    else:
                        add = re.search(end, line) is not None

                if add:
                    lines.append(content)
                    start, end, word, content = instructions.pop(0)
                    started = add = False

                lines.append(line)

        with path.open("w") as f:
            f.write("".join(lines))


def main():
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(required=True)

    parser_json_to_csv = subparsers.add_parser("json-to-csv")
    parser_json_to_csv.add_argument("infile")
    parser_json_to_csv.add_argument("outfile")
    parser_json_to_csv.add_argument("-q", "--quiet", action="store_true")
    parser_json_to_csv.set_defaults(func=json_to_csv)

    parser_add_indicator = subparsers.add_parser("add-indicator")
    parser_add_indicator.add_argument("code")
    parser_add_indicator.set_defaults(func=add_indicator)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
