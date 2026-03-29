"""Example contract suite for lawful event store adapters."""

from __future__ import annotations


class EventStoreContract:
    def make_store(self):  # pragma: no cover - example only
        raise NotImplementedError

    def test_append_then_load(self):
        store = self.make_store()
        store.append("stream-1", {"type": "Created"})
        events = store.load("stream-1")
        assert [event["type"] for event in events] == ["Created"]
