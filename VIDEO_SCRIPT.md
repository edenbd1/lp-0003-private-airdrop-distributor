# 🎬 LP-0003 — script vidéo (~7 min)

> **⚠️ Tap les liens directement sur ton phone — ne pas copy-paste (les URLs sont longues et peuvent être tronquées au copy).**

- Durée totale visée : **~7 minutes**
- Langue : English
- `🎬 ACTION` = ce que tu fais à l'écran
- `💬 SAY` = ce que tu lis à voix haute

> **Toutes les commandes de ce script ont été exécutées et vérifiées.** Les sorties
> annoncées sont les vraies. Si quelque chose ne sort pas comme écrit, arrête et
> dis-le-moi plutôt que d'improviser.

> **Le point délicat : l'explorer est en retard sur le séquenceur.** Mesuré :
> ~1 h 45 entre la confirmation en RPC et l'apparition sur l'explorer. Une
> transaction fraîche y affiche donc "not found" alors qu'elle est bien sur la
> chaîne — inutilisable en direct devant la caméra. Séparément, une transaction
> privacy ne publie ni `program_id` ni `instruction_data`, donc elle reste
> non-attribuable même une fois indexée, et ça c'est le sujet de la soumission. On
> ne mélange pas les deux : on lit la chaîne en RPC, qui répond immédiatement.

---

# ⚙️ Pré-vol (~3 min de prep)

## Terminal

**🎬 ACTION** :

```bash
cd $REPO
clear
```

Agrandis la police (⌘+ plusieurs fois — le texte doit être lisible en 1080p),
ferme Slack/Discord/notifications, fenêtre terminal en plein écran.

## Un seul onglet browser

**🎬 ACTION : ONGLET A (le repo)** — tap ce lien :

