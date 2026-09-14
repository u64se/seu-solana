use {
    mollusk_svm::{result::ProgramResult as MolluskResult, Mollusk},
    solana_account::Account,
    solana_instruction::{AccountMeta, Instruction},
    solana_program_error::ProgramError,
    solana_pubkey::Pubkey,
};

fn main() {
    if std::env::var("SBF_OUT_DIR").is_err() {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let deploy_dir = manifest_dir
            .parent()
            .map(|p| p.join("target/deploy"))
            .unwrap_or_else(|| std::path::PathBuf::from("../target/deploy"));
        std::env::set_var("SBF_OUT_DIR", deploy_dir);
    }

    let program_id = Pubkey::new_unique();
    let mut mollusk = Mollusk::new(&program_id, "u64se_seu");
    mollusk.warp_to_slot(100);

    let authority = Pubkey::new_unique();
    let (canonical_pda, bump) =
        Pubkey::find_program_address(&[b"u64se-seu", authority.as_ref()], &program_id);

    let authority_account = Account::new(100_000_000, 0, &Pubkey::default());

    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║       RUNNING SVM ONCHAIN ATTACK VECTOR & SECURITY SUITE         ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");

    // -------------------------------------------------------------------------
    // ATTACK 1: Missing Required Signature
    // -------------------------------------------------------------------------
    {
        let ix_data = vec![bump, 0, 1, 1, 0];
        let mut state_account = Account::new(10_000_000, 8, &program_id);
        state_account.data = vec![0u8; 8];

        let ix = Instruction::new_with_bytes(
            program_id,
            &ix_data,
            vec![
                AccountMeta::new(canonical_pda, false),
                AccountMeta::new_readonly(authority, false), // ATTACK: signer = false
            ],
        );

        let res = mollusk.process_instruction(
            &ix,
            &[
                (canonical_pda, state_account),
                (authority, authority_account.clone()),
            ],
        );

        assert_eq!(
            res.program_result,
            MolluskResult::Failure(ProgramError::MissingRequiredSignature)
        );
        println!("  [PASS] Attack 1: Missing signature successfully rejected with MissingRequiredSignature");
    }

    // -------------------------------------------------------------------------
    // ATTACK 2: Re-initialization Attack
    // -------------------------------------------------------------------------
    {
        let ix_data = vec![bump, 0, 1, 1, 0];
        let mut state_account = Account::new(10_000_000, 8, &program_id);
        state_account.data = vec![1, 2, 3, 4, 5, 6, 7, 8]; // ATTACK: already initialized (raw != 0)

        let ix = Instruction::new_with_bytes(
            program_id,
            &ix_data,
            vec![
                AccountMeta::new(canonical_pda, false),
                AccountMeta::new_readonly(authority, true),
            ],
        );

        let res = mollusk.process_instruction(
            &ix,
            &[
                (canonical_pda, state_account),
                (authority, authority_account.clone()),
            ],
        );

        assert_eq!(
            res.program_result,
            MolluskResult::Failure(ProgramError::AccountAlreadyInitialized)
        );
        println!("  [PASS] Attack 2: Re-initialization successfully rejected with AccountAlreadyInitialized");
    }

    // -------------------------------------------------------------------------
    // ATTACK 3: PDA Derivation Breach (Spoofed State PDA)
    // -------------------------------------------------------------------------
    {
        let spoofed_pda = Pubkey::new_unique(); // ATTACK: spoofed address
        let ix_data = vec![bump, 0, 1, 1, 0];
        let mut state_account = Account::new(10_000_000, 8, &program_id);
        state_account.data = vec![0u8; 8];

        let ix = Instruction::new_with_bytes(
            program_id,
            &ix_data,
            vec![
                AccountMeta::new(spoofed_pda, false),
                AccountMeta::new_readonly(authority, true),
            ],
        );

        let res = mollusk.process_instruction(
            &ix,
            &[
                (spoofed_pda, state_account),
                (authority, authority_account.clone()),
            ],
        );

        assert_eq!(
            res.program_result,
            MolluskResult::Failure(ProgramError::InvalidSeeds)
        );
        println!("  [PASS] Attack 3: Spoofed PDA successfully rejected with InvalidSeeds");
    }

    // -------------------------------------------------------------------------
    // ATTACK 4: Invalid Account Data Length (7 bytes instead of 8)
    // -------------------------------------------------------------------------
    {
        let ix_data = vec![bump, 0, 1, 1, 0];
        let mut state_account = Account::new(10_000_000, 7, &program_id); // ATTACK: 7 bytes
        state_account.data = vec![0u8; 7];

        let ix = Instruction::new_with_bytes(
            program_id,
            &ix_data,
            vec![
                AccountMeta::new(canonical_pda, false),
                AccountMeta::new_readonly(authority, true),
            ],
        );

        let res = mollusk.process_instruction(
            &ix,
            &[
                (canonical_pda, state_account),
                (authority, authority_account.clone()),
            ],
        );

        assert_eq!(
            res.program_result,
            MolluskResult::Failure(ProgramError::InvalidAccountData)
        );
        println!("  [PASS] Attack 4: Invalid data size (7b) rejected with InvalidAccountData");
    }

    // -------------------------------------------------------------------------
    // ATTACK 5: Readonly State Account Mutation Attempt
    // -------------------------------------------------------------------------
    {
        let ix_data = vec![bump, 0, 1, 1, 0];
        let mut state_account = Account::new(10_000_000, 8, &program_id);
        state_account.data = vec![0u8; 8];

        let ix = Instruction::new_with_bytes(
            program_id,
            &ix_data,
            vec![
                AccountMeta::new_readonly(canonical_pda, false), // ATTACK: passed as readonly
                AccountMeta::new_readonly(authority, true),
            ],
        );

        let res = mollusk.process_instruction(
            &ix,
            &[
                (canonical_pda, state_account),
                (authority, authority_account.clone()),
            ],
        );

        assert_eq!(
            res.program_result,
            MolluskResult::Failure(ProgramError::InvalidAccountData)
        );
        println!("  [PASS] Attack 5: Readonly state account rejected with InvalidAccountData");
    }

    println!("════════════════════════════════════════════════════════════════════");
    println!("  ALL 5 SVM SECURITY ATTACK VECTORS SUCCESSFULLY NEUTRALIZED!");
    println!("════════════════════════════════════════════════════════════════════");
}
