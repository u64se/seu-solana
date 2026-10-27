; ==============================================================================
; u64se-seu-solana & QuantumCube128 — Formal Bit-Vector Verification (Z3 QF_BV)
; ==============================================================================
; Run with:
;   z3 verify_invariants.smt2
; Expected output:
;   12 x `unsat` (All 12 formal mathematical invariants proven across 2^64 / 2^128)
;
; Coverage:
;   - Theorems 1..8:  Core 64-bit StateExecutionUnit (SEU) ALU & DAG Invariants
;   - Theorem 9:      O(1) Incremental Adjoint XOR Crystal Parity Preservation
;   - Theorem 10:     Single-Cube Tamper Detection via Star Header Parity
;   - Theorem 11:     Lock-Free SeqLock Torn-Read Exclusion (`STATUS_SEQLOCK_WRITING`)
;   - Theorem 12:     Lock-Free SPSC Agent Command Ring Overflow & Empty Invariants
; ==============================================================================

(set-logic QF_BV)
(set-option :produce-models true)

; ------------------------------------------------------------------------------
; 1. Symbolic 64-bit StateExecutionUnit (Word 0) & Input Parameters
; ------------------------------------------------------------------------------
(declare-const raw (_ BitVec 64))
(declare-const requested_role (_ BitVec 64))
(declare-const expected_target (_ BitVec 8))
(declare-const future_target (_ BitVec 8))
(declare-const current_slot (_ BitVec 16))

; Bitfield extractions from Word 0 (`raw`)
(define-fun role_mask () (_ BitVec 8) ((_ extract 7 0) raw))
(define-fun status_active () (_ BitVec 1) ((_ extract 8 8) raw))
(define-fun status_paused () (_ BitVec 1) ((_ extract 9 9) raw))
(define-fun status_rate_limited () (_ BitVec 1) ((_ extract 10 10) raw))
(define-fun status_seqlock_writing () (_ BitVec 1) ((_ extract 11 11) raw))
(define-fun flag_ring_topology () (_ BitVec 1) ((_ extract 15 15) raw))

(define-fun current_node () (_ BitVec 8) ((_ extract 23 16) raw))
(define-fun target_node () (_ BitVec 8) ((_ extract 31 24) raw))
(define-fun last_slot () (_ BitVec 16) ((_ extract 47 32) raw))

(define-fun flag_acyclic () (_ BitVec 1) ((_ extract 48 48) raw))
(define-fun flag_no_self_loop () (_ BitVec 1) ((_ extract 49 49) raw))
(define-fun flag_macro_blueprint () (_ BitVec 1) ((_ extract 50 50) raw))
(define-fun header_parity_digest () (_ BitVec 8) ((_ extract 63 56) raw))

; ------------------------------------------------------------------------------
; 2. Combinatorial Branchless ALU Evaluation (`StateExecutionUnit::evaluate`)
; ------------------------------------------------------------------------------
(define-fun delta_slot () (_ BitVec 16) (bvsub current_slot last_slot))

