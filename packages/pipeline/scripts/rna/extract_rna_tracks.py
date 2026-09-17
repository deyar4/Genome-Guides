#!/usr/bin/env python3
"""Extract splice junctions/consensus and clustered RNA loci from a GTF and FASTA."""

import argparse
import gzip
import re
from collections import Counter, defaultdict
from pathlib import Path

import pyarrow as pa
import pyarrow.parquet as pq
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "common"))
from resource_args import add_resource_args


ATTR_RE = re.compile(r'(?:^|[;\s])([A-Za-z_][\w.-]*)\s*[= ]\s*"?(.*?)(?:"?)(?:;|$)')
RC = str.maketrans("ACGTNacgtn", "TGCANtgcan")


def parse_attrs(text):
    return {k: v.strip().strip('"') for k, v in ATTR_RE.findall(text)}


def rna_biotype(transcript_id, gene_name, raw_attrs):
    if transcript_id.startswith(("NM_", "XM_")):
        return "protein_coding"
    text = f"{gene_name} {raw_attrs}".lower()
    for label in ("trna", "rrna", "snorna", "snrna", "mirna"):
        if label in text:
            return label
    return "non_coding"


class FastaIndex:
    def __init__(self, fasta):
        self.fasta = str(fasta)
        self.index = {}
        with open(self.fasta + ".fai") as handle:
            for line in handle:
                name, length, offset, line_bases, line_width = line.rstrip().split("\t")[:5]
                self.index[name] = tuple(map(int, (length, offset, line_bases, line_width)))

    def fetch(self, chrom, start, end, strand="+"):
        if chrom not in self.index or end <= start:
            return ""
        length, offset, line_bases, line_width = self.index[chrom]
        start, end = max(0, start), min(length, end)
        chunks = []
        with open(self.fasta, "rb") as handle:
            pos = start
            while pos < end:
                line = pos // line_bases
                in_line = pos % line_bases
                take = min(end - pos, line_bases - in_line)
                handle.seek(offset + line * line_width + in_line)
                chunks.append(handle.read(take).decode("ascii"))
                pos += take
        seq = "".join(chunks).upper()
        return seq.translate(RC)[::-1] if strand == "-" else seq


def write(path, rows, schema):
    path.parent.mkdir(parents=True, exist_ok=True)
    writer = None
    try:
        for offset in range(0, len(rows), 65536):
            batch = rows[offset:offset + 65536]
            table = pa.table({
                key: pa.array([row.get(key) for row in batch], type=kind)
                for key, kind in schema.items()
            })
            if writer is None:
                writer = pq.ParquetWriter(path, table.schema, compression="zstd")
            writer.write_table(table)
    finally:
        if writer is not None:
            writer.close()
    if writer is None:
        pq.write_table(
            pa.Table.from_pylist([], schema=pa.schema([
                pa.field(key, kind) for key, kind in schema.items()
            ])),
            path,
            compression="zstd",
        )


class ParquetRowWriter:
    def __init__(self, path, schema):
        self.path = path
        self.schema = pa.schema([pa.field(key, kind) for key, kind in schema.items()])
        self.rows = []
        self.writer = None

    def append(self, row):
        self.rows.append(row)
        if len(self.rows) >= 65536:
            self.flush()

    def flush(self):
        if not self.rows:
            return
        table = pa.Table.from_pylist(self.rows, schema=self.schema)
        if self.writer is None:
            self.writer = pq.ParquetWriter(self.path, self.schema, compression="zstd")
        self.writer.write_table(table)
        self.rows.clear()

    def close(self):
        self.flush()
        if self.writer is not None:
            self.writer.close()
        else:
            pq.write_table(pa.Table.from_pylist([], schema=self.schema), self.path, compression="zstd")


