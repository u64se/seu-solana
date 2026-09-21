# Formal Verification

Traditional smart contract auditing relies on manual code reviews and dynamic fuzzing tests. While necessary, fuzzing cannot test all $2^{64}$ possible input states of an invariant engine.

`u64se-seu-solana` achieves **mathematical proof of correctness** through **Formal Verification via Satisfiability Modulo Theories (SMT)** using Microsoft Research's **Z3 solver**.

---

## The QF_BV SMT Logic

All 8 safety invariants are codified into SMT-LIB2 format under the **Quantifier-Free Bit-Vector Theory (`QF_BV`)**. 

Under `QF_BV`:
- All variables are modeled as exact bit-vectors: `(_ BitVec 8)`, `(_ BitVec 16)`, `(_ BitVec 64)`.
- Operations are bit-exact hardware primitives: `bvand`, `bvor`, `bvadd`, `bvsub`, `bvlshr`.
- The solver searches the entire state space of $2^{64}$ bit combinations to find any counter-example where a safety violation occurs while `fault == 0`.

When Z3 returns `unsat` for a theorem, it is a formal mathematical guarantee that **no input state exists in the universe** that can bypass the safety invariant.
