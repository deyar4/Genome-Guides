from pydantic import BaseModel
from typing import Optional

class ProteinStats(BaseModel):
    total_proteins: int
    avg_length: float
    median_length: float
    max_length: int
    min_length: int

class AminoAcidComposition(BaseModel):
    amino_acid: str
    count: int
    percentage: float

class HomopolymerTract(BaseModel):
    amino_acid: str
    start: int
    end: int
    length: int
    protein_id: Optional[str] = None
