"""Every GET route must answer 200/404/422 (never 500) when the build has no data, and must reject injection payloads.

Parameters are filled from the OpenAPI schema, so new endpoints are covered automatically.
"""
import pytest
from fastapi.testclient import TestClient

from app.main import app

client = TestClient(app, raise_server_exceptions=False)
SPEC = client.get("/api/v1/openapi.json").json()


def sample(schema, name, evil=False):
    if name in ("build", "genome"):
        return "no_such_build"
    t = schema.get("type") or (schema.get("anyOf") or [{}])[0].get("type")
    if t == "integer":
        return max(schema.get("minimum", 0), schema.get("exclusiveMinimum", 0) + 1 if "exclusiveMinimum" in schema else 1)
    if t == "number":
        return 1.0
    if t == "boolean":
        return "true"
    return "x' OR '1'='1" if evil else "chr1"


def cases(evil):
    for path, item in SPEC["paths"].items():
        op = item.get("get")
        if not op:
            continue
        url, params = path, {}
        for p in op.get("parameters", []):
            if not p.get("required") and not evil:
                continue
            v = sample(p["schema"], p["name"], evil)
            if p["in"] == "path":
                url = url.replace("{%s}" % p["name"], str(v))
            else:
                params[p["name"]] = v
        yield pytest.param(url, params, id=path)


@pytest.mark.parametrize("url,params", list(cases(False)))
def test_no_data_never_500(url, params):
    assert client.get(url, params=params).status_code != 500


@pytest.mark.parametrize("url,params", list(cases(True)))
def test_injection_payload_never_200_or_500(url, params):
    has_text = any(v == "x' OR '1'='1" for v in params.values()) or "x'" in url
    status = client.get(url, params=params).status_code
    assert status != 500
    if has_text:
        assert status in (404, 422)


def test_every_string_parameter_is_allowlisted():
    """A new free-text parameter that reaches SQL must be added to app/core/validation.py, or this test fails."""
    from app.core.validation import FREE_TEXT_PARAMS, IDENTIFIER_PARAMS
    covered = IDENTIFIER_PARAMS | FREE_TEXT_PARAMS
    missing = set()
    for path, item in SPEC["paths"].items():
        for p in item.get("get", {}).get("parameters", []):
            schema = p["schema"]
            types = {schema.get("type")} | {a.get("type") for a in schema.get("anyOf", [])}
            if "string" in types and p["name"] not in covered:
                missing.add((path, p["name"]))
    assert not missing, missing
