from fastapi import APIRouter, Query, Depends, HTTPException
import duckdb
from typing import List, Dict, Any
from app.api.dependencies import get_track_service
from app.services.duckdb_track_service import DuckDBTrackService
from app.schemas.proteins import ProteinStats, AminoAcidComposition, HomopolymerTract

router = APIRouter()

@router.get("/stats/size", response_model=ProteinStats)
def get_size_stats(
    build: str = Query(...),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    """Protein length summary (residues) from protein_lengths.parquet: total_proteins, mean, median, max and min length.
    404 when the track is missing (it used to return all zeros, indistinguishable from a real zero)."""
    path = track_service.data_dir / "tracks" / build / "all" / "protein_lengths.parquet"
    if not path.exists():
        raise HTTPException(status_code=404, detail="Track 'protein_lengths.parquet' not found for build '" + build + "'")
    rows = track_service.execute_query(f"""
        SELECT COUNT(*) AS total_proteins,
               AVG(length) AS avg_length, MEDIAN(length) AS median_length,
               MAX(length) AS max_length, MIN(length) AS min_length
        FROM read_parquet('{path}')
    """)
    row = rows[0] if rows else {}
    return ProteinStats(
        total_proteins=int(row.get("total_proteins", 0) or 0),
        avg_length=float(row.get("avg_length", 0) or 0),
        median_length=float(row.get("median_length", 0) or 0),
        max_length=int(row.get("max_length", 0) or 0),
        min_length=int(row.get("min_length", 0) or 0),
    )

@router.get("/stats/composition", response_model=List[AminoAcidComposition])
def get_composition(
    build: str = Query(...),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    """Genome-wide amino acid composition from amino_acid_composition.parquet as [{amino_acid, count, percentage}] (percentage =
    the track's frequency_pct), in file order. Returns [] if the track is empty; a missing track is handled globally as 404."""
    try:
        path = track_service.data_dir / "tracks" / build / "all" / "amino_acid_composition.parquet"
        query = f"SELECT amino_acid, count, frequency_pct as percentage FROM read_parquet('{path}')"
        data = track_service.execute_query(query)
        return data if data else []
    except duckdb.IOException:
        raise  # handled globally as 404 (no data for this build)
    except (duckdb.Error, OSError, ValueError) as exc:
        raise HTTPException(status_code=500, detail="Failed to query protein composition") from exc

@router.get("/stats/terminal")
def get_terminal_composition(
    build: str = Query(...),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    """N- and C-terminal amino acid composition from protein_terminal_bias.parquet: per amino_acid the counts and
    percentages at each terminus and n_to_c_bias_ratio, as stored (UNVERIFIED: the terminal window length and the ratio
    definition, see the pipeline script). Returns [] if empty; a missing track is handled globally as 404."""
    try:
        path = track_service.data_dir / "tracks" / build / "all" / "protein_terminal_bias.parquet"
        query = f"SELECT amino_acid, n_terminal_count, n_terminal_pct, c_terminal_count, c_terminal_pct, n_to_c_bias_ratio FROM read_parquet('{path}')"
        data = track_service.execute_query(query)
        return data if data else []
    except duckdb.IOException:
        raise  # handled globally as 404 (no data for this build)
    except (duckdb.Error, OSError, ValueError) as exc:
        raise HTTPException(status_code=500, detail="Failed to query terminal composition") from exc

@router.get("/top/largest")
def get_largest(
    build: str = Query(...),
    limit: int = Query(10, gt=0, le=100),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    """The limit longest proteins from protein_lengths.parquet, by length (residues) descending then protein_id. Only protein_id
    and length are filled; gene_id, gene_name, transcript_id, sequence and is_mitochondrial are always null because the
    protein-to-gene mapping is unavailable (OPEN_QUESTIONS Q-protein-gene). 404 when the track is missing."""
    path = track_service.data_dir / "tracks" / build / "all" / "protein_lengths.parquet"
    if not path.exists():
        raise HTTPException(status_code=404, detail=f"Track 'protein_lengths.parquet' not found for build '{build}'")
    rows = track_service.execute_query(
        f"""SELECT protein_id, length
            FROM read_parquet('{path}')
            ORDER BY length DESC, protein_id
            LIMIT {limit}"""
    )
    return [
        {
            "protein_id": row.get("protein_id", ""),
            "gene_id": None,  # protein -> gene mapping not available (OPEN, Q-protein-gene)
            "gene_name": None,
            "transcript_id": None,
            "sequence": None,
            "length": int(row.get("length", 0) or 0),
            "is_mitochondrial": None,
        }
        for row in rows
    ]

@router.get("/tracts/homopolymers", response_model=List[HomopolymerTract])
def get_homopolymers(
    build: str = Query(...),
    min_length: int = Query(5, ge=1, description="Minimum tract length in residues"),
    limit: int = Query(100, gt=0, le=5000),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    """Single-amino-acid tracts of at least min_length residues from protein_homopolymers.parquet as {amino_acid, start, end,
    length, protein_id}, longest first, then protein_id and start, capped at limit with no total. start/end are the stored
    start_pos/end_pos (UNVERIFIED: coordinate base within the protein). Returns [] if empty; a missing track is handled
    globally as 404."""
    try:
        path = track_service.data_dir / "tracks" / build / "all" / "protein_homopolymers.parquet"
        query = f"""SELECT amino_acid, start_pos as start, end_pos as end, repeat_len as length, protein_id
                    FROM read_parquet('{path}') WHERE repeat_len >= ?
                    ORDER BY repeat_len DESC, protein_id, start_pos LIMIT ?"""
        data = track_service.execute_query(query, [min_length, limit])
        return data if data else []
    except duckdb.IOException:
        raise  # handled globally as 404 (no data for this build)
    except (duckdb.Error, OSError, ValueError) as exc:
        raise HTTPException(status_code=500, detail="Failed to query protein homopolymers") from exc


@router.get("/rich")
def get_rich(
    build: str,
    aa: str,
    threshold: float = 15,
    limit: int = Query(50, gt=0, le=1000),
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    """Proteins whose dominant amino acid is aa (case-insensitive) with dominant_frequency_pct >= threshold (percent, default
    15), from protein_richness.parquet, ordered by that percentage then length, capped at limit. Includes length,
    distinct_amino_acids and shannon_entropy as stored (log base UNVERIFIED here); gene_id, gene_name, transcript_id,
    sequence, is_mitochondrial are always null (OPEN_QUESTIONS Q-protein-gene). 404 when the track is missing."""
    path = track_service.data_dir / "tracks" / build / "all" / "protein_richness.parquet"
    if not path.exists():
        raise HTTPException(status_code=404, detail=f"Track 'protein_richness.parquet' not found for build '{build}'")
    rows = track_service.execute_query(f"""
        SELECT protein_id, length, distinct_amino_acids, dominant_amino_acid,
               dominant_frequency_pct, shannon_entropy
        FROM read_parquet('{path}')
        WHERE UPPER(dominant_amino_acid) = ?
          AND dominant_frequency_pct >= ?
        ORDER BY dominant_frequency_pct DESC, length DESC
        LIMIT ?
    """, [aa.upper(), threshold, limit])
    return [
        {
            "protein_id": row.get("protein_id", ""),
            "gene_id": None,  # protein -> gene mapping not available (OPEN, Q-protein-gene)
            "gene_name": None,
            "transcript_id": None,
            "sequence": None,
            "length": int(row.get("length", 0) or 0),
            "is_mitochondrial": None,
            "distinct_amino_acids": int(row.get("distinct_amino_acids", 0) or 0),
            "dominant_amino_acid": row.get("dominant_amino_acid"),
            "dominant_frequency_pct": float(row.get("dominant_frequency_pct", 0) or 0),
            "shannon_entropy": float(row.get("shannon_entropy", 0) or 0),
        }
        for row in rows
    ]
