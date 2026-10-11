//! Locked, append-only persistence for the simulated write-budget planner.
//! CRC32 detects accidental corruption; it is not authentication or anti-rollback.

use crate::wear_leveling::{
    SlotConfig, SlotSnapshot, WearError, WearLeveler, WriteLease, WriteOutcome, MAX_SLOTS,
};
use std::fmt;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

const MAGIC: &[u8; 8] = b"PNXWL001";
const SLOT_BYTES: usize = 20;
const RECORD_BYTES: usize = 17;
const MAX_RECORDS: u64 = 1_000_000;
const MAX_BYTES: usize = 16 + MAX_SLOTS * SLOT_BYTES + MAX_RECORDS as usize * RECORD_BYTES;

#[derive(Debug)]
pub enum JournalError {
    Io(io::Error),
    Wear(WearError),
    Corrupt(&'static str),
    Locked,
    Full,
    Poisoned,
}

impl fmt::Display for JournalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "Journal I/O failed: {error}"),
            Self::Wear(error) => error.fmt(formatter),
            Self::Corrupt(reason) => write!(formatter, "Invalid journal: {reason}"),
            Self::Locked => {
                formatter.write_str("Journal is already locked by another handle or process")
            }
            Self::Full => {
                formatter.write_str("Journal record limit reached; reviewed compaction is required")
            }
            Self::Poisoned => {
                formatter.write_str("Journal write failed; close and validate before reuse")
            }
        }
    }
}

impl std::error::Error for JournalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Wear(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for JournalError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<WearError> for JournalError {
    fn from(error: WearError) -> Self {
        Self::Wear(error)
    }
}

#[derive(Clone, Copy)]
enum Event {
    Reserve = 1,
    Commit = 2,
    Refund = 3,
    Retire = 4,
}

struct Journal {
    file: File,
    sequence: u64,
    poisoned: bool,
}

impl Journal {
    fn append(&mut self, event: Event, id: u32) -> Result<(), JournalError> {
        if self.poisoned {
            return Err(JournalError::Poisoned);
        }
        if self.sequence >= MAX_RECORDS {
            self.poisoned = true;
            return Err(JournalError::Full);
        }
        let sequence = self.sequence + 1;
        let mut record = [0_u8; RECORD_BYTES];
        record[..8].copy_from_slice(&sequence.to_le_bytes());
        record[8..12].copy_from_slice(&id.to_le_bytes());
        record[12] = event as u8;
        let checksum = crc32(&record[..13]);
        record[13..].copy_from_slice(&checksum.to_le_bytes());
        if let Err(error) = self
            .file
            .write_all(&record)
            .and_then(|_| self.file.sync_all())
        {
            self.poisoned = true;
            return Err(error.into());
        }
        self.sequence = sequence;
        Ok(())
    }
}

pub struct JournaledWearLeveler {
    planner: WearLeveler,
    journal: Journal,
}

impl JournaledWearLeveler {
    /// Explicit initialization only; existing files are never overwritten.
    pub fn create(
        path: impl AsRef<Path>,
        configuration: Vec<SlotConfig>,
    ) -> Result<Self, JournalError> {
        let planner = WearLeveler::new(configuration)?;
        let mut options = OpenOptions::new();
        options.read(true).append(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path.as_ref())?;
        lock(&file)?;
        let mut header = Vec::with_capacity(16 + planner.slots().len() * SLOT_BYTES);
        header.extend_from_slice(MAGIC);
        header.extend_from_slice(&(planner.slots().len() as u32).to_le_bytes());
        for slot in planner.slots() {
            header.extend_from_slice(&slot.id.to_le_bytes());
            header.extend_from_slice(&slot.write_limit.to_le_bytes());
            header.extend_from_slice(&slot.charged_writes.to_le_bytes());
        }
        header.extend_from_slice(&crc32(&header).to_le_bytes());
        file.write_all(&header)?;
        file.sync_all()?;
        #[cfg(unix)]
        {
            let parent = path
                .as_ref()
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            File::open(parent)?.sync_all()?;
        }
        Ok(Self {
            planner,
            journal: Journal {
                file,
                sequence: 0,
                poisoned: false,
            },
        })
    }

    /// Requires an existing, valid journal; missing/corrupt files never reset state.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, JournalError> {
        let metadata = std::fs::symlink_metadata(path.as_ref())?;
        if !metadata.file_type().is_file() {
            return Err(JournalError::Corrupt("expected a regular file"));
        }
        let mut file = OpenOptions::new().read(true).append(true).open(path)?;
        lock(&file)?;
        if !file.metadata()?.is_file() || file.metadata()?.len() > MAX_BYTES as u64 {
            return Err(JournalError::Corrupt(
                "file exceeds the size limit or is not regular",
            ));
        }
        let mut bytes = Vec::new();
        file.seek(SeekFrom::Start(0))?;
        Read::by_ref(&mut file)
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_BYTES {
            return Err(JournalError::Corrupt("file exceeds the size limit"));
        }
        let (planner, sequence) = replay(&bytes)?;
        Ok(Self {
            planner,
            journal: Journal {
                file,
                sequence,
                poisoned: false,
            },
        })
    }

    pub fn slots(&self) -> impl ExactSizeIterator<Item = SlotSnapshot> + '_ {
        self.planner.slots()
    }
    pub fn remaining_writes(&self) -> u128 {
        self.planner.remaining_writes()
    }
    pub fn is_poisoned(&self) -> bool {
        self.journal.poisoned
    }

    /// No lease is returned until its reservation record has been flushed.
    pub fn reserve_write(&mut self) -> Result<JournaledWriteLease<'_>, JournalError> {
        if self.journal.poisoned {
            return Err(JournalError::Poisoned);
        }
        let lease = self.planner.reserve_write()?;
        self.journal.append(Event::Reserve, lease.slot_id())?;
        Ok(JournaledWriteLease {
            lease,
            journal: &mut self.journal,
        })
    }

    pub fn retire(&mut self, id: u32) -> Result<(), JournalError> {
        if self.journal.poisoned {
            return Err(JournalError::Poisoned);
        }
        if !self.planner.slots().any(|slot| slot.id == id) {
            return Err(WearError::UnknownSlot(id).into());
        }
        self.journal.append(Event::Retire, id)?;
        self.planner.retire(id)?;
        Ok(())
    }
}

