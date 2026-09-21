# 5-Byte Instruction Wire ABI

`u64se-seu-solana` defines a compact 5-byte instruction wire format that minimizes network payload and eliminates deserialization overhead.

```
+------------+------------+------------+------------+----------------+
| Byte 0     | Byte 1     | Byte 2     | Byte 3     | Byte 4         |
| bump (u8)  | tag (u8)   | role (u8)  | target(u8) | future_tgt(u8) |
+------------+------------+------------+------------+----------------+
```

---

## Byte-by-Byte Wire Encoding

| Offset | Field | Type | Description |
| :---: | :--- | :---: | :--- |
| `0` | `bump` | `u8` | Canonical PDA bump seed (for $O(1)$ derivation without search loop) |
| `1` | `tag` | `u8` | `0`: `TAG_GENESIS` (initialize/reset), `1`: `TAG_TRANSITION` (step) |
| `2` | `role` | `u8` | **GENESIS**: initial `allowed_roles_mask`<br>**TRANSITION**: `requested_role` (single-hot) |
| `3` | `target` | `u8` | Expected target FSM coordinate $S_{i+1}$ |
| `4` | `future_target` | `u8` | **GENESIS**: initial topology configuration flags<br>**TRANSITION**: projected forward coordinate $S_{i+2}$ |

---

## Instruction Tags

### `TAG_GENESIS` (`0x00`)
Initializes an FSM at coordinate $S_0$:
- Sets the `allowed_roles_mask` from `byte [2]`.
- Configures graph topology:
  - `0x00`: Default canonical DAG (`FLAG_ACYCLIC | FLAG_NO_SELF_LOOP`)
  - `bit 0` (`0x01`): Strict forward acyclicity
  - `bit 1` (`0x02`): Self-loop prohibition
  - `bit 7` (`0x80`): Ring/Torus topology ($S_n \to S_0$ wrap permitted)
- Current node set to $0$, target node set to $1$.

### `TAG_TRANSITION` (`0x01`)
Executes an atomic forward step $S_i \to S_{i+1}$:
- Verifies caller authority signature.
- Evaluates branchless ALU invariants against the 64-bit state word.
- Updates node coordinates and stamps the current slot anchor.

---

## Account List Required

```
Account 0: [writable, signer] Fee Payer / Authority
Account 1: [read-only] Program ID (FCm2jTA6aiWqrfgtJQgBEZ5dyY9cfBXP8jas3TMTs19H)
Account 2: [read-only, optional] Clock Sysvar (11111111111111111111111111111111)
```

Total transaction payload is **under 200 bytes** on the wire.
