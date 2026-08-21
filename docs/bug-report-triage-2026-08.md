# Bug report triage — August 2026

A 27-finding report was filed against `c18124b`. Every code quote in it is accurate. Most of the
reachability and impact claims are not.

This records what we acted on and, more usefully, why we did not act on the rest. It exists so the
same findings do not get re-triaged from scratch next time a similar report arrives.

## Outcome

| Verdict | Count | Findings |
| --- | --- | --- |
| Fixed | 8 | 2, 5, 6, 7, 9, 22, 27, plus one the report missed |
| Filed, not patched | 2 | 9 (durability half), 13 |
| Not worth fixing | 17 | 1, 3, 4, 8, 10, 11, 12, 14, 15, 16, 17, 18, 19, 21, 23, 24, 25, 26 |
| False | 1 | 20 |

Issues #419–426, #435. PRs #427–434.

## Findings we are not fixing

### 1 — Network private key returns success when not persisted

Real. `load_private_key` warns and returns the unpersisted key, so the node gets a new PeerId on the
next restart.

Not worth fixing because the blast radius is small. Nothing economic depends on the libp2p identity;
mining rewards use the miner key. `trusted_peers` lists *other* nodes and defaults to empty. Peer
scores are in-memory and reset every restart anyway. The trigger needs the network dir to be
unwritable while the process otherwise runs.

The usual suggested fix also makes things worse: returning `Err` on an unreadable or invalid key file
turns "regenerate and carry on" into "refuses to boot until an operator deletes the file."

### 3 — Transient ENR read failure overwrites a newer persisted ENR

Real but narrow. `if let Ok(...) = File::open` collapses "absent" and "unreadable" into one branch.

Corrupt ENR *content* is already handled correctly — it reaches `Enr::from_str`, fails, and takes the
existing warn-and-regenerate path. `read_to_string` only fails on an I/O error or non-UTF-8 bytes, and
a torn write of base64 text is still valid UTF-8. If `File::open` fails on permissions, the subsequent
`File::create` fails too, so the file is not destroyed. The ENR also sits in the same directory as
`network/key`; a case where the key reads fine but the ENR does not is contrived.

We fixed the atomicity half in #425 instead. We deliberately did **not** make an unreadable `enr.dat`
a hard startup failure — an ENR is trivially regenerable from the key, so that trades a self-healing
annoyance for a guaranteed outage.

### 4 — Top-level `ZGS_NODE__...` environment overrides are discarded

Real symptom, wrong stated cause. Nothing strips the values at `mod.rs:142`. `ZGS_NODE__DB_DIR`
produces the key `db_dir`, `ZgsConfig` has no such field and no `deny_unknown_fields`, so serde drops
it before `raw_conf` is ever consulted.

Not worth fixing because env-var config was never a supported path. `run/zgs.sh` passes its
environment through as CLI flags, which works. Nothing documents the `ZGS_NODE__` prefix.

Acting on the report's implied fix would be actively dangerous: the TOML keys are flat top-level keys
that only `RawConfiguration::parse` reads, so removing that line resets every field to its default and
the node silently ignores the entire config file.

Related dead code, noted for whoever wires this up properly: `.list_separator(" ")` is inert, because
all list/bool/number parsing is gated behind `try_parsing`, which is never enabled.

### 8 — Append-Merkle leaves an open transaction after invalid subtree depth

The mechanism is real: `start_transaction` runs before the fallible `append_subtree_inner`, and
`start_transaction` panics if a transaction is already open.

The report's named trigger is dead. `depth = height.as_usize() + 1` with `height: U256` means depth 0
needs `height == u64::MAX` exactly, and `log_manager.rs:932` wraps before `append_subtree` is called.

There is a live adjacent trigger — a reorg after restart reverting a tx not in `delta_nodes_map` — but
it needs a genuine reorg, and the chain has BFT finality with log sync trailing by
`confirmation_block_count`.

Separately worth knowing: `height.as_usize()` at `log_entry_fetcher.rs:772` panics outright for
`height > usize::MAX`, killing the log-sync task. Not in the report.

### 10, 18 — Loops never terminate after their channel closes

Both are structurally dead code, not merely unreachable.

For 10, `chunk_pool::unbounded` is the only construction site; `run(mut self)` consumes the handler, so
the loop's own future owns an `Arc<MemoryChunkPool>` holding a live sender. `recv()` cannot return
`None` while the loop is alive.

For 18, the sync senders are held for the whole process lifetime by the router service, every jsonrpsee
handler, and the auto-sync batchers. The only path where they all drop is runtime teardown, by which
point `exit_future` has already cancelled the task and `shutdown_on_idle` bounds the rest.

### 11 — Chunk pool loses cached uploads when broadcast lags

Requires ~25000 transactions broadcast while a tokio task is starved of scheduling. That is total
runtime starvation, at which point the node has worse problems. The capacity is a hardcoded const with
no knob to shrink it.

### 12 — One unbounded task spawned per synced transaction

