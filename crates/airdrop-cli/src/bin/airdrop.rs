//! Host tooling for LP-0003.
//!
//! A distributor builds an eligibility tree and publishes its root on chain; a
//! recipient turns their private entry into the arguments the `claim` instruction
//! takes. This binary does both halves so the end-to-end flow is scriptable.
//!
//!   airdrop demo-distribution --count 12 --id <hex32> --out dist/
//!   airdrop claim-args --dir dist/ --index 3 --out claim3.args
//!
//! The witness is encoded with risc0's serde, byte-for-byte what the guest reads
//! via `read_lee_inputs`, so the arguments this emits compose correctly on the
//! privacy path.

use airdrop_core::{
    build_eligibility_tree, compute_claim_marker, compute_claim_nullifier,
    compute_eligibility_leaf, derive_account_id, derive_npk, ClaimStatement, ClaimWitness,
};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(name = "airdrop", about = "LP-0003 distributor and claim tooling")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Build a self-contained demo distribution: generate `count` recipients,
    /// assign allocations, build the tree, and write the distribution root plus
    /// each recipient's private claim package.
    DemoDistribution {
        #[arg(long)]
        count: usize,
        /// 32-byte distribution id as hex. Also the on-chain distribution PDA seed.
        #[arg(long)]
        id: String,
        /// Base allocation; recipient i receives base + i*step.
        #[arg(long, default_value_t = 100)]
        base: u128,
        #[arg(long, default_value_t = 10)]
        step: u128,
        /// Pad the published bundle up to a multiple of this many rows with dummy
        /// rows, so the row count does not reveal the true number of recipients.
        #[arg(long, default_value_t = 1)]
        pad: usize,
        /// Output directory for distribution.json and recipients.json.
        #[arg(long)]
        out: String,
    },
    /// Emit the SPEL `claim` arguments for one recipient of a demo distribution.
    ClaimArgs {
        /// Directory produced by demo-distribution.
        #[arg(long)]
        dir: String,
        /// Recipient index.
        #[arg(long)]
        index: usize,
        #[arg(long)]
        out: String,
    },
    /// Claim from the published encrypted bundle: given only your secret and the
    /// bundle, find and open your row, verify it reconstructs the committed leaf,
    /// and emit the SPEL `claim` arguments. This is the recipient-side flow that
    /// needs no per-recipient private channel.
    ClaimFromBundle {
        /// Directory produced by demo-distribution (holds distribution.json and bundle.json).
        #[arg(long)]
        dir: String,
        /// Your 32-byte secret (nsk) as hex.
        #[arg(long)]
        nsk: String,
        #[arg(long)]
        out: String,
    },
}

#[derive(Serialize, Deserialize)]
struct DistributionFile {
    id_hex: String,
    root_hex: String,
    count: usize,
}

#[derive(Serialize, Deserialize)]
struct Recipient {
    nsk_hex: String,
    identifier: u128,
    allocation: u128,
    salt_hex: String,
    leaf_index: u64,
    merkle_path_hex: Vec<String>,
}

fn hex32(s: &str) -> Result<[u8; 32]> {
    let v = hex::decode(s).context("invalid hex")?;
    anyhow::ensure!(v.len() == 32, "expected 32 bytes, got {}", v.len());
    let mut a = [0u8; 32];
    a.copy_from_slice(&v);
    Ok(a)
}

fn rand32(tag: &[u8]) -> [u8; 32] {
    // Domain-separated OS randomness, so demo recipients get distinct secrets.
    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed).expect("OS randomness");
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(tag);
    h.update(seed);
    h.finalize().into()
}

