#![allow(unexpected_cfgs)]

use pinocchio::{
    account::AccountView,
    default_panic_handler,
    error::ProgramError,
    no_allocator,
    sysvars::{clock::Clock, Sysvar},
    Address, ProgramResult,
};

#[cfg(not(feature = "no-entrypoint"))]
use pinocchio::program_entrypoint;

#[cfg(any(target_os = "solana", target_arch = "bpf", target_arch = "sbf"))]
use solana_define_syscall::definitions::sol_set_return_data;

// Zero-heap constraint & zero-copy entrypoint
no_allocator!();
default_panic_handler!();

// Embedded Solana security.txt standard (Neodyme / SolanaFM / Solana Explorer)
#[cfg(not(feature = "no-entrypoint"))]
#[cfg_attr(any(target_os = "solana", target_arch = "bpf", target_arch = "sbf"), link_section = ".security.txt")]
#[allow(dead_code)]
#[no_mangle]
pub static SECURITY_TXT: &str = concat!(
    "=======BEGIN SECURITY.TXT V1=======\0",
    "name\0u64se-seu-solana\0",
    "project_url\0https://github.com/u64se/seu-solana\0",
    "contacts\0link:https://github.com/u64se/seu-solana/security/advisories/new,email:security@rag.engineering\0",
    "policy\0https://github.com/u64se/seu-solana/blob/main/SECURITY.md\0",
    "preferred_languages\0en,ru\0",
    "source_code\0https://github.com/u64se/seu-solana\0",
    "auditors\0Z3 SMT Solver (QF_BV Formal Invariants), Mollusk SVM\0",
    "=======END SECURITY.TXT V1=======\0"
);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateExecutionUnit(pub u64);

pub type StateRegister = StateExecutionUnit;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct FaultMask(pub u8);

pub type ViolationMask = FaultMask;

impl FaultMask {
    pub const PASS: u8 = 0b0000_0000;
    pub const INACTIVE_OR_PAUSED: u8 = 1 << 0;
    pub const ROLE_UNAUTHORIZED: u8 = 1 << 1;
    pub const TARGET_MISMATCH: u8 = 1 << 2;
    pub const SELF_LOOP_FORBIDDEN: u8 = 1 << 3;
    pub const BACKWARD_FORBIDDEN: u8 = 1 << 4;
    pub const SLOT_TIME_SKEW: u8 = 1 << 5;
    pub const RATE_LIMITED: u8 = 1 << 6;
    pub const VALUE_OVERFLOW: u8 = 1 << 7;

    #[inline(always)]
    pub const fn is_pass(&self) -> bool {
        self.0 == Self::PASS
    }
}

impl StateExecutionUnit {
    // bit 0..7: role authorization bitmask
    pub const ROLE_SWAP: u64 = 1 << 0;
    pub const ROLE_BUY_NFT: u64 = 1 << 1;
    pub const ROLE_ORACLE_UPDATE: u64 = 1 << 2;
    pub const ROLE_GOVERNANCE: u64 = 1 << 3;
    pub const ROLE_DRAIN: u64 = 1 << 4;
    pub const ROLE_FLASHLOAN: u64 = 1 << 5;
    pub const ROLE_MASK: u64 = 0xFF;

    // bit 8..15: operational status & topology flags
    pub const STATUS_ACTIVE: u64 = 1 << 8;
    pub const STATUS_PAUSED: u64 = 1 << 9;
    pub const STATUS_RATE_LIMITED: u64 = 1 << 10;
    pub const FLAG_RING_TOPOLOGY: u64 = 1 << 15;

    // bit 16..31: fsm node coordinates
    pub const CURRENT_NODE_SHIFT: u32 = 16;
    pub const TARGET_NODE_SHIFT: u32 = 24;
    pub const NODE_BYTE_MASK: u64 = 0xFF;

    // bit 32..47: temporal boundary
    pub const EPOCH_SLOT_SHIFT: u32 = 32;
    pub const EPOCH_SLOT_MASK: u64 = 0xFFFF;
    pub const INACTIVITY_TIMEOUT_SLOTS: u16 = 100;

    // bit 48..63: graph topology constraints
    pub const FLAG_ACYCLIC: u64 = 1 << 48;
    pub const FLAG_NO_SELF_LOOP: u64 = 1 << 49;

