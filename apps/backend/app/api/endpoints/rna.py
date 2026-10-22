from fastapi import APIRouter, Depends, HTTPException, Query
from typing import List, Dict, Any, Optional
from pydantic import BaseModel

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
    """Non-coding RNA overview: overall_stats {biotype_counts per ncrna_subclass from ncrna_detailed_census.parquet, cluster_counts
    per rna_type, total_nc_rna, total_clusters} and cluster_summaries rows from rna_cluster_summary.parquet. Each part is
    empty when its track is missing; no 404 is raised."""
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

CLUSTER_LIMIT = 5000  # rows returned by /clusters; the response reports total and truncated


class RNAClusterResponse(BaseModel):
    total: int
    returned: int
    truncated: bool
    clusters: List[RNACluster]


@router.get("/clusters", response_model=RNAClusterResponse)
def get_clusters(
    build: str = Query(...),
    rna_type: Optional[str] = Query(None),
    chrom: Optional[str] = Query(None),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    """RNA gene clusters from rna_clusters.parquet filtered by rna_type and/or chrom, ordered by chrom, start, cluster_id.
    Returns {total, returned, truncated, clusters}, capped at 5000. genes is the comma-separated stored string split into a
    list. Coordinates are returned as stored in the track. Empty response (not 404) when the track is missing."""
    path = track_service.data_dir / "tracks" / build / "all" / "rna_clusters.parquet"
    if not path.exists():
        return RNAClusterResponse(total=0, returned=0, truncated=False, clusters=[])
    filters, params = [], []
    if rna_type:
        filters.append("rna_type = ?")
        params.append(rna_type)
    if chrom:
        filters.append("chrom = ?")
        params.append(chrom)
    where = f"WHERE {' AND '.join(filters)}" if filters else ""
    total = track_service.execute_query(f"SELECT COUNT(*) AS n FROM read_parquet('{path}') {where}", params)[0]["n"]
    rows = track_service.execute_query(
        f"""SELECT cluster_id, rna_type, chrom, "start", "end", size, genes
            FROM read_parquet('{path}')
            {where}
            ORDER BY chrom, start, cluster_id
            LIMIT {CLUSTER_LIMIT}""",
        params,
    )
    for row in rows:
        genes = row.get("genes") or ""
        row["genes"] = genes.split(",") if isinstance(genes, str) and genes else []
    return RNAClusterResponse(total=total, returned=len(rows), truncated=total > len(rows), clusters=rows)

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
    """Splice-site consensus from splice_site_consensus.parquet as [{site_type, consensus_seq, pwm, total_sites}], optionally for
    one site_type, ordered by site_type. pwm maps each of A, C, G, T to a list of per-position values as stored
    (UNVERIFIED: whether they are frequencies or counts). Returns [] when the track is missing."""
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
    """Annotated intron junction records from splice_sites.parquet as {chrom, start, end, transcript_id, gene_id, strand,
    junction_number, donor_seq, acceptor_seq, is_canonical}, where start/end are the stored intron_start/intron_end.
    site_type 'donor' or 'acceptor' keeps rows with that sequence non-null (other values are ignored); chrom filters the
    contig. Ordered by chrom and start, bare list capped at limit with no total. Returns [] when the track is missing."""
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
