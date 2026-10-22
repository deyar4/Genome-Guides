"""extract_nested_genes.py: synthetic edge cases through the real extractor, plus a naive independent recomputation."""
import subprocess
import sys
from pathlib import Path

import duckdb
import pytest

ROOT = Path(__file__).resolve().parents[4]
EXTRACT = ROOT / "packages/pipeline/scripts/genes/extract_annotation_tracks.py"
NESTED = ROOT / "packages/pipeline/scripts/genes/extract_nested_genes.py"


def gtf(contig, gene_id, tx, strand, exons, cds=False):
    """exons: 0-based half-open (start, end). Emits transcript, exon and optional CDS lines (1-based inclusive)."""
    lo, hi = min(s for s, _ in exons), max(e for _, e in exons)
    tx = f"{contig}_{tx}"  # transcript ids are unique across contigs (a precondition of the nested step)
    attrs = f'gene_id "{gene_id}"; transcript_id "{tx}";'
    lines = [f"{contig}\tt\ttranscript\t{lo + 1}\t{hi}\t.\t{strand}\t.\t{attrs}"]
    for n, (s, e) in enumerate(sorted(exons), 1):
        lines.append(f'{contig}\tt\texon\t{s + 1}\t{e}\t.\t{strand}\t.\t{attrs} exon_number "{n}";')
    if cds:
        s, e = sorted(exons)[0]
        lines.append(f"{contig}\tt\tCDS\t{s + 1}\t{e}\t.\t{strand}\t0\t{attrs}")
    return lines


def run_nested(tmp_path, lines, contigs):
    g = tmp_path / "a.gtf"
    g.write_text("\n".join(lines) + "\n")
    out = tmp_path / "tracks"
    subprocess.run([sys.executable, str(EXTRACT), "-i", str(g), "-o", str(out)], check=True)
    fai = tmp_path / "x.fai"
    fai.write_text("".join(f"{c}\t100000\t0\t60\t61\n" for c in contigs))
    res = subprocess.run([sys.executable, str(NESTED), "--tracks-dir", str(out), "--fai", str(fai),
                          "--out-parquet", str(tmp_path / "p.parquet"), "--out-summary", str(tmp_path / "s.tsv")],
                         capture_output=True, text=True)
    return res, tmp_path / "p.parquet", tmp_path / "s.tsv"


def pairs(path):
    return duckdb.sql(f"""SELECT contig, host_gene_id, host_locus, guest_gene_id, guest_locus, relationship_type,
        strand_relation, is_direct, guest_coding_class, in_host_intron, overlaps_host_exon FROM '{path}'
        ORDER BY contig, host_start, host_end, host_gene_id, guest_start, guest_end, guest_gene_id""").fetchall()


