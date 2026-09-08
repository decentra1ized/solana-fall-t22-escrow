# Escrow

This example demonstrates how to implement a trustless token escrow using the Anchor framework on Solana. Two parties can exchange SPL tokens without requiring mutual trust — the program holds the maker's tokens in a vault until the taker fulfills the agreed terms, or the maker cancels and reclaims them.

This fork extends the base program with a **minimum lifetime (time lock)** on every escrow: an offer cannot be cancelled for `CANCEL_DELAY_SECONDS` (5 minutes) after creation. The motivation, formal properties, and verification strategy are described in [§4](#4-the-time-lock) and [§6](#6-security-analysis).

---

## Table of contents

1. [Motivation](#1-motivation)
2. [Program architecture](#2-program-architecture)
3. [Account model](#3-account-model)
4. [The time lock](#4-the-time-lock)
5. [Instruction reference](#5-instruction-reference)
6. [Security analysis](#6-security-analysis)
7. [Testing methodology](#7-testing-methodology)
8. [Building and running the tests](#8-building-and-running-the-tests)
9. [Limitations and future work](#9-limitations-and-future-work)

---

## 1. Motivation

An escrow solves a coordination problem: two parties want to trade tokens and neither wants to send first. The program holds the goods instead of a person. The maker states terms ("100 of token A for 50 of token B"), deposits their token A into a vault the program controls, and anyone may take the offer — both movements settle in a single atomic transaction. If nobody takes it, the maker reclaims the deposit. Nobody ever has to trust the other side.

The base program, however, permits the maker to cancel **at any instant, including the instant a taker's transaction is in flight**. This has two consequences:

- **The offer is a free option.** The maker retains all upside — if the market moves their way, someone takes the offer; if it moves against them, they cancel at zero cost. In traditional finance such an option carries a premium; here the cost is externalized onto takers.
- **Takers pay to race.** A taker submits a transaction, the maker's cancel lands first, and the taker's transaction fails after the taker has already paid the fee. Repeated at scale, this is a griefing vector.

A minimum lifetime makes the offer *credible*: if the maker cannot cancel for five minutes, then for five minutes the offer genuinely means what it says, and a taker can act on it without racing.

## 2. Program architecture

Three instructions, one state account, one vault.

```
                ┌────────────┐
   make         │   escrow   │   PDA ["escrow", maker, seed]
  ───────►      │  (state)   │   terms: mints, amounts, seed, bump,
                └────────────┘   created_at
                     ▲ owns
                     │ authority
                ┌────────────┐
                │  vault_a   │   ATA of escrow PDA for mint_a
                └────────────┘   holds the maker's token A

  take:  taker_ata_b ──amount_b──► maker_ata_b      (same tx)
         vault_a      ──amount_a──► taker_ata_a     (atomic)

  cancel: vault_a ──amount_a──► maker_ata_a, then both accounts close
```

| Instruction | Caller | Effect |
|---|---|---|
| `make` | maker | Creates the escrow PDA, stamps `created_at`, moves token A into the vault |
| `take` | anyone | Taker's token B → maker, vault's token A → taker, atomically; closes escrow and vault |
| `cancel` | maker only | After the time lock elapses: vault's token A → maker, escrow and vault closed |

Because the vault's authority is a PDA, no human key can move the deposited tokens; only the program can, and only by the rules written in it. This is what "trustless" means in practice — not that everyone is honest, but that dishonesty has nowhere to happen.

### Source layout

| File | Holds |
|---|---|
| `programs/escrow/src/state/escrow.rs` | The `Escrow` account struct |
| `programs/escrow/src/instructions/make.rs` | Escrow creation, vault funding, `created_at` stamping |
| `programs/escrow/src/instructions/take.rs` | The atomic swap |
| `programs/escrow/src/instructions/cancel.rs` | Time-lock check, token return, account closing |
| `programs/escrow/src/error.rs` | Program errors, including `TimeLockActive` |
| `programs/escrow/src/constants.rs` | `CANCEL_DELAY_SECONDS` and the PDA seed constant |
| `programs/escrow/tests/` | LiteSVM integration tests |

## 3. Account model

The `Escrow` state account:

```rust
#[account]
#[derive(InitSpace)]
pub struct Escrow {
    pub maker: Pubkey,      // The maker of the escrow
    pub mint_a: Pubkey,     // The mint of the token being offered by the maker
    pub mint_b: Pubkey,     // The mint of the token being requested by the maker
    pub amount_a: u64,      // The amount of the token being offered by the maker
    pub amount_b: u64,      // The amount of the token being requested by the maker
    pub seed: u16,          // The seed used for PDA derivation
    pub bump: u8,           // The bump used for PDA derivation
    pub created_at: i64,    // Unix timestamp (Clock sysvar) at which the escrow was made
}
```

- **maker** — the account that created the offer; the only signer `cancel` accepts.
- **mint_a / mint_b** — the token offered and the token requested. Validated on `take` via `has_one` constraints.
- **amount_a / amount_b** — the terms of the trade.
- **seed** — a `u16` the maker picks, so one maker can run several concurrent offers; the PDA is derived from `["escrow", maker, seed.to_le_bytes()]`.
- **bump** — the canonical bump, stored so CPI signer seeds never need to re-derive it.
- **created_at** — *(added in this fork)* the `Clock` sysvar's `unix_timestamp` at creation, the epoch from which the time lock is measured.

Two type-level decisions are worth recording:

- **`i64`, not `u64`.** Solana's `unix_timestamp` is a signed 64-bit integer — seconds since 1970, negative meaning before 1970. No negative value is reachable here, but matching the type the runtime supplies avoids a cast, and casts are where sign bugs are born.
- **Field appended last.** `created_at` is appended after `bump` rather than inserted mid-struct. Anchor deserializes by field name, so reordering happens to be tolerated here, but appending is the habit that keeps compatibility with any reader that consumes the account at fixed byte offsets.

The account size is computed as `8 + Escrow::INIT_SPACE`; the `InitSpace` derive recomputed the discriminator-inclusive size automatically when the field was added, which is 8 additional bytes of rent per escrow.

## 4. The time lock

### 4.1 Semantics

An escrow created at time `t₀` is **locked** for every `t < t₀ + CANCEL_DELAY_SECONDS` and **unlocked** for every `t ≥ t₀ + CANCEL_DELAY_SECONDS`.

- The comparison is **inclusive at the boundary**: `now >= created_at + CANCEL_DELAY_SECONDS` permits cancellation at exactly `t₀ + 300`. "Five minutes have passed" is read as `≥`, not `>`.
- The check is the **first statement** in `cancel`'s handler — it runs before any CPI, so a rejected cancellation moves no tokens and closes no accounts.
- Failure is expressed with a named error (`TimeLockActive`) rather than a bare constraint, so clients can distinguish "too early" from any other failure.

```rust
let now = Clock::get()?.unix_timestamp;
let unlock_at = ctx.accounts.escrow.created_at
    .checked_add(CANCEL_DELAY_SECONDS)
    .ok_or_else(|| error!(ErrorCode::TimeLockActive))?;
require!(now >= unlock_at, ErrorCode::TimeLockActive);
```

### 4.2 The constant

```rust
#[constant]
pub const CANCEL_DELAY_SECONDS: i64 = 300; // 5 minutes
```

The delay lives in `constants.rs` rather than inline in the handler for two reasons: a named constant tells the reader what `300` means, and it puts the value in one place when "can we make it ten minutes?" arrives. The `#[constant]` attribute additionally exports it into the IDL, so a frontend can read the same value instead of hardcoding its own copy and drifting out of sync.

### 4.3 The clock, and why it can be trusted here

Solana programs cannot read an operating system clock; the runtime exposes the **`Clock` sysvar**, updated with the current slot, epoch, and a `unix_timestamp` agreed on by validators.

That timestamp is *agreed, not measured*: it is derived from validator votes, can drift a handful of seconds from any wall clock, and is not perfectly monotonic. For a five-minute lock this is immaterial — a few seconds of drift or a small backward step shifts the effective lock duration trivially. It would *not* be immaterial for anything needing sub-second precision or strict ordering, and any future parameter change should be re-evaluated against this assumption.

The addition is performed with `checked_add` rather than `+`. In release builds Rust wraps on overflow instead of panicking: a corrupted `created_at` near `i64::MAX` plus 300 would wrap to a huge negative number, making `now >= unlock_at` trivially true — the lock would *open* instead of holding. With `checked_add`, the overflow branch maps to `TimeLockActive`, i.e. **fail-closed**: if the unlock time cannot be computed, the lock stays shut. This is unreachable with a real clock, but "cannot happen" is how audit findings start; checked arithmetic is the default whenever a number came from stored state.

### 4.4 What the lock does not do

- It does not restrict `take`. A taker may fulfill the offer at any time, including while the lock is active — the lock constrains only the maker's exit.
- It does not make the offer irrevocable. After five minutes the maker may still cancel; the lock buys *takers a race-free window*, not the maker a commitment forever.
- It is not a fee mechanism, a Dutch-auction decay, or a commitment game — those would each require additional state and are out of scope.

## 5. Instruction reference

### `make`

Creates the escrow PDA from `["escrow", maker, seed]`, initializes the vault ATA owned by that PDA, stamps `created_at` from the `Clock` sysvar, and moves `amount_a` of mint A from the maker's ATA into the vault via `transfer_checked`.

Key accounts: `maker` (signer, mut), `mint_a`, `mint_b`, `escrow` (init, PDA), `maker_ata_a` (mut), `vault_a` (init, ATA of escrow), `system_program`, `token_program`, `associated_token_program`.

### `take`

Executes the swap atomically: `amount_b` flows from the taker's ATA for mint B to the maker's ATA for mint B, then `amount_a` flows from the vault to the taker's ATA for mint A, the second CPI signed by the escrow PDA seeds `["escrow", maker, seed, bump]`. Both the escrow and the vault are closed, rent returning to taker and maker respectively.

Key accounts: `taker` (signer, mut), `maker` (mut), `escrow` (mut, closed, `has_one = mint_a/b`), `mint_a`, `mint_b`, `taker_ata_a` (init_if_needed), `taker_ata_b` (mut), `maker_ata_b` (init_if_needed), `vault_a` (mut), plus the three programs.

### `cancel`

After the time lock elapses, returns the vault's entire balance (`vault_a.amount`) to the maker's ATA via a PDA-signed `transfer_checked`, then closes the vault and the escrow, reclaiming rent to the maker.

Key accounts: `maker` (signer), `escrow` (mut, closed, PDA re-derived from the stored `seed` and `bump`), `mint_a`, `maker_ata_a` (mut), `vault_a` (mut), `token_program`.

## 6. Security analysis

**Threat model.** The maker is potentially adversarial toward takers; the taker is potentially adversarial toward the maker; the program and the validator majority are trusted (the latter for clock agreement).

| Threat | Mitigation |
|---|---|
| Maker cancels in front of an in-flight `take` | Time lock guarantees a race-free window after `make` |
| Maker withdraws the deposit outside program rules | Vault authority is the escrow PDA; no human key can sign for it |
| Anyone other than the maker cancels | `cancel` requires the maker as `Signer` and re-derives the PDA from the stored `seed`/`bump` |
| Wrong mint or wrong escrow passed to `take` | `has_one = mint_a/b` constraints and PDA re-derivation bind the accounts to the stored terms |
| Arithmetic wraparound opening the lock | `checked_add` with fail-closed error mapping |
| Partial settlement | Both legs of `take` are CPIs in one transaction; the runtime's atomicity means either all succeed or all fail |

**Residual risks.** The `unix_timestamp` a validator clique reports is the sole source of time; a sufficiently faulty supermajority could distort lock durations, though not steal funds. Cancellation remains possible after the lock, so a taker acting late still races — the lock narrows the window, it does not eliminate it. The `Escrow` account layout changed in this fork; any off-chain indexer reading the account at fixed offsets must be updated (see §3).

## 7. Testing methodology

Tests are Rust integration tests under `programs/escrow/tests/`, executed by `cargo test` against **LiteSVM**, an in-process Solana VM. The compiled program is embedded with `include_bytes!("../../../target/deploy/escrow.so")`, so `anchor build` must precede `cargo test` — the harness fails with a missing-file compile error otherwise, not a test failure.

**Time travel.** Waiting five real minutes is not a test strategy, and LiteSVM's `warp_to_slot` does not help: it moves the slot while leaving `unix_timestamp` untouched, and `unix_timestamp` is what the program reads. The tests instead rewrite the sysvar itself:

```rust
let mut clock = svm.get_sysvar::<Clock>();
clock.unix_timestamp += seconds;
svm.set_sysvar(&clock);
```

`anchor_lang::prelude::Clock` is the same type LiteSVM wants, so no additional dependency is needed. Two properties of a fresh LiteSVM matter for reading the tests: it starts at `unix_timestamp = 0` (so `make` stamps `created_at = 0`, and the clock is advanced from zero, not from today), and setting the sysvar changes the clock from that point on — hence the ordering *make → move clock → cancel*.

**Suite.**

| Test | File | Asserts |
|---|---|---|
| `test_make` | `test_make.rs` | `make` funds the vault with `amount_a` and populates every escrow field |
| `cancel_returns_the_tokens_to_the_maker` | `test_cancel.rs` | After the lock, cancel returns the full deposit and closes both accounts (regression test for the `close_vault` signer seeds, which omitted the escrow seed on the base branch) |
| `cancel_before_the_time_lock_fails` | `test_cancel.rs` | Cancel at `t = created_at` fails with `TimeLockActive`; vault balance, maker balance, and escrow existence are all unchanged |
| `cancel_exactly_at_the_boundary_succeeds` | `test_cancel.rs` | Cancel at exactly `created_at + 300` succeeds — the only test that can distinguish `>=` from `>` |
| `cancel_after_the_time_lock_succeeds` | `test_cancel.rs` | Cancel at `created_at + 301` succeeds and closes the vault |

The rejection test is *honest* in the specific sense that it does not merely check an error occurred: it asserts the named error appeared in the logs **and** that the failure moved nothing — no tokens left the vault, none arrived at the maker, the escrow still exists. A check that failed *after* moving tokens would be a worse bug than no check at all.

## 8. Building and running the tests

```bash
anchor build   # produces target/deploy/escrow.so — required before cargo test
cargo test     # runs the LiteSVM integration tests
```

Order matters: the test harness embeds the compiled `.so`, so building first is required after every change to the program.

## 9. Limitations and future work

- **Fixed delay.** `CANCEL_DELAY_SECONDS` is a compile-time constant. A per-escrow, maker-chosen delay (stored in the account, bounded by a program maximum) would be more expressive at the cost of 8 bytes and one instruction argument.
- **Clock granularity.** `unix_timestamp` has second resolution and validator-vote-derived drift; use cases needing stricter time guarantees should consider slot-height-based locks or an oracle.
- **No partial fills.** An offer is taken whole or not at all; splitting `amount_a`/`amount_b` across takers would require a remainder model.
- **No expiry.** The offer lives until taken or cancelled; an `expires_at` field could let stale offers lapse automatically, though garbage collection incentives would need care.
- **Single-hop swaps.** Multi-leg trades (A↔B↔C) or cross-program offers are out of scope of this design.

---

*This document describes the program as of the `feat/escrow-timelock` branch.*
