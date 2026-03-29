def read_user_name(fetcher):
    try:
        return fetcher()
    except KeyError:
        return "unknown"
