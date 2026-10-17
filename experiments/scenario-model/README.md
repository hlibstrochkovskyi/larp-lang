# Selective commit model

This separate Rust experiment tests the state transition proposed in the LARP
specification. It is not a language implementation or the future VM runtime.

```sh
cargo test -p larp-scenario-model --locked --offline
```

The model supports scalar field maps, performed writes, last claimed values,
idempotent field selection, final-value commit, and discard on drop/unwind.
Schema errors are Rust results; LARP would reject many of them statically.
Snapshots use sorted field names rather than language declaration order. Float
equality, arrays, options, and error values are outside this minimal model.

Rust's exclusive borrow of the root prevents concurrent real mutation here.
The LARP compiler must enforce its own alias/effect/region rules, and the VM
must provide view receivers and proper GC roots. These tests establish neither
compiler soundness nor correctness of the future runtime. A Rust unwind test
only checks model discard; LARP fatal errors must use explicit VM error paths.