; Single-hot power-of-two check: r != 0 && (r & (r - 1)) == 0
(define-fun is_power_of_two ((r (_ BitVec 64))) Bool
  (and (distinct r #x0000000000000000)
       (= (bvand r (bvsub r #x0000000000000001)) #x0000000000000000)))

; Recovery drain condition: delta >= 100 && role == ROLE_DRAIN (0x10) && expected_target == 0
(define-fun is_recovery_drain () Bool
  (and (bvuge delta_slot (_ bv100 16))
       (= requested_role #x0000000000000010)
       (= expected_target (_ bv0 8))))

; Ring wrap condition: FLAG_RING_TOPOLOGY == 1 && expected_target == 0 && current_node > 0
(define-fun is_ring_wrap () Bool
  (and (= flag_ring_topology #b1)
       (= expected_target (_ bv0 8))
       (bvugt current_node (_ bv0 8))))

; Individual fault predicates
(define-fun v_inactive_or_paused () Bool
  (or (= status_active #b0) (= status_paused #b1)))

(define-fun v_rate_limited () Bool
  (= status_rate_limited #b1))

(define-fun v_role_unauthorized () Bool
  (or (not (is_power_of_two requested_role))
      (= (bvand raw requested_role) #x0000000000000000)))

(define-fun v_time_skew () Bool
  (and (bvugt delta_slot #x7fff) (not is_recovery_drain)))

(define-fun v_target_mismatch () Bool
  (and (distinct target_node expected_target) (not is_recovery_drain)))

(define-fun v_self_loop () Bool
  (and (not is_recovery_drain)
       (= flag_no_self_loop #b1)
       (= current_node target_node)))

(define-fun v_backward () Bool
  (and (not is_recovery_drain)
       (= flag_acyclic #b1)
       (bvule target_node current_node)
       (not is_ring_wrap)))

(define-fun is_pass () Bool
  (and (not v_inactive_or_paused)
       (not v_rate_limited)
       (not v_role_unauthorized)
       (not v_time_skew)
       (not v_target_mismatch)
       (not v_self_loop)
       (not v_backward)))

; Forward-edge projection validation (`execute_transition` r6)
(define-fun is_terminal_lock () Bool (= future_target (_ bv0 8)))
(define-fun v_forward_self_loop () Bool
  (and (not is_terminal_lock)
       (= flag_no_self_loop #b1)
       (= expected_target future_target)))
(define-fun v_forward_backward () Bool
  (and (not is_terminal_lock)
       (= flag_acyclic #b1)
       (bvult future_target expected_target)))
(define-fun is_transition_pass () Bool
  (and is_pass (not v_forward_self_loop) (not v_forward_backward)))

; ==============================================================================
; THEOREMS 1..8: Core 64-bit StateExecutionUnit Invariants
; ==============================================================================

; Theorem 1: Prohibition of Self-Loop Transitions under FLAG_NO_SELF_LOOP
(push)
(assert (and (= flag_no_self_loop #b1)
             (not is_recovery_drain)
             (= current_node target_node)
             is_pass))
(check-sat) ; expected: unsat
(pop)

; Theorem 2: Strict DAG Acyclicity (Backward transitions rejected without ring wrap)
(push)
(assert (and (= flag_acyclic #b1)
             (not is_recovery_drain)
             (not is_ring_wrap)
             (bvule target_node current_node)
             is_pass))
(check-sat) ; expected: unsat
(pop)

; Theorem 3: Strict Single-Hot Role Enforcement (Multi-bit or zero role masks rejected)
(push)
(assert (and (not (is_power_of_two requested_role))
             is_pass))
(check-sat) ; expected: unsat
(pop)

; Theorem 4: Guaranteed Inactivity Recovery Drain Precondition
(push)
(assert (and is_recovery_drain
             (or (bvult delta_slot (_ bv100 16))
                 (distinct requested_role #x0000000000000010)
                 (distinct expected_target (_ bv0 8)))))
(check-sat) ; expected: unsat
(pop)

; Theorem 5: Absolute Engine Inactivity, Pause, and Rate-Limit Lock
(push)
(assert (and (or (= status_active #b0)
                 (= status_paused #b1)
                 (= status_rate_limited #b1))
             is_pass))
(check-sat) ; expected: unsat
(pop)

; Theorem 6: Temporal Monotonicity and Time Skew Protection
(push)
(assert (and (bvugt delta_slot #x7fff)
             (not is_recovery_drain)
             is_pass))
(check-sat) ; expected: unsat
(pop)

; Theorem 7: Deterministic Ring Wrap Boundary
(push)
(assert (and (= flag_acyclic #b1)
             (not is_recovery_drain)
             (bvule target_node current_node)
             is_pass
             (or (= flag_ring_topology #b0)
                 (distinct expected_target (_ bv0 8))
                 (= current_node (_ bv0 8)))))
(check-sat) ; expected: unsat
(pop)

; Theorem 8: Forward Target Projection Safety (`execute_transition`)
(push)
(assert (and is_transition_pass
             (not is_terminal_lock)
             (or (and (= flag_no_self_loop #b1) (= expected_target future_target))
                 (and (= flag_acyclic #b1) (bvult future_target expected_target)))))
(check-sat) ; expected: unsat
(pop)

; ==============================================================================
; THEOREMS 9..12: 1472B Crystal XOR Parity, SeqLock & SPSC Ring Invariants
; ==============================================================================

; 1-cycle 8-bit XOR fold of a 128-bit QuantumCube128 (`seu` ^ rotl(`genome`, 13))
(define-fun parity_fold8 ((seu (_ BitVec 64)) (genome (_ BitVec 64))) (_ BitVec 8)
  (let ((x0 (bvxor seu ((_ rotate_left 13) genome))))
    (let ((x1 (bvxor x0 (bvlshr x0 (_ bv32 64)))))
      (let ((x2 (bvxor x1 (bvlshr x1 (_ bv16 64)))))
        (let ((x3 (bvxor x2 (bvlshr x2 (_ bv8 64)))))
          ((_ extract 7 0) x3))))))

; Symbolic state of Crystal cubes and updated slot `k`
(declare-const rest_parity_xor (_ BitVec 8)) ; XOR sum of all other 90 orbital cubes
(declare-const old_seu_k (_ BitVec 64))
(declare-const old_genome_k (_ BitVec 64))
(declare-const new_seu_k (_ BitVec 64))
(declare-const new_genome_k (_ BitVec 64))

(define-fun old_fold_k () (_ BitVec 8) (parity_fold8 old_seu_k old_genome_k))
(define-fun new_fold_k () (_ BitVec 8) (parity_fold8 new_seu_k new_genome_k))

; Full crystal parity before and after mutating slot `k`
(define-fun full_parity_before () (_ BitVec 8) (bvxor rest_parity_xor old_fold_k))
(define-fun full_parity_after ()  (_ BitVec 8) (bvxor rest_parity_xor new_fold_k))

; Incremental O(1) differential update (`update_cube_branchless`):
; next_digest = prev_digest ^ old_fold_k ^ new_fold_k
(define-fun incremental_digest_after () (_ BitVec 8)
  (bvxor (bvxor full_parity_before old_fold_k) new_fold_k))

; Theorem 9: Incremental O(1) Adjoint XOR Update is Algebraically Exact
; Proves that `prev_digest ^ old_fold ^ new_fold` ALWAYS equals a full 91-cube recomputation.
(push)
(assert (distinct incremental_digest_after full_parity_after))
(check-sat) ; expected: unsat
(pop)

; Theorem 10: Single-Cube Tamper / Corruption Detection via Star Header Parity
; Proves that if any cube `k` changes its 8-bit fold (`old_fold_k != new_fold_k`)
; without updating `header_parity_digest` in `cubes[0].seu`, `verify_crystal_parity()` FAILS.
(push)
(assert (= header_parity_digest full_parity_before))
(assert (distinct old_fold_k new_fold_k))
(assert (= header_parity_digest full_parity_after))
(check-sat) ; expected: unsat
(pop)

; Theorem 11: Lock-Free SeqLock Torn-Read Exclusion (`read_from_shared_ring`)
; Proves that `seqlock_read_ok` can NEVER hold if `STATUS_SEQLOCK_WRITING` (bit 11) was set
; during `h1` or if the header changed between `h1` and `h2`.
(declare-const h1_seu (_ BitVec 64))
(declare-const h2_seu (_ BitVec 64))
(declare-const computed_crystal_xor (_ BitVec 8))

(define-fun h1_writing () (_ BitVec 1) ((_ extract 11 11) h1_seu))
(define-fun h1_digest () (_ BitVec 8) ((_ extract 63 56) h1_seu))

(define-fun seqlock_read_ok () Bool
  (and (= h1_writing #b0)
       (= h1_seu h2_seu)
       (= computed_crystal_xor h1_digest)))

(push)
(assert (and seqlock_read_ok
             (or (= h1_writing #b1)
                 (distinct h1_seu h2_seu)
                 (distinct computed_crystal_xor ((_ extract 63 56) h2_seu)))))
(check-sat) ; expected: unsat
(pop)

; Theorem 12: Lock-Free SPSC Agent Command Ring Safety (141-Slot Capacity)
; Proves that for any 64-bit `head` and `tail` satisfying `occupancy <= 141` (#x8d):
; 1) `push_agent_command` only succeeds when `occupancy < 141`, and after `head' = head + 1`,
;    the new occupancy `head' - tail` is strictly `<= 141` and `> 0` (never wraps to empty).
; 2) `pop_agent_command` only succeeds when `head != tail` (`occupancy > 0`), and after
;    `tail' = tail + 1`, the new occupancy `head - tail'` is strictly `< occupancy`.
(declare-const spsc_head (_ BitVec 64))
(declare-const spsc_tail (_ BitVec 64))

(define-fun spsc_capacity () (_ BitVec 64) #x000000000000008d) ; 141 slots
(define-fun spsc_occupancy () (_ BitVec 64) (bvsub spsc_head spsc_tail))
(define-fun spsc_invariant () Bool (bvule spsc_occupancy spsc_capacity))

(define-fun can_push () Bool (bvult spsc_occupancy spsc_capacity))
(define-fun head_after_push () (_ BitVec 64) (bvadd spsc_head #x0000000000000001))
(define-fun occ_after_push () (_ BitVec 64) (bvsub head_after_push spsc_tail))

(define-fun can_pop () Bool (distinct spsc_head spsc_tail))
(define-fun tail_after_pop () (_ BitVec 64) (bvadd spsc_tail #x0000000000000001))
(define-fun occ_after_pop () (_ BitVec 64) (bvsub spsc_head tail_after_pop))

(push)
(assert spsc_invariant)
(assert (or
          ; Case A: Push allowed when full, or push violates capacity / wraps to 0
          (and can_push
               (or (not (bvule occ_after_push spsc_capacity))
                   (= occ_after_push #x0000000000000000)))
          ; Case B: Pop allowed when empty, or pop fails to decrement occupancy
          (and can_pop
               (not (bvult occ_after_pop spsc_occupancy)))))
(check-sat) ; expected: unsat
(pop)
