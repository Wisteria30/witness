class FakeUserRepository:
    def get(self, user_id: str) -> dict:
        return {"id": user_id}
