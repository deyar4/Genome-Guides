from fastapi import APIRouter, Query, Depends
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
    path = track_service.data_dir / "tracks" / build / "all" / "protein_lengths.parquet"
    if not path.exists():
        return ProteinStats(total_proteins=0, avg_length=0.0, median_length=0.0, max_length=0, min_length=0)
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
    try:
        path = track_service.data_dir / "tracks" / build / "all" / "amino_acid_composition.parquet"
        query = f"SELECT amino_acid, count, frequency_pct as percentage FROM read_parquet('{path}')"
        data = track_service.execute_query(query)
        return data if data else []
    except Exception:
        return []

@router.get("/stats/terminal")
def get_terminal_composition(
    build: str = Query(...),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    try:
        path = track_service.data_dir / "tracks" / build / "all" / "protein_terminal_bias.parquet"
        query = f"SELECT amino_acid, n_terminal_count, n_terminal_pct, c_terminal_count, c_terminal_pct, n_to_c_bias_ratio FROM read_parquet('{path}')"
        data = track_service.execute_query(query)
        return data if data else []
    except Exception:
        return []

@router.get("/top/largest")
def get_largest(
    build: str = Query(...),
    limit: int = Query(10, gt=0, le=100),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    path = track_service.data_dir / "tracks" / build / "all" / "protein_lengths.parquet"
    if not path.exists():
        return []
    rows = track_service.execute_query(
        f"""SELECT protein_id, length
            FROM read_parquet('{path}')
            ORDER BY length DESC, protein_id
            LIMIT {limit}"""
    )
    return [
        {
            "protein_id": row.get("protein_id", ""),
            "gene_id": row.get("protein_id", ""),
            "gene_name": None,
            "transcript_id": "",
            "sequence": "",
            "length": int(row.get("length", 0) or 0),
            "is_mitochondrial": False,
        }
        for row in rows
    ]

@router.get("/tracts/homopolymers", response_model=List[HomopolymerTract])
def get_homopolymers(
    build: str = Query(...),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    try:
        path = track_service.data_dir / "tracks" / build / "all" / "protein_homopolymers.parquet"
        query = f"SELECT amino_acid, start_pos as start, end_pos as end, repeat_len as length, protein_id FROM read_parquet('{path}')"
        data = track_service.execute_query(query)
        return data if data else []
    except Exception:
        return []


@router.get("/rich")
def get_rich(
    build: str,
    aa: str,
    threshold: float = 15,
    limit: int = 50,
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    path = track_service.data_dir / "tracks" / build / "all" / "protein_richness.parquet"
    if not path.exists():
        return []
    rows = track_service.execute_query(f"""
        SELECT protein_id, length, distinct_amino_acids, dominant_amino_acid,
               dominant_frequency_pct, shannon_entropy
        FROM read_parquet('{path}')
        WHERE UPPER(dominant_amino_acid) = '{aa.upper()}'
          AND dominant_frequency_pct >= {threshold}
        ORDER BY dominant_frequency_pct DESC, length DESC
        LIMIT {limit}
    """)
    return [
        {
            "protein_id": row.get("protein_id", ""),
            "gene_id": row.get("protein_id", ""),
            "gene_name": None,
            "transcript_id": "",
            "sequence": "",
            "length": int(row.get("length", 0) or 0),
            "is_mitochondrial": False,
            "distinct_amino_acids": int(row.get("distinct_amino_acids", 0) or 0),
            "dominant_amino_acid": row.get("dominant_amino_acid"),
            "dominant_frequency_pct": float(row.get("dominant_frequency_pct", 0) or 0),
            "shannon_entropy": float(row.get("shannon_entropy", 0) or 0),
        }
        for row in rows
    ]
