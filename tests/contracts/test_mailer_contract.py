"""Example contract suite for lawful mailer adapters."""

from __future__ import annotations


class MailerContract:
    def make_mailer(self):  # pragma: no cover - example only
        raise NotImplementedError

    def test_send_records_delivery(self):
        mailer = self.make_mailer()
        delivery = mailer.send("to@example.com", "subject", "body")
        assert delivery.message_id
