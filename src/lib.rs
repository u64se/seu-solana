#![allow(unexpected_cfgs)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "solana")]
use pinocchio::{
    account::AccountView,
    default_panic_handler,
    error::ProgramError,
    no_allocator,
    sysvars::{clock::Clock, Sysvar},
    Address, ProgramResult,
};

#[cfg(all(feature = "solana", not(feature = "no-entrypoint")))]
#[allow(unused_imports)]
use pinocchio::program_entrypoint;

#[cfg(all(feature = "solana", any(target_os = "solana", target_arch = "bpf", target_arch = "sbf")))]
use solana_define_syscall::definitions::sol_set_return_data;

// Zero-heap constraint & zero-copy entrypoint
#[cfg(feature = "solana")]
no_allocator!();
#[cfg(feature = "solana")]
default_panic_handler!();

// Embedded Solana security.txt standard (Neodyme / SolanaFM / Solana Explorer)
#[cfg(all(feature = "solana", not(feature = "no-entrypoint")))]
#[cfg_attr(any(target_os = "solana", target_arch = "bpf", target_arch = "sbf"), link_section = ".security.txt")]
#[allow(dead_code)]
#[no_mangle]
pub static SECURITY_TXT: &str = concat!(
    "=======BEGIN SECURITY.TXT V1=======\0",
    "name\0u64se-seu-solana\0",
    "project_url\0https://github.com/u64se/seu-solana\0",
    "contacts\0link:https://github.com/u64se/seu-solana/security/advisories/new,email:security@rag.engineering\0",
    "policy\0https://github.com/u64se/seu-solana/blob/main/SECURITY.md\0",
    "preferred_languages\0en,ua\0",
    "source_code\0https://github.com/u64se/seu-solana\0",
    "auditors\0Z3 SMT Solver (QF_BV Formal Invariants), Mollusk SVM\0",
    "=======END SECURITY.TXT V1=======\0"
);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::Pod, bytemuck::Zeroable))]
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

pub const ERROR_INVALID_INSTRUCTION_DATA: u32 = 1001;
pub const ERROR_INVALID_ARGUMENT: u32 = 1002;
pub const ERROR_ACCOUNT_ALREADY_INITIALIZED: u32 = 1003;
pub const ERROR_UNINITIALIZED_ACCOUNT: u32 = 1004;

impl StateExecutionUnit {
    // bit 0..7: role authorization bitmask
    pub const ROLE_SWAP: u64 = 1 << 0;
    pub const ROLE_BUY_NFT: u64 = 1 << 1;
    pub const ROLE_ORACLE_UPDATE: u64 = 1 << 2;
    pub const ROLE_GOVERNANCE: u64 = 1 << 3;
    pub const ROLE_DRAIN: u64 = 1 << 4;
    pub const ROLE_FLASHLOAN: u64 = 1 << 5;
    pub const ROLE_CONCRESCE: u64 = 1 << 6;
    pub const ROLE_IGNITE_STAR: u64 = 1 << 7;
    pub const ROLE_MASK: u64 = 0xFF;

    // bit 8..15: operational status & topology flags
    pub const STATUS_ACTIVE: u64 = 1 << 8;
    pub const STATUS_PAUSED: u64 = 1 << 9;
    pub const STATUS_RATE_LIMITED: u64 = 1 << 10;
    pub const STATUS_SEQLOCK_WRITING: u64 = 1 << 11;
    pub const FLAG_RING_TOPOLOGY: u64 = 1 << 15;

    // bit 16..31: fsm node coordinates
    pub const CURRENT_NODE_SHIFT: u32 = 16;
    pub const TARGET_NODE_SHIFT: u32 = 24;
    pub const NODE_BYTE_MASK: u64 = 0xFF;

    // bit 32..47: temporal boundary
    pub const EPOCH_SLOT_SHIFT: u32 = 32;
    pub const EPOCH_SLOT_MASK: u64 = 0xFFFF;
    pub const INACTIVITY_TIMEOUT_SLOTS: u16 = 100;

    // bit 48..63: graph topology constraints & crystal parity digest
    pub const FLAG_ACYCLIC: u64 = 1 << 48;
    pub const FLAG_NO_SELF_LOOP: u64 = 1 << 49;
    pub const FLAG_MACRO_BLUEPRINT: u64 = 1 << 50;

    pub const PARITY_DIGEST_SHIFT: u32 = 56;
    pub const PARITY_DIGEST_MASK: u64 = 0xFF00_0000_0000_0000;

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

