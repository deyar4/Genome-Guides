"""Toy-FASTA checks of the seqkit/bedtools shell steps behind assembly_gaps and homopolymer_genomic_tracts.

seqkit locate with a regex reports every suffix of a run as its own hit; the rules merge them into maximal runs. The shell
text below mirrors the rules (test_rule_text_still_merges guards that they still contain the merge).
"""
import re
import shutil
import subprocess
from pathlib import Path

import pytest

pytestmark = pytest.mark.skipif(
    shutil.which("seqkit") is None or shutil.which("bedtools") is None, reason="seqkit/bedtools not on PATH")

RULES = Path(__file__).resolve().parents[2] / "workflow" / "rules"


def sh(cmd: str, cwd: Path) -> list[tuple]:
    out = subprocess.run(["bash", "-euo", "pipefail", "-c", cmd], cwd=cwd, capture_output=True, text=True, check=True).stdout
    return [tuple(int(x) if x.isdigit() else x for x in line.split("\t")) for line in out.splitlines()]


def test_gaps_are_maximal_half_open_runs(tmp_path):
    # N at 2-4 (0-based), n at 6-8, contig d starts with N (0-1); lowercase counts, other IUPAC codes (R) do not
    (tmp_path / "t.fa").write_text(">c\nACNNNAnnnGGR\n>d\nNNAC\n")
    rows = sh("""seqkit locate -i -r -P -p '"N+"' --bed t.fa | bedtools merge -i - | awk 'BEGIN{OFS="\\t"} {print $1, $2, $3, "N", 0, "+"}'""",
              tmp_path)
    assert [(r[0], r[1], r[2]) for r in rows] == [("c", 2, 5), ("c", 6, 9), ("d", 0, 2)]


def test_homopolymers_ignore_case_break_on_n_and_stop_at_contigs(tmp_path):
    # contig c: 12 G + 3 g = one tract [9, 24); 9 A (too short); 10 C then N then 10 C = two tracts. Contig d: T x 10.
    (tmp_path / "t.fa").write_text(">c\nTTTTTTTTT" "GGGGGGGGGGGGggg" "AAAAAAAAA" "C" "CCCCCCCCC" "N" "CCCCCCCCCC\n>d\nTTTTTTTTTT\n")
    rows = sh("""for b in G A C T; do seqkit locate -i -r -P -p '"['$b']{10,}"' --bed t.fa | bedtools merge -i - | awk -v b="$b" '{print $1 "\\t" $2 "\\t" $3 "\\t" b}'; done""",
              tmp_path)
    got = sorted((r[0], r[1], r[2], r[3]) for r in rows)
    assert ("c", 9, 24, "G") in got
    assert not [r for r in got if r[3] == "A"]
    assert sum(1 for r in got if r[3] == "C") == 2
    assert ("d", 0, 10, "T") in got


def test_rule_text_still_merges():
    text = (RULES / "trivial_cli.smk").read_text()
    block = text[text.index("rule extract_assembly_gaps"):text.index("rule convert_gc_skew_to_parquet")]
    assert "bedtools merge" in block
    assert re.search(r"bedtools merge", (RULES / "dna.smk").read_text())