fn demo_distribution(
    count: usize,
    id: [u8; 32],
    base: u128,
    step: u128,
    pad: usize,
    out: &str,
) -> Result<()> {
    let mut recipients = Vec::new();
    let mut leaves = Vec::new();
    for i in 0..count {
        let nsk = rand32(format!("lp-0003-demo-nsk-{i}").as_bytes());
        let identifier = 0u128;
        let allocation = base + step * i as u128;
        let salt = rand32(format!("lp-0003-demo-salt-{i}").as_bytes());
        let account_id = derive_account_id(&derive_npk(&nsk), identifier);
        let leaf = compute_eligibility_leaf(&account_id, allocation, &salt);
        leaves.push(leaf);
        recipients.push((nsk, identifier, allocation, salt));
    }

    let (root, paths) = build_eligibility_tree(&leaves);

    std::fs::create_dir_all(out)?;
    let dist = DistributionFile {
        id_hex: hex::encode(id),
        root_hex: hex::encode(root),
        count,
    };
    std::fs::write(
        format!("{out}/distribution.json"),
        serde_json::to_string_pretty(&dist)?,
    )?;

    let recs: Vec<Recipient> = recipients
        .into_iter()
        .enumerate()
        .map(|(i, (nsk, identifier, allocation, salt))| Recipient {
            nsk_hex: hex::encode(nsk),
            identifier,
            allocation,
            salt_hex: hex::encode(salt),
            leaf_index: paths[i].0,
            merkle_path_hex: paths[i].1.iter().map(hex::encode).collect(),
        })
        .collect();
    std::fs::write(
        format!("{out}/recipients.json"),
        serde_json::to_string_pretty(&recs)?,
    )?;

    // The publishable encrypted bundle: each recipient's claim data sealed to a
    // key derived from their secret, in a shuffled order so position leaks
    // nothing. A recipient needs only this file and their nsk to claim.
    let mut bundle: Vec<airdrop_crypto::EncryptedRow> = recs
        .iter()
        .map(|r| {
            let nsk = hex32(&r.nsk_hex)?;
            let payload = airdrop_crypto::RowPayload {
                allocation: r.allocation,
                salt: hex32(&r.salt_hex)?,
                leaf_index: r.leaf_index,
                merkle_path: r
                    .merkle_path_hex
                    .iter()
                    .map(|s| hex32(s))
                    .collect::<Result<_>>()?,
            };
            Ok(airdrop_crypto::encrypt_row(
                &airdrop_crypto::enc_public_key(&nsk),
                &serde_json::to_vec(&payload)?,
                None,
            ))
        })
        .collect::<Result<_>>()?;
    // Pad with indistinguishable dummy rows so the published count does not reveal
    // the real number of recipients, then shuffle so order carries nothing.
    if pad > 1 {
        let target = bundle.len().div_ceil(pad) * pad;
        while bundle.len() < target {
            bundle.push(airdrop_crypto::dummy_row());
        }
    }
    bundle.sort_by_key(|row| row.ephemeral_public);
    std::fs::write(
        format!("{out}/bundle.json"),
        serde_json::to_string_pretty(&bundle)?,
    )?;

    println!("distribution id   {}", dist.id_hex);
    println!("eligibility root  {}", dist.root_hex);
    println!("recipients        {count}");
    println!(
        "wrote             {out}/distribution.json, {out}/recipients.json, {out}/bundle.json"
    );
    Ok(())
}

fn claim_from_bundle(dir: &str, nsk_hex: &str, out: &str) -> Result<()> {
    let dist: DistributionFile =
        serde_json::from_slice(&std::fs::read(format!("{dir}/distribution.json"))?)?;
    let bundle: Vec<airdrop_crypto::EncryptedRow> =
        serde_json::from_slice(&std::fs::read(format!("{dir}/bundle.json"))?)?;
    let nsk = hex32(nsk_hex)?;
    let keys = airdrop_crypto::derive_enc_keypair(&nsk);

    let distribution_id = hex32(&dist.id_hex)?;
    let distribution_root = hex32(&dist.root_hex)?;
    let identifier = 0u128;

    let nullifier = compute_claim_nullifier(&distribution_id, &nsk);
    let marker_seed = compute_claim_marker(&distribution_id, &nullifier);
    let destination = derive_account_id(&derive_npk(&nsk), identifier);

    // Trial-open each row and keep the one whose payload actually reconstructs a
    // leaf that anchors to the committed root. Scanning for a *valid* row, not
    // merely the first that decrypts, makes the recipient robust to junk or
    // maliciously crafted rows placed in the open bundle: a row that opens but
    // does not anchor (or a low-order header meant to open for everyone) is
    // skipped rather than aborting the claim. It also enforces the trust
    // boundary, since a malicious distributor cannot make us prove a leaf that is
    // not in the set: such a row fails `claim` and is discarded here.
    let (payload, witness, statement) = bundle
        .iter()
        .filter_map(|row| {
            let pt = airdrop_crypto::decrypt_row(&keys, row)?;
            let payload: airdrop_crypto::RowPayload = serde_json::from_slice(&pt).ok()?;
            let witness = ClaimWitness {
                nsk,
                identifier,
                allocation: payload.allocation,
                salt: payload.salt,
                merkle_path: payload.merkle_path.clone(),
                leaf_index: payload.leaf_index,
                destination,
            };
            let statement = ClaimStatement {
                distribution_root,
                distribution_id,
                allocation: payload.allocation,
                nullifier,
                destination,
            };
            airdrop_core::claim(&witness, &statement).ok()?;
            Some((payload, witness, statement))
        })
        .next()
        .context("no row in the bundle opens for this secret and anchors to the committed root")?;

    let instruction = airdrop_core::ClaimInstruction {
        witness,
        statement,
    };
    let words: Vec<u32> = risc0_zkvm::serde::to_vec(&instruction)?;
    let hexq = |b: &[u8; 32]| format!("'{}'", hex::encode(b));
    let lines = [
        format!(
            "--witness-words '{}'",
            words.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
        ),
        format!("--distribution-root {}", hexq(&distribution_root)),
        format!("--distribution-id {}", hexq(&distribution_id)),
        format!("--allocation {}", payload.allocation),
        format!("--nullifier {}", hexq(&nullifier)),
        format!("--claim-marker-seed {}", hexq(&marker_seed)),
        format!("--destination {}", hexq(&destination)),
    ];
    std::fs::write(out, lines.join("\n") + "\n")?;
    println!("opened your row from the bundle");
    println!("allocation       {}", payload.allocation);
    println!("nullifier        {}", hex::encode(nullifier));
    println!("marker seed      {}", hex::encode(marker_seed));
    println!("wrote            {out}");
    Ok(())
}

