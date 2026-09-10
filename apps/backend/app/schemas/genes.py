from pydantic import BaseModel
from typing import List, Optional

class BiotypeStat(BaseModel):
    biotype: str
    count: int

class GeneSummary(BaseModel):
    total_genes: int
    biotype_counts: List[BiotypeStat]
    avg_exons_per_transcript: float

class GeneChromosomeStat(BaseModel):
    chrom: str
    gene_count: int
    density: float

class TopGene(BaseModel):
    gene_id: str
    gene_name: Optional[str] = None
    biotype: Optional[str] = None
    chrom: str
    value: float
    secondary_value: Optional[float] = None
    rank: int
    start: Optional[int] = None
    end: Optional[int] = None
    strand: Optional[str] = None

class GeneSearchResponse(BaseModel):
    gene_id: str
    gene_name: Optional[str] = None
    biotype: Optional[str] = None
    chrom: str
    start: Optional[int] = None
    end: Optional[int] = None

class NestedGenePair(BaseModel):
    host_gene_id: str
    host_gene_name: Optional[str] = None
    host_biotype: Optional[str] = None
    guest_gene_id: str
    guest_gene_name: Optional[str] = None
    guest_biotype: Optional[str] = None
    chrom: str
    guest_start: int
    guest_end: int
    strand_relation: str

class NestedGeneSummary(BaseModel):
    total_pairs: int
    same_strand: int
    opposite_strand: int
    sample: List[NestedGenePair]
