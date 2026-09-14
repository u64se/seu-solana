use std::time::Instant;
use u64se_seu_solana::StateExecutionUnit;

fn main() {
    // r0: initial register (ROLE_SWAP | ACTIVE | S0->S1 | slot 100 | ACYCLIC | NO_SELF_LOOP)
    let seu = StateExecutionUnit::new(
        StateExecutionUnit::ROLE_SWAP as u8,
        (StateExecutionUnit::STATUS_ACTIVE >> 8) as u8,
        0,
        1,
        100,
        ((StateExecutionUnit::FLAG_ACYCLIC | StateExecutionUnit::FLAG_NO_SELF_LOOP) >> 48) as u16,
    );

    // assert: valid transition
    let m0 = seu.evaluate(StateExecutionUnit::ROLE_SWAP, 1, 101);
    assert!(m0.is_pass());

    // assert: self-loop violation
    let m1 = seu.evaluate(StateExecutionUnit::ROLE_SWAP, 0, 101);
    assert!(!m1.is_pass());

    // bench: 1M evaluations (honest black_box)
    let n = 1_000_000;
    let t0 = Instant::now();
    let mut acc = 0u64;
    for i in 0..n {
        let m = seu.evaluate(
            std::hint::black_box(StateExecutionUnit::ROLE_SWAP),
            std::hint::black_box(1),
            std::hint::black_box((100 + (i & 0x1F)) as u16),
        );
        acc = acc.wrapping_add(std::hint::black_box(m.0 as u64));
    }
    let dt = t0.elapsed();

    println!(
        "u64se-seu: ok (state=0x{:016x} bench={:.2}ns/op throughput={}ops/s)",
        seu.0,
        dt.as_nanos() as f64 / n as f64,
        (n as f64 / dt.as_secs_f64()) as u64
    );
}
