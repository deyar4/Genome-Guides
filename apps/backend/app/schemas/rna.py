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

class RNASummary(BaseModel):
    overall_stats: RNASummaryOverallStats
    cluster_summaries: List[RNAClusterSummary]

class HostGuestRelationship(BaseModel):
    host_gene_id: str
    host_gene_name: Optional[str] = None
    guest_gene_id: str
    guest_gene_name: Optional[str] = None
    guest_biotype: str
    relationship_type: str
    is_same_strand: bool

class SpliceSiteConsensus(BaseModel):
    site_type: str
    consensus_seq: str
    pwm: Dict[str, List[float]]
    total_sites: int
