# @torimi/protocol-handshake

The Torimi protocol version handshake: the matching logic that reconciles the bundle's encoder version with the host's decoder version, turning a mismatch into an explicit error. It is framework- and platform-independent and is shared by the web and native hosts.

This is a small matching library consumed transitively by the Torimi hosts, not installed directly by app authors.

Part of the Torimi/Tsubame lockstep release train — keep every `@hayate/*`, `@tsubame/*`, `@torimi/*`, `torimi`, and `create-torimi` package on the **same version**. Start at the [`torimi`](https://www.npmjs.com/package/torimi) README.

Alpha (0.x): no backward-compatibility guarantees.