fn claim_args(dir: &str, index: usize, out: &str) -> Result<()> {
    let dist: DistributionFile =
        serde_json::from_slice(&std::fs::read(format!("{dir}/distribution.json"))?)?;
    let recs: Vec<Recipient> =
        serde_json::from_slice(&std::fs::read(format!("{dir}/recipients.json"))?)?;
    let r = recs.get(index).context("recipient index out of range")?;

    let distribution_id = hex32(&dist.id_hex)?;
    let distribution_root = hex32(&dist.root_hex)?;
    let nsk = hex32(&r.nsk_hex)?;
    let salt = hex32(&r.salt_hex)?;
    let merkle_path: Vec<[u8; 32]> = r
        .merkle_path_hex
        .iter()
        .map(|s| hex32(s))
        .collect::<Result<_>>()?;

    let nullifier = compute_claim_nullifier(&distribution_id, &nsk);
    let marker_seed = compute_claim_marker(&distribution_id, &nullifier);
    // Default destination: the recipient's own account. Bound into the proof, so
    // the submission cannot redirect the allocation elsewhere.
    let destination = derive_account_id(&derive_npk(&nsk), r.identifier);

    let witness = ClaimWitness {
        nsk,
        identifier: r.identifier,
        allocation: r.allocation,
        salt,
        merkle_path,
        leaf_index: r.leaf_index,
        destination,
    };
    let statement = ClaimStatement {
        distribution_root,
        distribution_id,
        allocation: r.allocation,
        nullifier,
        destination,
    };

    // Sanity: prove locally that the witness satisfies the statement before
    // asking the chain to. Fails loudly here rather than after minutes of proving.
    let leaf = airdrop_core::claim(&witness, &statement)
        .map_err(|e| anyhow::anyhow!("witness does not satisfy the statement: {e:?}"))?;

    // Encode the witness exactly as the guest reads it.
    let instruction = airdrop_core::ClaimInstruction {
        witness: witness.clone(),
        statement: statement.clone(),
    };
    let words: Vec<u32> = risc0_zkvm::serde::to_vec(&instruction)?;

    let hexq = |b: &[u8; 32]| format!("'{}'", hex::encode(b));
    let lines = [
        format!(
            "--witness-words '{}'",
            words.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
        ),
        format!("--distribution-root {}", hexq(&distribution_root)),
        format!("--distribution-id {}", hexq(&distribution_id)),
        format!("--allocation {}", r.allocation),
        format!("--nullifier {}", hexq(&nullifier)),
        format!("--claim-marker-seed {}", hexq(&marker_seed)),
        format!("--destination {}", hexq(&destination)),
    ];
    std::fs::write(out, lines.join("\n") + "\n")?;

    println!("recipient        {index}");
    println!("account leaf     {}", hex::encode(leaf));
    println!("nullifier        {}", hex::encode(nullifier));
    println!("marker seed      {}   (the claim marker PDA seed)", hex::encode(marker_seed));
    println!("allocation       {}", r.allocation);
    println!("witness          {} u32 words", words.len());
    println!("wrote            {out}");
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::DemoDistribution { count, id, base, step, pad, out } => {
            demo_distribution(count, hex32(&id)?, base, step, pad, &out)
        }
        Cmd::ClaimArgs { dir, index, out } => claim_args(&dir, index, &out),
        Cmd::ClaimFromBundle { dir, nsk, out } => claim_from_bundle(&dir, &nsk, &out),
    }
}
