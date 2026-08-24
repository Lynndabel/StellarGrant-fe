# Fix four bugs in stellar-grants: syndicate payout auth, dispute escrow deadlock, reviewer whitelist bypass, grant TTL

This PR bundles four independent fixes to the `stellar-grants` Soroban contract: two security/authorization gaps and two data-integrity bugs, each found in a different module.

---

## #869 — Require authorization on `record_payout_allocation`

**Files:** `contracts/contracts/stellar-grants/src/lib.rs`, `contracts/contracts/stellar-grants/src/syndication.rs`

Unlike its sibling syndicate entry points (`form_syndicate`, `join_syndicate`, `close_syndicate`, `withdraw_syndicate`), `record_payout_allocation` took no `Address`/caller parameter at all, so authorization was structurally impossible to enforce. Any external caller could invoke it for any grant/milestone — including ones they had no relationship to — with an arbitrary `payout` value, overwriting the stored `SyndicatePayouts` allocation record that syndicate payout accounting/audit paths rely on.

- Added a `caller: Address` parameter to the `record_payout_allocation` entry point and the `syndication::record_payout_allocation` function.
- `caller.require_auth()` at the entry point, and `syndication::record_payout_allocation` now rejects with `Unauthorized` unless `caller == syndicate.lead`.
- New test: a stranger address cannot set a payout allocation for a syndicate they're not part of.

## #872 — Unlock escrow before release in dispute resolution

**File:** `contracts/contracts/stellar-grants/src/dispute.rs`

`raise_dispute` locks a grant's escrow account. `resolve_dispute` released funds to the winning party and only called `escrow::unlock` *after* that release — but `escrow::release`/`escrow::release_to_funders` both refuse to run while `locked` is `true`. Since the release call used `?`, `resolve_dispute` returned early on `EscrowLocked` for exactly the outcomes that matter (`ResolvedForContributor`, and `ResolvedForFunder` with non-empty funders), and `unlock` was never reached. The escrow's `locked` flag stayed `true` permanently — freezing not just the disputed milestone, but every future release/refund for that grant.

- Moved `escrow::unlock` to run *before* the release calls, since release should be permitted once the dispute has reached a final outcome.
- New tests: set up a grant with a real `EscrowAccount` funded via the normal `grant_fund` deposit flow, raise a dispute (locking escrow), resolve it for the contributor and separately for the funder (non-empty funders), and assert the release succeeds, the milestone amount is paid out, and the escrow is unlocked and usable for the next milestone's payout.

## #873 — Enforce whitelist, dedup, and max-reviewers cap in `accept_request`

**File:** `contracts/contracts/stellar-grants/src/reviewer_pool.rs`

`accept_request` pushed `reviewer` onto `grant.reviewers` with no whitelist check, no dedup check, and no cap check — unlike `internal_grant_create` and `multi_grant::add_reviewer_to_grant`, which both enforce the `GlobalReviewer` whitelist and `protocol_cfg.max_reviewers`. Since `request_reviewer` unconditionally resets a request back to `Pending`, a non-whitelisted reviewer could be requested and accepted repeatedly, bypassing the whitelist entirely via this code path and growing `grant.reviewers` past `max_reviewers`.

- `accept_request` now checks `whitelist::is_allowed(.., WhitelistScope::GlobalReviewer)`, rejects a reviewer already present in `grant.reviewers`, and rejects once the cap (`protocol_cfg.max_reviewers`) is reached — matching the errors (`AddressNotWhitelisted`, `AlreadyRegistered`, `ReviewerLimitExceeded`) used by the other two call sites.
- New test covering all three invariants through `accept_request` specifically: a non-whitelisted reviewer is rejected, an already-accepted reviewer can't be re-added, and the cap is enforced.

## #870 — Extend TTL on grant storage access to prevent archival

**File:** `contracts/contracts/stellar-grants/src/storage/helpers.rs`

`get_grant`/`set_grant` were the only frequently-accessed entities in this file that never called the file's `Self::bump()` TTL-extension helper — every comparable accessor (`get_escrow_account`, `get_syndicate_grant`, `get_multisig_proposal`, `get_dispute`, etc.) does. A grant dormant longer than the network's minimum persistent-entry TTL risked being archived, after which every dependent operation (milestone submission, escrow release, dispute filing) would start failing with `GrantNotFound` even though the grant — and any escrowed funds — logically still existed, with no entrypoint to restore it.

- `get_grant`/`set_grant` now call `Self::bump()`, consistent with the rest of the file.
- New test confirms the grant entry's persistent TTL is extended after both a read and a write.

---

## Verification

```
cargo fmt --check -p stellar-grants
cargo clippy -p stellar-grants --lib -- -D warnings
cargo test -p stellar-grants --lib -- dispute::tests reviewer_pool::tests syndication::tests storage::helpers::tests
```

All pass clean, including every new test added above.

**Note on the wider test suite:** the repo's committed `Cargo.lock` pins `soroban-sdk 25.3.0`, but a large number of pre-existing tests elsewhere in the crate (unrelated to this PR — e.g. `access_control.rs`, and the original tests already in `storage/helpers.rs`) call storage functions without wrapping them in `env.as_contract()`, which this SDK version rejects at runtime. Running the full `cargo test -p stellar-grants --lib` currently fails ~337 pre-existing tests for this reason, independent of anything in this PR. Also included here: a one-line fix to `grant_index.rs`'s test module, which was missing a `format!` import and blocked the entire test binary from compiling at all — needed just to be able to run and verify the tests above.

---

Closes #869
Closes #872
Closes #873
Closes #870
