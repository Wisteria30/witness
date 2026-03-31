# policy-approved: REQ-123 locale fallback is specified by product requirements
lang = LocalePolicy.default_locale(payload.get("lang", "ja-JP"))
