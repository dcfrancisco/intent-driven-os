def hello_name(name: str) -> str:
    """Return a greeting for a non-empty name."""
    if not name.strip():
        raise ValueError("name must not be empty")
    return f"Hello, {name}!"
