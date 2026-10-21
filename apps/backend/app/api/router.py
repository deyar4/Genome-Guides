from fastapi import APIRouter, Depends

from app.core.validation import validate_inputs

from .endpoints import health, tracks, summary, dna, genes, rna, proteins, system

api_router = APIRouter(dependencies=[Depends(validate_inputs)])

api_router.include_router(health.router, tags=["health"])
api_router.include_router(tracks.router, prefix="/tracks", tags=["tracks"])
api_router.include_router(summary.router, prefix="/summary", tags=["summary"])

api_router.include_router(dna.router, prefix="/dna", tags=["dna"])
api_router.include_router(genes.router, prefix="/genes", tags=["genes"])
api_router.include_router(rna.router, prefix="/rna", tags=["rna"])
api_router.include_router(proteins.router, prefix="/proteins", tags=["proteins"])
api_router.include_router(system.router, prefix="/system", tags=["system"])
