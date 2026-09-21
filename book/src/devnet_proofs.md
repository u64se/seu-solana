# Live Devnet Verification

`u64se-seu-solana` is deployed, active, and verified on the Solana Devnet cluster.

---

## On-Chain Program Details

- **Program Name**: `u64se-seu-solana`
- **Program ID**: [`FCm2jTA6aiWqrfgtJQgBEZ5dyY9cfBXP8jas3TMTs19H`](https://explorer.solana.com/address/FCm2jTA6aiWqrfgtJQgBEZ5dyY9cfBXP8jas3TMTs19H?cluster=devnet)
- **Deployment Transaction**: [`de54uQ6ofN582vPPbNuFyfvjMAuptXhCzsdF2PRFJcn5B8CS8aKL69cp8uQvM5pTb1FPqtjkiszoToCiMtJsw1t`](https://explorer.solana.com/tx/de54uQ6ofN582vPPbNuFyfvjMAuptXhCzsdF2PRFJcn5B8CS8aKL69cp8uQvM5pTb1FPqtjkiszoToCiMtJsw1t?cluster=devnet)
- **Binary Format**: Stripped SBF Shared Object (4,720 bytes)
- **Framework**: Pinocchio (`no_std`, `no_allocator!`)

---

## Security.txt Compliance

In compliance with the Solana `security.txt` standard (Neodyme / SolanaFM / Solana Explorer), the binary embeds cryptographic metadata into the `.security.txt` section:

```
name: u64se-seu-solana
project_url: https://github.com/u64se/seu-solana
contacts: link:https://github.com/u64se/seu-solana/security/advisories/new,email:security@rag.engineering
policy: https://github.com/u64se/seu-solana/blob/main/SECURITY.md
auditors: Z3 SMT Solver (QF_BV Formal Invariants), Mollusk SVM
```
