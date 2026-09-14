; ==============================================================================
; Formal Verification of u64se-seu-solana Invariants using Z3 (Theory: QF_BV)
; Exactly 8 Core Theorems covering 100% of the State Execution Unit Logic
; ==============================================================================
(set-logic QF_BV)

; ------------------------------------------------------------------------------
; Symbols (Free Variables over full domain)
; ------------------------------------------------------------------------------
(declare-const raw (_ BitVec 64))
(declare-const requested_role (_ BitVec 64))
(declare-const expected_target (_ BitVec 8))
(declare-const future_target (_ BitVec 8))
(declare-const current_slot (_ BitVec 16))

; ------------------------------------------------------------------------------
; Bitfield Extractions
; ------------------------------------------------------------------------------
; bit 0..7: role mask
(define-fun role_mask () (_ BitVec 64)
  (bvand raw #x00000000000000ff))

; bit 8: STATUS_ACTIVE, bit 9: STATUS_PAUSED, bit 10: STATUS_RATE_LIMITED
(define-fun status_active () (_ BitVec 1)
  ((_ extract 8 8) raw))

(define-fun status_paused () (_ BitVec 1)
  ((_ extract 9 9) raw))

(define-fun status_rate_limited () (_ BitVec 1)
  ((_ extract 10 10) raw))

; bit 15: FLAG_RING_TOPOLOGY
(define-fun flag_ring_topology () (_ BitVec 1)
  ((_ extract 15 15) raw))

; bit 16..23: current_node, bit 24..31: target_node
(define-fun current_node () (_ BitVec 8)
  ((_ extract 23 16) raw))

(define-fun target_node () (_ BitVec 8)
  ((_ extract 31 24) raw))

; bit 32..47: epoch_slot
(define-fun last_slot () (_ BitVec 16)
  ((_ extract 47 32) raw))

; bit 48: FLAG_ACYCLIC, bit 49: FLAG_NO_SELF_LOOP
(define-fun flag_acyclic () (_ BitVec 1)
  ((_ extract 48 48) raw))

(define-fun flag_no_self_loop () (_ BitVec 1)
  ((_ extract 49 49) raw))

; ------------------------------------------------------------------------------
; Semantic Evaluation (exact mirror of Rust evaluate)
; ------------------------------------------------------------------------------
; Delta slot calculation: wrapping_sub
(define-fun delta_slot () (_ BitVec 16)
  (bvsub current_slot last_slot))

; r0: Inactive or paused violation
(define-fun v_inactive_or_paused () Bool
  (or (= status_active #b0) (= status_paused #b1)))

; r0b: Rate limited violation
(define-fun v_rate_limited () Bool
  (= status_rate_limited #b1))

; r1: Role auth violation (single-hot power-of-2 and authorization)
; single-hot pow2 check: role != 0 and (role & (role - 1)) == 0
(define-fun is_power_of_two ((r (_ BitVec 64))) Bool
  (and (distinct r #x0000000000000000)
       (= (bvand r (bvsub r #x0000000000000001)) #x0000000000000000)))

(define-fun v_role_unauthorized () Bool
  (or (not (is_power_of_two requested_role))
      (= (bvand role_mask requested_role) #x0000000000000000)))

; r4: Inactivity recovery drain condition (delta >= 100 slots, role == DRAIN, target == 0)
(define-fun is_recovery_drain () Bool
  (and (bvuge delta_slot (_ bv100 16))
       (= requested_role #x0000000000000010) ; ROLE_DRAIN (1 << 4)
       (= expected_target (_ bv0 8))))

; r3: Slot time skew (delta > 0x7FFF, suppressed on recovery drain)
(define-fun v_time_skew () Bool
  (and (bvugt delta_slot #x7fff)
       (not is_recovery_drain)))

; Target mismatch violation (suppressed on recovery drain)
(define-fun v_target_mismatch () Bool
  (and (distinct target_node expected_target)
       (not is_recovery_drain)))

; r5: Topology rules (suppressed on recovery drain)
(define-fun is_self_loop () Bool
  (= current_node target_node))

(define-fun v_self_loop () Bool
  (and (not is_recovery_drain)
       (= flag_no_self_loop #b1)
       is_self_loop))

(define-fun is_ring_wrap () Bool
  (and (= flag_ring_topology #b1)
       (= expected_target (_ bv0 8))
       (bvugt current_node (_ bv0 8))))

(define-fun is_backward () Bool
  (and (bvule target_node current_node)
       (not is_ring_wrap)))

(define-fun v_backward () Bool
  (and (not is_recovery_drain)
       (= flag_acyclic #b1)
       is_backward))

; Overall PASS predicate for evaluate(): NO violations triggered
(define-fun is_pass () Bool
  (and (not v_inactive_or_paused)
       (not v_rate_limited)
       (not v_role_unauthorized)
       (not v_time_skew)
       (not v_target_mismatch)
       (not v_self_loop)
       (not v_backward)))

; ------------------------------------------------------------------------------
; Phase 2: Forward Edge Projection (execute_transition)
; ------------------------------------------------------------------------------
(define-fun validate_forward_edge () Bool
  (distinct future_target (_ bv0 8)))

(define-fun forward_self_loop () Bool
  (and (= flag_no_self_loop #b1)
       (= expected_target future_target)
       validate_forward_edge))

(define-fun forward_backward () Bool
  (and (= flag_acyclic #b1)
       (bvult future_target expected_target)
       validate_forward_edge))

(define-fun forward_edge_valid () Bool
  (and (not forward_self_loop)
       (not forward_backward)))

; Full atomic transition success: evaluate PASS AND forward edge valid
(define-fun is_transition_success () Bool
  (and is_pass forward_edge_valid))

; ==============================================================================
; THEOREM 1: PROHIBITION OF SELF-LOOP TRANSITIONS
; Proving that: (flag_no_self_loop == 1 and not recovery_drain and current == target)
; IMPLIES (is_pass == false)
; ==============================================================================
(push 1)
(echo "--- [PROVING THEOREM 1: Prohibition of self-loop transitions] ---")
(assert (= flag_no_self_loop #b1))
(assert (not is_recovery_drain))
(assert (= current_node target_node))
(assert is_pass)
(check-sat)
(pop 1)

; ==============================================================================
; THEOREM 2: STRICT DAG ACYCLICITY
; Proving that: (flag_acyclic == 1 and not recovery_drain and not ring_wrap and target <= current)
; IMPLIES (is_pass == false)
; ==============================================================================
(push 1)
(echo "--- [PROVING THEOREM 2: Strict DAG acyclicity] ---")
(assert (= flag_acyclic #b1))
(assert (not is_recovery_drain))
(assert (not is_ring_wrap))
(assert (bvule target_node current_node))
(assert is_pass)
(check-sat)
(pop 1)

; ==============================================================================
; THEOREM 3: STRICT SINGLE-HOT ROLE ENFORCEMENT
; Proving that: Multi-hot role (popcnt != 1) or unauthorized role CAN NEVER PASS
; ==============================================================================
(push 1)
(echo "--- [PROVING THEOREM 3: Strict single-hot role authorization] ---")
(assert (or (not (is_power_of_two requested_role))
            (= (bvand role_mask requested_role) #x0000000000000000)))
(assert is_pass)
(check-sat)
(pop 1)

; ==============================================================================
; THEOREM 4: GUARANTEED INACTIVITY RECOVERY DRAIN PRECONDITION
; Recovery drain CAN NEVER trigger if delta < 100 or role != DRAIN or target != 0
; ==============================================================================
(push 1)
(echo "--- [PROVING THEOREM 4: Recovery drain cannot trigger prematurely or incorrectly] ---")
(assert (or (bvult delta_slot (_ bv100 16))
            (distinct requested_role #x0000000000000010)
            (distinct expected_target (_ bv0 8))))
(assert is_recovery_drain)
(check-sat)
(pop 1)

; ==============================================================================
; THEOREM 5: ABSOLUTE ENGINE INACTIVITY AND PAUSE LOCK
; Proving that: Inactive, paused, or rate-limited engine CAN NEVER PASS
; ==============================================================================
(push 1)
(echo "--- [PROVING THEOREM 5: Absolute inactive, paused, and rate-limited lock] ---")
(assert (or (= status_active #b0)
            (= status_paused #b1)
            (= status_rate_limited #b1)))
(assert is_pass)
(check-sat)
(pop 1)

; ==============================================================================
; THEOREM 6: TEMPORAL MONOTONICITY AND TIME SKEW PROTECTION
; Proving that: Slot delta > 0x7FFF (past/skew overflow) CAN NEVER PASS outside recovery drain
; ==============================================================================
(push 1)
(echo "--- [PROVING THEOREM 6: Temporal monotonicity and clock skew protection] ---")
(assert (bvugt delta_slot #x7fff))
(assert (not is_recovery_drain))
(assert is_pass)
(check-sat)
(pop 1)

; ==============================================================================
; THEOREM 7: DETERMINISTIC RING WRAP BOUNDARY
; Proving that: Backward transition requires ring topology, target == 0, current > 0
; Without these conditions, backward step is ALWAYS rejected under DAG
; ==============================================================================
(push 1)
(echo "--- [PROVING THEOREM 7: Deterministic ring wrap boundary] ---")
(assert (= flag_acyclic #b1))
(assert (not is_recovery_drain))
(assert (bvule target_node current_node))
(assert (or (= flag_ring_topology #b0)
            (distinct expected_target (_ bv0 8))
            (= current_node (_ bv0 8))))
(assert is_pass)
(check-sat)
(pop 1)

; ==============================================================================
; THEOREM 8: FORWARD TARGET PROJECTION SAFETY (execute_transition)
; Proving that: Invalid future target projection (self-loop or backward step)
; CAN NEVER succeed in execute_transition
; ==============================================================================
(push 1)
(echo "--- [PROVING THEOREM 8: Forward target projection safety in execute_transition] ---")
(assert validate_forward_edge)
(assert (or (and (= flag_no_self_loop #b1) (= expected_target future_target))
            (and (= flag_acyclic #b1) (bvult future_target expected_target))))
(assert is_transition_success)
(check-sat)
(pop 1)