#[must_use = "An unresolved journaled lease remains charged and quarantined on disk"]
pub struct JournaledWriteLease<'a> {
    lease: WriteLease<'a>,
    journal: &'a mut Journal,
}

impl JournaledWriteLease<'_> {
    pub fn slot_id(&self) -> u32 {
        self.lease.slot_id()
    }

    pub fn complete(self, outcome: WriteOutcome) -> Result<(), JournalError> {
        let event = match outcome {
            WriteOutcome::Committed => Some(Event::Commit),
            WriteOutcome::NotWritten => Some(Event::Refund),
            WriteOutcome::Unknown => None,
        };
        if let Some(event) = event {
            self.journal.append(event, self.lease.slot_id())?;
        }
        self.lease.complete(outcome);
        Ok(())
    }
}

fn lock(file: &File) -> Result<(), JournalError> {
    match file.try_lock() {
        Ok(()) => Ok(()),
        Err(TryLockError::WouldBlock) => Err(JournalError::Locked),
        Err(TryLockError::Error(error)) => Err(error.into()),
    }
}

fn replay(bytes: &[u8]) -> Result<(WearLeveler, u64), JournalError> {
    if bytes.len() < 16 || &bytes[..8] != MAGIC {
        return Err(JournalError::Corrupt("invalid format/version"));
    }
    let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if count == 0 || count > MAX_SLOTS {
        return Err(JournalError::Corrupt("invalid slot count"));
    }
    let header_end = 12 + count * SLOT_BYTES;
    if bytes.len() < header_end + 4
        || crc32(&bytes[..header_end])
            != u32::from_le_bytes(bytes[header_end..header_end + 4].try_into().unwrap())
    {
        return Err(JournalError::Corrupt(
            "incomplete header or checksum mismatch",
        ));
    }
    let mut configuration = Vec::with_capacity(count);
    for slot in bytes[12..header_end].chunks_exact(SLOT_BYTES) {
        configuration.push(SlotConfig {
            id: u32::from_le_bytes(slot[..4].try_into().unwrap()),
            write_limit: u64::from_le_bytes(slot[4..12].try_into().unwrap()),
            charged_writes: u64::from_le_bytes(slot[12..20].try_into().unwrap()),
        });
    }
    let mut planner = WearLeveler::new(configuration)
        .map_err(|_| JournalError::Corrupt("invalid initial configuration"))?;
    let records = &bytes[header_end + 4..];
    if records.len() % RECORD_BYTES != 0 || records.len() / RECORD_BYTES > MAX_RECORDS as usize {
        return Err(JournalError::Corrupt("incomplete tail or too many records"));
    }
    let mut sequence = 0;
    let mut pending = None;
    for record in records.chunks_exact(RECORD_BYTES) {
        sequence += 1;
        if u64::from_le_bytes(record[..8].try_into().unwrap()) != sequence
            || crc32(&record[..13]) != u32::from_le_bytes(record[13..17].try_into().unwrap())
        {
            return Err(JournalError::Corrupt(
                "record checksum or sequence mismatch",
            ));
        }
        let id = u32::from_le_bytes(record[8..12].try_into().unwrap());
        match record[12] {
            1 => {
                let lease = planner
                    .reserve_write()
                    .map_err(|_| JournalError::Corrupt("ineligible reservation"))?;
                if lease.slot_id() != id {
                    return Err(JournalError::Corrupt("unexpected reserved slot"));
                }
                lease.complete(WriteOutcome::Unknown);
                pending = Some(id);
            }
            2 | 3 => {
                if pending != Some(id) {
                    return Err(JournalError::Corrupt(
                        "completion has no matching reservation",
                    ));
                }
                planner
                    .replay_completion(
                        id,
                        if record[12] == 2 {
                            WriteOutcome::Committed
                        } else {
                            WriteOutcome::NotWritten
                        },
                    )
                    .map_err(|_| JournalError::Corrupt("invalid completion"))?;
                pending = None;
            }
            4 => {
                planner
                    .retire(id)
                    .map_err(|_| JournalError::Corrupt("unknown retired slot"))?;
                if pending == Some(id) {
                    pending = None;
                }
            }
            _ => return Err(JournalError::Corrupt("unknown event type")),
        }
    }
    Ok((planner, sequence))
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn failed_append_poisoning_prevents_an_issued_lease() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("failure.wearlog");
        let configuration = vec![SlotConfig {
            id: 0,
            write_limit: 10,
            charged_writes: 0,
        }];
        drop(JournaledWearLeveler::create(&path, configuration.clone()).unwrap());
        let before = std::fs::read(&path).unwrap();
        let mut planner = JournaledWearLeveler {
            planner: WearLeveler::new(configuration).unwrap(),
            journal: Journal {
                file: File::open(&path).unwrap(),
                sequence: 0,
                poisoned: false,
            },
        };
        assert!(matches!(planner.reserve_write(), Err(JournalError::Io(_))));
        assert!(planner.is_poisoned());
        assert!(matches!(
            planner.reserve_write(),
            Err(JournalError::Poisoned)
        ));
        assert!(matches!(planner.retire(0), Err(JournalError::Poisoned)));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(planner.slots().next().unwrap().charged_writes, 1);
    }

    #[test]
    fn rejects_crc_valid_invalid_transitions_and_sequences() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("invalid-events.wearlog");
        drop(
            JournaledWearLeveler::create(
                &path,
                (0..2)
                    .map(|id| SlotConfig {
                        id,
                        write_limit: 10,
                        charged_writes: 0,
                    })
                    .collect(),
            )
            .unwrap(),
        );
        let header = std::fs::read(&path).unwrap();
        for events in [
            vec![(1_u64, 0_u32, 2_u8)],
            vec![(1, 0, 3)],
            vec![(1, 0, 9)],
            vec![(2, 0, 1)],
            vec![(1, 99, 1)],
            vec![(1, 0, 1), (2, 1, 2)],
            vec![(1, 0, 1), (2, 0, 2), (3, 0, 2)],
            vec![(1, 0, 1), (2, 1, 1), (3, 0, 2)],
            vec![(1, 0, 1), (2, 0, 4), (3, 0, 2)],
        ] {
            let mut bytes = header.clone();
            for (sequence, id, event) in events {
                let mut record = Vec::new();
                record.extend_from_slice(&sequence.to_le_bytes());
                record.extend_from_slice(&id.to_le_bytes());
                record.push(event);
                record.extend_from_slice(&crc32(&record).to_le_bytes());
                bytes.extend_from_slice(&record);
            }
            assert!(matches!(replay(&bytes), Err(JournalError::Corrupt(_))));
        }
    }

    #[test]
    fn oversized_files_and_full_journals_fail_closed() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("limits.wearlog");
        let mut planner = JournaledWearLeveler::create(
            &path,
            vec![SlotConfig {
                id: 0,
                write_limit: 10,
                charged_writes: 0,
            }],
        )
        .unwrap();
        planner.journal.sequence = MAX_RECORDS;
        assert!(matches!(planner.reserve_write(), Err(JournalError::Full)));
        assert!(planner.is_poisoned());
        assert!(matches!(
            planner.reserve_write(),
            Err(JournalError::Poisoned)
        ));
        drop(planner);
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(MAX_BYTES as u64 + 1)
            .unwrap();
        assert!(matches!(
            JournaledWearLeveler::open(&path),
            Err(JournalError::Corrupt(_))
        ));
    }
}
