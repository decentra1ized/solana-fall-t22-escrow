// Tests for `cancel`: the five minute time lock, and the `close_vault` signer seeds.
//
// LiteSVM's clock never advances on its own, so each test sets the Clock sysvar
// explicitly to place `cancel` before, at, or after the unlock time.
//
// Each file in tests/ is its own crate, so the setup helpers are copied from
// test_make.rs rather than shared.

use anchor_lang::{
    prelude::Clock, solana_program::instruction::Instruction, InstructionData, AccountDeserialize, ToAccountMetas,
};
use escrow::CANCEL_DELAY_SECONDS;
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

/// Moves the cluster clock forward by `seconds`.
///
/// `warp_to_slot` advances the slot but leaves `unix_timestamp` untouched, so it
/// cannot exercise a time lock — the sysvar has to be rewritten directly.
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

fn escrow_created_at(svm: &LiteSVM, escrow: &Pubkey) -> i64 {
    let account = svm.get_account(escrow).expect("escrow should exist");
    escrow::Escrow::try_deserialize(&mut account.data.as_slice())
        .expect("should deserialize as an escrow")
        .created_at
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

    // The maker must already hold the tokens `make` is about to move into the vault.
    let maker_ata_a = get_associated_token_address(&maker_pk, &mint_a_pk);
    setup_token_account(svm, maker_ata_a, mint_a_pk, maker_pk, AMOUNT_A);

    let (escrow_pda, _bump) = Pubkey::find_program_address(
        &[b"escrow", maker_pk.as_ref(), &SEED.to_le_bytes()],
        &escrow::id(),
    );

    // `make` creates the vault, so only derive its address here.
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

// ---------- the test ----------

#[test]
fn cancel_after_the_time_lock_returns_the_tokens_to_the_maker() {
    let mut svm = setup_svm();
    let (maker, escrow_pda, mint_a, maker_ata_a, vault_a) = setup_escrow(&mut svm);

    // Precondition: `make` moved the tokens out of the maker and into the vault.
    assert_eq!(token_amount(&svm, &maker_ata_a), 0, "maker should be empty after make");
    assert_eq!(token_amount(&svm, &vault_a), AMOUNT_A, "vault should hold the deposit");
    // The escrow is locked for CANCEL_DELAY_SECONDS; step past it.
    advance_clock(&mut svm, CANCEL_DELAY_SECONDS + 1);
    // Past the lock, so this should go through.
    send(
        &mut svm,
        &maker,
        build_cancel_ix(&maker.pubkey(), escrow_pda, mint_a, maker_ata_a, vault_a),
    )
    .expect("cancel should return the maker's tokens and close the vault");

    // The deposit came home, whole.
    assert_eq!(
        token_amount(&svm, &maker_ata_a),
        AMOUNT_A,
        "maker should have every token back"
    );

    // Both accounts are gone and their rent was reclaimed.
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
fn cancel_before_the_time_lock_is_rejected() {
    let mut svm = setup_svm();
    let (maker, escrow_pda, mint_a, maker_ata_a, vault_a) = setup_escrow(&mut svm);

    // No clock advance: `cancel` lands in the same instant as `make`.
    let err = send(
        &mut svm,
        &maker,
        build_cancel_ix(&maker.pubkey(), escrow_pda, mint_a, maker_ata_a, vault_a),
    )
    .unwrap_err();

    let logs = err.meta.logs.join("\n");
    assert!(
        logs.contains("TimeLockActive"),
        "expected the time lock error, got: {logs}"
    );

    // Nothing moved — the deposit is still locked up.
    assert_eq!(token_amount(&svm, &vault_a), AMOUNT_A, "vault should be untouched");
}

#[test]
fn cancel_exactly_at_the_boundary_succeeds() {
    let mut svm = setup_svm();
    let (maker, escrow_pda, mint_a, maker_ata_a, vault_a) = setup_escrow(&mut svm);

    // Land on created_at + CANCEL_DELAY_SECONDS exactly: the first legal second.
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = escrow_created_at(&svm, &escrow_pda) + CANCEL_DELAY_SECONDS;
    svm.set_sysvar(&clock);

    send(
        &mut svm,
        &maker,
        build_cancel_ix(&maker.pubkey(), escrow_pda, mint_a, maker_ata_a, vault_a),
    )
    .expect("cancel should be allowed at exactly created_at + CANCEL_DELAY_SECONDS");

    assert_eq!(token_amount(&svm, &maker_ata_a), AMOUNT_A);
}