type Outcome = { kind: "found"; value: string } | { kind: "missing" }

export function render(outcome: Outcome): string {
  switch (outcome.kind) {
    case "found":
      return outcome.value
    case "missing":
      return "missing"
    default: {
      const neverValue: never = outcome
      return neverValue
    }
  }
}
