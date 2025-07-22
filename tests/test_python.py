import json
from pathlib import Path

import ocdscardinal
import pytest

BASEDIR = Path("tests") / "fixtures" / "coverage"


@pytest.mark.parametrize(
    ("infile", "outfile"), zip(sorted(BASEDIR.glob("*.jsonl")), sorted(BASEDIR.glob("*.expected")), strict=True)
)
def test_coverage(infile, outfile):
    with outfile.open() as f:
        expected = json.load(f)

    assert ocdscardinal.coverage(str(infile)) == expected