→ **[Repo : edenbd1/lp-0003-private-airdrop-distributor](https://github.com/edenbd1/lp-0003-private-airdrop-distributor)**

> **Pas d'onglet block explorer.** Deux raisons distinctes. Un, l'explorer accuse
> ~1 h 45 de retard sur le séquenceur, donc la preuve générée en direct à la
> scène 3 n'y sera pas encore : une page "not found" en caméra ne prouverait rien.
> Deux, une transaction privacy est non-attribuable par construction (ni
> `program_id` ni `instruction_data`). On lit la chaîne en RPC à la place, aux
> scènes 2 et 3, qui répond immédiatement et fait foi.

## Basecamp (pour la scène 4)

**🎬 ACTION** : Basecamp **déjà lancé et à jour**, avec le `.lgx` committé installé
(la tuile `lp-0003-airdrop` visible dans la sidebar), et un dossier de distribution
démo prêt à pointer. Réinstalle depuis le package committé avant d'enregistrer :

```bash
LGX=$LOGOS_PACKAGE/build/lgx   # pas sur le PATH, chemin complet
DST=~/Library/Application\ Support/Logos/LogosBasecamp/plugins/lp-0003-airdrop
rm -rf "$DST" && "$LGX" extract app/lp-0003-airdrop.lgx --variant darwin-arm64 --output /tmp/x
mkdir -p "$DST" && cp -R /tmp/x/darwin-arm64/. "$DST/"
printf darwin-arm64 > "$DST/variant"
tar xzOf app/lp-0003-airdrop.lgx manifest.json > "$DST/manifest.json"
# puis relance Basecamp et vérifie que la tuile apparaît et que le claim marche
```

## Vérif de dernière seconde (30 s, avant d'enregistrer)

**🎬 ACTION** :

```bash
CLAIM_TX=441ccd15e7b5eac388a0849481e95db409f1b6f23a202b6ee1a3ce37ae112c86 \
NULLIFIER=3db769e851c291d82cb79d717f1256710bb67b06a50cc52bea3f4ae1fea32b99 \
DISTRIBUTION_ID=b100000000000000000000000000000000000000000000000000000000000001 \
./scripts/verify-onchain-claim.sh
```

Tu dois voir **cinq blocs OK** et la ligne finale `VERIFIED`. Si oui → QuickTime →
Screen Recording → Démarre. Sinon → stop, préviens-moi.

---

# SCÈNE 1 — Intro (0:00 – 0:45)

**🎬 ACTION** : Terminal vide

**💬 SAY** :

> "Hi, I'm Eden. This is my submission for Logos Lambda Prize L-P zero-zero-zero-three — a Private Allowlist and Airdrop Distributor."

**💬 SAY** :

> "A distributor commits an eligibility set on chain. An eligible recipient claims their allocation — and the chain records only that a claim happened, never which address made it. An observer who knows every candidate address still cannot tell who claimed."

**💬 SAY** :

> "The membership proof is genuinely verified on chain, and everything is live on the public L-E-Z testnet. Let me show you, starting from a clean clone."

---

# SCÈNE 2 — Le demo (0:45 – 3:15)

**🎬 ACTION** : Tape lentement :

```bash
./scripts/demo.sh
```

**🎬 ACTION** : Entrée. Laisse défiler, **puis scrolle doucement vers le haut**
pendant que tu parles.

**💬 SAY** (pendant que ça tourne) :

> "This runs from a clean clone. No funded account, no local sequencer for the local steps."

**🎬 ACTION** : Scrolle jusqu'en haut, sur `== 0. environment`

**💬 SAY** :

> "First line — R-I-S-C zero dev mode equals zero. Real proofs, no mock receipts. That's required by the brief, and it's the first thing I show."

**🎬 ACTION** : Scrolle sur `== 1.`, `== 2.`, `== 3.`

**💬 SAY** :

> "Ten adversarial tests on the claim logic — non-members, borrowed Merkle paths, invented roots, forged nullifiers, inflated allocations, redirected destinations — plus two that round-trip the tree builder at every set size, so twelve here. Ten more on the encrypted bundle. Then eight against the built verifier binary, run through the sequencer's own executor — same executor, same input order, same thirty-two megabyte session limit the chain applies. A rejection you see there is the rejection the chain performs."

**🎬 ACTION** : Scrolle sur `== 4.` et `== 5.`

**💬 SAY** :

> "Now the recipient flow. The distributor publishes one encrypted bundle, padded here to eight rows so it hides how many recipients there really are. A recipient opens it with only their own secret — they scan every row, skip the padding, and keep the one row whose data reconstructs a leaf that anchors to the committed root."

**🎬 ACTION** : Scrolle sur `== 7. compute cost`

**💬 SAY** :

> "Measured compute cost: a claim is three hundred eighteen thousand user cycles, one and a half percent of the public budget."

**🎬 ACTION** : Scrolle sur `== 8. what an observer sees` — **ralentis ici**

**💬 SAY** (le cœur de la soumission — prends ton temps) :

> "This is the whole point. On chain, a claim is a single marker address. That address is a hash of a nullifier, and the nullifier is a hash of the recipient's secret. Someone who knows every eligible address cannot compute that nullifier, so they cannot map this marker back to any of them. That is the unlinkability property."

**🎬 ACTION** : Scrolle tout en bas, sur `== 9. the live deployment`

**💬 SAY** :

> "And the last step reaches the public testnet and verifies a claim that is actually deployed — five checks. The transaction is privacy-preserving, its receipt is a real Succinct STARK, not a dev-mode fake, and the marker is owned by the verifier program. That receipt could not exist unless a membership proof was verified on chain."

---

# SCÈNE 3 — Une preuve, générée en direct (3:15 – 5:15)

**🎬 ACTION** : Explique que le demo n'a pas *prouvé* — il a exécuté. Maintenant on
génère une vraie preuve.

**💬 SAY** :

> "The demo executed the logic; it did not prove. Proving is the expensive part, so let me generate a real one now, live, with dev mode still off."

**🎬 ACTION** : Lance la génération d'une vraie preuve. `spel` doit être sur le
PATH. `WALLET_BIN` + les home dirs sont nécessaires pour le `sync-private` avant le
claim (sinon le commitment est périmé et le claim est rejeté). Copier-coller, puis
remplace juste les deux ids par tes comptes financé/autorisé :

Le stack cible **LEZ v0.2.4**, donc on utilise le wallet v0.2.4 et le
spel **vendoré** (`vendor/spel`, porté à v0.2.4). Le claim signe avec un compte
privé jetable créé à la volée — pas de `CLAIMANT` à fournir. Prépare **avant de
filmer** (hors caméra) un compte public financé via le faucet :

```bash
export PATH="$REPO/vendor/spel/target/release:$HOME/.cargo/bin:$HOME/.risc0/bin:$PATH"
export WALLET_BIN=$LEZ_SRC/target/release/wallet   # a LEZ v0.2.4 checkout   # LEZ v0.2.4
export SPEL_BIN=$REPO/vendor/spel/target/release/spel
export DYLD_FALLBACK_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks
export LEE_WALLET_HOME_DIR=~/.lez-wallet NSSA_WALLET_HOME_DIR=~/.lez-wallet
mkdir -p ~/.lez-wallet
echo '{ "sequencers": [{ "sequencer_addr": "https://testnet.lez.logos.co" }], "seq_poll_timeout": "30s", "seq_tx_poll_max_blocks": 15, "seq_poll_max_retries": 10, "seq_block_poll_max_amount": 100, "calibration_limit": 100 }' > ~/.lez-wallet/wallet_config.json
"$WALLET_BIN" account new public                       # note l'account_id imprimé
export SIGNER=<l_account_id_imprimé>
"$WALLET_BIN" auth-transfer init --account-id "Public/$SIGNER"
"$WALLET_BIN" pinata claim --to "Public/$SIGNER"       # +150 LEZ ; répète 1-2x si besoin
```

Puis, caméra qui tourne, tu ne tapes que :

```bash
./scripts/prove-one-claim.sh
```

**💬 SAY** (pendant que ça démarre) :

> "This commits one fresh distribution, then proves and submits a single real claim against it. Watch the proving step. It takes minutes, not seconds, because it generates a real STARK, and the privacy circuit then recursively verifies the chained call inside it. Dev mode is off, so these are real proofs. The script times it and prints what it measured, so you get this machine's number rather than a claim."

> **🎬 NOTE POST-PROD** : l'attente de proving (plusieurs minutes, ligne
> `proving locally...`) doit être **accélérée en post** (×8 à ×16), MAIS le
> terminal reste visible et continu — on ne coupe pas, on accélère, pour qu'il
> soit clair que rien n'est truqué.
>
> Ne jamais annoncer une durée à la voix : elle dépend de la machine et de la
> charge, l'horloge à l'écran est la seule source. Une version précédente disait
> « two and a half minutes » pendant que le terminal en affichait neuf.

