from pydantic import BaseModel
from typing import Dict, List, Optional

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
    chrom: str
    host_gene_id: str
    host_locus: int
    host_gene_name: Optional[str] = None
    host_coding_class: str  # protein_coding = some transcript has annotated CDS, else non_coding
    host_start: int
    host_end: int
    guest_gene_id: str
    guest_locus: int
    guest_gene_name: Optional[str] = None
    guest_coding_class: str
    guest_start: int
    guest_end: int
    relationship_type: str  # intronic | exonic_overlap | mixed | span_only
    strand_relation: str  # same | opposite | ambiguous
    is_direct: bool

class NestedGeneSummary(BaseModel):
    total_pairs: int
    direct_pairs: int
    same_strand: int
    opposite_strand: int
    ambiguous_strand: int
    by_type: Dict[str, int]
    distinct_hosts: int
    distinct_guests: int
    first_pairs: List[NestedGenePair]  # the first `limit` pairs in coordinate order, NOT a random sample
