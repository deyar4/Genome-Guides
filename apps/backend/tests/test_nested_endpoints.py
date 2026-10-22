"""/genes/nested/summary and /rna/relationships over a small nested_gene_pairs fixture."""
import duckdb
import pytest
from fastapi.testclient import TestClient

from app.api.dependencies import get_track_service
from app.main import app
from app.services.duckdb_track_service import DuckDBTrackService

COLS = ("contig, host_gene_id, host_locus, host_gene_name, host_start, host_end, host_coding_class, "
        "guest_gene_id, guest_locus, guest_gene_name, guest_start, guest_end, guest_coding_class, "
        "relationship_type, strand_relation, is_direct")
ROWS = [
    ("chr2", "H1", 0, "h1", 0, 1000, "protein_coding", "G1", 0, None, 100, 200, "non_coding", "intronic", "opposite", True),
    ("chr2", "H1", 0, "h1", 0, 1000, "protein_coding", "G2", 0, "g2", 300, 400, "protein_coding", "exonic_overlap", "same", True),
    ("chr10", "H3", 0, "h3", 0, 500, "non_coding", "G4", 1, "g4", 10, 20, "non_coding", "mixed", "ambiguous", False),
    ("chr10", "P", 0, "p", 0, 900, "protein_coding", "G5", 0, "g5", 10, 20, "non_coding", "intronic", "same", True),
]


@pytest.fixture(scope="module")
def client(tmp_path_factory):
    root = tmp_path_factory.mktemp("data")
    out = root / "tracks" / "s" / "all"
    out.mkdir(parents=True)
    def lit(v):
        return "NULL" if v is None else (f"'{v}'" if isinstance(v, str) else str(v).upper() if isinstance(v, bool) else str(v))
    values = ", ".join("(" + ", ".join(lit(v) for v in row) + ")" for row in ROWS)
    duckdb.connect().execute(f"COPY (SELECT * FROM (VALUES {values}) t({COLS})) TO '{out}/nested_gene_pairs.parquet' (FORMAT PARQUET)")
    service = DuckDBTrackService(str(root))
    app.dependency_overrides[get_track_service] = lambda: service
    yield TestClient(app)
    app.dependency_overrides.clear()


def test_summary_counts_and_first_pairs_in_file_order(client):
    r = client.get("/api/v1/genes/nested/summary", params={"build": "s", "limit": 2}).json()
    assert (r["total_pairs"], r["direct_pairs"], r["same_strand"], r["opposite_strand"], r["ambiguous_strand"]) == (4, 3, 2, 1, 1)
    assert r["by_type"] == {"exonic_overlap": 1, "intronic": 2, "mixed": 1}
    assert (r["distinct_hosts"], r["distinct_guests"]) == (3, 4)
    assert [p["guest_gene_id"] for p in r["first_pairs"]] == ["G1", "G2"]
    assert r["first_pairs"][0]["guest_gene_name"] is None  # missing names stay null


def test_relationships_only_guests_without_cds_and_paging(client):
    r = client.get("/api/v1/rna/relationships", params={"build": "s", "limit": 2}).json()
    assert (r["total"], r["returned"], r["truncated"]) == (3, 2, True)
    assert [x["guest_gene_id"] for x in r["relationships"]] == ["G1", "G4"]
    assert [x["is_same_strand"] for x in r["relationships"]] == [False, None]
    r = client.get("/api/v1/rna/relationships", params={"build": "s", "limit": 2, "offset": 2}).json()
    assert (r["returned"], r["truncated"]) == (1, False)


def test_relationships_filters_and_missing_track(client):
    assert client.get("/api/v1/rna/relationships", params={"build": "s", "contig": "chr10"}).json()["total"] == 2
    assert client.get("/api/v1/rna/relationships", params={"build": "s", "host_gene_id": "P", "contig": "chr2"}).json()["total"] == 0
    assert client.get("/api/v1/rna/relationships", params={"build": "nope"}).status_code == 404
    assert client.get("/api/v1/genes/nested/summary", params={"build": "nope"}).status_code == 404
    assert client.get("/api/v1/rna/relationships", params={"build": "s", "limit": 0}).status_code == 422