## Pendant que ça prouve — l'architecture

La preuve prend plusieurs minutes de temps machine. Le brief demande au
constructeur d'expliquer l'architecture et les décisions d'implémentation : c'est
ici que ce contenu va, sinon la vidéo a deux minutes et demie de silence.

**💬 SAY** :

> "While that proves, let me explain what it is actually doing, because the architecture is the submission. The first thing I checked on LEZ was whether a public transaction verifies a proof. It does not. The sequencer re-executes the program host-side, in a function whose own comment says execute the program, without proving. Anything built there would be a membership check wearing a zero-knowledge costume, and that is the ground earlier submissions in this programme were rejected on."

**💬 SAY** :

> "The path that works is the privacy-preserving transaction. The client proves locally, LEZ's privacy circuit composes each chained call with a real env verify over the callee's program output, and the sequencer then checks the resulting receipt against a circuit id pinned in the node. For that composition to happen, the thing being proved has to be a LEZ program itself, emitting a program output. That is why the claim circuit exists in the shape it does, rather than as a plain guest I verify myself, off chain, and ask you to trust."

**💬 SAY** :

> "The second decision is anchoring. A membership proof establishes membership against whatever root the statement names, which on its own is worthless: anyone can build a one-leaf tree holding themselves. So the root is anchored by address. Create distribution initialises an account whose address derives from the distribution id and the root, and claim requires that account to be owned by this verifier. An invented root resolves to an address nobody ever created, and the claim is rejected before the program body runs."

**💬 SAY** :

> "The third is the nullifier. Each claim occupies a marker account seeded by a hash of the distribution and a secret only the recipient holds. Because it commits to that secret, an observer who knows every eligible address still cannot compute it, so the marker cannot be mapped back to anyone. And because the account is created with init, a second claim by the same recipient targets an address that is already taken, and fails on chain. One recipient, one claim, and no identity revealed."

**🎬 ACTION** : Le script imprime `proved + submitted in NNNs`, attend
l'atterrissage, puis lance lui-même la vérification cinq-sur-cinq. Laisse-la
s'afficher jusqu'à `VERIFIED`.

**💬 SAY** (quand le `VERIFIED` apparaît) :

> "There it is. A real proof, dev mode off, submitted on the privacy path, and the script reads it straight back off the chain: a Succinct STARK the sequencer verified, and the marker owned by the verifier. One note on the explorer. This claim landed seconds ago, and the explorer runs about an hour and three quarters behind the sequencer — I measured it — so it will not have this hash yet. The R-P-C has it immediately, which is why verification reads the chain directly. And once the explorer does catch up, what it shows is the transaction type, the proof size, and the marker address. No program id, no instruction data, nothing that names a distribution or an address. The privacy property, rendered by a third party."

---

# SCÈNE 4 — Le paquet Basecamp, vérifié (~1 min)

**🎬 ACTION** : Tape :

```bash
python3 scripts/package-lgx.py --verify app/lp-0003-airdrop.lgx
```

**💬 SAY** :

> "The Basecamp deliverable is a package, so let me check it rather than assert it. This recomputes every hash in the manifest from the archive's own contents, for both variants — darwin arm64 and linux amd64 — because the reviewer runs Linux and a package carrying only one platform is one they cannot open."

**🎬 ACTION** : Puis, pour montrer ce que l'app exécute réellement :