def test_edge_cases(tmp_path):
    L = []
    # c1 identical spans: not nested; shared start: nested
    L += gtf("c1", "A", "A1", "+", [(100, 500)]) + gtf("c1", "B", "B1", "+", [(100, 500)])
    L += gtf("c1", "C", "C1", "+", [(100, 300)])                                    # C inside A and B (shared start)
    # c2 intron [100,300): guest equal to the intron (intronic); guest touching the next exon start; single-exon host
    L += gtf("c2", "H", "H1", "+", [(0, 100), (300, 400)], cds=True) + gtf("c2", "G1", "G1a", "-", [(100, 300)])
    L += gtf("c2", "G2", "G2a", "+", [(150, 300)])
    L += gtf("c2", "S", "S1", "+", [(1000, 2000)]) + gtf("c2", "G3", "G3a", "+", [(1100, 1200)])   # single-exon host
    # c3 adjacent exons (no intron): guest in the second exon is exonic_overlap
    L += gtf("c3", "H", "H1", "+", [(0, 100), (100, 200)]) + gtf("c3", "G", "Ga", "+", [(100, 150)])
    # c4 mixed: T1 has intron [100,300) containing the guest, T2 has an exon over it
    L += gtf("c4", "H", "T1", "+", [(0, 100), (300, 400)]) + gtf("c4", "H", "T2", "+", [(50, 350)])
    L += gtf("c4", "G", "Ga", "+", [(150, 200)])
    # c5 chain A contains B contains g; two overlapping non-nested hosts both containing g
    L += gtf("c5", "A", "A1", "+", [(0, 1000)]) + gtf("c5", "B", "B1", "-", [(100, 900)]) + gtf("c5", "g", "g1", "+", [(300, 400)])
    L += gtf("c5", "P", "P1", "+", [(2000, 2600)]) + gtf("c5", "Q", "Q1", "+", [(2400, 3000)]) + gtf("c5", "h", "h1", "+", [(2450, 2550)])
    # c6 same gene_id at two distant loci (two units); the guest sits in the second locus' span only; abutting = 2 loci
    L += gtf("c6", "X", "X1", "+", [(0, 100)]) + gtf("c6", "X", "X2", "+", [(5000, 6000)]) + gtf("c6", "y", "y1", "+", [(5100, 5200)])
    L += gtf("c6", "Z", "Z1", "+", [(9000, 9100)]) + gtf("c6", "Z", "Z2", "+", [(9100, 9200)])
    # c7 a gene with transcripts on both strands is ambiguous
    L += gtf("c7", "M", "M1", "+", [(0, 500)]) + gtf("c7", "M", "M2", "-", [(0, 500)]) + gtf("c7", "n", "n1", "+", [(100, 200)])
    # c8 PAR-like: same gene_id on two contigs, each with its own guest
    L += gtf("c8", "P", "P1", "+", [(0, 1000)]) + gtf("c8", "q", "q1", "+", [(10, 20)])
    L += gtf("c9", "P", "P2", "+", [(0, 1000)]) + gtf("c9", "r", "r1", "-", [(10, 20)])
    res, out, summ = run_nested(tmp_path, L, [f"c{i}" for i in range(1, 10)])
    assert res.returncode == 0, res.stderr
    got = {(r[0], r[1], r[2], r[3], r[4]): r[5:] for r in pairs(out)}
    k = lambda c, h, hl, g, gl=0: (c, h, hl, g, gl)
    assert (("c1", "A", 0, "B", 0) not in got) and (("c1", "B", 0, "A", 0) not in got)
    assert k("c1", "A", 0, "C") in got and k("c1", "B", 0, "C") in got
    assert got[k("c2", "H", 0, "G1")][0] == "intronic" and got[k("c2", "H", 0, "G1")][1] == "opposite"
    assert got[k("c2", "H", 0, "G2")][0] == "intronic"           # ends exactly at the next exon start: no overlap
    assert got[k("c2", "S", 0, "G3")][0] == "exonic_overlap" and not got[k("c2", "S", 0, "G3")][4]
    assert got[k("c3", "H", 0, "G")][0] == "exonic_overlap" and got[k("c3", "H", 0, "G")][4:] == (False, True)
    assert got[k("c4", "H", 0, "G")][0] == "mixed"
    assert got[k("c5", "A", 0, "B")][2] is True and got[k("c5", "B", 0, "g")][2] is True
    assert got[k("c5", "A", 0, "g")][2] is False                  # chain: only (A,B) and (B,g) are direct
    assert got[k("c5", "P", 0, "h")][2] is True and got[k("c5", "Q", 0, "h")][2] is True
    assert k("c6", "X", 1, "y") in got and k("c6", "X", 0, "y") not in got        # second locus is its own unit
    assert not any(key[1] == "Z" for key in got)                                   # abutting loci, nothing nested
    assert got[k("c7", "M", 0, "n")][1] == "ambiguous"
    assert k("c8", "P", 0, "q") in got and k("c9", "P", 0, "r") in got and k("c8", "P", 0, "r") not in got
    assert got[k("c2", "H", 0, "G1")][3] == "non_coding"
    counts = dict(line.split("\t") for line in summ.read_text().splitlines())
    assert counts["identical_span_pairs"] == "1" and counts["gene_ids_split_into_loci"] == "2"  # X and Z
    assert sum(int(v) for key, v in counts.items() if key.startswith("relationship_type=")) == int(counts["nested_pairs"])
    assert sum(int(v) for key, v in counts.items() if key.startswith("strand_relation=")) == int(counts["nested_pairs"])


