"""Example contract suite for a lawful runtime adapter.

These files are documentation-grade examples for consumers of the plugin.
They show the shape of a contract suite that can back entries in
policy/adapters.yml.
"""

from __future__ import annotations


class UserRepositoryContract:
    def make_repo(self):  # pragma: no cover - example only
        raise NotImplementedError

    def test_round_trip(self):
        repo = self.make_repo()
        user = {"id": "u-1", "name": "Ada"}
        repo.save(user)
        assert repo.get("u-1") == user
