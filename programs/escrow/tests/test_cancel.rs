use anchor_lang::prelude::Clock;
use anchor_lang::{
    solana_program::instruction::Instruction, AccountDeserialize, InstructionData, ToAccountMetas,
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


fn setup_made_escrow() -> (LiteSVM, Keypair, Pubkey, Pubkey, Pubkey, Pubkey) {
    let program_id = escrow::id();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/escrow.so");
    svm.add_program(program_id, bytes).unwrap();

    let maker = Keypair::new();
    let mint_a = Keypair::new();
    let mint_b = Keypair::new();
    let maker_pk = maker.pubkey();
    let mint_a_pk = mint_a.pubkey();
    let mint_b_pk = mint_b.pubkey();

    svm.airdrop(&maker_pk, 10_000_000_000).unwrap();
    setup_mint(&mut svm, &mint_a, &maker_pk, 6);
    setup_mint(&mut svm, &mint_b, &maker_pk, 6);

    let seed: u16 = 42;
    let amount_a: u64 = 1_000_000;
    let amount_b: u64 = 500_000;

    let maker_ata_a = get_associated_token_address(&maker_pk, &mint_a_pk);
    setup_token_account(&mut svm, maker_ata_a, mint_a_pk, maker_pk, amount_a);

    let (escrow_pda, _) = Pubkey::find_program_address(
        &[b"escrow", maker_pk.as_ref(), &seed.to_le_bytes()],
        &program_id,
    );
    let vault_a = get_associated_token_address(&escrow_pda, &mint_a_pk);

    let instruction = Instruction::new_with_bytes(
        program_id,
        &escrow::instruction::Make {
            seed,
            amount_a,
            amount_b,
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

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&maker_pk), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&maker]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "make transaction failed: {:?}", res.err());

    (svm, maker, escrow_pda, mint_a_pk, maker_ata_a, vault_a)
}

fn try_cancel(
    svm: &mut LiteSVM,
    maker: &Keypair,
    escrow_pda: Pubkey,
    mint_a_pk: Pubkey,
    maker_ata_a: Pubkey,
    vault_a: Pubkey,
) -> Result<(), String> {
    let program_id = escrow::id();
    let maker_pk = maker.pubkey();

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

    match svm.send_transaction(tx) {
        Ok(_) => Ok(()),
        Err(err) => Err(err.meta.logs.join("\n")),
    }
}

#[test]
fn cancel_too_early_fails_with_time_lock_error() {
    let (mut svm, maker, escrow_pda, mint_a_pk, maker_ata_a, vault_a) = setup_made_escrow();

    let logs = try_cancel(&mut svm, &maker, escrow_pda, mint_a_pk, maker_ata_a, vault_a)
        .expect_err("cancel should fail before the delay");
    assert!(
        logs.contains("TimeLockActive"),
        "expected TimeLockActive in failure logs, got: {logs}"
    );
}

#[test]
fn cancel_after_five_minutes_succeeds() {
    let (mut svm, maker, escrow_pda, mint_a_pk, maker_ata_a, vault_a) = setup_made_escrow();

    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp += 301;
    svm.set_sysvar(&clock);

    try_cancel(&mut svm, &maker, escrow_pda, mint_a_pk, maker_ata_a, vault_a)
        .expect("cancel should succeed after the delay");
}

#[test]
fn cancel_at_exact_boundary_succeeds() {
    let (mut svm, maker, escrow_pda, mint_a_pk, maker_ata_a, vault_a) = setup_made_escrow();

    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp += 300;
    svm.set_sysvar(&clock);

    try_cancel(&mut svm, &maker, escrow_pda, mint_a_pk, maker_ata_a, vault_a)
        .expect("cancel should succeed exactly at the delay");
}
