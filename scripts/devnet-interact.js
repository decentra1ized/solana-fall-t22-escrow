const { Connection, Keypair, PublicKey, Transaction, TransactionInstruction, LAMPORTS_PER_SOL, SystemProgram } = require("@solana/web3.js");
const { createMint, getOrCreateAssociatedTokenAccount, mintTo } = require("@solana/spl-token");
const { readFileSync } = require("fs");

const PROGRAM_ID = new PublicKey("A8BAP8MgVSYqGpMxC2rcfsbtWJRQdhKk4RAf5Qube3Y1");
const DEVNET_RPC = "https://api.devnet.solana.com";
const TOKEN_PROGRAM = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ASSOC_TOKEN_PROGRAM = new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

const connection = new Connection(DEVNET_RPC, "confirmed");
const payer = Keypair.fromSecretKey(Buffer.from(JSON.parse(readFileSync("~/.config/solana/id.json", "utf8"))));
const MAKE_DISCRIMINATOR = Buffer.from([138, 227, 232, 77, 223, 166, 96, 197]);
const CANCEL_DISCRIMINATOR = Buffer.from([232, 219, 223, 41, 219, 236, 220, 190]);

function makeData(seed, amountA, amountB) {
  const data = Buffer.alloc(8 + 2 + 8 + 8);
  MAKE_DISCRIMINATOR.copy(data, 0);
  data.writeUInt16LE(seed, 8);
  data.writeBigUInt64LE(amountA, 10);
  data.writeBigUInt64LE(amountB, 18);
  return data;
}

function cancelData() {
  return Buffer.from([232, 219, 223, 41, 219, 236, 220, 190]);
}

async function main() {
  console.log("Payer:", payer.publicKey.toString());
  console.log("Balance:", (await connection.getBalance(payer.publicKey)) / LAMPORTS_PER_SOL, "SOL");

  const mintA = await createMint(connection, payer, payer.publicKey, null, 6);
  const mintB = await createMint(connection, payer, payer.publicKey, null, 6);
  console.log("Mint A:", mintA.toString());
  console.log("Mint B:", mintB.toString());

  const makerAtaA = await getOrCreateAssociatedTokenAccount(connection, payer, mintA, payer.publicKey);
  if (makerAtaA.amount === 0n) {
    await mintTo(connection, payer, mintA, makerAtaA.address, payer, BigInt(1_000_000_000_000));
  }
  console.log("Maker ATA:", makerAtaA.address.toString());

  // TX1: make escrow #1
  const seed1 = 100;
  const escrowPda1 = PublicKey.findProgramAddressSync(
    [Buffer.from("escrow"), payer.publicKey.toBuffer(), Buffer.from([seed1 & 0xFF, (seed1 >> 8) & 0xFF])],
    PROGRAM_ID,
  )[0];
  const vaultA1 = PublicKey.findProgramAddressSync(
    [escrowPda1.toBuffer(), TOKEN_PROGRAM.toBuffer(), mintA.toBuffer()],
    ASSOC_TOKEN_PROGRAM,
  )[0];

  const tx1 = new Transaction().add(new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: payer.publicKey, isSigner: true, isWritable: true },
      { pubkey: mintA, isSigner: false, isWritable: false },
      { pubkey: mintB, isSigner: false, isWritable: false },
      { pubkey: escrowPda1, isSigner: false, isWritable: true },
      { pubkey: makerAtaA.address, isSigner: false, isWritable: true },
      { pubkey: vaultA1, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: TOKEN_PROGRAM, isSigner: false, isWritable: false },
      { pubkey: ASSOC_TOKEN_PROGRAM, isSigner: false, isWritable: false },
    ],
    data: makeData(seed1, BigInt(100_000), BigInt(50_000)),
  }));

  const sig1 = await connection.sendTransaction(tx1, [payer]);
  await connection.confirmTransaction(sig1);
  console.log("\nTX1 (make):", sig1);
  console.log("Escrow PDA 1:", escrowPda1.toString());
  console.log("Vault A 1:", vaultA1.toString());

  // TX2: make escrow #2
  const seed2 = 200;
  const escrowPda2 = PublicKey.findProgramAddressSync(
    [Buffer.from("escrow"), payer.publicKey.toBuffer(), Buffer.from([seed2 & 0xFF, (seed2 >> 8) & 0xFF])],
    PROGRAM_ID,
  )[0];
  const vaultA2 = PublicKey.findProgramAddressSync(
    [escrowPda2.toBuffer(), TOKEN_PROGRAM.toBuffer(), mintA.toBuffer()],
    ASSOC_TOKEN_PROGRAM,
  )[0];

  const tx2 = new Transaction().add(new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: payer.publicKey, isSigner: true, isWritable: true },
      { pubkey: mintA, isSigner: false, isWritable: false },
      { pubkey: mintB, isSigner: false, isWritable: false },
      { pubkey: escrowPda2, isSigner: false, isWritable: true },
      { pubkey: makerAtaA.address, isSigner: false, isWritable: true },
      { pubkey: vaultA2, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: TOKEN_PROGRAM, isSigner: false, isWritable: false },
      { pubkey: ASSOC_TOKEN_PROGRAM, isSigner: false, isWritable: false },
    ],
    data: makeData(seed2, BigInt(200_000), BigInt(100_000)),
  }));

  const sig2 = await connection.sendTransaction(tx2, [payer]);
  await connection.confirmTransaction(sig2);
  console.log("\nTX2 (make #2):", sig2);
  console.log("Escrow PDA 2:", escrowPda2.toString());
  console.log("Vault A 2:", vaultA2.toString());

  // TX3: cancel escrow #2 (should fail - time lock)
  const tx3 = new Transaction().add(new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: payer.publicKey, isSigner: true, isWritable: true },
      { pubkey: escrowPda2, isSigner: false, isWritable: true },
      { pubkey: mintA, isSigner: false, isWritable: false },
      { pubkey: makerAtaA.address, isSigner: false, isWritable: true },
      { pubkey: vaultA2, isSigner: false, isWritable: true },
      { pubkey: TOKEN_PROGRAM, isSigner: false, isWritable: false },
    ],
    data: cancelData(),
  }));

  try {
    const sig3 = await connection.sendTransaction(tx3, [payer]);
    await connection.confirmTransaction(sig3);
    console.log("\nTX3 (cancel):", sig3, "(succeeded)");
  } catch (err) {
    console.log("\nTX3 (cancel): REJECTED (expected - TimeLockActive)");
    console.log("Error:", (err.message || String(err)).substring(0, 200));
  }

  console.log("\n--- Explorer Links ---");
  console.log("TX1:", `https://explorer.solana.com/tx/${sig1}?cluster=devnet`);
  console.log("TX2:", `https://explorer.solana.com/tx/${sig2}?cluster=devnet`);
}

main().catch((err) => { console.error(err); process.exit(1); });
