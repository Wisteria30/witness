from src.infra.user_repo import SqlUserRepository


def build_service():
    return SqlUserRepository()
