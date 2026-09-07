use anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas};
use litesvm::types::FailedTransactionMetadata;
use litesvm::LiteSVM;
use solana_account::Account;
use solana_clock::Clock;
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

fn setup_svm() -> LiteSVM {
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/escrow.so");
    svm.add_program(escrow::id(), bytes).unwrap();
    svm
}

fn send(
    svm: &mut LiteSVM,
    payer: &Keypair,
    ix: Instruction,
) -> Result<litesvm::types::TransactionMetadata, FailedTransactionMetadata> {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx)
}

fn token_amount(svm: &LiteSVM, address: &Pubkey) -> u64 {
    let account = svm
        .get_account(address)
        .expect("token account should exist");
    TokenAccount::unpack(&account.data)
        .expect("should deserialize as a token account")
        .amount
}

fn assert_closed(svm: &LiteSVM, address: &Pubkey, label: &str) {
    assert!(
        svm.get_account(address).is_none_or(|a| a.data.is_empty()),
        "{label} should be closed"
    );
}

fn assert_logs_contain(err: &FailedTransactionMetadata, needle: &str) {
    assert!(
        err.meta.logs.iter().any(|l| l.contains(needle)),
        "expected log `{needle}`, got:\n{}",
        err.meta.pretty_logs()
    );
}

fn advance_clock(svm: &mut LiteSVM, seconds: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp += seconds;
    svm.set_sysvar(&clock);
}

struct LiveEscrow {
    maker: Keypair,
    escrow: Pubkey,
    mint_a: Pubkey,
    mint_b: Pubkey,
    vault_a: Pubkey,
}

fn setup_escrow(svm: &mut LiteSVM) -> LiveEscrow {
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

    LiveEscrow {
        maker,
        escrow: escrow_pda,
        mint_a: mint_a_pk,
        mint_b: mint_b_pk,
        vault_a,
    }
}

fn setup_taker(svm: &mut LiteSVM, mint_b: Pubkey, amount: u64) -> (Keypair, Pubkey) {
    let taker = Keypair::new();
    svm.airdrop(&taker.pubkey(), 10_000_000_000).unwrap();

    let taker_ata_b = get_associated_token_address(&taker.pubkey(), &mint_b);
    setup_token_account(svm, taker_ata_b, mint_b, taker.pubkey(), amount);

    (taker, taker_ata_b)
}

fn build_take_ix(
    taker: &Pubkey,
    maker: &Pubkey,
    escrow: Pubkey,
    mint_a: Pubkey,
    mint_b: Pubkey,
    vault_a: Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        escrow::id(),
        &escrow::instruction::Take {}.data(),
        escrow::accounts::Take {
            taker: *taker,
            maker: *maker,
            escrow,
            mint_a,
            mint_b,
            taker_ata_a: get_associated_token_address(taker, &mint_a),
            taker_ata_b: get_associated_token_address(taker, &mint_b),
            maker_ata_b: get_associated_token_address(maker, &mint_b),
            vault_a,
            system_program: anchor_lang::system_program::ID,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: spl_associated_token_account_interface::program::ID,
        }
        .to_account_metas(None),
    )
}

fn take_ix(escrow: &LiveEscrow, taker: &Pubkey) -> Instruction {
    build_take_ix(
        taker,
        &escrow.maker.pubkey(),
        escrow.escrow,
        escrow.mint_a,
        escrow.mint_b,
        escrow.vault_a,
    )
}

#[test]
fn take_swaps_tokens_after_timelock() {
    let mut svm = setup_svm();
    let escrow = setup_escrow(&mut svm);
    let (taker, taker_ata_b) = setup_taker(&mut svm, escrow.mint_b, AMOUNT_B);
    let taker_ata_a = get_associated_token_address(&taker.pubkey(), &escrow.mint_a);
    let maker_ata_b = get_associated_token_address(&escrow.maker.pubkey(), &escrow.mint_b);

    advance_clock(&mut svm, escrow::TIME_LOCK + 1);

    send(&mut svm, &taker, take_ix(&escrow, &taker.pubkey()))
        .expect("take should succeed after the lock");

    assert_eq!(
        token_amount(&svm, &taker_ata_a),
        AMOUNT_A,
        "taker should receive A"
    );
    assert_eq!(
        token_amount(&svm, &taker_ata_b),
        0,
        "taker should have spent B"
    );
    assert_eq!(
        token_amount(&svm, &maker_ata_b),
        AMOUNT_B,
        "maker should receive B"
    );
    assert_closed(&svm, &escrow.vault_a, "vault");
    assert_closed(&svm, &escrow.escrow, "escrow");
}

#[test]
fn take_rejects_while_timelock_is_active() {
    let mut svm = setup_svm();
    let escrow = setup_escrow(&mut svm);
    let (taker, _) = setup_taker(&mut svm, escrow.mint_b, AMOUNT_B);

    let err = send(&mut svm, &taker, take_ix(&escrow, &taker.pubkey()))
        .expect_err("take should fail before the lock expires");

    assert_logs_contain(&err, "Escrow in timelock");
}

#[test]
fn take_rejects_at_exact_timelock_boundary() {
    let mut svm = setup_svm();
    let escrow = setup_escrow(&mut svm);
    let (taker, _) = setup_taker(&mut svm, escrow.mint_b, AMOUNT_B);

    // require!(created_at + TIME_LOCK < now) — equality is still locked.
    advance_clock(&mut svm, escrow::TIME_LOCK);

    let err = send(&mut svm, &taker, take_ix(&escrow, &taker.pubkey()))
        .expect_err("take should fail when now == created_at + TIME_LOCK");

    assert_logs_contain(&err, "Escrow in timelock");
}

#[test]
fn take_rejects_when_taker_lacks_amount_b() {
    let mut svm = setup_svm();
    let escrow = setup_escrow(&mut svm);
    let (taker, _) = setup_taker(&mut svm, escrow.mint_b, AMOUNT_B - 1);

    advance_clock(&mut svm, escrow::TIME_LOCK + 1);

    let err = send(&mut svm, &taker, take_ix(&escrow, &taker.pubkey()))
        .expect_err("take should fail when the taker cannot pay amount_b");

    assert_logs_contain(&err, "insufficient funds");
}

#[test]
fn take_rejects_wrong_mint_b() {
    let mut svm = setup_svm();
    let escrow = setup_escrow(&mut svm);
    let decoy = Keypair::new();
    setup_mint(&mut svm, &decoy, &escrow.maker.pubkey(), 6);
    let (taker, _) = setup_taker(&mut svm, decoy.pubkey(), AMOUNT_B);

    advance_clock(&mut svm, escrow::TIME_LOCK + 1);

    let ix = build_take_ix(
        &taker.pubkey(),
        &escrow.maker.pubkey(),
        escrow.escrow,
        escrow.mint_a,
        decoy.pubkey(),
        escrow.vault_a,
    );

    let err = send(&mut svm, &taker, ix).expect_err("has_one = mint_b should reject a decoy mint");

    assert_logs_contain(&err, "ConstraintHasOne");
}
