#!/usr/bin/env python3
"""Create transcript, exon, intron, UTR and gene-architecture Parquet tracks."""

import argparse
import gzip
import re
import sqlite3
import tempfile
from collections import defaultdict
from pathlib import Path

import pyarrow as pa
import pyarrow.parquet as pq
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "common"))
from resource_args import add_resource_args


ATTR_RE = re.compile(r'(?:^|[;\s])([A-Za-z_][\w.-]*)\s*[= ]\s*"?(.*?)(?:"?)(?:;|$)')


def attrs(text):
    result = {}
    for key, value in ATTR_RE.findall(text):
        result[key] = value.strip().strip('"')
    return result


def write_table(path, rows, columns):
    path.parent.mkdir(parents=True, exist_ok=True)
    writer = None
    try:
        for offset in range(0, len(rows), 65536):
            batch = rows[offset:offset + 65536]
            arrays = {
                name: pa.array([row.get(name) for row in batch], type=kind)
                for name, kind in columns
            }
            table = pa.table(arrays)
            if writer is None:
                writer = pq.ParquetWriter(path, table.schema, compression="zstd")
            writer.write_table(table)
    finally:
        if writer is not None:
            writer.close()
    if writer is None:
        schema = pa.schema([(name, kind) for name, kind in columns])
        pq.write_table(pa.Table.from_pylist([], schema=schema), path, compression="zstd")


def write_cursor_table(path, cursor, columns):
    path.parent.mkdir(parents=True, exist_ok=True)
    writer = None
    try:
        while rows := cursor.fetchmany(65536):
            table = pa.Table.from_pylist([dict(row) for row in rows], schema=pa.schema([
                pa.field(name, kind) for name, kind in columns
            ]))
            if writer is None:
                writer = pq.ParquetWriter(path, table.schema, compression="zstd")
            writer.write_table(table)
    finally:
        if writer is not None:
            writer.close()
    if writer is None:
        pq.write_table(pa.Table.from_pylist([], schema=pa.schema([
            pa.field(name, kind) for name, kind in columns
        ])), path, compression="zstd")


