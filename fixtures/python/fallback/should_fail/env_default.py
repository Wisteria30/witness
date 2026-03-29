import os


def read_port() -> str:
    return os.getenv("PORT", "3000")
