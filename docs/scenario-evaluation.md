# Scenario evaluation cases

Status: design comparison, not compiled evidence. The Rust model exercises scalar
transitions separately; these sketches still require the real compiler and VM.
The purpose is to test whether `larp` earns its cost beyond the Founder example.

## Configuration tuning without applying I/O

Suppose `Plan` has two independent public fields: `mf workers` and
`mf retryDelayMs`, initialized to 2 and 1000. A pure estimate takes value inputs;
it does not benchmark a running service or contact an external system.

Ordinary explicit-copy version:

```larp
Plan trial = new Plan(current.workers, current.retryDelayMs);
trial.workers = 8;
trial.retryDelayMs = 250;
mf estimatedCost = trial.workers * 12;
if (estimatedCost <= budget) {
    current.workers = trial.workers;
}
```

Scenario version, with `mf budget = 100;` computed before entry:

```larp
larp (current) as CapacityTrial {
    flex current.workers = 8;
    current.retryDelayMs = 250;
    mf estimatedCost = current.workers * 12;
    if (estimatedCost <= budget) {
        canon current.workers;
    }
    palantir(current);
}
```

Expected real state afterward: workers=8, retryDelayMs=1000. With budget=90,
neither field changes. A scenario gives built-in baseline/performed inspection,
conditional field selection, and discard on failure. The explicit copy is already
short and understandable. The scenario benefit is primarily auditable intent;
brevity alone is not a convincing reason for new syntax.

Rejected variations: reading an outer live configuration object, sending an HTTP
request to measure capacity, or assigning `estimatedCost` to an outer variable.
Precompute value inputs, emit an explicit observation, or keep the whole workflow
outside a scenario. The restriction is intentional and must be measured for pain.

## Game-stat trial

Assume a `Stats` root with independent public `mf power` and `mf visualBonus`,
initially 10 and 0. Neither field encodes inventory or a conservation rule.

```larp
larp (stats) as TrainingTrial {
    stats.power += 5;
    flex stats.visualBonus = 500;
    mf projectedScore = stats.power * 2;
    if (projectedScore >= 30) {
        canon stats.power;
    }
    palantir(stats);
}
```

Expected real state: power=15, visualBonus=0. A fatal check before the closing
brace keeps both original fields. Reaching the brace with an unhandled
`fumble(...)` value still commits selected fields: a result does not cancel a block.

The useful distinction is between a claimed visual bonus and accepted power.
No scenario-local score is allowed to escape into an ordinary saved-score table.
An outer alias sees the committed power only after block completion; it cannot
be read inside the block.

## Counterexample: correlated account fields

Baseline balance=100 and reserved=20 satisfies an application policy. The performed
state sets balance=10 and reserved=0; selecting only balance produces balance=10,
reserved=20. Both fields are valid integers; their relationship is wrong.

The current proposal cannot prevent this. A public method that preserves the
relationship in P is insufficient if canon selects only half of its changes.
Do not advertise selective commit as preserving arbitrary object invariants.
Possible future solutions include grouped fields, whole-object commit, or a pure
validation predicate over the fully prepared next state. Each changes the contract
and needs its own RFC. Rejecting all useful state relationships would also make
the feature too weak; this is a real open product question.

## Decision criteria

Before expanding beyond value-only roots, implement both versions of these cases
through the compiler/VM and record:

- Whether a reader can predict which fields persist without tracking manual copies.
- The number of explicit snapshot/copy/commit operations and diagnostic quality.
- Whether restrictions force calculations outside the block so often that it adds little.
- Behavior under injected failure, alias attempts, and unsafe indirect calls.
- Time and memory overhead relative to explicit snapshots on measured workloads.

The current conclusion is provisional: lexical state isolation is implementable
under these restrictions, but its usefulness is still unproven. Keep the language
project independent of the experiment's outcome.