def extract(gtf, fasta, out_dir):
    exons = defaultdict(list)
    transcripts = {}
    ncrna = Counter()
    malformed_rows = 0
    open_fn = gzip.open if str(gtf).endswith(".gz") else open
    with open_fn(gtf, "rt") as handle:
        for line in handle:
            if not line.strip() or line.startswith("#"):
                continue
            f = line.rstrip("\n").split("\t")
            if len(f) < 9:
                malformed_rows += 1
                continue
            chrom, _, feature, start, end, _, strand, _, raw = f
            if feature not in {"transcript", "mRNA", "exon"}:
                continue
            a = parse_attrs(raw)
            tx = a.get("transcript_id") or a.get("ID")
            if not tx:
                continue
            gene = a.get("gene_id") or a.get("gene_name") or "UNKNOWN"
            name = a.get("gene_name") or gene
            biotype = a.get("gene_biotype") or a.get("gene_type") or rna_biotype(tx, name, raw)
            try:
                start0, end0 = int(start) - 1, int(end)
            except ValueError:
                malformed_rows += 1
                continue
            if feature in {"transcript", "mRNA"}:
                transcripts[tx] = (chrom, start0, end0, strand, gene, name, biotype)
                if biotype != "protein_coding":
                    ncrna[biotype] += 1
            else:
                exons[tx].append((start0, end0, chrom, strand, gene))

    fa = FastaIndex(fasta)
    out = Path(out_dir)
    out.mkdir(parents=True, exist_ok=True)
    junction_schema = {
        "contig": pa.string(), "intron_start": pa.uint64(), "intron_end": pa.uint64(),
        "transcript_id": pa.string(), "gene_id": pa.string(), "strand": pa.string(),
        "junction_number": pa.uint32(), "donor_seq": pa.string(), "acceptor_seq": pa.string(),
        "is_canonical": pa.uint8(),
    }
    junction_writer = ParquetRowWriter(out / "splice_sites.parquet", junction_schema)
    donor_counts, acceptor_counts = Counter(), Counter()
    pwm_counts = {"donor": [Counter() for _ in range(2)], "acceptor": [Counter() for _ in range(2)]}
    for tx, parts in exons.items():
        parts.sort()
        info = transcripts.get(tx)
        if not info:
            continue
        for number, (left, right) in enumerate(zip(parts, parts[1:]), 1):
            if right[0] <= left[1]:
                continue
            chrom, strand, gene = left[2], left[3], left[4]
            if strand == "-":
                donor = fa.fetch(chrom, right[0] - 2, right[0], strand)
                acceptor = fa.fetch(chrom, left[1], left[1] + 2, strand)
            else:
                donor = fa.fetch(chrom, left[1], left[1] + 2, strand)
                acceptor = fa.fetch(chrom, right[0] - 2, right[0], strand)
            donor_counts[donor] += 1
            acceptor_counts[acceptor] += 1
            for i, base in enumerate(donor):
                if i < 2:
                    pwm_counts["donor"][i][base] += 1
            for i, base in enumerate(acceptor):
                if i < 2:
                    pwm_counts["acceptor"][i][base] += 1
            junction_writer.append({
                "contig": chrom, "intron_start": left[1], "intron_end": right[0],
                "transcript_id": tx, "gene_id": gene, "strand": strand,
                "junction_number": number, "donor_seq": donor, "acceptor_seq": acceptor,
                "is_canonical": int(donor in {"GT", "GC"} and acceptor == "AG"),
            })

    clusters = []
    by_type = defaultdict(list)
    for tx, (chrom, start, end, strand, gene, name, biotype) in transcripts.items():
        rna_type = biotype
        by_type[(chrom, rna_type)].append((start, end, gene))
    for (chrom, rna_type), values in by_type.items():
        values.sort()
        current = None
        for start, end, gene in values:
            if current and start <= current["end"] + 1000:
                current["end"] = max(current["end"], end)
                current["genes"].append(gene)
            else:
                if current:
                    clusters.append(current)
                current = {"chrom": chrom, "rna_type": rna_type, "start": start, "end": end, "genes": [gene]}
        if current:
            clusters.append(current)
    cluster_rows = [{
        "cluster_id": f"rna_cluster_{i:06d}", "rna_type": c["rna_type"], "chrom": c["chrom"],
        "start": c["start"], "end": c["end"], "size": len(c["genes"]),
        "genes": ",".join(sorted(set(c["genes"]))),
    } for i, c in enumerate(clusters, 1)]

    junction_writer.close()
    consensus = []
    pwm_type = pa.struct([
        pa.field("A", pa.list_(pa.float32())), pa.field("C", pa.list_(pa.float32())),
        pa.field("G", pa.list_(pa.float32())), pa.field("T", pa.list_(pa.float32())),
    ])
    for site, counts in (("donor", donor_counts), ("acceptor", acceptor_counts)):
        total = sum(counts.values())
        sequence = counts.most_common(1)[0][0] if counts else ""
        consensus.append({
            "site_type": site, "consensus_seq": sequence, "total_sites": total,
            "pwm": {base: [
                pwm_counts[site][i][base] / total if total else 0.0 for i in range(2)
            ] for base in "ACGT"},
        })
    write(out / "splice_site_consensus.parquet", consensus, {
        "site_type": pa.string(), "consensus_seq": pa.string(), "total_sites": pa.uint64(),
        "pwm": pwm_type,
    })
    write(out / "ncrna_detailed_census.parquet", [
        {"ncrna_subclass": k, "count": v} for k, v in sorted(ncrna.items())
    ], {"ncrna_subclass": pa.string(), "count": pa.uint64()})
    iso = Counter(t[4] for t in transcripts.values())
    write(out / "gene_isoform_counts.parquet", [
        {"gene_name": k, "isoform_count": v} for k, v in sorted(iso.items())
    ], {"gene_name": pa.string(), "isoform_count": pa.uint64()})
    write(out / "rna_clusters.parquet", cluster_rows, {
        "cluster_id": pa.string(), "rna_type": pa.string(), "chrom": pa.string(),
        "start": pa.uint64(), "end": pa.uint64(), "size": pa.uint32(), "genes": pa.string()
    })
    summary = []
    by_summary = defaultdict(list)
    for c in cluster_rows:
        by_summary[c["rna_type"]].append(c)
    for rna_type, rows in sorted(by_summary.items()):
        largest = max(rows, key=lambda r: r["size"])
        summary.append({
            "rna_type": rna_type, "num_clusters": len(rows),
            "avg_cluster_size": sum(r["size"] for r in rows) / len(rows),
            "max_cluster_size": largest["size"],
            "largest_cluster_location": f'{largest["chrom"]}:{largest["start"]}-{largest["end"]}',
        })
    write(out / "rna_cluster_summary.parquet", summary, {
        "rna_type": pa.string(), "num_clusters": pa.uint64(), "avg_cluster_size": pa.float64(),
        "max_cluster_size": pa.uint32(), "largest_cluster_location": pa.string()
    })
    if malformed_rows:
        print(f"Warning: skipped {malformed_rows} malformed annotation rows.", file=sys.stderr)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("-g", "--gtf", required=True)
    p.add_argument("-f", "--fasta", required=True)
    p.add_argument("-o", "--out-dir", required=True)
    add_resource_args(p)
    a = p.parse_args()
    extract(a.gtf, a.fasta, a.out_dir)
