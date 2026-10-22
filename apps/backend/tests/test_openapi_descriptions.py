from app.main import app


def test_every_operation_has_a_description():
    missing, todo = [], []
    for path, ops in app.openapi()["paths"].items():
        for method, op in ops.items():
            text = (op.get("description") or "").strip()
            if not text:
                missing.append(f"{method.upper()} {path}")
            if "TODO" in text:
                todo.append(f"{method.upper()} {path}")
    assert not missing, f"operations without description: {missing}"
    assert not todo, f"descriptions containing TODO: {todo}"
