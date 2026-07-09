# @hayate/protocol-spec

The Hayate–Tsubame Protocol Contract: the JSON spec (JSON Schema validated) that defines the wire contract between Hayate and Tsubame (ADR-0053). It is the single source of truth from which Tsubame's codegen derives wire constants and adapter vocabulary.

This is a spec package, not a runtime. It is consumed transitively by Tsubame's protocol codegen (`@tsubame/protocol-generated`) rather than installed directly by app authors.

Part of the Torimi/Tsubame lockstep release train — keep every `@hayate/*`, `@tsubame/*`, `@torimi/*`, `torimi`, and `create-torimi` package on the **same version**. Start at the [`torimi`](https://www.npmjs.com/package/torimi) README.

Alpha (0.x): no backward-compatibility guarantees.
