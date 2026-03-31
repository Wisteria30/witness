// policy-approved: ADR-7 demo label fallback is documented
export const label = DemoLabelPolicy.resolve(apiValue ?? "demo")