The spawn is real and deliberate, per the comment at the site. Tasks that do real work are limited to
roots present in `segment_cache`, capped by the cache budget. The recv loop consumes one event per
iteration and each no-op task completes in microseconds. `write_segment` also bails at `max_writings`
(default 16). The task explosion the report describes is not what the code produces.

### 14 — Recovery does not persist progress through blocks with no logs

Real. A restart during a recovery scan re-scans from the last block containing a Submit log.

Cost is `eth_getLogs` pages of 999 blocks at 50 ms each — roughly 50 s of RPC per million empty blocks.
On galileo and mainnet, submissions are frequent enough that the re-scan window is minutes of chain.
No wrong data results.

### 15 — Log-query page-size reduction underflows at `log_page_size == 1`

Requires ten consecutive "exceeds the max limit" responses for the same page. At page size 1 that means
a single block whose Submit logs exceed the provider's result cap, which block gas limits make
implausible for the flow contract. In that scenario log sync cannot progress regardless of the halving
bug. The panic variant additionally needs `log_sync_start_block_number = 0`.

### 16 — Finalized-block cleanup underflows at low height

Needs both a chain fewer than ~100 blocks old *and* a provider error. That means a fresh devnet or e2e
chain, never galileo or mainnet. The two clamps that follow also cap `safe_block_number`, so the
"almost all block hashes treated as finalized" consequence cannot happen.

### 17 — Auto-sync misses reorgs when its broadcast receiver lags

The swallow is real, but the code path is dead by default. `SerialBatcher::start` is spawned only when
`neighbors_only` is false; it defaults to true, and the only places it is set false are two Rust test
fixtures. A real `Reverted` event additionally needs a reorg deeper than `confirmation_block_count`,
which BFT finality makes essentially impossible.

### 19 — Mining and submitter loops busy-spin after broadcast closure

Only at teardown. The single sender is a field of `MineContextWatcher` moved into its task future, and
that task is an infinite `select!` with no `break` or `return`. It can only drop if the exit signal
cancels the task or it panics — at which point the node is shutting down anyway.

### 20 — `miner_cpu_percentage > 100` underflow

**False.** `100 - cpu_percent` is guarded by `else if cpu_percent < 100`. No path reaches it with
`cpu_percent > 100`. `cpu_percent` is an immutable local bound once, so it cannot change between the
guard and the use.

The report missed the footgun that *is* reachable in the same expression: `miner_cpu_percentage = 0`
silently disables mining via the `cpu_percent > 0` guard. Fixed in #426.

### 21 — Timed-out sync requests still execute later

Reachable, and not exotically — the sync service is a single task and several handlers block it on
direct RocksDB calls, so a 3 s deadline is reachable under a GetChunks backlog.

Not worth fixing because executing a stale request is not harmful. The work is idempotent and the
caller has already given up. There is a `common_channel_sync.timeout` counter for exactly this, so it
is observable if it becomes common.

### 23 — Blocking tasks outlive the exit signal

Trivially reachable — every storage read/write goes through `spawn_blocking` and shutdown during one is
normal. It produces no observable defect: `shutdown_timeout` deliberately waits 15 s for these threads,
and each closure is one atomic `LogStore` call.

### 24 — Task-executor metrics leak with no runtime handle

Every cited site reads as claimed. Requires `HandleProvider::Runtime`, which the node binary never
constructs — `environment.rs:102` always passes a cloned `Handle`, which never fails to upgrade.
Reachable only in network integration tests, and only for two of the four functions.

### 25 — Unix shutdown panics if all signal registrations fail

Needs SIGTERM, SIGINT and SIGHUP registration to all fail in one process. With `enable_all()` and
non-forbidden signal kinds, that means three consecutive failures inside `signal_hook_registry`. No
config the node accepts can cause it.

### 26 — `HashSetDelay::get` returns a stale deadline

The defect is real: `update_timeout` resets the expiration but never rewrites `MapEntry.value`.

No caller exists. The only consumer of the crate is `peer_manager`, which uses `insert`, `remove` and
`poll_next_unpin` only. The apparent second consumer in `version-meld/discv5` uses its own local copy.

## Client-side findings

The companion report's five `0g-storage-client` findings were all real and are already fixed: PRs #163
(CLIENT-1, CLIENT-5) and #164 (CLIENT-2, CLIENT-3) are merged, #166 (CLIENT-4) is open.

One unchecked close remains at `transfer/hot_downloader.go:140`. That is an *input* file. Close errors
on read handles carry no data-loss signal, so it is correct as written.

## Pattern

All 27 node findings and all 5 client findings come from one mechanical sweep: an error return value
that is dropped or only logged. That is a reasonable thing to grep for, and it did surface four real
problems. But the sweep cannot tell whether the discarded value was reachable, whether the existing
else-branch already recovers, or whether the suggested fix breaks something. Roughly two thirds of the
findings were unreachable, already handled, or false, and two of the proposed fixes would have
converted working self-healing paths into hard startup failures.

Worth applying to the next such report: check the else-branch before believing the impact claim, and
check what the suggested fix does to a node that is already in the bad state.
