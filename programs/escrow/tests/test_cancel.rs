// Regression tests for the escrow time lock.
//
// Escrows can no longer be cancelled for 5 minutes after creation.
// test_cancel_too_early verifies that cancellation is rejected before
// the time lock has elapsed (and that the error is TimeLockActive).
// test_cancel_at_boundary verifies that cancellation succeeds at exactly
// created_at + 300 seconds (the >= boundary).
// test_cancel_late_enough verifies that cancellation succeeds after
// the time lock has elapsed (created_at + 301 seconds).
//
// Each file in tests/ is its own crate, so the setup helpers are
// copied from test_make.rs rather than shared.

use anchor_lang::{
    prelude::Clock,
    solana_program::instruction::Instruction,
    InstructionData, ToAccountMetas,
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

fn setup_token_account(svm: &mut LiteSVM, address: Pubkey, mint: Pubkey, owner: Pubkey, amount: u64) {
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

// ---------- the tests ----------

#[test]
fn test_cancel_too_early() {
    // Cancel immediately after make: should fail due to 5-minute time lock.
    let program_id = escrow::id();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/escrow.so");
    svm.add_program(program_id, bytes).unwrap();

    let (maker, mint_a, maker_ata_a, escrow_pda, vault_a) = setup_escrow_for_cancel(&mut svm, 1, 1_000_000, 500_000);
    let maker_pk = maker.pubkey();
    let mint_a_pk = mint_a.pubkey();

    let instruction = Instruction::new_with_bytes(
        program_id,
        &escrow::instruction::Cancel {}.data(),
        escrow::accounts::Cancel {
            maker: maker_pk,
            escrow: escrow_pda,
            mint_a: mint_a_pk,
            maker_ata_a,
            vault_a,
            token_program: TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&maker_pk), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[maker]).unwrap();

    let res = svm.send_transaction(tx);
    let err = res.unwrap_err();
    let logs = err.meta.logs.join("\n");
    assert!(
        logs.contains("TimeLockActive"),
        "expected TimeLockActive error, got: {logs}"
    );
}

#[test]
fn test_cancel_at_boundary() {
    // At exactly 5 minutes: cancel should succeed (>= boundary).
    let program_id = escrow::id();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/escrow.so");
    svm.add_program(program_id, bytes).unwrap();

    let (maker, mint_a, maker_ata_a, escrow_pda, vault_a) =
        setup_escrow_for_cancel(&mut svm, 3, 1_000_000, 500_000);
    let maker_pk = maker.pubkey();
    let mint_a_pk = mint_a.pubkey();

    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp += 300;
    svm.set_sysvar(&clock);

    let instruction = Instruction::new_with_bytes(
        program_id,
        &escrow::instruction::Cancel {}.data(),
        escrow::accounts::Cancel {
            maker: maker_pk,
            escrow: escrow_pda,
            mint_a: mint_a_pk,
            maker_ata_a,
            vault_a,
            token_program: TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&maker_pk), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[maker]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "cancel should have succeeded at boundary: {:?}",
        res.err()
    );
}

#[test]
fn test_cancel_late_enough() {
    // Advance clock past the 5-minute time lock, then cancel: should succeed.
    let program_id = escrow::id();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/escrow.so");
    svm.add_program(program_id, bytes).unwrap();

    let (maker, mint_a, maker_ata_a, escrow_pda, vault_a) = setup_escrow_for_cancel(&mut svm, 2, 1_000_000, 500_000);
    let maker_pk = maker.pubkey();
    let mint_a_pk = mint_a.pubkey();

    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp += 301;
    svm.set_sysvar(&clock);

    let instruction = Instruction::new_with_bytes(
        program_id,
        &escrow::instruction::Cancel {}.data(),
        escrow::accounts::Cancel {
            maker: maker_pk,
            escrow: escrow_pda,
            mint_a: mint_a_pk,
            maker_ata_a,
            vault_a,
            token_program: TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&maker_pk), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[maker]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "cancel should have succeeded after 5 minutes: {:?}", res.err());
}

fn setup_escrow_for_cancel(svm: &mut LiteSVM, seed: u16, amount_a: u64, amount_b: u64) -> (Keypair, Keypair, Pubkey, Pubkey, Pubkey) {
    // Creates a funded maker and a live escrow (used for both time lock tests).
    let program_id = escrow::id();
    let maker = Keypair::new();
    let mint_a = Keypair::new();
    let mint_b = Keypair::new();

    let maker_pk = maker.pubkey();
    let mint_a_pk = mint_a.pubkey();

    svm.airdrop(&maker_pk, 10_000_000_000).unwrap();
    setup_mint(svm, &mint_a, &maker_pk, 6);
    setup_mint(svm, &mint_b, &maker_pk, 6);

    let maker_ata_a = get_associated_token_address(&maker_pk, &mint_a_pk);
    setup_token_account(svm, maker_ata_a, mint_a_pk, maker_pk, amount_a);

    let (escrow_pda, _bump) = Pubkey::find_program_address(
        &[b"escrow", maker_pk.as_ref(), &seed.to_le_bytes()],
        &program_id,
    );
    let vault_a = get_associated_token_address(&escrow_pda, &mint_a_pk);

    let instruction = Instruction::new_with_bytes(
        program_id,
        &escrow::instruction::Make { seed, amount_a, amount_b }.data(),
        escrow::accounts::Make {
            maker: maker_pk,
            mint_a: mint_a_pk,
            mint_b: mint_b.pubkey(),
            escrow: escrow_pda,
            maker_ata_a,
            vault_a,
            system_program: anchor_lang::system_program::ID,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: spl_associated_token_account_interface::program::ID,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&maker_pk), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&maker]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "make transaction failed: {:?}", res.err());

    (maker, mint_a, maker_ata_a, escrow_pda, vault_a)
}