    // constructor: bitfield pack
    #[inline(always)]
    pub const fn new(
        role: u8,
        status: u8,
        current_node: u8,
        target_node: u8,
        epoch_slot: u16,
        flags: u16,
    ) -> Self {
        let r = (role as u64) & Self::ROLE_MASK;
        let s = ((status as u64) & 0xFF) << 8;
        let c = ((current_node as u64) & Self::NODE_BYTE_MASK) << Self::CURRENT_NODE_SHIFT;
        let t = ((target_node as u64) & Self::NODE_BYTE_MASK) << Self::TARGET_NODE_SHIFT;
        let slot = ((epoch_slot as u64) & Self::EPOCH_SLOT_MASK) << Self::EPOCH_SLOT_SHIFT;
        let fl = (flags as u64) << 48;
        Self(r | s | c | t | slot | fl)
    }

    // evaluate: branchless combinatorial validation
    #[inline(always)]
    pub fn evaluate(
        &self,
        requested_role: u64,
        expected_target: u8,
        current_slot: u16,
    ) -> FaultMask {
        let raw = self.0;
        let mut mask: u8 = 0;

        // r0: status
        let inactive = ((raw & Self::STATUS_ACTIVE) == 0) as u8;
        let paused = ((raw & Self::STATUS_PAUSED) != 0) as u8;
        mask |= (inactive | paused) * FaultMask::INACTIVE_OR_PAUSED;

        let rate_limited = ((raw & Self::STATUS_RATE_LIMITED) != 0) as u8;
        mask |= rate_limited * FaultMask::RATE_LIMITED;

        // r1: role pow2 & authorization (popcnt single-hot zero-branch)
        let not_power_of_two = (requested_role.count_ones() != 1) as u8;
        let role_unauthorized = ((raw & requested_role) == 0) as u8;
        mask |= (not_power_of_two | role_unauthorized) * FaultMask::ROLE_UNAUTHORIZED;

        // r2: coordinate match
        let current = ((raw >> Self::CURRENT_NODE_SHIFT) & Self::NODE_BYTE_MASK) as u8;
        let target = ((raw >> Self::TARGET_NODE_SHIFT) & Self::NODE_BYTE_MASK) as u8;

        // r3: slot temporal monotonicity
        let last_slot = ((raw >> Self::EPOCH_SLOT_SHIFT) & Self::EPOCH_SLOT_MASK) as u16;
        let delta = current_slot.wrapping_sub(last_slot);

        // r4: recovery drain fallback (branchless bitwise)
        let is_inactivity_timeout = (delta >= Self::INACTIVITY_TIMEOUT_SLOTS) as u8;
        let is_drain_role = (requested_role == Self::ROLE_DRAIN) as u8;
        let wants_drain_to_zero = (expected_target == 0) as u8;
        let is_recovery_drain = is_inactivity_timeout & is_drain_role & wants_drain_to_zero;

        // time skew is suppressed during recovery drain to allow rescuing dormant states (>3.64h)
        let time_skew = ((delta > 0x7FFF) as u8) & (1 ^ is_recovery_drain);
        mask |= time_skew * FaultMask::SLOT_TIME_SKEW;

        // suppress target mismatch during recovery drain
        let target_mismatch = ((target != expected_target) as u8) & (1 ^ is_recovery_drain);
        mask |= target_mismatch * FaultMask::TARGET_MISMATCH;

        // r5: DAG topology constraints (0 branches, pure mask multiplication)
        let apply_topology = 1 ^ is_recovery_drain;

        let flag_no_self_loop = ((raw & Self::FLAG_NO_SELF_LOOP) != 0) as u8;
        let is_self_loop = (current == target) as u8;
        mask |= apply_topology * flag_no_self_loop * is_self_loop * FaultMask::SELF_LOOP_FORBIDDEN;

        let is_ring_wrap = (((raw & Self::FLAG_RING_TOPOLOGY) != 0) as u8)
            & wants_drain_to_zero
            & ((current > 0) as u8);
        let flag_acyclic = ((raw & Self::FLAG_ACYCLIC) != 0) as u8;
        let is_backward = ((target <= current) as u8) & (1 ^ is_ring_wrap);
        mask |= apply_topology * flag_acyclic * is_backward * FaultMask::BACKWARD_FORBIDDEN;

        FaultMask(mask)
    }

