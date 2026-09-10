from fastapi import APIRouter, Query

router = APIRouter()

@router.get("/genomes")
def get_genomes():
    return ["t2t", "chrm"]

@router.get("/health")
def get_health():
    return {"status": "ok", "version": "0.1.0"}

@router.get("/ready")
def get_ready():
    return {"status": "ready", "components": {"db": "ok"}}
