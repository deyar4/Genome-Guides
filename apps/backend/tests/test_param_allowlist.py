"""Every string request parameter must be covered by the input allowlist (app/core/validation.py): handlers build file paths
and SQL text from them, so a new string parameter has to be added to IDENTIFIER_PARAMS or FREE_TEXT_PARAMS."""
from app.core.validation import FREE_TEXT_PARAMS, IDENTIFIER_PARAMS
from app.main import app


def _is_string(schema: dict) -> bool:
    types = [schema.get("type")] + [a.get("type") for a in schema.get("anyOf", [])]
    return "string" in types


def test_all_string_parameters_are_allowlisted():
    uncovered = set()
    for path, ops in app.openapi()["paths"].items():
        for op in ops.values():
            for param in op.get("parameters", []):
                if _is_string(param["schema"]) and param["name"] not in IDENTIFIER_PARAMS | FREE_TEXT_PARAMS:
                    uncovered.add((param["name"], path))
    assert not uncovered, sorted(uncovered)