    // execute_transition: deterministic state transition with forward edge validation
    #[inline(always)]
    pub fn execute_transition(
        &self,
        requested_role: u64,
        next_target: u8,
        future_target: u8,
        current_slot: u16,
    ) -> Result<StateExecutionUnit, FaultMask> {
        let mask = self.evaluate(requested_role, next_target, current_slot);
        if !mask.is_pass() {
            return Err(mask);
        }

        // r6: forward edge validation (prevent unauthorized forward target projection)
        // future_target == 0 is valid as explicit terminal lock or ring reset
        let raw = self.0;
        let is_terminal_lock = (future_target == 0) as u8;
        let validate_forward_edge = 1 ^ is_terminal_lock;

        let forward_self_loop = (((raw & Self::FLAG_NO_SELF_LOOP) != 0) as u8)
            & ((next_target == future_target) as u8)
            & validate_forward_edge;

        let forward_backward = (((raw & Self::FLAG_ACYCLIC) != 0) as u8)
            & ((future_target < next_target) as u8)
            & validate_forward_edge;

        let forward_fault = (forward_self_loop * FaultMask::SELF_LOOP_FORBIDDEN)
            | (forward_backward * FaultMask::BACKWARD_FORBIDDEN);

        if forward_fault != 0 {
            return Err(FaultMask(forward_fault));
        }

        let cleared = self.0
            & !(Self::NODE_BYTE_MASK << Self::CURRENT_NODE_SHIFT)
            & !(Self::NODE_BYTE_MASK << Self::TARGET_NODE_SHIFT)
            & !(Self::EPOCH_SLOT_MASK << Self::EPOCH_SLOT_SHIFT);

        let updated = cleared
            | ((next_target as u64) << Self::CURRENT_NODE_SHIFT)
            | ((future_target as u64) << Self::TARGET_NODE_SHIFT)
            | ((current_slot as u64) << Self::EPOCH_SLOT_SHIFT);

        Ok(StateExecutionUnit(updated))
    }

