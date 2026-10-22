from typing import List, Optional, Dict
from pydantic import BaseModel

class AssemblyStats(BaseModel):
    total_length: int
    contig_count: int
    n50: int
    l50: int
    n90: Optional[int] = None
    l90: Optional[int] = None
    min_len: Optional[int] = None
    max_len: Optional[int] = None
    mean_length: Optional[float] = None
    median_length: Optional[float] = None
    gc_content: Optional[float] = None

class GenomeSection(BaseModel):
    a: int
    c: int
    g: int
    t: int
    n: int
    gc_content: float

class DNAStats(BaseModel):
    assembly: AssemblyStats
    nuclear_composition: GenomeSection
    mitochondrial_composition: Optional[GenomeSection] = None
    gene_count: Optional[int] = None
    protein_coding_count: Optional[int] = None

class Chromosome(BaseModel):
    name: str
    length: int
    gc_content: Optional[float] = None
    gc_skew: Optional[float] = None
    is_mitochondrial: bool
    ssr_count: Optional[int] = None

class ChromosomeListResponse(BaseModel):
    count: int
    chromosomes: List[Chromosome]

class DinucleotideStats(BaseModel):
    pair: str
    observed_count: int
    observed_freq: float
    expected_freq: float
    oe_ratio: float

class GenomeDinucleotides(BaseModel):
    total_dinucleotides: int
    base_frequencies: Dict[str, float]
    dinucleotides: List[DinucleotideStats]

class TelomereHit(BaseModel):
    """Interval from the T2T telomere BED (half-open, 0-based). The BED has no strand, repeat count or sequence."""
    chrom: str
    start: int
    end: int
    length: int
    source: str

class CentromereRegion(BaseModel):
    chrom: str
    start: int
    end: int
    length: int
    gc_content: Optional[float] = None
    sequence_sample: Optional[str] = None

class AssemblyGapHit(BaseModel):
    chrom: str
    start: int
    end: int
    length: int
    gap_type: str

class CpGIslandHit(BaseModel):
    chrom: str
    start: int
    end: int
    length: int
    gc_content: float
    obs_exp_ratio: float
    cpg_count: int
    n_windows: int
    strict: bool

class CpGIslandResponse(BaseModel):
    total: int  # all islands matching the filters, before limit/offset
    returned: int
    truncated: bool  # True iff offset + returned < total
    islands: List[CpGIslandHit]

class KmerFrequency(BaseModel):
    kmer: str
    count: int
    frequency: float

class KmerResponse(BaseModel):
    k: int
    total_kmers: int
    unique_kmers: int
    kmers: List[KmerFrequency]
    # Convention of packages/pipeline/scripts/common/count_kmers.py: whole assembly (the chrom path segment and
    # start/end are ignored), forward strand as written (not canonical, no reverse-complement merge), upper-cased
    # (soft-masking ignored), only A/C/G/T k-mers; a k-mer never spans an N, another symbol or a contig boundary.
    scope: str = "genome-wide"
    strand: str = "forward, not canonical"
    alphabet: str = "ACGT only; k-mers containing N are skipped; soft-masked bases counted"

class SSRSummary(BaseModel):
    period: int
    name: str
    count: int
    total_length: int
    avg_repeat_count: float

class SSRHit(BaseModel):
    chrom: str
    start: int
    end: int
    motif: str
    period: int
    repeat_count: int
    sequence: str
    length: int
    period_name: str
    complexity: float

class SSRDensityWindow(BaseModel):
    chrom: str
    start: int
    end: int
    count: int

class IsochoreStats(BaseModel):
    chrom: str
    start: int
    end: int
    gc_content: float
    isochore_class: str

class SequenceWindow(BaseModel):
    chrom: str
    start: int
    end: int
    sequence: str