def test_coding_class_comes_from_cds_not_from_the_transcript_prefix(tmp_path):
    L = gtf("c1", "H", "H1", "+", [(0, 1000)]) + gtf("c1", "NM_like", "NM_000001", "+", [(100, 200)]) \
        + gtf("c1", "RPL1", "RPL1_tx", "+", [(300, 400)], cds=True)
    res, out, _ = run_nested(tmp_path, L, ["c1"])
    rows = {r[3]: r[8] for r in pairs(out)}
    assert rows == {"NM_like": "non_coding", "RPL1": "protein_coding"}


def test_precondition_failure_stops_the_step(tmp_path):
    L = gtf("c1", "H", "H1", "+", [(0, 100)])
    g = tmp_path / "a.gtf"
    g.write_text("\n".join(L) + "\n")
    out = tmp_path / "tracks"
    subprocess.run([sys.executable, str(EXTRACT), "-i", str(g), "-o", str(out)], check=True)
    # corrupt: the same transcript id on a second contig in exons
    duckdb.sql(f"""COPY (SELECT * FROM '{out}/exons.parquet' UNION ALL
        SELECT 'c2', start_pos, end_pos, strand, transcript_id, gene_id, exon_number, exon_id FROM '{out}/exons.parquet')
        TO '{tmp_path}/e2.parquet' (FORMAT PARQUET)""")
    (out / "exons.parquet").unlink()
    (tmp_path / "e2.parquet").rename(out / "exons.parquet")
    fai = tmp_path / "x.fai"
    fai.write_text("c1\t10\t0\t60\t61\nc2\t10\t0\t60\t61\n")
    res = subprocess.run([sys.executable, str(NESTED), "--tracks-dir", str(out), "--fai", str(fai),
                          "--out-parquet", str(tmp_path / "p.parquet"), "--out-summary", str(tmp_path / "s.tsv")],
                         capture_output=True, text=True)
    assert res.returncode != 0 and "Precondition failed" in res.stderr


# ---------- independent naive recomputation (own loci, own introns from exons, brute-force direct rule) ----------
def naive_pairs(tx_rows, exon_rows, cds_by_tx):
    """tx_rows: (contig, gene_id, tx, strand, start, end); exon_rows: (contig, tx, start, end)."""
    from collections import defaultdict
    exons = defaultdict(list)
    for contig, tx, s, e in exon_rows:
        exons[(contig, tx)].append((s, e))
    by_gene = defaultdict(list)
    for row in tx_rows:
        by_gene[(row[0], row[1])].append(row)
    units = []
    for (contig, gid), rows in by_gene.items():
        rows.sort(key=lambda r: (r[4], r[5], r[2]))
        cluster, maxend = [], None
        for r in rows:
            if cluster and r[4] >= maxend:
                units.append((contig, gid, len(units), cluster))
                cluster, maxend = [], None
            cluster.append(r)
            maxend = r[5] if maxend is None else max(maxend, r[5])
        units.append((contig, gid, len(units), cluster))
    info = []
    for contig, gid, _, cl in units:
        locus = sum(1 for u in info if u["contig"] == contig and u["gene_id"] == gid)
        info.append({"contig": contig, "gene_id": gid, "locus": locus, "start": min(r[4] for r in cl),
                     "end": max(r[5] for r in cl), "txs": [r[2] for r in cl], "strands": {r[3] for r in cl},
                     "cds": any(cds_by_tx.get(r[2], 0) > 0 for r in cl)})
    out = set()
    by_contig = defaultdict(list)
    for i, u in enumerate(info):
        by_contig[u["contig"]].append(i)
    base = {}
    for contig, idx in by_contig.items():
        for h in idx:
            for g in idx:
                H, G = info[h], info[g]
                if h != g and H["start"] <= G["start"] and G["end"] <= H["end"] and (H["start"], H["end"]) != (G["start"], G["end"]):
                    base[(h, g)] = True
    for (h, g) in base:
        H, G = info[h], info[g]
        in_intron = overlaps = False
        for tx in H["txs"]:
            ex = sorted(exons[(H["contig"], tx)])
            for (s1, e1), (s2, e2) in zip(ex, ex[1:]):
                if e1 < s2 and e1 <= G["start"] and G["end"] <= s2:
                    in_intron = True
            overlaps = overlaps or any(s < G["end"] and G["start"] < e for s, e in ex)
        direct = not any((h, m) in base and (m, g) in base for m in by_contig[H["contig"]])
        typ = ("mixed" if in_intron and overlaps else "intronic" if in_intron else "exonic_overlap" if overlaps else "span_only")
        stranded = len(H["strands"]) == 1 and len(G["strands"]) == 1
        rel = "ambiguous" if not stranded else ("same" if H["strands"] == G["strands"] else "opposite")
        out.add((H["contig"], H["gene_id"], H["locus"], G["gene_id"], G["locus"], typ, rel, direct,
                 "protein_coding" if G["cds"] else "non_coding"))
    return out


