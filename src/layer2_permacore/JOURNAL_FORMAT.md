# Simulated wear journal, version 1

The `wear_journal` module persists allocation metadata only. It does not persist actuator register values or logical application data. All integers use unsigned little-endian encoding. The file is append-only and locked exclusively for the owner's lifetime; no sidecar lock file or snapshot replacement is used.

## Initial header

| Field | Size | Validation |
| --- | --- | --- |
| Magic/version | 8 bytes | Exact ASCII `PNXWL001` |
| Slot count | 4 bytes | 1..4096 |
| Initial slot entries | 20 bytes each | ID u32, write limit u64, charged writes u64 |
| Header CRC32 | 4 bytes | Covers every preceding header byte |

Initial slots are healthy, sorted by ID, uniquely identified, have positive limits, and cannot start above their limit. `create` validates configuration before using atomic create-new semantics. It never truncates an existing file. Headers are flushed; Unix also synchronizes the parent directory. Existing state must be opened explicitly rather than recreated on an error.

## Event records

Each event is exactly 17 bytes:

| Field | Size | Validation |
| --- | --- | --- |
| Sequence | 8 bytes | Starts at 1 and increases by exactly 1 |
| Slot ID | 4 bytes | Must identify a valid slot |
| Event kind | 1 byte | One of the kinds below |
| CRC32 | 4 bytes | Covers the first 13 bytes of this record |

CRC uses the standard reflected CRC32 polynomial `0xedb88320`, initial/final XOR `0xffffffff`; the check value for ASCII `123456789` is `0xcbf43926`.

| Kind | Value | Replay rule |
| --- | --- | --- |
| Reserve | 1 | Must match the policy's next eligible slot; charges one write and quarantines it |
| Commit | 2 | Must complete the latest matching reservation; preserves its charge and marks healthy |
| Refund | 3 | Must complete the latest matching reservation; refunds one write and marks healthy |
| Retire | 4 | Permanently excludes the named slot without erasing charges |

Unknown outcomes and dropped leases need no additional record: the previously flushed reservation already preserves quarantine. A later reservation abandons the earlier unresolved reservation without making it healthy. A completion for that earlier reservation is invalid. Public leases are consumed on completion; replay-only completion access is crate-private.

## Failure and durability rules

- Reserve/commit/refund/retire writes use `write_all` followed by `sync_all`. Reserve exposes no lease before success; completion restores eligibility only after its event flush succeeds.
- An append or sync failure poisons the live owner. No further reservation or retirement is permitted; the pending slot remains conservatively charged/quarantined. Close it and validate the journal before resuming.
- Replay validates the complete file before returning a planner. Unknown versions, bad CRCs, sequence gaps, invalid transitions, and any partial trailing record are fatal. There is no fallback to a prefix, silent repair, reset, or automatic retry.
- Limit: 1000000 records; maximum file size is 17081936 bytes with 4096 slots. Loading is bounded, but replay still has the underlying planner's O(slots) selection cost per reservation. Compaction is deferred.
- CRC32 detects accidental corruption; it does not authenticate a writer or prevent deliberate edits, copied-old-state rollback, or whole-record removal. Keep the directory trusted and require separate authenticated/monotonic metadata before hardware integration.
- File locking coordinates cooperating local writers; filesystem/platform semantics and hostile directory replacement are outside this guarantee. Symlink/non-regular final paths are rejected on load, but this is not a hostile-filesystem sandbox.
- Process-interruption tests demonstrate restart behavior under the tested filesystem. They do not establish physical power-loss durability, controller flush correctness, or hardware endurance. Missing state is fatal unless an operator explicitly initializes a new simulation.

Reference: Rust's [File locking and synchronization APIs](https://doc.rust-lang.org/std/fs/struct.File.html). No journal migration or quarantine reactivation is implemented in version 1.
