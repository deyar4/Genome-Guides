from fastapi import APIRouter, Depends, Query, HTTPException
import duckdb
from typing import List, Optional
from pathlib import Path
from app.api.dependencies import get_track_service
from app.services.duckdb_track_service import DuckDBTrackService
from app.core.track_meta import GC_PCT_SQL, GC_SKEW_SQL
from app.schemas.dna import (
    DNAStats, AssemblyStats, GenomeSection,
    ChromosomeListResponse, Chromosome,
    GenomeDinucleotides, DinucleotideStats,
    TelomereHit, CentromereRegion, AssemblyGapHit, CpGIslandHit,
    KmerResponse, KmerFrequency, SSRSummary, SSRHit, SSRDensityWindow,
    IsochoreStats
)

router = APIRouter()


def _require_window_multiple(track_service, path, size: int, what: str) -> None:
    """Bins must be whole multiples of the base window so each is an exact aggregate (D36)."""
    base = track_service.window_size(path)
    if size % base:
        raise HTTPException(status_code=422, detail=f"{what} must be a multiple of the base window ({base} bp)")


def _opt(value):
    """NULL stays None (explicit missing), never 0."""
    return None if value is None else float(value)


def _track_path(track_service: DuckDBTrackService, build: str, name: str) -> Path:
    return track_service.data_dir / "tracks" / build / "all" / name


def _read_track(track_service: DuckDBTrackService, build: str, name: str) -> Path | None:
    path = _track_path(track_service, build, name)
    return path if path.exists() else None

def _fasta_path(track_service: DuckDBTrackService, build: str) -> Path:
    path = track_service.data_dir / "raw" / f"{build}.fa"
    if not path.exists():
        raise HTTPException(status_code=404, detail=f"Sequence FASTA not found for build '{build}'")
    return path


def _fasta_index(path: Path) -> dict[str, tuple[int, int, int, int]]:
    index_path = Path(f"{path}.fai")
    if not index_path.exists():
        raise HTTPException(status_code=503, detail=f"FASTA index not found for '{path.name}'")
    result = {}
    for line in index_path.read_text().splitlines():
        name, length, offset, line_bases, line_width = line.split("\t")
        result[name] = (int(length), int(offset), int(line_bases), int(line_width))
    return result


def _read_sequence(track_service: DuckDBTrackService, build: str, chrom: str, start: int, end: int) -> str:
    if start < 0 or end <= start:
        raise HTTPException(status_code=422, detail="Sequence coordinates must satisfy 0 <= start < end")
    path = _fasta_path(track_service, build)
    index = _fasta_index(path)
    if chrom not in index:
        raise HTTPException(status_code=404, detail=f"Chromosome '{chrom}' not found")
    length, offset, line_bases, line_width = index[chrom]
    if end > length:
        raise HTTPException(status_code=422, detail=f"End coordinate exceeds {chrom} length ({length})")

    chunks = []
    with path.open("rb") as fasta:
        position = start
        while position < end:
            line_start = position // line_bases
            in_line = position % line_bases
            take = min(end - position, line_bases - in_line)
            fasta.seek(offset + line_start * line_width + in_line)
            chunks.append(fasta.read(take).decode("ascii"))
            position += take
    return "".join(chunks).upper()

