# Security & Invariant Models

Security in `u64se-seu-solana` is grounded in two intuitive mental models:

1. **The Session Passport**: Client-side ticket validation where state is passed along like a metro transit pass.
2. **The On-Chain Circuit Breaker (Fuse)**: An automated hardware kill switch that trips the moment an invariant is breached.

Together, these models prevent capital drain, re-entrancy, and execution stalls without needing centralized multisig intervention.
