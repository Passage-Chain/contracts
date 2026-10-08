# Accidental NFT mint prevention — review candidate

These two isolated historical workspaces prevent every supported NFT creation path for the 17 reviewed existing accidental contracts. They preserve current storage, metadata, ownership, transfers, approvals, authorized burns and unfinished-import gates. They do not delete tokens or change shared minters. The nonexistent eighteenth address and optional dangling-minter guard are excluded.

The current root workspace and its production contracts are unchanged. Source is integrated on current `main` base `3f63cac59e615db1d39bec09974b2b2a6ee231e4`, rather than replacing current contracts with an old branch. Historical ABI/storage sources are isolated here: v020 derives from `e114ed88c03908d5c251aa2763ca3c8a627a9d66`; v030 derives from `52049f58df98abed7bcac53a3820b0e862c9ce20`. Adjacent historical-review patches show the focused semantic differences. Historical minter and whitelist copies are test dependencies, not rollout artifacts. Minter execution/storage logic is unchanged; its historical unconditional migrate entry-point annotation has a library-feature guard needed for fixture linkage. URL dependency pins and locked release profiles preserve the validated compiler inputs.

## Invariant and compatibility

`ExecuteMsg::Mint` and batch/import `ExecuteMsg::Migrate` reject before authorization, iteration or writes, including empty imports and calls from privileged minters. No alternate mint execution variant exists in these deployed families. NFT contract migration requires a reviewed target address, original CW2 contract/version, expected current minter, count and gate. It writes no storage. Wasmd independently authorizes the actual migration sender. The migration message adds required compare-and-swap fields; existing execute/query wire schemas and storage layout remain unchanged. CW2 stays at its historical version to permit storage-preserving rollback. Resolve implementation identity through verified code checksum, not CW2 version alone.

`MigrationDone` retains its historical minter authorization. Migration does not invoke it or unlock gates. Six existing unfinished imports affect **7,406 tokens**: MetaHuahua contracts beginning `pasg14hj2…` (600), `pasg1suhg…` (650), `pasg1yyca…` (700), `pasg1yw4x…` (50), `pasg1stwd…` (2,401), and accidental Kira `pasg14rse…` (3,005). Their exact addresses and recorded gates are in `per-contract-plan.json`. Changes to these restrictions are a separate proposal. Legitimate Kira `pasg1kwdranvwf6vl2grh99layugwdnph6um2x0c0l8qyvrcgsjcykuhs6vaz9p` is outside the target list and rejects retirement migration.

## Reproducible Linux build and focused tests

Use **Rust/Cargo 1.69.0**, target `wasm32-unknown-unknown`, and each committed Cargo.lock (`cosmwasm-std 1.1.9`, `cw721-base 0.13.2`, `cw-storage-plus 0.13.4`). Each sub-workspace carries `rust-toolchain.toml`. No floating optimizer image or toolchain is involved.

To reproduce the exact hashes, provision Rust in a dedicated Linux task root under `rust/cargo` and `rust/rustup`, including the wasm target, then run:

```sh
python3 releases/mint-prevention/build-release.py --task-root /absolute/dedicated-build-root
```

The script copies each source to `ROOT/contracts/v020` or `ROOT/contracts/v030`, sets explicit Cargo/Rustup homes under ROOT, compiles independently with `-C target-cpu=mvp --remap-path-prefix=ROOT=/build`, checks hashes, copies each result immediately and runs four synthetic tests per family. Use `--offline` after dependencies are cached. Native tests deliberately omit the Wasm-only `target-cpu=mvp` flag. Repeat with a fresh target directory for independent reproduction. Do not combine the two package versions or use a minter/library-feature build as an NFT artifact.

| Artifact | Bytes | SHA256 |
| --- | ---: | --- |
| retired-v020.wasm (8 contracts) | 694,243 | `da009a69d2186b20f2274224c0e4e0a48e9e78da3c27cae9fa1da20f808bbbb3` |
| retired-v030.wasm (9 contracts) | 694,243 | `ba7e79695cd848c14e3a83891725aeefe1872f90c5bd6d99d69697979f928f28` |

Initial host builds differed because diagnostic strings contained host paths; final canonicalized Linux builds reproduced byte-for-byte. Integrated source reproduces these same final hashes, so runtime evidence applies to the actual candidates. If bytes change, stop and rerun affected runtime checks. Older Windows hashes are not release candidates. Source-only review avoids publishing binary uploads or private fixtures.

## Validation and limits

`integration-validation.json` records the integrated build results; `validation-summary.json` contains sanitized aggregate results. Prior isolated runtime tests used the actual running application commit `fb3005f8c9bdd149aeac1ad3f0900a4f136bb6e5`, wasmd 0.50.0, wasmvm 1.5.0, Go 1.22.11 and all 17 captured NFT snapshots. All mint/import rejections, state neutrality, holder behavior and rollback passed. Full signed CheckTx → FinalizeBlock → Commit covered 109 committed transactions and 20 CheckTx rejections using disposable in-memory keys and no public peers. Accepting/rejecting receiver callbacks, ownership rollback and receiver-authorized return passed. Executed failed purchases retained only transaction fees; purchase payments, templates, counters and NFT/minter state rolled back. CheckTx rejections deducted no fee.

Observed limits: 100 million block gas; 22,020,096 block bytes; 1 MiB maximum transaction; minimum gas price 12.5 upasg/gas. Source Wasm limit is 819,200 bytes. Signed uploads were at most 694,513 bytes and 9,090,282 total gas. The original 2-million-gas keeper fixture was not silently raised: a separate signed fixture explicitly copied observed production limits. Refresh all limits before execution.

Persistent indexer testing retained 109 raw transaction results and 9,759 NFT fixtures; 102 transactions passed normal engine projection, all 17 histories retained original→retired→original intervals and unchanged names/symbols, and two later transfers persisted. Two accepting SendNft events additionally passed interpreter and persistent NFT projector checks with historical-owner fixtures. Seven synthetic receiver helper transactions were explicitly excluded from normal engine projection; receiver-return helper indexing is not claimed. Current indexer source testing is not proof of its deployed revision. Twenty-one identity/migration tests passed. Known upstream token0001 fixture failures are baseline failures; focused synthetic tests are self-contained and do not require private snapshots.

## Rollout, rollback and review boundary

See [ROLLOUT.md](ROLLOUT.md) and `per-contract-plan.json`. Registry registration is a separate indexer review/change, before any NFT migration; public code IDs are unassigned. PR #26, PR #320 and legitimate metadata repair remain independent. No schema/data repair, endpoint change or deployment is included. Human review and separately scoped production approval are required. Rollback retains intervening holder changes but restores original mint capability.

Budget at the observed fee policy: **428.125 PASG — four hundred twenty-eight and one-eighth PASG — for two uploads and 17 migrations**, excluding retries. Optional rollback of all 17 adds **53.125 PASG**. Refresh before requesting execution approval.

Private ownership snapshots, genesis exports, signing material, credentials and diagnostic logs are excluded. Test-only source lives in release fixtures; no production signing or broadcasting is performed by these build/test scripts.
