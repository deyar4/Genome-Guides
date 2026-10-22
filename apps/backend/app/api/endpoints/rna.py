from fastapi import APIRouter, Depends, HTTPException, Query
from typing import List, Dict, Any, Optional

from app.api.dependencies import get_track_service
from app.services.duckdb_track_service import DuckDBTrackService
from app.schemas.rna import (
    RNASummary,
    RNACluster,
    HostGuestRelationship,
    HostGuestResponse,
    SpliceSiteConsensus
)

router = APIRouter()

@router.get("/stats/summary", response_model=RNASummary)
def get_summary(
    build: str = Query(...),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    base = track_service.data_dir / "tracks" / build / "all"
    census_path = base / "ncrna_detailed_census.parquet"
    cluster_summary_path = base / "rna_cluster_summary.parquet"
    census = track_service.execute_query(
        f"SELECT ncrna_subclass, count FROM read_parquet('{census_path}')"
    ) if census_path.exists() else []
    summaries = track_service.execute_query(
        f"""SELECT rna_type, num_clusters, avg_cluster_size, max_cluster_size,
                   largest_cluster_location
            FROM read_parquet('{cluster_summary_path}')
            ORDER BY rna_type"""
    ) if cluster_summary_path.exists() else []
    biotype_counts = {
        row.get("ncrna_subclass", "unknown"): int(row.get("count", 0) or 0)
        for row in census
    }
    cluster_counts = {
        row.get("rna_type", "unknown"): int(row.get("num_clusters", 0) or 0)
        for row in summaries
    }

    return {
        "overall_stats": {
            "biotype_counts": biotype_counts,
            "cluster_counts": cluster_counts,
            "total_nc_rna": sum(biotype_counts.values()),
            "total_clusters": sum(cluster_counts.values())
        },
        "cluster_summaries": summaries
    }

@router.get("/clusters", response_model=List[RNACluster])
def get_clusters(
    build: str = Query(...),
    rna_type: Optional[str] = Query(None),
    chrom: Optional[str] = Query(None),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    path = track_service.data_dir / "tracks" / build / "all" / "rna_clusters.parquet"
    if not path.exists():
        return []
    filters, params = [], []
    if rna_type:
        filters.append("rna_type = ?")
        params.append(rna_type)
    if chrom:
        filters.append("chrom = ?")
        params.append(chrom)
    where = f"WHERE {' AND '.join(filters)}" if filters else ""
    rows = track_service.execute_query(
        f"""SELECT cluster_id, rna_type, chrom, "start", "end", size, genes
            FROM read_parquet('{path}')
            {where}
            ORDER BY chrom, start
            LIMIT 5000""",
        params,
    )
    for row in rows:
        genes = row.get("genes") or ""
        row["genes"] = genes.split(",") if isinstance(genes, str) and genes else []
    return rows

@router.get("/relationships", response_model=HostGuestResponse)
def get_relationships(
    build: str = Query(...),
    contig: Optional[str] = Query(None, description="gene_id alone is ambiguous for PAR genes; without contig all matches are returned"),
    host_gene_id: Optional[str] = Query(None),
    guest_gene_id: Optional[str] = Query(None),
    limit: int = Query(100, ge=1, le=1000),
    offset: int = Query(0, ge=0),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    """Nested pairs whose guest has no annotated CDS in any transcript (docs/analyses/02_nested_genes.md, D43).
    'non_coding' here is not an RNA subclass; the annotation gives none."""
    path = track_service.data_dir / "tracks" / build / "all" / "nested_gene_pairs.parquet"
    if not path.exists():
        raise HTTPException(status_code=404, detail=f"Track 'nested_gene_pairs.parquet' not found for build '{build}'")
    where, params = ["guest_coding_class = 'non_coding'"], []
    for column, value in (("contig", contig), ("host_gene_id", host_gene_id), ("guest_gene_id", guest_gene_id)):
        if value:
            where.append(f"{column} = ?")
            params.append(value)
    clause = " AND ".join(where)
    total = int(track_service.execute_query(
        f"SELECT COUNT(*) AS n FROM read_parquet('{path}') WHERE {clause}", params)[0]["n"])
    rows = track_service.execute_query(f"""
        SELECT contig, host_gene_id, host_locus, host_gene_name, guest_gene_id, guest_locus, guest_gene_name,
               guest_start, guest_end, guest_coding_class, relationship_type, strand_relation,
               CASE strand_relation WHEN 'same' THEN TRUE WHEN 'opposite' THEN FALSE ELSE NULL END AS is_same_strand
        FROM read_parquet('{path}', file_row_number = true) WHERE {clause}
        ORDER BY file_row_number LIMIT ? OFFSET ?""", params + [limit, offset])
    return HostGuestResponse(total=total, returned=len(rows), truncated=offset + len(rows) < total, relationships=rows)

@router.get("/splice-sites", response_model=List[SpliceSiteConsensus])
def get_splice_consensus(
    build: str = Query(...),
    site_type: Optional[str] = Query(None),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    path = track_service.data_dir / "tracks" / build / "all" / "splice_site_consensus.parquet"
    if not path.exists():
        return []
    where = "WHERE site_type = ?" if site_type else ""
    rows = track_service.execute_query(
        f"""SELECT site_type, consensus_seq, pwm, total_sites
            FROM read_parquet('{path}') {where}
            ORDER BY site_type""",
        [site_type] if site_type else [],
    )
    for row in rows:
        pwm = row.get("pwm") or {}
        row["pwm"] = {
            base: [float(value) for value in (pwm.get(base) or [])]
            for base in "ACGT"
        }
        row["total_sites"] = int(row.get("total_sites", 0) or 0)
    return rows


@router.get("/splice-sites/records")
def get_splice_sites(
    build: str = Query(...),
    site_type: Optional[str] = Query(None),
    chrom: Optional[str] = Query(None),
    limit: int = Query(1000, gt=0, le=5000),
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    path = track_service.data_dir / "tracks" / build / "all" / "splice_sites.parquet"
    if not path.exists():
        return []
    filters, params = [], []
    if site_type == "donor":
        filters.append("donor_seq IS NOT NULL")
    elif site_type == "acceptor":
        filters.append("acceptor_seq IS NOT NULL")
    if chrom:
        filters.append("contig = ?")
        params.append(chrom)
    where = f"WHERE {' AND '.join(filters)}" if filters else ""
    return track_service.execute_query(
        f"""SELECT contig AS chrom, intron_start AS start, intron_end AS end,
                   transcript_id, gene_id, strand, junction_number,
                   donor_seq, acceptor_seq, is_canonical
            FROM read_parquet('{path}') {where}
            ORDER BY contig, intron_start
            LIMIT {limit}""",
        params,
    )
