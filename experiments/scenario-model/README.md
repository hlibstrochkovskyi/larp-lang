# Selective commit model

[Project overview](../../README.md) · [Scenario semantics](../../docs/spec/v0.1.md#8-hypothetical-state) · [Runtime architecture](../../docs/architecture.md#scenario-implementation)

This small Rust experiment isolates the baseline/performed/selection/claim
transition behind `larp`, `flex`, and `canon`. It is a reference model, separate
from the compiler and VM that now execute LARP scenarios.

Run its tests from the repository root:

```sh
cargo test -p larp-scenario-model --locked --offline
```

The model supports scalar field maps, performed writes, last claimed values,
idempotent field selection, final-value commit, and discard on drop/unwind.
Calling `canon` selects a field; a successful `commit` copies its final performed
value, including writes made after selection. Dropping an uncommitted scenario
leaves the baseline intact.

Schema errors are Rust results; the LARP compiler rejects many of them statically.
Snapshots use sorted field names rather than language declaration order. Float
equality, arrays, options, and error values are outside this minimal model.

Rust's exclusive borrow of the root prevents concurrent real mutation here.
The production compiler and VM enforce their own alias/effect/region rules and
manage view receivers and GC roots. These model tests establish neither compiler
soundness nor runtime correctness; see the separate
[scenario validation](../../docs/validation-scenarios.md) for executed language
checks. A Rust unwind test only checks model discard; LARP fatal errors use
explicit VM error paths.
