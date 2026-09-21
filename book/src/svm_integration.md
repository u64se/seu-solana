# Solana SVM Integration

`u64se-seu-solana` interfaces natively with the Solana Sealevel Virtual Machine (SVM) through zero-copy byte buffers and raw SBF syscalls.

---

## Operating Modes

`u64se-seu-solana` can operate in two primary on-chain modes:

### Mode 1: Stateless Session Proof Verifier (Zero Rent)
In this mode, the program does not persist state to a writable account on-chain. Instead:
1. The caller provides the current state word, target coordinates, and authority signature in instruction data.
2. The SBF kernel executes the branchless ALU invariant pipeline (~8 CU).
3. If valid, the new verified state word is set into Solana's return data buffer via `sol_set_return_data` and emitted as a high-speed verification receipt.
4. **Economic Result**: **0 Lamports rent, 0 write locks, 0 account contention**.

### Mode 2: Persistent State PDA (8 Bytes)
For applications requiring on-chain persistence:
1. A canonical PDA is derived via `["u64se-seu", authority_pubkey, bump]`.
2. The account allocation is fixed at exactly **8 bytes** (`size_of::<u64>()`).
3. State transitions write the new 64-bit word directly into `account.data[0..8]`.
4. **Rent Cost**: Minimum rent-exempt reserve for 8 bytes (~0.00089 SOL).
