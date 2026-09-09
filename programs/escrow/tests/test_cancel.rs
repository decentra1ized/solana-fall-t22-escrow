// Tests for the escrow cancellation time lock.
//
// A maker cannot cancel for CANCEL_DELAY_SECONDS (300s) after creating the escrow.
// This proves both sides of that boundary: too early is rejected, and the moment
// the lock lifts, cancellation succeeds and returns the maker's tokens.
//
// Each file in tests/ is its own crate, so the setup helpers are copied from
// test_make.rs rather than shared.

use anchor_lang::{
    prelude::Clock, solana_program::instruction::Instruction, InstructionData, ToAccountMetas,
};
use litesvm::LiteSVM;
use solana_account::Account;
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;
use spl_associated_token_account_interface::address::get_associated_token_address;
use spl_token_interface::{
    state::{Account as TokenAccount, AccountState, Mint},
    ID as TOKEN_PROGRAM_ID,
};

const SEED: u16 = 42;
const AMOUNT_A: u64 = 1_000_000;
const AMOUNT_B: u64 = 500_000;
const CANCEL_DELAY_SECONDS: i64 = 300;

// ---------- copied from test_make.rs ----------

fn setup_mint(svm: &mut LiteSVM, mint: &Keypair, authority: &Pubkey, decimals: u8) {
    let state = Mint {
        mint_authority: COption::Some(*authority),
        supply: 0,
        decimals,
        is_initialized: true,
        freeze_authority: COption::None,
    };
    let mut data = [0u8; Mint::LEN];
    Mint::pack(state, &mut data).unwrap();
    svm.set_account(
        mint.pubkey(),
        Account {
            lamports: 1_000_000_000,
            data: data.to_vec(),
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

fn setup_token_account(
    svm: &mut LiteSVM,
    address: Pubkey,
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
) {
    let state = TokenAccount {
        mint,
        owner,
        amount,
        delegate: COption::None,
        state: AccountState::Initialized,
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    };
    let mut data = [0u8; TokenAccount::LEN];
    TokenAccount::pack(state, &mut data).unwrap();
    svm.set_account(
        address,
        Account {
            lamports: 1_000_000_000,
            data: data.to_vec(),
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

// ---------- helpers ----------

fn setup_svm() -> LiteSVM {
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/escrow.so");
    svm.add_program(escrow::id(), bytes).unwrap();
    svm
}

/// Advances the on-chain clock's unix_timestamp by `seconds`.
///
/// `warp_to_slot` moves the slot but leaves unix_timestamp untouched, so this is
/// the only way to actually move wall-clock time forward in a LiteSVM test.
fn advance_clock(svm: &mut LiteSVM, seconds: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp += seconds;
    svm.set_sysvar(&clock);
}

/// Sends one instruction signed by `payer`.
///
/// Takes `&Keypair` rather than `Keypair` so the caller can reuse the maker for a
/// second transaction — a `&Keypair` is itself a `Signer`.
fn send(
    svm: &mut LiteSVM,
    payer: &Keypair,
    ix: Instruction,
) -> Result<litesvm::types::TransactionMetadata, litesvm::types::FailedTransactionMetadata> {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx)
}

fn token_amount(svm: &LiteSVM, address: &Pubkey) -> u64 {
    let account = svm.get_account(address).expect("token account should exist");
    TokenAccount::unpack(&account.data)
        .expect("should deserialize as a token account")
        .amount
}

/// Creates a funded maker and a live escrow holding AMOUNT_A of mint A.
///
/// Returns (maker, escrow PDA, mint A, maker's ATA for A, the escrow's vault).
fn setup_escrow(svm: &mut LiteSVM) -> (Keypair, Pubkey, Pubkey, Pubkey, Pubkey) {
    let maker = Keypair::new();
    let mint_a = Keypair::new();
    let mint_b = Keypair::new();

    let maker_pk = maker.pubkey();
    let mint_a_pk = mint_a.pubkey();
    let mint_b_pk = mint_b.pubkey();

    svm.airdrop(&maker_pk, 10_000_000_000).unwrap();

    setup_mint(svm, &mint_a, &maker_pk, 6);
    setup_mint(svm, &mint_b, &maker_pk, 6);

    let maker_ata_a = get_associated_token_address(&maker_pk, &mint_a_pk);
    setup_token_account(svm, maker_ata_a, mint_a_pk, maker_pk, AMOUNT_A);

    let (escrow_pda, _bump) = Pubkey::find_program_address(
        &[b"escrow", maker_pk.as_ref(), &SEED.to_le_bytes()],
        &escrow::id(),
    );

    let vault_a = get_associated_token_address(&escrow_pda, &mint_a_pk);

    let ix = Instruction::new_with_bytes(
        escrow::id(),
        &escrow::instruction::Make {
            seed: SEED,
            amount_a: AMOUNT_A,
            amount_b: AMOUNT_B,
        }
        .data(),
        escrow::accounts::Make {
            maker: maker_pk,
            mint_a: mint_a_pk,
            mint_b: mint_b_pk,
            escrow: escrow_pda,
            maker_ata_a,
            vault_a,
            system_program: anchor_lang::system_program::ID,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: spl_associated_token_account_interface::program::ID,
        }
        .to_account_metas(None),
    );

    send(svm, &maker, ix).expect("make should succeed");

    (maker, escrow_pda, mint_a_pk, maker_ata_a, vault_a)
}

fn build_cancel_ix(
    maker: &Pubkey,
    escrow: Pubkey,
    mint_a: Pubkey,
    maker_ata_a: Pubkey,
    vault_a: Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        escrow::id(),
        &escrow::instruction::Cancel {}.data(),
        escrow::accounts::Cancel {
            maker: *maker,
            escrow,
            mint_a,
            maker_ata_a,
            vault_a,
            token_program: TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    )
}

// ---------- the tests ----------

#[test]
fn cancel_too_early_is_rejected() {
    let mut svm = setup_svm();
    let (maker, escrow_pda, mint_a, maker_ata_a, vault_a) = setup_escrow(&mut svm);

    // No time has passed since `make` (a fresh LiteSVM starts at unix_timestamp = 0).
    let res = send(
        &mut svm,
        &maker,
        build_cancel_ix(&maker.pubkey(), escrow_pda, mint_a, maker_ata_a, vault_a),
    );

    let err = res.unwrap_err();
    let logs = err.meta.logs.join("\n");
    assert!(
        logs.contains("TimeLockActive"),
        "expected the time lock error, got: {logs}"
    );
}

#[test]
fn cancel_after_the_lock_returns_the_tokens_to_the_maker() {
    let mut svm = setup_svm();
    let (maker, escrow_pda, mint_a, maker_ata_a, vault_a) = setup_escrow(&mut svm);

    assert_eq!(token_amount(&svm, &maker_ata_a), 0, "maker should be empty after make");
    assert_eq!(token_amount(&svm, &vault_a), AMOUNT_A, "vault should hold the deposit");

    // Move the clock past the lock. This changes time from this point forward,
    // so it must happen after `make` (which already stamped created_at = 0).
    advance_clock(&mut svm, CANCEL_DELAY_SECONDS + 1);

    send(
        &mut svm,
        &maker,
        build_cancel_ix(&maker.pubkey(), escrow_pda, mint_a, maker_ata_a, vault_a),
    )
    .expect("cancel should return the maker's tokens and close the vault");

    assert_eq!(
        token_amount(&svm, &maker_ata_a),
        AMOUNT_A,
        "maker should have every token back"
    );
    assert!(
        svm.get_account(&vault_a).is_none_or(|a| a.data.is_empty()),
        "vault should be closed"
    );
    assert!(
        svm.get_account(&escrow_pda).is_none_or(|a| a.data.is_empty()),
        "escrow should be closed"
    );
}

#[test]
fn cancel_exactly_at_the_boundary_succeeds() {
    let mut svm = setup_svm();
    let (maker, escrow_pda, mint_a, maker_ata_a, vault_a) = setup_escrow(&mut svm);

    // Exactly created_at + 300: the boundary itself must be allowed (>=, not >).
    advance_clock(&mut svm, CANCEL_DELAY_SECONDS);

    let res = send(
        &mut svm,
        &maker,
        build_cancel_ix(&maker.pubkey(), escrow_pda, mint_a, maker_ata_a, vault_a),
    );

    assert!(
        res.is_ok(),
        "cancelling at exactly the boundary should succeed, not just after it"
    );
}
