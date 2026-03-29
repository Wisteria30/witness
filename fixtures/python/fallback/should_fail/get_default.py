def read_timeout(config: dict) -> int:
    return config.get("timeout", 30)