def test_naive_equals_sql_on_synthetic(tmp_path):
    import random
    rng = random.Random(3)
    L = []
    for c in range(3):
        for k in range(40):
            s = rng.randrange(0, 4000)
            n = rng.randint(1, 3)
            cuts = sorted(rng.sample(range(s, s + 600), 2 * n))
            ex = [(cuts[2 * i], cuts[2 * i + 1]) for i in range(n) if cuts[2 * i] < cuts[2 * i + 1]]
            if ex:
                L += gtf(f"c{c}", f"g{k % 25}", f"t{c}_{k}", rng.choice("+-"), ex, cds=rng.random() < 0.4)
    res, out, _ = run_nested(tmp_path, L, ["c0", "c1", "c2"])
    assert res.returncode == 0, res.stderr
    tracks = tmp_path / "tracks"
    tx = duckdb.sql(f"SELECT contig, gene_id, transcript_id, strand, start_pos, end_pos FROM '{tracks}/transcripts.parquet'").fetchall()
    ex = duckdb.sql(f"SELECT contig, transcript_id, start_pos, end_pos FROM '{tracks}/exons.parquet'").fetchall()
    cds = dict(duckdb.sql(f"SELECT transcript_id, cds_bases FROM '{tracks}/gene_architecture.parquet'").fetchall())
    expected = naive_pairs(tx, ex, cds)
    got = {(r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], r[8]) for r in pairs(out)}
    assert len(expected) > 20
    assert got == expected


@pytest.mark.skipif(not (ROOT / "data/tracks/t2t/all/nested_gene_pairs.parquet").exists(), reason="no t2t tracks")
def test_naive_equals_sql_on_t2t():
    d = ROOT / "data/tracks/t2t/all"
    tx = duckdb.sql(f"SELECT contig, gene_id, transcript_id, strand, start_pos, end_pos FROM '{d}/transcripts.parquet'").fetchall()
    ex = duckdb.sql(f"SELECT contig, transcript_id, start_pos, end_pos FROM '{d}/exons.parquet'").fetchall()
    cds = dict(duckdb.sql(f"SELECT transcript_id, cds_bases FROM '{d}/gene_architecture.parquet'").fetchall())
    expected = naive_pairs(tx, ex, cds)
    got = {(r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], r[8]) for r in pairs(d / "nested_gene_pairs.parquet")}
    assert got == expected
    assert not any(r[5] == "span_only" for r in got)  # impossible under the locus rule (spec 02, section 1)