    #[inline(always)]
    pub const fn current_node(&self) -> u8 {
        ((self.0 >> Self::CURRENT_NODE_SHIFT) & Self::NODE_BYTE_MASK) as u8
    }

    #[inline(always)]
    pub const fn target_node(&self) -> u8 {
        ((self.0 >> Self::TARGET_NODE_SHIFT) & Self::NODE_BYTE_MASK) as u8
    }

    #[inline(always)]
    pub const fn last_slot(&self) -> u16 {
        ((self.0 >> Self::EPOCH_SLOT_SHIFT) & Self::EPOCH_SLOT_MASK) as u16
    }

    #[inline(always)]
    pub const fn hamming_heat(&self, next: Self) -> u8 {
        (self.0 ^ next.0).count_ones() as u8
    }

    #[inline(always)]
    pub const fn parity_digest(&self) -> u8 {
        ((self.0 >> Self::PARITY_DIGEST_SHIFT) & 0xFF) as u8
    }

    #[inline(always)]
    pub const fn with_parity_digest(&self, digest: u8) -> Self {
        let cleared = self.0 & !Self::PARITY_DIGEST_MASK;
        Self(cleared | ((digest as u64) << Self::PARITY_DIGEST_SHIFT))
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

    // transition alias for ergonomics
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

pub const TAG_GENESIS: u8 = 0;
pub const TAG_TRANSITION: u8 = 1;
pub const TAG_BATCH: u8 = 2;
pub const TAG_STAR_CHECKPOINT: u8 = 3;

/// Pure instruction processor core, free of SBF syscalls
pub fn process_seu_instruction_core(
    _has_signer: bool,
    mut pda_state_bytes: Option<&mut [u8]>,
    current_slot: u16,
    instruction_data: &[u8],
) -> Result<StateExecutionUnit, u32> {
    if instruction_data.len() < 3 {
        return Err(ERROR_INVALID_INSTRUCTION_DATA);
    }

    // TAG_BATCH (2): In-memory multi-hop ALU pipeline
    if instruction_data[0] == TAG_BATCH {
        let role_byte = instruction_data[1];
        let hop_count = instruction_data[2] as usize;
        if hop_count == 0 || instruction_data.len() < 3 + hop_count * 3 {
            return Err(ERROR_INVALID_INSTRUCTION_DATA);
        }

        let flags =
            ((StateExecutionUnit::FLAG_ACYCLIC | StateExecutionUnit::FLAG_NO_SELF_LOOP) >> 48) as u16;
        let mut status = (StateExecutionUnit::STATUS_ACTIVE >> 8) as u8;
        status |= (StateExecutionUnit::FLAG_RING_TOPOLOGY >> 8) as u8;

        let first_curr = instruction_data[3];
        let first_tgt = instruction_data[4];

        let mut seu = if let Some(ref pda) = pda_state_bytes {
            let val = u64::from_le_bytes(pda[0..8].try_into().unwrap());
            if val != 0 {
                StateExecutionUnit(val)
            } else {
                StateExecutionUnit::new(role_byte, status, first_curr, first_tgt, current_slot, flags)
            }
        } else {
            StateExecutionUnit::new(role_byte, status, first_curr, first_tgt, current_slot, flags)
        };

        for i in 0..hop_count {
            let offset = 3 + i * 3;
            let curr = instruction_data[offset];
            let tgt = instruction_data[offset + 1];
            let fut = instruction_data[offset + 2];

            // Verify continuous route connectivity
            if curr != seu.current_node() {
                return Err(FaultMask::TARGET_MISMATCH as u32);
            }

            seu = seu
                .execute_transition(role_byte as u64, tgt, fut, current_slot)
                .map_err(|mask| mask.0 as u32)?;
        }

        if let Some(ref mut pda) = pda_state_bytes {
            pda[0..8].copy_from_slice(&seu.0.to_le_bytes());
        }

        return Ok(seu);
    }

    // Single-hop / 5-byte wire:
    if instruction_data.len() < 5 {
        return Err(ERROR_INVALID_INSTRUCTION_DATA);
    }

    // Auto-detect format:
    // 1) Legacy client bump prefix: [bump, tag, role, target, future] where bump > 10 and tag <= 3
    // 2) Canonical stateless wire: [tag, role, current_node, target_node, future_target]
    let (tag, role_byte, current_node, target_node, future_target) =
        if instruction_data[0] > 10 && instruction_data[1] <= 3 {
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

    let flags = match future_target {
        0x01 => (StateExecutionUnit::FLAG_ACYCLIC >> 48) as u16,
        0x02 => (StateExecutionUnit::FLAG_NO_SELF_LOOP >> 48) as u16,
        0x80 => (StateExecutionUnit::FLAG_RING_TOPOLOGY >> 48) as u16,
        _ => ((StateExecutionUnit::FLAG_ACYCLIC | StateExecutionUnit::FLAG_NO_SELF_LOOP) >> 48) as u16,
    };

    let mut status = (StateExecutionUnit::STATUS_ACTIVE >> 8) as u8;
    if (current_node == 4 && target_node == 0) || future_target == 0x80 {
        status |= (StateExecutionUnit::FLAG_RING_TOPOLOGY >> 8) as u8;
    }

    match tag {
        TAG_GENESIS => {
            if target_node == 0 {
                return Err(ERROR_INVALID_ARGUMENT);
            }
            if let Some(ref pda) = pda_state_bytes {
                let existing = u64::from_le_bytes(pda[0..8].try_into().unwrap());
                if existing != 0 {
                    return Err(ERROR_ACCOUNT_ALREADY_INITIALIZED);
                }
            }
            let seu = StateExecutionUnit::new(
                role_byte,
                status,
                current_node,
                target_node,
                current_slot,
                flags,
            );
            if let Some(ref mut pda) = pda_state_bytes {
                pda[0..8].copy_from_slice(&seu.0.to_le_bytes());
            }
            Ok(seu)
        }
        TAG_TRANSITION => {
            let next_seu = if let Some(ref mut pda) = pda_state_bytes {
                let existing = u64::from_le_bytes(pda[0..8].try_into().unwrap());
                if existing == 0 {
                    return Err(ERROR_UNINITIALIZED_ACCOUNT);
                }
                let base_seu = StateExecutionUnit(existing);
                let transitioned = base_seu
                    .execute_transition(role_byte as u64, target_node, future_target, current_slot)
                    .map_err(|mask| mask.0 as u32)?;
                pda[0..8].copy_from_slice(&transitioned.0.to_le_bytes());
                transitioned
            } else {
                let seu = StateExecutionUnit::new(
                    role_byte,
                    status,
                    current_node,
                    target_node,
                    current_slot,
                    flags,
                );
                seu.execute_transition(role_byte as u64, target_node, future_target, current_slot)
                    .map_err(|mask| mask.0 as u32)?
            };
            Ok(next_seu)
        }
        TAG_STAR_CHECKPOINT => {
            // [TAG_STAR_CHECKPOINT, role, target_node, future_target, parity_digest]
            let parity = future_target;
            let next_seu = if let Some(ref mut pda) = pda_state_bytes {
                let existing = u64::from_le_bytes(pda[0..8].try_into().unwrap());
                let base_seu = StateExecutionUnit(existing);
                let mut transitioned = base_seu
                    .execute_transition(role_byte as u64, target_node, 0, current_slot)
                    .map_err(|mask| mask.0 as u32)?;
                transitioned.0 |= StateExecutionUnit::FLAG_MACRO_BLUEPRINT;
                transitioned = transitioned.with_parity_digest(parity);
                pda[0..8].copy_from_slice(&transitioned.0.to_le_bytes());
                transitioned
            } else {
                let seu = StateExecutionUnit::new(
                    role_byte,
                    status,
                    current_node,
                    target_node,
                    current_slot,
                    flags,
                );
                let mut transitioned = seu
                    .execute_transition(role_byte as u64, target_node, 0, current_slot)
                    .map_err(|mask| mask.0 as u32)?;
                transitioned.0 |= StateExecutionUnit::FLAG_MACRO_BLUEPRINT;
                transitioned.with_parity_digest(parity)
            };
            Ok(next_seu)
        }
        _ => Err(ERROR_INVALID_INSTRUCTION_DATA),
    }
}

#[cfg(feature = "solana")]
fn map_core_error(err: u32) -> ProgramError {
    match err {
        ERROR_INVALID_INSTRUCTION_DATA => ProgramError::InvalidInstructionData,
        ERROR_INVALID_ARGUMENT => ProgramError::InvalidArgument,
        ERROR_ACCOUNT_ALREADY_INITIALIZED => ProgramError::AccountAlreadyInitialized,
        ERROR_UNINITIALIZED_ACCOUNT => ProgramError::UninitializedAccount,
        custom => ProgramError::Custom(custom),
    }
}

#[cfg(feature = "solana")]
pub fn process_instruction(
    _program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let has_signer = if !accounts.is_empty() {
        let signed = accounts.iter().any(|acc| acc.is_signer());
        if !signed {
            return Err(ProgramError::MissingRequiredSignature);
        }
        true
    } else {
        false
    };

    let current_slot = match Clock::get() {
        Ok(c) => (c.slot & 0xFFFF) as u16,
        Err(_) => 100,
    };

    if !accounts.is_empty() {
        let pda_account = &mut accounts[0];
        let dlen = pda_account.data_len();
        if dlen != 0 && dlen != 8 {
            return Err(ProgramError::InvalidAccountData);
        }
        if dlen == 8 {
            if !pda_account.is_writable() {
                return Err(ProgramError::InvalidAccountData);
            }
            let mut data_ref = pda_account.try_borrow_mut()?;
            let _next_seu = process_seu_instruction_core(
                has_signer,
                Some(&mut data_ref[..8]),
                current_slot,
                instruction_data,
            )
            .map_err(map_core_error)?;

            #[cfg(any(target_os = "solana", target_arch = "bpf", target_arch = "sbf"))]
            unsafe {
                sol_set_return_data(_next_seu.0.to_le_bytes().as_ptr(), 8);
            }

            return Ok(());
        }
    }

    let _next_seu = process_seu_instruction_core(
        has_signer,
        None,
        current_slot,
        instruction_data,
    )
    .map_err(map_core_error)?;

    #[cfg(any(target_os = "solana", target_arch = "bpf", target_arch = "sbf"))]
    unsafe {
        sol_set_return_data(_next_seu.0.to_le_bytes().as_ptr(), 8);
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
        // next = 1, future = 1 (forbidden self-loop ahead)
        let res = seu.execute_transition(StateExecutionUnit::ROLE_SWAP, 1, 1, 101);
        assert_eq!(
            res.unwrap_err().0,
            FaultMask::SELF_LOOP_FORBIDDEN
        );

        // next = 2, future = 1 (forbidden backward jump ahead)
        let seu2 = StateExecutionUnit::new(
            StateExecutionUnit::ROLE_SWAP as u8,
            (StateExecutionUnit::STATUS_ACTIVE >> 8) as u8,
            1,
            2,
            100,
            ((StateExecutionUnit::FLAG_ACYCLIC | StateExecutionUnit::FLAG_NO_SELF_LOOP) >> 48)
                as u16,
        );
        let res2 = seu2.execute_transition(StateExecutionUnit::ROLE_SWAP, 2, 1, 101);
        assert_eq!(
            res2.unwrap_err().0,
            FaultMask::BACKWARD_FORBIDDEN
        );
    }

    #[test]
    fn test_inactivity_recovery_drain() {
        let seu = StateExecutionUnit::new(
            (StateExecutionUnit::ROLE_SWAP | StateExecutionUnit::ROLE_DRAIN) as u8,
            (StateExecutionUnit::STATUS_ACTIVE >> 8) as u8,
            3,
            4,
            100,
            ((StateExecutionUnit::FLAG_ACYCLIC | StateExecutionUnit::FLAG_NO_SELF_LOOP) >> 48)
                as u16,
        );
        // Inactivity delta = 100 slots exactly -> drain role permitted to state 0
        let mask = seu.evaluate(StateExecutionUnit::ROLE_DRAIN, 0, 200);
        assert!(mask.is_pass());

        // Under 100 slots (e.g. 99) -> drain role forbidden
        let mask_early = seu.evaluate(StateExecutionUnit::ROLE_DRAIN, 0, 199);
        assert_eq!(
            mask_early.0 & FaultMask::TARGET_MISMATCH,
            FaultMask::TARGET_MISMATCH
        );
    }

    #[test]
    fn test_inactivity_recovery_drain_under_time_skew() {
        let seu = StateExecutionUnit::new(
            (StateExecutionUnit::ROLE_SWAP | StateExecutionUnit::ROLE_DRAIN) as u8,
            (StateExecutionUnit::STATUS_ACTIVE >> 8) as u8,
            3,
            4,
            100,
            ((StateExecutionUnit::FLAG_ACYCLIC | StateExecutionUnit::FLAG_NO_SELF_LOOP) >> 48)
                as u16,
        );
        // Dormant state recovery: delta = 0x8001 (> 0x7FFF) with ROLE_DRAIN to 0
        let current_slot = 100u16.wrapping_add(0x8001);
        let mask = seu.evaluate(StateExecutionUnit::ROLE_DRAIN, 0, current_slot);
        assert!(mask.is_pass());
    }

    #[test]
    fn test_ring_topology_wrap_and_rejection() {
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
        let wire = [
            TAG_BATCH, StateExecutionUnit::ROLE_SWAP as u8, 5,
            0, 1, 2,
            1, 2, 3,
            2, 3, 4,
            3, 4, 0,
            4, 0, 1,
        ];
        let res = process_seu_instruction_core(true, None, 100, &wire);
        assert!(res.is_ok());
    }

    #[test]
    fn test_batch_execution_backward_violation_rejected() {
        let wire = [
            TAG_BATCH, StateExecutionUnit::ROLE_SWAP as u8, 2,
            0, 1, 2,
            1, 2, 1,
        ];
        let res = process_seu_instruction_core(true, None, 100, &wire);
        assert!(res.is_err());
    }

    #[test]
    fn test_batch_execution_self_loop_rejected() {
        let wire = [
            TAG_BATCH, StateExecutionUnit::ROLE_SWAP as u8, 2,
            0, 1, 2,
            1, 1, 2,
        ];
        let res = process_seu_instruction_core(true, None, 100, &wire);
        assert!(res.is_err());
    }

    #[test]
    fn test_batch_route_discontinuity_rejected() {
        let mut pda_bytes = [0u8; 8];
        let wire = [
            TAG_BATCH, StateExecutionUnit::ROLE_SWAP as u8, 2,
            0, 1, 2,
            3, 4, 5,
        ];
        let res = process_seu_instruction_core(true, Some(&mut pda_bytes), 100, &wire);
        assert!(res.is_err());
    }

    #[test]
    fn test_stateful_pda_lifecycle_genesis_transition_drain_and_star_checkpoint() {
        let mut pda_bytes = [0u8; 8];

        // 1. TAG_GENESIS (role=ROLE_SWAP | ROLE_DRAIN, curr=0, tgt=1, flags=0) at slot 100
        let genesis_wire = [TAG_GENESIS, (StateExecutionUnit::ROLE_SWAP | StateExecutionUnit::ROLE_DRAIN) as u8, 0, 1, 0];
        let res_gen = process_seu_instruction_core(true, Some(&mut pda_bytes), 100, &genesis_wire);
        assert!(res_gen.is_ok());
        let seu_gen = res_gen.unwrap();
        assert_eq!(seu_gen.current_node(), 0);
        assert_eq!(seu_gen.target_node(), 1);
        assert_eq!(seu_gen.last_slot(), 100);
        assert_eq!(u64::from_le_bytes(pda_bytes), seu_gen.0);

        // 2. Repeat TAG_GENESIS on initialized account must be REJECTED
        let res_gen_dup = process_seu_instruction_core(true, Some(&mut pda_bytes), 101, &genesis_wire);
        assert_eq!(res_gen_dup.err(), Some(ERROR_ACCOUNT_ALREADY_INITIALIZED));

        // 3. TAG_TRANSITION (S0 -> S1, fut=S2) at slot 105
        let trans_wire = [TAG_TRANSITION, StateExecutionUnit::ROLE_SWAP as u8, 0, 1, 2];
        let res_trans = process_seu_instruction_core(true, Some(&mut pda_bytes), 105, &trans_wire);
        assert!(res_trans.is_ok());
        let seu_trans = res_trans.unwrap();
        assert_eq!(seu_trans.current_node(), 1);
        assert_eq!(seu_trans.target_node(), 2);
        assert_eq!(seu_trans.last_slot(), 105);
        assert_eq!(u64::from_le_bytes(pda_bytes), seu_trans.0);

        // 4. Inactivity: advance slot by 145 slots (105 + 145 = 250). ROLE_DRAIN recovery to S0 (fut=S1)
        let drain_wire = [TAG_TRANSITION, StateExecutionUnit::ROLE_DRAIN as u8, 1, 0, 1];
        let res_drain = process_seu_instruction_core(true, Some(&mut pda_bytes), 250, &drain_wire);
        assert!(res_drain.is_ok());
        let seu_drain = res_drain.unwrap();
        assert_eq!(seu_drain.current_node(), 0);
        assert_eq!(seu_drain.target_node(), 1);
        assert_eq!(seu_drain.last_slot(), 250);

        // 5. TAG_STAR_CHECKPOINT with parity digest 0x42
        let star_wire = [TAG_STAR_CHECKPOINT, StateExecutionUnit::ROLE_SWAP as u8, 0, 1, 0x42];
        let res_star = process_seu_instruction_core(true, Some(&mut pda_bytes), 255, &star_wire);
        assert!(res_star.is_ok());
        let seu_star = res_star.unwrap();
        assert_eq!(seu_star.parity_digest(), 0x42);
        assert!((seu_star.0 & StateExecutionUnit::FLAG_MACRO_BLUEPRINT) != 0);
        assert_eq!(u64::from_le_bytes(pda_bytes), seu_star.0);
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
