#!/usr/bin/env python3
"""Create transcript, exon, intron, UTR and gene-architecture Parquet tracks."""

import argparse
import gzip
import re
from collections import defaultdict
from pathlib import Path

import pyarrow as pa
import pyarrow.parquet as pq


ATTR_RE = re.compile(r'(?:^|[;\s])([A-Za-z_][\w.-]*)\s*[= ]\s*"?(.*?)(?:"?)(?:;|$)')


def attrs(text):
    result = {}
    for key, value in ATTR_RE.findall(text):
        result[key] = value.strip().strip('"')
    return result


def write_table(path, rows, columns):
    path.parent.mkdir(parents=True, exist_ok=True)
    arrays = {}
    for name, kind in columns:
        values = [row.get(name) for row in rows]
        arrays[name] = pa.array(values, type=kind)
    pq.write_table(pa.table(arrays), path, compression="zstd")


def parse(gtf, out_dir):
    transcripts = {}
    exons = []
    utrs = []
    cds_by_tx = defaultdict(int)
    exons_by_tx = defaultdict(list)
    utr_by_tx = defaultdict(lambda: {"5": 0, "3": 0})
    genes = {}
    open_fn = gzip.open if str(gtf).endswith(".gz") else open

    with open_fn(gtf, "rt") as handle:
        for line in handle:
            if not line.strip() or line.startswith("#"):
                continue
            fields = line.rstrip("\n").split("\t")
            if len(fields) < 9:
                continue
            chrom, _, feature, start, end, _, strand, _, raw_attrs = fields
            try:
                start0, end0 = int(start) - 1, int(end)
            except ValueError:
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
                exons.append(row)
                exons_by_tx[tx].append((start0, end0, chrom, strand, gene))
            elif feature.lower() in {"5utr", "3utr", "utr"} and tx:
                side = "5" if feature.lower() == "5utr" else "3" if feature.lower() == "3utr" else "unknown"
                utrs.append({
                    "contig": chrom, "start_pos": start0, "end_pos": end0,
                    "strand": strand, "transcript_id": tx, "gene_id": gene,
                    "utr_type": side,
                })
                if side in {"5", "3"}:
                    utr_by_tx[tx][side] += end0 - start0
            elif feature == "CDS" and tx:
                cds_by_tx[tx] += end0 - start0

    introns = []
    architectures = []
    for tx, tx_exons in exons_by_tx.items():
        ordered = sorted(tx_exons)
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
            "utr5_bases": utr_by_tx[tx]["5"], "utr3_bases": utr_by_tx[tx]["3"],
            "cds_bases": cds_by_tx[tx],
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
    write_table(out / "exons.parquet", exons, [
        ("contig", pa.string()), ("start_pos", pa.uint64()), ("end_pos", pa.uint64()),
        ("strand", pa.string()), ("transcript_id", pa.string()), ("gene_id", pa.string()),
        ("exon_number", pa.uint32()), ("exon_id", pa.string()),
    ])
    write_table(out / "introns.parquet", introns, [
        ("contig", pa.string()), ("start_pos", pa.uint64()), ("end_pos", pa.uint64()),
        ("strand", pa.string()), ("transcript_id", pa.string()), ("gene_id", pa.string()),
        ("intron_number", pa.uint32()),
    ])
    write_table(out / "utrs.parquet", utrs, [
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


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("-i", "--input", required=True)
    parser.add_argument("-o", "--out-dir", required=True)
    args = parser.parse_args()
    parse(args.input, args.out_dir)
