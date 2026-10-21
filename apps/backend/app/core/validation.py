"""Input allowlists for values that end up in file paths or SQL text.

Many endpoints build SQL with f-strings (audit 05, P5). Parameterising every query is the long-term fix; until then every
request is checked here, before any handler runs, so a quote, slash or wildcard can never reach a query or a path.
"""
import re

from fastapi import HTTPException, Request

# Genome builds, contigs, track names, RNA/site types: start alphanumeric (so ".." is impossible), no quotes, slashes, spaces.
IDENTIFIER = re.compile(r"^[A-Za-z0-9_][A-Za-z0-9_.\-]{0,63}$")
# Gene ids/names and search text: additionally allow space, colon, plus. Still no quote, percent, backslash, semicolon.
FREE_TEXT = re.compile(r"^[A-Za-z0-9_][A-Za-z0-9_.\-:+ ]{0,63}$")

IDENTIFIER_PARAMS = {
    "build", "genome", "contig", "chrom", "track_name", "rna_type", "site_type", "name",
}
FREE_TEXT_PARAMS = {"q", "gene_id"}


class InvalidTrackRequest(ValueError):
    """Request is well-formed HTTP but names something that cannot be a valid identifier or range."""


def check_identifier(value: str, what: str) -> str:
    if not IDENTIFIER.match(value):
        raise InvalidTrackRequest(f"Invalid {what}: only letters, digits, '_', '.', '-' (max 64) are allowed")
    return value


def validate_inputs(request: Request) -> None:
    """Global dependency: reject disallowed characters in query and path parameters with 422."""
    values = {**request.query_params, **request.path_params}
    for key, value in values.items():
        value = str(value)
        if key in IDENTIFIER_PARAMS:
            pattern = IDENTIFIER
        elif key in FREE_TEXT_PARAMS:
            pattern = FREE_TEXT
        else:
            continue
        if not pattern.match(value):
            raise HTTPException(status_code=422, detail=f"Invalid value for '{key}'")
