from fastapi import APIRouter, Depends, Query, HTTPException
from typing import List, Dict, Any
from app.api.dependencies import get_track_service
from app.services.duckdb_track_service import DuckDBTrackService
from app.schemas.genes import (
    GeneSummary, BiotypeStat, GeneChromosomeStat,
    TopGene, GeneSearchResponse, NestedGeneSummary
)
import os

router = APIRouter()

@router.get("/stats/summary", response_model=GeneSummary)
def get_summary(
    build: str = Query(...),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    """Gene counts per biotype from gene_biotype_census.parquet (total_genes is their sum) and the mean exon_count over the rows of
    gene_architecture.parquet as avg_exons_per_transcript (UNVERIFIED: whether those rows are per transcript).
    Returns zeros and an empty list when the census track is missing; 0.0 for the mean when the architecture track is
    missing. A gene id shared by separate loci counts as one gene unit (OPEN_QUESTIONS Q-gene-units)."""
    biotype_file = track_service.data_dir / "tracks" / build / "all" / "gene_biotype_census.parquet"
    if not biotype_file.exists():
        return GeneSummary(total_genes=0, biotype_counts=[], avg_exons_per_transcript=0)
    
    res = track_service.execute_query(f"SELECT biotype, count FROM read_parquet('{biotype_file}')")
    
    biotypes = []
    total = 0
    for row in res:
        b = row.get("biotype", "unknown")
        c = int(row.get("count", 0))
        biotypes.append(BiotypeStat(biotype=b, count=c))
        total += c

    architecture_file = track_service.data_dir / "tracks" / build / "all" / "gene_architecture.parquet"
    avg_exons = 0.0
    if architecture_file.exists():
        architecture = track_service.execute_query(
            f"SELECT AVG(exon_count) AS avg_exons FROM read_parquet('{architecture_file}')"
        )
        avg_exons = float((architecture[0] if architecture else {}).get("avg_exons", 0) or 0)
    return GeneSummary(total_genes=total, biotype_counts=biotypes, avg_exons_per_transcript=avg_exons)

@router.get("/stats/chromosomes", response_model=List[GeneChromosomeStat])
def get_chromosome_stats(
    build: str = Query(...),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    """Per contig of gene_models.parquet: gene_count and density = gene_count / (MAX end_pos - MIN start_pos of its genes) * 1e6,
    i.e. genes per Mb of the span between the first and last gene, not of the contig length. Ordered by contig.
    Returns [] when the track is missing. Gene units follow OPEN_QUESTIONS Q-gene-units."""
    models_file = track_service.data_dir / "tracks" / build / "all" / "gene_models.parquet"
    if not models_file.exists():
        return []

    res = track_service.execute_query(f"""
        SELECT contig, count(1) as gene_count,
               MAX(end_pos) - MIN(start_pos) as covered_length
        FROM read_parquet('{models_file}')
        GROUP BY contig
        ORDER BY contig
    """)
    
    stats = []
    for row in res:
        c = row.get("contig", "")
        gc = int(row.get("gene_count", 0))
        ml = int(row.get("covered_length", 1) or 1)
        # Density per Mb
        den = (gc / ml) * 1_000_000 if ml > 0 else 0
        stats.append(GeneChromosomeStat(chrom=c, gene_count=gc, density=den))
        
    return stats

@router.get("/stats/biotypes", response_model=List[BiotypeStat])
def get_biotypes(
    build: str = Query(...),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    """Gene count per biotype as [{biotype, count}] read unchanged from gene_biotype_census.parquet (no ordering applied).
    Returns [] when the track is missing. Gene units and biotype assignment: see OPEN_QUESTIONS Q-gene-units."""
    biotype_file = track_service.data_dir / "tracks" / build / "all" / "gene_biotype_census.parquet"
    if not biotype_file.exists():
        return []
    
    res = track_service.execute_query(f"SELECT biotype, count FROM read_parquet('{biotype_file}')")
    
    biotypes = []
    for row in res:
        b = row.get("biotype", "unknown")
        c = int(row.get("count", 0))
        biotypes.append(BiotypeStat(biotype=b, count=c))
        
    return biotypes

@router.get("/top/largest", response_model=List[TopGene])
def get_largest(
    build: str = Query(...),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    """The 50 longest genes of gene_models.parquet, ranked by length = end_pos - start_pos (value, in bp) descending, with
    gene_id, gene_name, biotype, chrom, start, end and strand. Ties are in unspecified order. Returns [] when the track is
    missing. A gene_models row can merge several loci with one gene_id (OPEN_QUESTIONS Q-gene-units), which inflates length."""
    models_file = track_service.data_dir / "tracks" / build / "all" / "gene_models.parquet"
    if not models_file.exists():
        return []

    res = track_service.execute_query(f"""
        SELECT gene_id, gene_name, biotype, contig, start_pos, end_pos, strand,
               (end_pos - start_pos) as length
        FROM read_parquet('{models_file}')
        ORDER BY length DESC
        LIMIT 50
    """)
    
    results = []
    for i, row in enumerate(res):
        results.append(TopGene(
            gene_id=row.get("gene_id", ""),
            gene_name=row.get("gene_name"),
            biotype=row.get("biotype"),
            chrom=row.get("contig", ""),
            value=float(row.get("length", 0)),
            rank=i + 1,
            start=int(row.get("start_pos", 0)),
            end=int(row.get("end_pos", 0)),
            strand=row.get("strand")
        ))
        
    return results

@router.get("/top/most-exons", response_model=List[TopGene])
def get_most_exons(
    build: str = Query(...),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    """The 50 genes with the highest exon count: gene_architecture.parquet joined to gene_models.parquet on (gene_id, contig),
    value = MAX exon_count over the gene's architecture rows (UNVERIFIED: that this is the per-transcript maximum).
    Returns [] when either track is missing. Gene units: OPEN_QUESTIONS Q-gene-units."""
    architecture_file = track_service.data_dir / "tracks" / build / "all" / "gene_architecture.parquet"
    models_file = track_service.data_dir / "tracks" / build / "all" / "gene_models.parquet"
    if not architecture_file.exists() or not models_file.exists():
        return []

    res = track_service.execute_query(f"""
        SELECT m.gene_id, m.gene_name, m.biotype, m.contig, m.start_pos, m.end_pos, m.strand,
               MAX(a.exon_count) AS exon_count
        FROM read_parquet('{architecture_file}') a
        JOIN read_parquet('{models_file}') m ON a.gene_id = m.gene_id AND a.contig = m.contig
        GROUP BY m.gene_id, m.gene_name, m.biotype, m.contig, m.start_pos, m.end_pos, m.strand
        ORDER BY exon_count DESC
        LIMIT 50
    """)
    
    results = []
    for i, row in enumerate(res):
        results.append(TopGene(
            gene_id=row.get("gene_id", ""),
            gene_name=row.get("gene_name"),
            biotype=row.get("biotype"),
            chrom=row.get("contig", ""),
            value=float(row.get("exon_count", 0)),
            rank=i + 1,
            start=int(row.get("start_pos", 0)),
            end=int(row.get("end_pos", 0)),
            strand=row.get("strand")
        ))
        
    return results

@router.get("/search", response_model=List[GeneSearchResponse])
def search_genes(
    build: str = Query(...), 
    q: str = Query(...),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    """Case-insensitive substring search (ILIKE %q%) of q on gene_name and gene_id in gene_models.parquet. At most 50 rows,
    without ordering or total, as {gene_id, gene_name, biotype, chrom, start, end}. Returns [] when the track is missing
    or nothing matches."""
    models_file = track_service.data_dir / "tracks" / build / "all" / "gene_models.parquet"
    if not models_file.exists():
        return []

    res = track_service.execute_query(f"""
        SELECT gene_id, gene_name, biotype, contig, start_pos, end_pos
        FROM read_parquet('{models_file}')
        WHERE gene_name ILIKE ? OR gene_id ILIKE ?
        LIMIT 50
    """, [f"%{q}%", f"%{q}%"])
    
    results = []
    for row in res:
        results.append(GeneSearchResponse(
            gene_id=row.get("gene_id", ""),
            gene_name=row.get("gene_name"),
            biotype=row.get("biotype"),
            chrom=row.get("contig", ""),
            start=int(row.get("start_pos", 0)),
            end=int(row.get("end_pos", 0))
        ))
        
    return results

@router.get("/nested/summary", response_model=NestedGeneSummary)
def get_nested(
    build: str = Query(...),
    limit: int = Query(50, ge=1, le=500),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    """Nested gene pairs (docs/analyses/02_nested_genes.md, D43). first_pairs are the first `limit` pairs in
    coordinate order (contig in FASTA-index order), not a random sample."""
    path = _track_path(track_service, build, "nested_gene_pairs.parquet")
    agg = track_service.execute_query(f"""
        SELECT COUNT(*) AS total, COUNT(*) FILTER (WHERE is_direct) AS direct,
               COUNT(*) FILTER (WHERE strand_relation = 'same') AS same_strand,
               COUNT(*) FILTER (WHERE strand_relation = 'opposite') AS opposite_strand,
               COUNT(*) FILTER (WHERE strand_relation = 'ambiguous') AS ambiguous_strand,
               COUNT(DISTINCT (contig, host_gene_id, host_locus)) AS hosts,
               COUNT(DISTINCT (contig, guest_gene_id, guest_locus)) AS guests
        FROM read_parquet('{path}')""")[0]
    by_type = {r["relationship_type"]: int(r["n"]) for r in track_service.execute_query(
        f"SELECT relationship_type, COUNT(*) AS n FROM read_parquet('{path}') GROUP BY 1 ORDER BY 1")}
    rows = track_service.execute_query(f"""
        SELECT contig AS chrom, host_gene_id, host_locus, host_gene_name, host_coding_class, host_start, host_end,
               guest_gene_id, guest_locus, guest_gene_name, guest_coding_class, guest_start, guest_end,
               relationship_type, strand_relation, is_direct
        FROM read_parquet('{path}', file_row_number = true) ORDER BY file_row_number LIMIT ?""", [limit])
    return NestedGeneSummary(
        total_pairs=int(agg["total"]), direct_pairs=int(agg["direct"]), same_strand=int(agg["same_strand"]),
        opposite_strand=int(agg["opposite_strand"]), ambiguous_strand=int(agg["ambiguous_strand"]), by_type=by_type,
        distinct_hosts=int(agg["hosts"]), distinct_guests=int(agg["guests"]), first_pairs=rows)


def _track_path(track_service: DuckDBTrackService, build: str, name: str):
    path = track_service.data_dir / "tracks" / build / "all" / name
    if not path.exists():
        raise HTTPException(status_code=404, detail=f"Track '{name}' not found for build '{build}'")
    return path


@router.get("/region")
def get_region(
    build: str = Query(...),
    chrom: str = Query(...),
    start: int = Query(..., ge=0),
    end: int = Query(..., gt=0),
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    """Genes of gene_models.parquet on chrom overlapping [start, end) (gene.start < end and gene.end > start), ordered by
    start, at most 500, as a bare list {gene_id, gene_name, biotype, chrom, start, end, strand} (no truncation flag).
    Coordinates are the track's start_pos/end_pos. 404 when the track is missing; unknown contig gives []."""
    path = _track_path(track_service, build, "gene_models.parquet")
    return track_service.execute_query(f"""
        SELECT gene_id, gene_name, biotype, contig AS chrom,
               start_pos AS start, end_pos AS end, strand
        FROM read_parquet('{path}')
        WHERE contig = ? AND start_pos < ? AND end_pos > ?
        ORDER BY start_pos
        LIMIT 500
    """, [chrom, end, start])


@router.get("/density")
def get_density(
    build: str = Query(...),
    chrom: str = Query(...),
    bin_size: int = Query(1_000_000, gt=0),
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    """Number of genes per bin of bin_size bp on one contig, as [{start, count}] ordered by start. A gene is counted in the bin
    containing its start_pos (floor(start_pos / bin_size) * bin_size); empty bins are absent. Strand and gene length are
    ignored. 404 when the track is missing. Gene units: OPEN_QUESTIONS Q-gene-units."""
    path = _track_path(track_service, build, "gene_models.parquet")
    return track_service.execute_query(f"""
        SELECT CAST(FLOOR(start_pos / ?) * ? AS BIGINT) AS start,
               COUNT(*) AS count
        FROM read_parquet('{path}')
        WHERE contig = ?
        GROUP BY 1
        ORDER BY 1
    """, [bin_size, bin_size, chrom])


@router.get("/stats/sizes")
def get_size_stats(
    build: str = Query(...),
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    """Mean, median, min and max of length = end_pos - start_pos in bp for genes (gene_models.parquet), exons (exons.parquet)
    and introns (introns.parquet), as {genes, exons, introns}. A missing track gives all four values as 0.0, which is
    indistinguishable from a real zero. Gene lengths are affected by OPEN_QUESTIONS Q-gene-units."""
    def feature_stats(name: str):
        path = track_service.data_dir / "tracks" / build / "all" / name
        if not path.exists():
            return {"mean": 0.0, "median": 0.0, "min": 0.0, "max": 0.0}
        rows = track_service.execute_query(f"""
            SELECT AVG(end_pos - start_pos) AS mean,
                   median(end_pos - start_pos) AS median,
                   MIN(end_pos - start_pos) AS min,
                   MAX(end_pos - start_pos) AS max
            FROM read_parquet('{path}')
        """)
        stats = rows[0] if rows else {}
        return {key: float(stats.get(key, 0) or 0) for key in ("mean", "median", "min", "max")}

    return {
        "genes": feature_stats("gene_models.parquet"),
        "exons": feature_stats("exons.parquet"),
        "introns": feature_stats("introns.parquet"),
    }


SPACING_LIMIT = 1000  # rows returned by /spacing and /overlaps; the response reports total and truncated


@router.get("/spacing")
def get_spacing(
    build: str = Query(...),
    chrom: str | None = Query(None),
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    """Intergenic gaps from gene_spacing.parquet, largest first (distance = intergenic_dist, bp), optionally for one chrom, as
    {total, returned, truncated, items} capped at 1000 items. Each item gives the gap as [start, end) with end = start of
    gene 2 and start = end - distance (half-open, 0-based per code comment), plus gene_id_1 and gene_id_2. The covered-span
    definition is D39. 404 when a track is missing."""
    path = _track_path(track_service, build, "gene_spacing.parquet")
    models = _track_path(track_service, build, "gene_models.parquet")
    where = "WHERE s.contig = ?" if chrom else ""
    params = [chrom] if chrom else []
    # The gap is [end of the covering gene, start of gene 2) (half-open, 0-based);
    # the parquet keeps only its length, so the start of gene 2 (unique per (contig, gene_id)) gives both ends.
    total = track_service.execute_query(f"""
        SELECT COUNT(*) AS n FROM read_parquet('{path}') s
        JOIN read_parquet('{models}') m ON m.gene_id = s.gene2_id AND m.contig = s.contig
        {where}""", params)[0]["n"]
    items = track_service.execute_query(f"""
        SELECT s.contig AS chrom, s.gene1_id AS gene_id_1, s.gene2_id AS gene_id_2,
               CAST(m.start_pos AS BIGINT) - s.intergenic_dist AS start,
               CAST(m.start_pos AS BIGINT) AS end, s.intergenic_dist AS distance
        FROM read_parquet('{path}') s
        JOIN read_parquet('{models}') m ON m.gene_id = s.gene2_id AND m.contig = s.contig
        {where}
        ORDER BY s.intergenic_dist DESC, s.contig, s.gene1_id
        LIMIT {SPACING_LIMIT}
    """, params)
    return {"total": total, "returned": len(items), "truncated": total > len(items), "items": items}


@router.get("/overlaps")
def get_overlaps(
    build: str = Query(...),
    chrom: str | None = Query(None),
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    """Overlapping gene pairs from gene_overlaps.parquet, optionally for one chrom, ordered by chrom and gene ids, as {total,
    returned, truncated, items} capped at 1000 items. Each item has overlap_type ('same_strand' or 'opposite_strand'),
    overlap_len (bp, as stored) and contained (true when the track's contained = 1). 404 when the track is missing."""
    path = _track_path(track_service, build, "gene_overlaps.parquet")
    where = "WHERE contig = ?" if chrom else ""
    params = [chrom] if chrom else []
    total = track_service.execute_query(f"SELECT COUNT(*) AS n FROM read_parquet('{path}') {where}", params)[0]["n"]
    items = track_service.execute_query(f"""
        SELECT contig AS chrom, gene1_id AS gene_id_1, gene2_id AS gene_id_2,
               CASE WHEN same_strand = 1 THEN 'same_strand' ELSE 'opposite_strand' END AS overlap_type,
               overlap_len, contained = 1 AS contained
        FROM read_parquet('{path}')
        {where}
        ORDER BY chrom, gene_id_1, gene_id_2
        LIMIT {SPACING_LIMIT}
    """, params)
    return {"total": total, "returned": len(items), "truncated": total > len(items), "items": items}


@router.get("/cpg-association")
def get_cpg_association(
    build: str = Query(...),
    promoter_distance: int = Query(2000, gt=0),
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    """Counts genes with at least one CpG island within promoter_distance bp of the gene's start_pos, overall and per biotype
    (gene_models.parquet joined to cpg_islands.parquet on contig; island.start <= start_pos + d and island.end >= start_pos - d).
    Not strand-aware: start_pos is used for minus-strand genes too, where transcription starts at end_pos
    (OPEN_QUESTIONS Q-cpg point 3). Returns {with_cpg, without_cpg, total, percentage, by_biotype, note}; 404 when a track is missing."""
    genes = _track_path(track_service, build, "gene_models.parquet")
    cpg = _track_path(track_service, build, "cpg_islands.parquet")
    rows = track_service.execute_query(f"""
        SELECT g.biotype,
               COUNT(DISTINCT (g.contig, g.gene_id)) AS total,
               COUNT(DISTINCT CASE WHEN c.contig IS NOT NULL THEN (g.contig, g.gene_id) END) AS with_cpg
        FROM read_parquet('{genes}') g
        LEFT JOIN read_parquet('{cpg}') c
          ON c.contig = g.contig
         AND c.start_pos <= g.start_pos + {promoter_distance}
         AND c.end_pos >= g.start_pos - {promoter_distance}
        GROUP BY g.biotype
    """)
    total = sum(int(row.get("total", 0) or 0) for row in rows)
    with_cpg = sum(int(row.get("with_cpg", 0) or 0) for row in rows)
    by_biotype = {}
    for row in rows:
        biotype = row.get("biotype") or "unknown"
        group_total = int(row.get("total", 0) or 0)
        group_with = int(row.get("with_cpg", 0) or 0)
        by_biotype[biotype] = {
            "with_cpg": group_with,
            "without_cpg": max(0, group_total - group_with),
            "percentage": (group_with / group_total * 100) if group_total else 0,
        }
    return {
        "with_cpg": with_cpg,
        "without_cpg": max(0, total - with_cpg),
        "total": total,
        "percentage": (with_cpg / total * 100) if total else 0,
        "by_biotype": by_biotype,
        "note": f"Promoter association within {promoter_distance} bp of the annotated gene start.",
    }


@router.get("/{gene_id}")
def get_gene(
    gene_id: str,
    build: str = Query(...),
    chrom: str | None = Query(None),
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    """One gene by gene_id or gene_name (exact match) from gene_models.parquet with its transcripts (each with exons and, if
    utrs.parquet exists, utrs) and the architecture rows of gene_architecture.parquet. Genes present on several contigs
    (e.g. PAR) return the first copy by contig and start, or the copy on chrom, and list all other copies in other_copies.
    404 when no match (or none on chrom). Gene unit and first-transcript biotype/strand: OPEN_QUESTIONS Q-gene-units."""
    path = _track_path(track_service, build, "gene_models.parquet")
    rows = track_service.execute_query(f"""
        SELECT gene_id, gene_name, biotype, contig AS chrom,
               start_pos AS start, end_pos AS end, strand
        FROM read_parquet('{path}')
        WHERE (gene_id = ? OR gene_name = ?)
        ORDER BY contig, start_pos, gene_id
    """, [gene_id, gene_id])
    # PAR genes exist once per contig: the first copy (contig, start order) is returned unless chrom selects one, and
    # the remaining copies are always listed (also when chrom is given), never dropped silently.
    selected = [r for r in rows if r["chrom"] == chrom] if chrom else rows
    if not selected:
        raise HTTPException(status_code=404, detail=f"Gene '{gene_id}' not found")
    gene = selected[0]
    other_copies = [{"gene_id": r["gene_id"], "chrom": r["chrom"], "start": r["start"], "end": r["end"]}
                    for r in rows if r is not gene]
    gene_key = gene.get("gene_id", gene_id)
    base = track_service.data_dir / "tracks" / build / "all"
    transcripts_path = base / "transcripts.parquet"
    exons_path = base / "exons.parquet"
    utrs_path = base / "utrs.parquet"
    architecture_path = base / "gene_architecture.parquet"

    transcripts = []
    if transcripts_path.exists():
        tx_rows = track_service.execute_query(f"""
            SELECT transcript_id, gene_id, biotype,
                   start_pos AS start, end_pos AS end, strand
            FROM read_parquet('{transcripts_path}')
            WHERE gene_id = ? AND contig = ?
            ORDER BY start_pos
        """, [gene_key, gene["chrom"]])
        for tx in tx_rows:
            tx_id = tx["transcript_id"]
            tx["exons"] = []
            if exons_path.exists():
                tx["exons"] = track_service.execute_query(f"""
                    SELECT exon_number, start_pos AS start, end_pos AS end,
                           strand, transcript_id
                    FROM read_parquet('{exons_path}')
                    WHERE transcript_id = ?
                    ORDER BY start_pos
                """, [tx_id])
            if utrs_path.exists():
                tx["utrs"] = track_service.execute_query(f"""
                    SELECT start_pos AS start, end_pos AS end, strand,
                           transcript_id, utr_type
                    FROM read_parquet('{utrs_path}')
                    WHERE transcript_id = ?
                    ORDER BY start_pos
                """, [tx_id])
            transcripts.append(tx)

    architecture = []
    if architecture_path.exists():
        architecture = track_service.execute_query(f"""
            SELECT * FROM read_parquet('{architecture_path}')
            WHERE gene_id = ? AND contig = ?
            ORDER BY transcript_id
        """, [gene_key, gene["chrom"]])
    return {**gene, "other_copies": other_copies, "transcripts": transcripts, "architecture": architecture}