@router.get("/stats/summary", response_model=DNAStats)
def get_dna_summary(
    build: str = Query(..., description="Genome build (e.g. t2t, chrm)"),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    import os
    stats_file = track_service.data_dir / "tracks" / build / "all" / "sequence_stats.txt"
    if not stats_file.exists():
        raise HTTPException(status_code=404, detail="Stats file not found")
        
    query = f"SELECT * FROM read_csv('{stats_file}', header=True, sep='\t')"
    res = track_service.execute_query(query)
    
    if not res:
        raise HTTPException(status_code=404, detail="No stats available")
        
    row = res[0]
    total_length = int(row.get('sum_len', 0))
    gc_pct = float(row.get('GC(%)', 0))
    
    assembly = AssemblyStats(
        total_length=total_length,
        contig_count=int(row.get('num_seqs', 0)),
        n50=int(row.get('N50', 0)),
        l50=int(row.get('L50', row.get('N50_num', 0))),
        gc_content=gc_pct
    )
    
    # Estimate base counts from gc_pct
    gc_count = int(total_length * (gc_pct / 100))
    at_count = total_length - gc_count
    
    nuclear = GenomeSection(
        a=at_count // 2,
        t=at_count // 2,
        g=gc_count // 2,
        c=gc_count // 2,
        n=int(row.get('sum_n', 0)),
        gc_content=gc_pct
    )
    return DNAStats(
        assembly=assembly,
        nuclear_composition=nuclear
    )

@router.get("/stats/chromosomes", response_model=ChromosomeListResponse)
def get_chromosomes(
    build: str = Query(..., description="Genome build"),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    try:
        path = track_service.data_dir / "tracks" / build / "all" / "gc_content.parquet"
        query = f"SELECT contig as name, MAX(end_pos) as length, {GC_PCT_SQL} as gc_content, {GC_SKEW_SQL} as gc_skew FROM read_parquet('{path}') GROUP BY contig"
        res = track_service.execute_query(query)
        
        chroms = []
        for row in res:
            name = row.get('name', '')
            chroms.append(Chromosome(
                name=name,
                length=int(row.get('length', 0)),
                gc_content=_opt(row.get('gc_content')),
                gc_skew=_opt(row.get('gc_skew')),
                is_mitochondrial=name.lower() in ["chrm", "mt"],
                ssr_count=0,
                dinucleotides={"AA": 0}
            ))
        return ChromosomeListResponse(count=len(chroms), chromosomes=chroms)
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail=f"GC content track not found for build '{build}'")

@router.get("/stats/dinucleotides", response_model=GenomeDinucleotides)
def get_dinucleotides(
    build: str = Query(..., description="Genome build"),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    try:
        path = track_service.data_dir / "tracks" / build / "all" / "dinucleotide_frequencies.parquet"
        res = track_service.execute_query(f"SELECT * FROM read_parquet('{path}')")
        
        dinucs = []
        total = 0
        for row in res:
            dinucs.append(DinucleotideStats(
                pair=row.get('dinucleotide', ''),
                observed_count=int(row.get('count', 0)),
                observed_freq=float(row.get('frequency_pct', 0)) / 100.0,
                expected_freq=float(row.get('frequency_pct', 0)) / 100.0 / max(1.0, float(row.get('odds_ratio', 1.0))),
                oe_ratio=float(row.get('odds_ratio', 0))
            ))
            total += int(row.get('count', 0))
            
        return GenomeDinucleotides(
            total_dinucleotides=total,
            base_frequencies={"A": 0.25, "C": 0.25, "G": 0.25, "T": 0.25},
            dinucleotides=dinucs
        )
    except duckdb.IOException:
        raise  # handled globally as 404 (no data for this build)
    except (duckdb.Error, OSError, ValueError) as exc:
        raise HTTPException(status_code=500, detail="Failed to query dinucleotide track") from exc

@router.get("/telomeres", response_model=List[TelomereHit])
def get_telomeres(
    build: str = Query(..., description="Genome build"),
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    path = _read_track(track_service, build, "telomeres.parquet")
    if path is None:
        return []
    rows = track_service.execute_query(
        f"""SELECT chrom, "start" AS position, "end", '.' AS strand, 0 AS repeat_count,
                   '' AS sequence, COALESCE(name, 'telomere') AS location, length
            FROM read_parquet('{path}') ORDER BY chrom, "start" """
    )
    return rows

@router.get("/centromeres", response_model=List[CentromereRegion])
def get_centromeres(
    build: str = Query(..., description="Genome build"),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    path = _read_track(track_service, build, "centromeres.parquet")
    if path is None:
        return []
    return track_service.execute_query(
        f"""SELECT chrom, "start", "end", length, NULL::DOUBLE AS gc_content,
                   NULL::VARCHAR AS sequence_sample
            FROM read_parquet('{path}') ORDER BY chrom, "start" """
    )


@router.get("/stats/assembly", response_model=AssemblyStats)
def get_assembly_stats(
    build: str = Query(...),
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    return get_dna_summary(build, track_service).assembly


@router.get("/sequence")
def get_sequence(
    build: str = Query(...),
    chrom: str = Query(...),
    start: int = Query(..., ge=0),
    end: int = Query(..., gt=0),
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    return _read_sequence(track_service, build, chrom, start, end)


@router.get("/stats/{chrom}/gc-skew")
def get_gc_skew(
    chrom: str,
    build: str = Query(...),
    window_size: int = Query(10000, gt=0, le=100_000_000),
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    path = track_service.data_dir / "tracks" / build / "all" / "gc_content.parquet"
    if not path.exists():
        raise HTTPException(status_code=404, detail=f"GC content track not found for build '{build}'")
    _require_window_multiple(track_service, path, window_size, "window_size")
    rows = track_service.execute_query(
        f"""SELECT CAST(FLOOR(start_pos / {window_size}) * {window_size} AS BIGINT) AS start,
                   {GC_SKEW_SQL} AS gc_skew
            FROM read_parquet('{path}')
            WHERE contig = '{chrom}'
            GROUP BY 1
            ORDER BY 1"""
    )
    return rows

@router.get("/gaps", response_model=List[AssemblyGapHit])
def get_gaps(
    build: str = Query(..., description="Genome build"),
    chrom: str = Query(..., description="Chromosome"),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    try:
        path = track_service.data_dir / "tracks" / build / "all" / "assembly_gaps.parquet"
        res = track_service.execute_query(f"SELECT * FROM read_parquet('{path}') WHERE contig = '{chrom}'")
        gaps = []
        for r in res:
            gaps.append(AssemblyGapHit(
                chrom=r.get("contig", ""),
                start=int(r.get("start_pos", 0)),
                end=int(r.get("end_pos", 0)),
                length=int(r.get("end_pos", 0)) - int(r.get("start_pos", 0)),
                gap_type=r.get("gap_type", "N")
            ))
        return gaps
    except duckdb.IOException:
        raise  # handled globally as 404 (no data for this build)
    except (duckdb.Error, OSError, ValueError) as exc:
        raise HTTPException(status_code=500, detail="Failed to query assembly gaps") from exc

@router.get("/cpg-islands", response_model=List[CpGIslandHit])
def get_cpg_islands(
    build: str = Query(..., description="Genome build"),
    chrom: Optional[str] = Query(None, description="Chromosome"),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    try:
        path = track_service.data_dir / "tracks" / build / "all" / "cpg_islands.parquet"
        query = f"SELECT * FROM read_parquet('{path}') WHERE is_cpg_island = 1"
        if chrom:
            query += f" AND contig = '{chrom}'"
        query += " LIMIT 1000"
        res = track_service.execute_query(query)
        islands = []
        for r in res:
            islands.append(CpGIslandHit(
                chrom=r.get("contig", ""),
                start=int(r.get("start_pos", 0)),
                end=int(r.get("end_pos", 0)),
                length=int(r.get("end_pos", 0)) - int(r.get("start_pos", 0)),
                gc_content=float(r.get("gc_pct", 0)),
                obs_exp_ratio=float(r.get("obs_exp_ratio", 0))
            ))
        return islands
    except duckdb.IOException:
        raise  # handled globally as 404 (no data for this build)
    except (duckdb.Error, OSError, ValueError) as exc:
        raise HTTPException(status_code=500, detail="Failed to query CpG islands") from exc

@router.get("/stats/{chrom}/kmers", response_model=KmerResponse)
def get_kmers(
    chrom: str,
    build: str = Query(..., description="Genome build"),
    k: int = Query(6, ge=3, le=6, description="K-mer size (3 or 6)"),
    top_n: int = Query(20, ge=1, le=4096, description="Top N kmers"),
    start: Optional[int] = Query(None, description="Ignored: k-mer counts are genome-wide (kept for client compatibility)"),
    end: Optional[int] = Query(None, description="Ignored: k-mer counts are genome-wide (kept for client compatibility)"),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    if k not in (3, 6):
        raise HTTPException(status_code=422, detail="k must be 3 or 6")
    try:
        path = track_service.data_dir / "tracks" / build / "all" / f"kmers_{k}mer.parquet"
        totals = track_service.execute_query(f"SELECT SUM(count) AS total, COUNT(*) AS n FROM read_parquet('{path}')")[0]
        res = track_service.execute_query(f"SELECT kmer, count, frequency_pct as frequency FROM read_parquet('{path}') ORDER BY count DESC LIMIT {top_n}")
        kmers = []
        for r in res:
            kmers.append(KmerFrequency(kmer=r.get("kmer",""), count=int(r.get("count",0)), frequency=float(r.get("frequency",0))))
        return KmerResponse(k=k, total_kmers=int(totals['total'] or 0), unique_kmers=int(totals['n'] or 0), kmers=kmers)
    except duckdb.IOException:
        raise  # handled globally as 404 (no data for this build)
    except (duckdb.Error, OSError, ValueError) as exc:
        raise HTTPException(status_code=500, detail="Failed to query k-mer track") from exc

@router.get("/ssr", response_model=List[SSRHit])
def get_ssr(
    build: str = Query(...),
    chrom: str = Query(...),
    start: Optional[int] = Query(None, ge=0),
    end: Optional[int] = Query(None, gt=0),
    min_repeats: Optional[int] = Query(None, ge=1),
    period: Optional[int] = Query(None, ge=1, le=6),
    limit: int = Query(100, gt=0, le=5000),
    offset: int = Query(0, ge=0),
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    path = _read_track(track_service, build, "ssr_hits.parquet")
    if path is None:
        return []
    filters, params = ["chrom = ?"], [chrom]
    if start is not None:
        filters.append('"end" > ?')
        params.append(start)
    if end is not None:
        filters.append('"start" < ?')
        params.append(end)
    if min_repeats is not None:
        filters.append("repeat_count >= ?")
        params.append(min_repeats)
    if period is not None:
        filters.append("period = ?")
        params.append(period)
    return track_service.execute_query(
        f"""SELECT chrom, "start", "end", motif, period, repeat_count, sequence,
                   length, period_name, complexity
            FROM read_parquet('{path}')
            WHERE {' AND '.join(filters)}
            ORDER BY "start", "end"
            LIMIT ? OFFSET ?""",
        params + [limit, offset],
    )

@router.get("/ssr/summary", response_model=List[SSRSummary])
def get_ssr_summary(
    build: str = Query(...),
    chrom: Optional[str] = Query(None),
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    path = _read_track(track_service, build, "ssr_summary.parquet")
    if path is None:
        return []
    return track_service.execute_query(f"SELECT * FROM read_parquet('{path}') ORDER BY period")

@router.get("/ssr/density", response_model=List[SSRDensityWindow])
def get_ssr_density(
    build: str = Query(...),
    chrom: str = Query(...),
    window_size: int = Query(1000000, gt=0),
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    path = _read_track(track_service, build, "ssr_density.parquet")
    if path is None:
        return []
    return track_service.execute_query(
        f"""SELECT chrom, "start", "end", count
            FROM read_parquet('{path}')
            WHERE chrom = ?
            ORDER BY "start" """,
        [chrom],
    )


@router.get("/ssr/coverage")
def get_ssr_coverage(
    build: str = Query(...),
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    """Descriptive SSR coverage per period (genome-wide, D41). No null model: not a measure of enrichment."""
    path = _read_track(track_service, build, "ssr_coverage.parquet")
    if path is None:
        return []
    return track_service.execute_query(f"SELECT * FROM read_parquet('{path}') ORDER BY period")


@router.get("/isochores", response_model=List[IsochoreStats])
def get_isochores(
    build: str = Query(...),
    chrom: Optional[str] = Query(None),
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    path = _read_track(track_service, build, "isochores.parquet")
    if path is None:
        return []
    where = f"WHERE chrom = '{chrom}'" if chrom else ""
    return track_service.execute_query(
        f"""SELECT chrom, "start", "end", gc_content, isochore_class
            FROM read_parquet('{path}') {where}
            ORDER BY chrom, "start" """
    )

@router.get("/sequence/window")
def get_sequence_window(
    build: str = Query(...),
    chrom: str = Query(...),
    start: int = Query(0, ge=0),
    end: Optional[int] = Query(None, gt=0),
    step: int = Query(10000, gt=0, le=100_000_000),
    limit: int = Query(1000, gt=0, le=5000),
    track_service: DuckDBTrackService = Depends(get_track_service),
):
    path = track_service.data_dir / "tracks" / build / "all" / "gc_content.parquet"
    if not path.exists():
        raise HTTPException(status_code=404, detail=f"GC content track not found for build '{build}'")
    _require_window_multiple(track_service, path, step, "step")
    upper = end if end is not None else start + step * limit
    rows = track_service.execute_query(
        f"""SELECT CAST(FLOOR(start_pos / {step}) * {step} AS BIGINT) AS start,
                   CAST(FLOOR(start_pos / {step}) * {step} + {step} AS BIGINT) AS end,
                   {GC_PCT_SQL} AS gc_content,
                   {GC_SKEW_SQL} AS gc_skew
            FROM read_parquet('{path}')
            WHERE contig = '{chrom}' AND start_pos < {upper} AND end_pos > {start}
            GROUP BY 1, 2
            ORDER BY 1
            LIMIT {limit}"""
    )
    return rows
