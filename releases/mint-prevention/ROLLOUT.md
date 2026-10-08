# Staged execution proposal — no execution authorized

Adrian reviews and decides merge/deployment. This PR does not authorize code uploads, migration, governance, signing or broadcasting. Record receipts and UTC timestamps for every subsequently approved stage. Keep PR #26, PR #320 and legitimate metadata repair independent.

## 1. Authority and consistent preflight

- Verify the designated uploader's public address is permitted by current chain code-upload parameters. Confirm control privately through the team's approved wallet procedure, without exporting or revealing private keys. Creator/minter status is not migration authority.
- Verify each current contract admin independently against `per-contract-plan.json` and confirm the correct signing authority or explicit governance process. Never use an admin wallet for a different target by assumption.
- Refresh chain/application commit, wasmvm/wasmd versions, gas/byte limits, upload permissions, minimum gas price and account balances. The validated baseline is fb3005f8c9bdd149aeac1ad3f0900a4f136bb6e5 / wasmd 0.50.0 / wasmvm 1.5.0. An upgrade or different runtime requires compatibility review before execution.
- At one consistent recorded height, refresh each target's existence, original code/checksum, admin, CW2 version/family, minter, token count, unfinished-import flag, balances and discovered listings/escrow/offers/approvals/minter references. Stored height 21320616 is historical evidence, not a current authorization condition. Abort mismatches; do not edit expected fields merely to force migration.
- Preserve a private before-state capture for comparison, never publish ownership inventories. The two completed populated contracts may have legitimate holder activity; compare snapshots at the correct heights. Six unfinished-import gates/7,406 tokens must remain unchanged.

## 2. Separate approval: two uploads only

Build and verify `checksums.sha256`. Request approval specifying chain, uploader, exact artifacts/hashes, code-upload permissions and fee cap. Upload only after that approval. Record assigned public code IDs, tx hashes/heights, actual gas/fees and on-chain code checksums. Independently match checksums to both reviewed artifacts. Never reuse local fixture code IDs.

Tested upload limit: 15,000,000 gas each; observed maximum 9,090,282. At 12.5 upasg/gas, each fee is 187.5 PASG. Two uploads cost 375 PASG under that policy. No NFT migration is part of upload approval.

## 3. Separate indexer registration/readiness

Prepare an additive change in the owning indexer repository only after verified public IDs exist. Two identities use family `pg721-metadata-onchain`, adapter `pg721-metadata@1`, chain `passage-2` and the exact uppercase checksums here. Preserve all original identities and migration intervals. IDs remain unassigned at review time; no executable configuration with placeholder/guessed IDs is published.

Have Adrian review and approve its publication/deployment/import separately. Verify the production registry persistence and active cache resolve both code IDs/checksums, and establish the deployed indexer revision. Confirm migration event interpretation selects the intended adapter and checkpoint/failure state is healthy. Do not migrate any NFT until production indexing is demonstrably ready. This requires no metadata repair, raw API exclusion or database evidence deletion.

## 4. First migration stage: one empty target per family

These are proposals based on the bounded inventory; recheck emptiness and low dependencies before approval:

| Stage | Artifact | Empty candidate | Original code | Basis |
| --- | --- | --- | ---: | --- |
| A | v020 | `pasg1cnuw3f076wgdyahssdkd0g3nr96ckq8cwa2mh029fn5mgf2fmcmsvjjdxf` | 1 | Zero NFTs/balances; no discovered dependency |
| B | v030 | `pasg1ez4us9vdu7ly0sqle43c55qrp85vm76lz6n26yjsustl7fhz0t9s9ag6ea` | 25 | Zero NFTs/balances; no discovered dependency |

Request a separate migration approval naming these exact two contracts, verified destination IDs/checksums, refreshed compare-and-swap message fields, authorities, gas/fee caps, indexer readiness and rollback. Run sequentially. After A, verify before B; after B, pause for review before the other 15.

For each: compare complete storage immediately before/after migration; migration should change only runtime code identity, not contract storage. Verify code checksum/CW2/minter/count/gate and query behavior. Confirm migration transaction success and indexer raw result, canonical ContractMigrated operation, projection completion, code interval, unchanged collection metadata and absence of new projection errors. Use query/simulation or isolated reproduction to check rejected mint/import paths; public probe transactions require their own explicit authorization and incur fees. Do not change shared minters or invoke MigrationDone.

## 5. Remaining contracts, after first-stage verification and new approval

Approve the exact remaining list and refreshed per-contract preconditions. Execute sequentially or in explicitly approved small batches; verify each transaction and indexer completion before proceeding. No holder operations, burns, metadata repairs or restrictions changes are rollout steps. Investigate any mismatch rather than replaying or bypassing a failed precondition.

Tested migration limit: 250,000 gas; observed maximum 145,406. Fee at observed minimum: 3.125 PASG each; all 17 total 53.125 PASG. Combined two-upload/17-migration estimate: **428.125 PASG (four hundred twenty-eight and one-eighth PASG)**, excluding retries. Refresh all estimates before execution approval.

## Stop conditions and state-preserving rollback

Stop on changed runtime/limits, unexpected state/admin/dependencies, checksum mismatch, unauthorized upload/migration, failed simulation, excessive gas, non-neutral storage, altered holder gates, incorrect registry resolution, lagging/uncompleted projection, metadata changes or any holder regression. Do not blindly retry, perform replay/data repair or proceed to later contracts.

Rollback requires separate authorization for exact affected targets and fees unless expressly covered by the approved execution plan. Reconfirm current admin, code and minter; migrate back to that target's recorded original code ID/checksum using the original message `{"minter":"<verified-current-minter>"}`. Preserve the current NFT storage, approvals, owners and all intervening legitimate transfers/burns. Never restore the historical storage snapshot. Keep original registry identities and verify the new rollback interval, queries, gates, metadata and projector completion.

**Rollback restores original mint/import capability.** Record that risk and any need for a separately approved containment response. Tested rollback limit is 250,000 gas; maximum observed 137,530. Optional rollback of all 17 adds **53.125 PASG**, excluding retries.
