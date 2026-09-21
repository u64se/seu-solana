# Recovery Drain & Grace Timeout

A common criticism of strict acyclic state machines is the dead-lock hazard:
> *"If an agent is strictly forbidden from stepping backward ($S_{i+1} > S_i$), what happens if an oracle fails to respond at step S3? Are the agent's funds locked forever?"*

---

## The Grace Timeout Mechanism

To eliminate deadlock while preserving mathematical acyclicity during normal operation, `u64se-seu-solana` includes a formal **Grace Timeout / Emergency Drain Invariant** (formally proven in Theorem 4).

```rust
pub const INACTIVITY_TIMEOUT_SLOTS: u16 = 100; // ~40 seconds on Solana
```

### The Three Strict Prerequisites for Recovery Drain
A backward transition to $S_0$ is permitted if and only if **all three conditions** are simultaneously satisfied:

1. **Temporal Expiration**:
   $$\Delta_{\text{slot}} = \text{current\_slot.wrapping\_sub}(\text{anchor\_slot}) > 100$$
   The state machine has been completely inactive for at least 100 slots (~40 seconds).
2. **Authorized Role**:
   $$\text{requested\_role} = \text{ROLE\_DRAIN (0x10)}$$
   Only the authority holding recovery rights can trigger the drain.
3. **Canonical Reset Target**:
   $$S_{i+1} = 0$$
   The destination must strictly be Genesis node $0$, unlocking and refunding escrowed funds to the owner.

If an attacker attempts to call `ROLE_DRAIN` while $\Delta \le 100$, or attempts to drain to any node other than $0$, the attempt is blocked with `FaultMask::BACKWARD_FORBIDDEN (0x10)`.

This guarantees that:
- Deadlock is mathematically impossible.
- Premature drainage is formally unexploitable.