    // transition alias for backwards ergonomics
    #[inline(always)]
    pub fn transition(
        &self,
        requested_role: u64,
        next_target: u8,
        future_target: u8,
        current_slot: u16,
    ) -> Result<StateExecutionUnit, FaultMask> {
        self.execute_transition(requested_role, next_target, future_target, current_slot)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
program_entrypoint!(process_instruction, 2);

pub const TAG_GENESIS: u8 = 0;
pub const TAG_TRANSITION: u8 = 1;
pub const TAG_BATCH: u8 = 2;

pub fn process_instruction(
    _program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    // r0: signer check (zero-copy flag)
    if !accounts.is_empty() {
        let has_signer = accounts.iter().any(|acc| acc.is_signer());
        if !has_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
    }

    // wire minimum length
    if instruction_data.len() < 3 {
        return Err(ProgramError::InvalidInstructionData);
    }

    // r1: Clock sysvar reading
    let current_slot = match Clock::get() {
        Ok(c) => (c.slot & 0xFFFF) as u16,
        Err(_) => 100,
    };

    // TAG_BATCH (2): In-memory native multi-hop ALU pipeline
    // Wire layout: [tag=2, role_byte, hop_count, (curr, tgt, fut) * hop_count]
    if instruction_data[0] == TAG_BATCH {
        let role_byte = instruction_data[1];
        let hop_count = instruction_data[2] as usize;
        if hop_count == 0 || instruction_data.len() < 3 + hop_count * 3 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let flags =
            ((StateExecutionUnit::FLAG_ACYCLIC | StateExecutionUnit::FLAG_NO_SELF_LOOP) >> 48) as u16;
        let mut status = (StateExecutionUnit::STATUS_ACTIVE >> 8) as u8;
        status |= (StateExecutionUnit::FLAG_RING_TOPOLOGY >> 8) as u8;

        let first_curr = instruction_data[3];
        let first_tgt = instruction_data[4];

        let mut seu = StateExecutionUnit::new(
            role_byte,
            status,
            first_curr,
            first_tgt,
            current_slot,
            flags,
        );

        for i in 0..hop_count {
            let offset = 3 + i * 3;
            let tgt = instruction_data[offset + 1];
            let fut = instruction_data[offset + 2];

            seu = seu
                .execute_transition(role_byte as u64, tgt, fut, current_slot)
                .map_err(|mask| ProgramError::Custom(mask.0 as u32))?;
        }

        #[cfg(any(target_os = "solana", target_arch = "bpf", target_arch = "sbf"))]
        unsafe {
            sol_set_return_data(seu.0.to_le_bytes().as_ptr(), 8);
        }

        return Ok(());
    }

    // Single-hop wire: at least 5 bytes
    if instruction_data.len() < 5 {
        return Err(ProgramError::InvalidInstructionData);
    }

    // Parse wire data:
    // Auto-detect format:
    // 1) Legacy client bump prefix: [bump, tag, role, target, future] where bump > 10 and tag <= 1
    // 2) Canonical stateless wire: [tag, role, current_node, target_node, future_target]
    let (tag, role_byte, current_node, target_node, future_target) =
        if instruction_data[0] > 10 && instruction_data[1] <= 1 {
            let tag = instruction_data[1];
            let role = instruction_data[2];
            let target = instruction_data[3];
            let future = instruction_data[4];
            let current = if target == 0 { 4 } else { target.saturating_sub(1) };
            (tag, role, current, target, future)
        } else {
            (
                instruction_data[0],
                instruction_data[1],
                instruction_data[2],
                instruction_data[3],
                instruction_data[4],
            )
        };

    let flags =
        ((StateExecutionUnit::FLAG_ACYCLIC | StateExecutionUnit::FLAG_NO_SELF_LOOP) >> 48) as u16;
    let mut status = (StateExecutionUnit::STATUS_ACTIVE >> 8) as u8;
    if current_node == 4 && target_node == 0 {
        status |= (StateExecutionUnit::FLAG_RING_TOPOLOGY >> 8) as u8;
    }

    let seu = StateExecutionUnit::new(
        role_byte,
        status,
        current_node,
        target_node,
        current_slot,
        flags,
    );

    #[allow(unused_variables)]
    let next_state = match tag {
        TAG_GENESIS => {
            if target_node == 0 {
                return Err(ProgramError::InvalidArgument);
            }
            seu
        }
        TAG_TRANSITION => {
            seu.execute_transition(role_byte as u64, target_node, future_target, current_slot)
                .map_err(|mask| ProgramError::Custom(mask.0 as u32))?
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    };


    #[cfg(any(target_os = "solana", target_arch = "bpf", target_arch = "sbf"))]
    unsafe {
        sol_set_return_data(next_state.0.to_le_bytes().as_ptr(), 8);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_forward_transition() {
        let seu = StateExecutionUnit::new(
            StateExecutionUnit::ROLE_SWAP as u8,
            (StateExecutionUnit::STATUS_ACTIVE >> 8) as u8,
            0,
            1,
            100,
            ((StateExecutionUnit::FLAG_ACYCLIC | StateExecutionUnit::FLAG_NO_SELF_LOOP) >> 48)
                as u16,
        );
        let res = seu.execute_transition(StateExecutionUnit::ROLE_SWAP, 1, 2, 101);
        assert!(res.is_ok());
    }

    #[test]
    fn test_self_loop_rejection() {
        let seu = StateExecutionUnit::new(
            StateExecutionUnit::ROLE_SWAP as u8,
            (StateExecutionUnit::STATUS_ACTIVE >> 8) as u8,
            1,
            1,
            100,
            (StateExecutionUnit::FLAG_NO_SELF_LOOP >> 48) as u16,
        );
        let mask = seu.evaluate(StateExecutionUnit::ROLE_SWAP, 1, 101);
        assert_eq!(
            mask.0 & FaultMask::SELF_LOOP_FORBIDDEN,
            FaultMask::SELF_LOOP_FORBIDDEN
        );
    }

    #[test]
    fn test_unauthorized_forward_edge_rejection() {
        let seu = StateExecutionUnit::new(
            StateExecutionUnit::ROLE_SWAP as u8,
            (StateExecutionUnit::STATUS_ACTIVE >> 8) as u8,
            0,
            1,
            100,
            ((StateExecutionUnit::FLAG_ACYCLIC | StateExecutionUnit::FLAG_NO_SELF_LOOP) >> 48)
                as u16,
        );

        // 1. self-loop: next_target == future_target
        let res_self = seu.execute_transition(StateExecutionUnit::ROLE_SWAP, 1, 1, 101);
        assert_eq!(
            res_self.unwrap_err(),
            FaultMask(FaultMask::SELF_LOOP_FORBIDDEN)
        );

        // 2. valid forward transition to node 1 with future_target = 2
        let seu1 = seu
            .execute_transition(StateExecutionUnit::ROLE_SWAP, 1, 2, 101)
            .expect("valid step");

        // 3. backward transition rejection: current target is 2, attempt future_target = 1 (< 2)
        let res_back = seu1.execute_transition(StateExecutionUnit::ROLE_SWAP, 2, 1, 102);
        assert_eq!(
            res_back.unwrap_err(),
            FaultMask(FaultMask::BACKWARD_FORBIDDEN)
        );
    }

    #[test]
    fn test_inactivity_recovery_drain() {
        let seu = StateExecutionUnit::new(
            StateExecutionUnit::ROLE_DRAIN as u8,
            (StateExecutionUnit::STATUS_ACTIVE >> 8) as u8,
            4,
            5,
            100,
            ((StateExecutionUnit::FLAG_ACYCLIC | StateExecutionUnit::FLAG_NO_SELF_LOOP) >> 48)
                as u16,
        );
        // inactivity timeout reached at slot 205 (delta 105 >= 100), target expected is 0 (drain)
        let mask = seu.evaluate(StateExecutionUnit::ROLE_DRAIN, 0, 205);
        assert!(mask.is_pass());
    }

    #[test]
    fn test_inactivity_recovery_drain_under_time_skew() {
        let seu = StateExecutionUnit::new(
            StateExecutionUnit::ROLE_DRAIN as u8,
            (StateExecutionUnit::STATUS_ACTIVE >> 8) as u8,
            4,
            5,
            100,
            ((StateExecutionUnit::FLAG_ACYCLIC | StateExecutionUnit::FLAG_NO_SELF_LOOP) >> 48)
                as u16,
        );
        // slot 40,000 -> delta = 39,900 > 0x7FFF (time skew under normal flow)
        // recovery drain MUST pass without deadlocking
        let mask = seu.evaluate(StateExecutionUnit::ROLE_DRAIN, 0, 40_000);
        assert!(mask.is_pass());

        // whereas normal role SWAP under the same time skew MUST fail with SLOT_TIME_SKEW
        let mask_swap = seu.evaluate(StateExecutionUnit::ROLE_SWAP, 5, 40_000);
        assert_eq!(
            mask_swap.0 & FaultMask::SLOT_TIME_SKEW,
            FaultMask::SLOT_TIME_SKEW
        );
    }

    #[test]
    fn test_ring_topology_wrap_and_rejection() {
        // 1. With FLAG_RING_TOPOLOGY set: current = 5, transition to target = 0 is PERMITTED
        let seu_ring = StateExecutionUnit::new(
            StateExecutionUnit::ROLE_SWAP as u8,
            ((StateExecutionUnit::STATUS_ACTIVE | StateExecutionUnit::FLAG_RING_TOPOLOGY) >> 8)
                as u8,
            5,
            0,
            100,
            (StateExecutionUnit::FLAG_ACYCLIC >> 48) as u16,
        );
        let mask = seu_ring.evaluate(StateExecutionUnit::ROLE_SWAP, 0, 101);
        assert!(mask.is_pass());

        // 2. Without FLAG_RING_TOPOLOGY: backward transition 5 -> 0 is FORBIDDEN under DAG
        let seu_no_ring = StateExecutionUnit::new(
            StateExecutionUnit::ROLE_SWAP as u8,
            (StateExecutionUnit::STATUS_ACTIVE >> 8) as u8,
            5,
            0,
            100,
            (StateExecutionUnit::FLAG_ACYCLIC >> 48) as u16,
        );
        let mask_err = seu_no_ring.evaluate(StateExecutionUnit::ROLE_SWAP, 0, 101);
        assert_eq!(
            mask_err.0 & FaultMask::BACKWARD_FORBIDDEN,
            FaultMask::BACKWARD_FORBIDDEN
        );
    }

    #[test]
    fn test_slot_wraparound_16bit() {
        // last_slot = 0xFFF0, current_slot = 0x0010 -> wrapping delta = 32 <= 0x7FFF
        let seu = StateExecutionUnit::new(
            StateExecutionUnit::ROLE_SWAP as u8,
            (StateExecutionUnit::STATUS_ACTIVE >> 8) as u8,
            0,
            1,
            0xFFF0,
            ((StateExecutionUnit::FLAG_ACYCLIC | StateExecutionUnit::FLAG_NO_SELF_LOOP) >> 48)
                as u16,
        );
        let mask = seu.evaluate(StateExecutionUnit::ROLE_SWAP, 1, 0x0010);
        assert!(mask.is_pass());
    }

    #[test]
    fn test_status_flags_paused_and_rate_limited() {
        // STATUS_PAUSED
        let seu_paused = StateExecutionUnit::new(
            StateExecutionUnit::ROLE_SWAP as u8,
            (StateExecutionUnit::STATUS_PAUSED >> 8) as u8,
            0,
            1,
            100,
            0,
        );
        let mask_p = seu_paused.evaluate(StateExecutionUnit::ROLE_SWAP, 1, 101);
        assert_eq!(
            mask_p.0 & FaultMask::INACTIVE_OR_PAUSED,
            FaultMask::INACTIVE_OR_PAUSED
        );

        // STATUS_RATE_LIMITED
        let seu_rl = StateExecutionUnit::new(
            StateExecutionUnit::ROLE_SWAP as u8,
            ((StateExecutionUnit::STATUS_ACTIVE | StateExecutionUnit::STATUS_RATE_LIMITED) >> 8)
                as u8,
            0,
            1,
            100,
            0,
        );
        let mask_rl = seu_rl.evaluate(StateExecutionUnit::ROLE_SWAP, 1, 101);
        assert_eq!(mask_rl.0 & FaultMask::RATE_LIMITED, FaultMask::RATE_LIMITED);
    }

    #[test]
    fn test_batch_execution_5_hop_cycle() {
        // Complete 5-hop Torus cycle in 1 batch instruction
        // Wire: [tag=2, role=1, count=5, (curr, tgt, fut) * 5]
        let wire = [
            TAG_BATCH, StateExecutionUnit::ROLE_SWAP as u8, 5,
            0, 1, 2, // hop 0: S0 -> S1 (fut S2)
            1, 2, 3, // hop 1: S1 -> S2 (fut S3)
            2, 3, 4, // hop 2: S2 -> S3 (fut S4)
            3, 4, 0, // hop 3: S3 -> S4 (fut S0)
            4, 0, 1, // hop 4: S4 -> S0 (fut S1, Torus Ring Wrap)
        ];
        let res = process_instruction(&Address::default(), &mut [], &wire);
        assert!(res.is_ok());
    }

    #[test]
    fn test_batch_execution_backward_violation_rejected() {
        // Wire: hop 1 attempts forbidden backward jump (S2 -> S1)
        let wire = [
            TAG_BATCH, StateExecutionUnit::ROLE_SWAP as u8, 2,
            0, 1, 2,
            1, 2, 1, // S1 -> S2 with future S1 (< S2)
        ];
        let res = process_instruction(&Address::default(), &mut [], &wire);
        assert!(res.is_err());
    }

    #[test]
    fn test_batch_execution_self_loop_rejected() {
        // Wire: hop 1 attempts forbidden stutter (S1 -> S1)
        let wire = [
            TAG_BATCH, StateExecutionUnit::ROLE_SWAP as u8, 2,
            0, 1, 2,
            1, 1, 2, // self loop S1 -> S1
        ];
        let res = process_instruction(&Address::default(), &mut [], &wire);
        assert!(res.is_err());
    }
}

#[cfg(test)]
mod proptest_suite {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_never_panics_on_arbitrary_input(
            raw in any::<u64>(),
            role in any::<u64>(),
            target in any::<u8>(),
            slot in any::<u16>()
        ) {
            let seu = StateExecutionUnit(raw);
            let _ = seu.evaluate(role, target, slot);
        }

        #[test]
        fn test_multi_hot_role_always_rejected(
            raw in any::<u64>(),
            role in any::<u64>().prop_filter("multi-hot", |r| r.count_ones() != 1),
            target in any::<u8>(),
            slot in any::<u16>()
        ) {
            let seu = StateExecutionUnit(raw);
            let mask = seu.evaluate(role, target, slot);
            prop_assert_eq!(mask.0 & FaultMask::ROLE_UNAUTHORIZED, FaultMask::ROLE_UNAUTHORIZED);
        }
    }
}
