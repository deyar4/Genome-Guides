from pydantic import BaseModel
from typing import List, Dict, Optional

class RNACluster(BaseModel):
    cluster_id: str
    rna_type: str
    chrom: str
    start: int
    end: int
    size: int
    genes: List[str]

class RNAClusterSummary(BaseModel):
    rna_type: str
    num_clusters: int
    avg_cluster_size: float
    max_cluster_size: int
    largest_cluster_location: str

class RNASummaryOverallStats(BaseModel):
    biotype_counts: Dict[str, int]
    cluster_counts: Dict[str, int]
    total_nc_rna: int
    total_clusters: int
    # biotype_counts counts transcripts (feature "transcript"/"mRNA") whose id is not NM_/XM_, by gene name or
    # attribute text (extract_rna_tracks.rna_biotype); it is not a count of genes.
    census_unit: str = "transcripts"

class RNASummary(BaseModel):
    overall_stats: RNASummaryOverallStats
    cluster_summaries: List[RNAClusterSummary]

class HostGuestRelationship(BaseModel):
    contig: str
    host_gene_id: str
    host_locus: int
    host_gene_name: Optional[str] = None
    guest_gene_id: str
    guest_locus: int
    guest_gene_name: Optional[str] = None
    guest_start: int
    guest_end: int
    guest_coding_class: str  # non_coding = no annotated CDS in any transcript
    relationship_type: str
    strand_relation: str
    is_same_strand: Optional[bool] = None  # None when the strand relation is ambiguous

class HostGuestResponse(BaseModel):
    total: int
    returned: int
    truncated: bool
    relationships: List[HostGuestRelationship]

class SpliceSiteConsensus(BaseModel):
    site_type: str
    consensus_seq: str
    pwm: Dict[str, List[float]]
    total_sites: int
