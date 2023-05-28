#!/usr/bin/env python
import re
from pathlib import Path
from textwrap import dedent

import click

directory = Path(__file__).resolve().parent


@click.group()
def cli():
    pass


@cli.command()
@click.argument("code")
def add_indicator(code):
    """
    Add boilerplate for a new indicator.
    """

    lower = code.lower()
    upper = code.upper()
    letter, number = upper[0], upper[1:]
    templates = directory / "docs" / "contributing" / "templates"

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
                (r"struct Settings {", r"^}\n", upper, f"    pub {upper}: Option<HashMap<String, String>>,\n"),
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
            directory / "docs" / "examples" / "settings.ini",
            [
                (r"\[[A-Z]\d{3}", r"", upper, f"[{upper}]\n"),
            ],
        )
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


if __name__ == "__main__":
    cli()