```bash
tar xzf app/lp-0003-airdrop.lgx -C /tmp/pkg && /tmp/pkg/variants/darwin-arm64/airdrop --help
```

**💬 SAY** :

> "And this is the binary the package ships. The Basecamp module is a thin Qt surface that shells out to exactly this C-L-I — so what the app computes is what you are watching compute here. There is no second implementation to drift, and nothing the G-U-I can claim that this command cannot show."

---

# SCÈNE 6 — Closing (7:15 – 8:00)

**🎬 ACTION** : Passe sur l'ONGLET A (le repo)

**💬 SAY** :

> "To summarize. Two programs deployed on the public L-E-Z testnet, byte-identical to what's in the repository — you can check that from the deployment transaction hash. Two distributions committed, and twenty-three privacy-preserving claims landed and independently verifiable. Documented compute cost, a SPEL I-D-L, the Basecamp app you just saw loading and running, a demo script that runs from a clean clone, and green C-I with the adversarial tests running against the deployed binary on every push."

**💬 SAY** :

> "Repository at github dot com slash eden-b-d-one slash l-p dash zero zero zero three dash private dash airdrop dash distributor. The privacy model, with the residual leakage named rather than hidden, is in docs slash privacy dash model dot M-D. Thank you for reviewing."

**🎬 ACTION** : Attends 2 secondes en silence

**🎬 ACTION** : Stop l'enregistrement

---

# 📝 Post-recording

1. QuickTime → Export → 1080p mp4 → `lp-0003-submission.mp4`
2. Accélère la portion de proving de la scène 3 (×8 à ×16), terminal continu visible
3. YouTube Studio → tap **[studio.youtube.com](https://studio.youtube.com)** → Upload → **Unlisted**
4. **Title** : `LP-0003 Private Allowlist / Airdrop Distributor — Lambda Prize submission (edenbd1)`
5. **Description** :

   ```
   Submission demo for Logos Lambda Prize LP-0003 — Private Allowlist / Airdrop Distributor.

   A private airdrop on the Logos Execution Zone where a recipient claims an
   allocation without revealing which address claimed, and the membership proof
   is genuinely verified on chain via LEZ's privacy-preserving transaction path.

   Repo:
   https://github.com/edenbd1/lp-0003-private-airdrop-distributor

   In that repo:
     docs/DEPLOYMENT.md      — every transaction hash, and how to re-verify each
     docs/privacy-model.md   — the threat model, and what is deliberately not hidden
     artifacts/e2e/claims.tsv — the 23 live claims

   Public testnet:
   https://testnet.lez.logos.co
   ```

6. Reviens avec **l'URL YouTube + "OK submit la PR"** → je prépare la PR, tu la relis, puis je l'ouvre

---

# 🆘 Cheat sheet — prononciation

- **LEZ** = "L-E-Z" (épelle)
- **SPEL** = "spell"
- **PDA** = "P-D-A" (épelle)
- **RISC0** = "risk zero"
- **nsk** = "N-S-K" (épelle)
- **npk** = "N-P-K" (épelle)
- **env::verify** = "env verify"
- **STARK** = "stark"
- **SHA256** = "SHA two fifty-six"
- **Diffie-Hellman** = "DIFF-ee HELL-man"
- **nullifier** = "NULL-ifier"
- **IDL** = "I-D-L" (épelle)
- **dladdr** = "D-L-addr" (épelle D-L, puis "adder")
- **Basecamp** = "Basecamp"
- **.lgx** = "dot L-G-X" (épelle)
- **edenbd1** = "eden-B-D-one"

---

# ⏱️ Timing récap

| Scène | Durée |
|---|---|
| 1. Intro | 45s |
| 2. demo.sh | 2m30 |
| 3. Preuve en direct | variable — le proving gouverne |
| 4. Le paquet Basecamp | 1m00 |
| 6. Closing | 45s |
| **Total** | **~7 min hors proving** |

> **Les horodatages absolus ont été retirés.** La scène 3 dure ce que dure la
> preuve, et cela dépend de la machine : sur un projet voisin, la même opération est
> passée de 150 s à 437 s en changeant de version de LEZ, et à 935 s avec une
> autre preuve tournant en parallèle. Toutes les scènes suivantes glissent
> d'autant. Lance `pgrep -fl r0vm` avant de filmer — s'il sort quelque chose,
> attends.

Le brief demande une narration qui explique l'architecture et les décisions, pas
un screencast muet. La scène 3 montre une vraie génération de preuve avec
`RISC0_DEV_MODE=0` ; la scène 4 vérifie le paquet Basecamp au lieu de l'affirmer.

**Tu peux y aller. 🎬**