def parse(gtf, out_dir):
    transcripts = {}
    genes = {}
    malformed_rows = 0
    open_fn = gzip.open if str(gtf).endswith(".gz") else open
    database = tempfile.NamedTemporaryFile(prefix="genome-guides-annotation-", suffix=".sqlite", delete=False)
    database.close()
    con = sqlite3.connect(database.name)
    con.row_factory = sqlite3.Row
    con.executescript("""
        CREATE TABLE exons (
            tx TEXT, start INTEGER, end INTEGER, contig TEXT, strand TEXT,
            gene TEXT, exon_number INTEGER, exon_id TEXT
        );
        CREATE TABLE utrs (
            tx TEXT, start INTEGER, end INTEGER, contig TEXT, strand TEXT,
            gene TEXT, utr_type TEXT
        );
        CREATE TABLE cds (tx TEXT PRIMARY KEY, bases INTEGER NOT NULL DEFAULT 0);
    """)
    with open_fn(gtf, "rt") as handle:
        for line in handle:
            if not line.strip() or line.startswith("#"):
                continue
            fields = line.rstrip("\n").split("\t")
            if len(fields) < 9:
                malformed_rows += 1
                continue
            chrom, _, feature, start, end, _, strand, _, raw_attrs = fields
            try:
                start0, end0 = int(start) - 1, int(end)
            except ValueError:
                malformed_rows += 1
                continue
            a = attrs(raw_attrs)
            tx = a.get("transcript_id") or a.get("ID")
            gene = a.get("gene_id") or a.get("gene_name") or a.get("Parent") or "UNKNOWN"
            name = a.get("gene_name") or gene
            biotype = a.get("gene_biotype") or a.get("gene_type") or (
                "protein_coding" if (tx or "").startswith(("NM_", "XM_")) else "non_coding"
            )

            if feature in {"transcript", "mRNA"} and tx:
                transcripts[tx] = {
                    "contig": chrom, "start_pos": start0, "end_pos": end0,
                    "strand": strand, "transcript_id": tx, "gene_id": gene,
                    "gene_name": name, "biotype": biotype,
                }
                old = genes.get(gene)
                if old is None:
                    genes[gene] = {
                        "contig": chrom, "start_pos": start0, "end_pos": end0,
                        "strand": strand, "gene_id": gene, "gene_name": name,
                        "biotype": biotype,
                    }
                else:
                    old["start_pos"] = min(old["start_pos"], start0)
                    old["end_pos"] = max(old["end_pos"], end0)
            elif feature == "exon" and tx:
                row = {
                    "contig": chrom, "start_pos": start0, "end_pos": end0,
                    "strand": strand, "transcript_id": tx, "gene_id": gene,
                    "exon_number": int(a["exon_number"]) if a.get("exon_number", "").isdigit() else None,
                    "exon_id": a.get("exon_id") or f"{tx}:{start0}-{end0}",
                }
                con.execute(
                    "INSERT INTO exons VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    (tx, start0, end0, chrom, strand, gene, row["exon_number"], row["exon_id"]),
                )
            elif feature.lower() in {"5utr", "3utr", "utr"} and tx:
                side = "5" if feature.lower() == "5utr" else "3" if feature.lower() == "3utr" else "unknown"
                con.execute(
                    "INSERT INTO utrs VALUES (?, ?, ?, ?, ?, ?, ?)",
                    (tx, start0, end0, chrom, strand, gene, side),
                )
            elif feature == "CDS" and tx:
                con.execute(
                    "INSERT INTO cds(tx, bases) VALUES (?, ?) "
                    "ON CONFLICT(tx) DO UPDATE SET bases = bases + excluded.bases",
                    (tx, end0 - start0),
                )
    con.executescript("CREATE INDEX exons_tx ON exons (tx); CREATE INDEX utrs_tx ON utrs (tx);")
    con.commit()

    introns = []
    architectures = []
    for tx in transcripts:
        ordered = con.execute(
            "SELECT start, end, contig, strand, gene FROM exons "
            "WHERE tx = ? ORDER BY start, end", (tx,)
        ).fetchall()
        if not ordered:
            continue
        for number, (left, right) in enumerate(zip(ordered, ordered[1:]), 1):
            if right[0] > left[1]:
                introns.append({
                    "contig": left[2], "start_pos": left[1], "end_pos": right[0],
                    "strand": left[3], "transcript_id": tx, "gene_id": left[4],
                    "intron_number": number,
                })
        t = transcripts.get(tx, {})
        architectures.append({
            "transcript_id": tx, "gene_id": t.get("gene_id", ordered[0][4]),
            "gene_name": t.get("gene_name", t.get("gene_id", ordered[0][4])),
            "contig": t.get("contig", ordered[0][2]),
            "strand": t.get("strand", ordered[0][3]),
            "exon_count": len(ordered), "intron_count": max(0, len(ordered) - 1),
            "exon_bases": sum(e[1] - e[0] for e in ordered),
            "intron_bases": sum(r[0] - l[1] for l, r in zip(ordered, ordered[1:])),
            "utr5_bases": con.execute(
                "SELECT COALESCE(SUM(end - start), 0) FROM utrs WHERE tx = ? AND utr_type = '5'", (tx,)
            ).fetchone()[0],
            "utr3_bases": con.execute(
                "SELECT COALESCE(SUM(end - start), 0) FROM utrs WHERE tx = ? AND utr_type = '3'", (tx,)
            ).fetchone()[0],
            "cds_bases": (con.execute(
                "SELECT COALESCE(bases, 0) FROM cds WHERE tx = ?", (tx,)
            ).fetchone() or (0,))[0],
        })

    out = Path(out_dir)
    write_table(out / "gene_models.parquet", list(genes.values()), [
        ("contig", pa.string()), ("start_pos", pa.uint64()), ("end_pos", pa.uint64()),
        ("strand", pa.string()), ("gene_id", pa.string()), ("gene_name", pa.string()),
        ("biotype", pa.string()),
    ])
    density = defaultdict(int)
    for gene in genes.values():
        density[(gene["contig"], (gene["start_pos"] // 100000) * 100000)] += 1
    write_table(out / "gene_density.parquet", [
        {"contig": c, "start_pos": s, "end_pos": s + 100000, "count": n}
        for (c, s), n in sorted(density.items())
    ], [("contig", pa.string()), ("start_pos", pa.uint64()), ("end_pos", pa.uint64()), ("count", pa.uint64())])
    counts = defaultdict(int)
    for gene in genes.values():
        counts[gene["biotype"]] += 1
    write_table(out / "gene_biotype_census.parquet", [
        {"biotype": k, "count": v} for k, v in sorted(counts.items())
    ], [("biotype", pa.string()), ("count", pa.uint64())])

    write_table(out / "transcripts.parquet", list(transcripts.values()), [
        ("contig", pa.string()), ("start_pos", pa.uint64()), ("end_pos", pa.uint64()),
        ("strand", pa.string()), ("transcript_id", pa.string()), ("gene_id", pa.string()),
        ("gene_name", pa.string()), ("biotype", pa.string()),
    ])
    exon_cursor = con.execute("""
        SELECT contig, start AS start_pos, end AS end_pos, strand,
               tx AS transcript_id, gene AS gene_id, exon_number, exon_id
        FROM exons ORDER BY rowid
    """)
    write_cursor_table(out / "exons.parquet", exon_cursor, [
        ("contig", pa.string()), ("start_pos", pa.uint64()), ("end_pos", pa.uint64()),
        ("strand", pa.string()), ("transcript_id", pa.string()), ("gene_id", pa.string()),
        ("exon_number", pa.uint32()), ("exon_id", pa.string()),
    ])
    write_table(out / "introns.parquet", introns, [
        ("contig", pa.string()), ("start_pos", pa.uint64()), ("end_pos", pa.uint64()),
        ("strand", pa.string()), ("transcript_id", pa.string()), ("gene_id", pa.string()),
        ("intron_number", pa.uint32()),
    ])
    utr_cursor = con.execute("""
        SELECT contig, start AS start_pos, end AS end_pos, strand,
               tx AS transcript_id, gene AS gene_id, utr_type
        FROM utrs ORDER BY rowid
    """)
    write_cursor_table(out / "utrs.parquet", utr_cursor, [
        ("contig", pa.string()), ("start_pos", pa.uint64()), ("end_pos", pa.uint64()),
        ("strand", pa.string()), ("transcript_id", pa.string()), ("gene_id", pa.string()),
        ("utr_type", pa.string()),
    ])
    write_table(out / "gene_architecture.parquet", architectures, [
        ("transcript_id", pa.string()), ("gene_id", pa.string()), ("gene_name", pa.string()),
        ("contig", pa.string()), ("strand", pa.string()), ("exon_count", pa.uint32()),
        ("intron_count", pa.uint32()), ("exon_bases", pa.uint64()), ("intron_bases", pa.uint64()),
        ("utr5_bases", pa.uint64()), ("utr3_bases", pa.uint64()), ("cds_bases", pa.uint64()),
    ])
    if malformed_rows:
        print(f"Warning: skipped {malformed_rows} malformed annotation rows.", file=sys.stderr)
    con.close()
    Path(database.name).unlink(missing_ok=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("-i", "--input", required=True)
    parser.add_argument("-o", "--out-dir", required=True)
    add_resource_args(parser)
    args = parser.parse_args()
    parse(args.input, args.out_dir)
