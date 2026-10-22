"""Genes sharing a gene_id on different contigs (PAR copies) must stay separate records."""
import subprocess
import sys
from pathlib import Path

import duckdb

ROOT = Path(__file__).resolve().parents[4]
SCRIPT = ROOT / "packages/pipeline/scripts/genes/extract_annotation_tracks.py"


def gtf_line(contig, start, end, tx):
    attrs = f'gene_id "G1"; transcript_id "{tx}"; gene_name "G"; gene_biotype "protein_coding";'
    return f"{contig}\ttest\ttranscript\t{start}\t{end}\t.\t+\t.\t{attrs}\n"


def test_same_gene_id_on_two_contigs_gives_two_genes(tmp_path):
    gtf = tmp_path / "a.gtf"
    gtf.write_text(gtf_line("chrX", 101, 200, "T1") + gtf_line("chrY", 1001, 1100, "T2"))
    out = tmp_path / "out"
    subprocess.run([sys.executable, str(SCRIPT), "-i", str(gtf), "-o", str(out)], check=True)
    rows = duckdb.sql(
        f"SELECT contig, start_pos, end_pos FROM '{out}/gene_models.parquet' ORDER BY contig"
    ).fetchall()
    assert rows == [("chrX", 100, 200), ("chrY", 1000, 1100)]
