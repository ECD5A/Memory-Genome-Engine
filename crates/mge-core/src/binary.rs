#[cfg(not(windows))]
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::errors::{MgeError, Result};

const MAGIC: [u8; 8] = *b"MGEFILE\0";
const FORMAT_VERSION: u16 = 1;
const CHECKSUM_LEN: usize = 32;
const HEADER_LEN: usize = MAGIC.len() + 1 + 2 + 1 + 8 + CHECKSUM_LEN;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileKind {
    Manifest = 1,
    MarkerDictionary = 2,
    HotLog = 3,
    HotRecord = 4,
    Page = 5,
    PageIndex = 6,
    MarkerIndex = 7,
    FuseIndex = 8,
    HotSnapshot = 9,
}

impl FileKind {
    fn from_u8(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Manifest),
            2 => Ok(Self::MarkerDictionary),
            3 => Ok(Self::HotLog),
            4 => Ok(Self::HotRecord),
            5 => Ok(Self::Page),
            6 => Ok(Self::PageIndex),
            7 => Ok(Self::MarkerIndex),
            8 => Ok(Self::FuseIndex),
            9 => Ok(Self::HotSnapshot),
            _ => Err(storage_error(format!("unknown file kind id {value}"))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manifest => "manifest",
            Self::MarkerDictionary => "marker_dictionary",
            Self::HotLog => "hot_log",
            Self::HotRecord => "hot_record",
            Self::Page => "page",
            Self::PageIndex => "page_index",
            Self::MarkerIndex => "marker_index",
            Self::FuseIndex => "fuse_index",
            Self::HotSnapshot => "hot_snapshot",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecId {
    None = 0,
    MessagePack = 1,
    MessagePackZstd = 2,
    MessagePackEncrypted = 3,
    MessagePackZstdEncrypted = 4,
}

impl CodecId {
    fn from_u8(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::MessagePack),
            2 => Ok(Self::MessagePackZstd),
            3 => Ok(Self::MessagePackEncrypted),
            4 => Ok(Self::MessagePackZstdEncrypted),
            _ => Err(storage_error(format!("unknown codec id {value}"))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::MessagePack => "messagepack",
            Self::MessagePackZstd => "messagepack_zstd",
            Self::MessagePackEncrypted => "messagepack_encrypted",
            Self::MessagePackZstdEncrypted => "messagepack_zstd_encrypted",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedFrame {
    pub kind: FileKind,
    pub codec: CodecId,
    pub payload: Vec<u8>,
}

pub fn encode_frame(kind: FileKind, codec: CodecId, payload: &[u8]) -> Result<Vec<u8>> {
    let payload_len = u64::try_from(payload.len())
        .map_err(|_| MgeError::InvalidInput("payload is larger than u64".to_string()))?;
    let checksum = Sha256::digest(payload);
    let mut output = Vec::with_capacity(HEADER_LEN + payload.len());

    output.extend_from_slice(&MAGIC);
    output.push(kind as u8);
    output.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    output.push(codec as u8);
    output.extend_from_slice(&payload_len.to_le_bytes());
    output.extend_from_slice(&checksum);
    output.extend_from_slice(payload);

    Ok(output)
}

pub fn decode_frame(bytes: &[u8], expected_kind: FileKind) -> Result<DecodedFrame> {
    let (frame, next_offset) = decode_frame_at(bytes, 0, expected_kind)?;
    if next_offset != bytes.len() {
        return Err(storage_error(format!(
            "{} has {} trailing bytes after payload",
            expected_kind.as_str(),
            bytes.len() - next_offset
        )));
    }
    Ok(frame)
}

pub fn decode_frame_at(
    bytes: &[u8],
    offset: usize,
    expected_kind: FileKind,
) -> Result<(DecodedFrame, usize)> {
    if bytes.len().saturating_sub(offset) < HEADER_LEN {
        return Err(storage_error(format!(
            "truncated {} header at byte {offset}",
            expected_kind.as_str()
        )));
    }

    let header = &bytes[offset..offset + HEADER_LEN];
    if header[..MAGIC.len()] != MAGIC {
        return Err(storage_error(format!(
            "wrong magic for {}",
            expected_kind.as_str()
        )));
    }

    let actual_kind = FileKind::from_u8(header[MAGIC.len()])?;
    if actual_kind != expected_kind {
        return Err(storage_error(format!(
            "wrong file kind: expected {}, found {}",
            expected_kind.as_str(),
            actual_kind.as_str()
        )));
    }

    let version_offset = MAGIC.len() + 1;
    let version = u16::from_le_bytes([header[version_offset], header[version_offset + 1]]);
    if version != FORMAT_VERSION {
        return Err(storage_error(format!(
            "unsupported version {version} for {}",
            expected_kind.as_str()
        )));
    }

    let codec_offset = version_offset + 2;
    let codec = CodecId::from_u8(header[codec_offset])?;

    let len_offset = codec_offset + 1;
    let mut payload_len_bytes = [0u8; 8];
    payload_len_bytes.copy_from_slice(&header[len_offset..len_offset + 8]);
    let payload_len = u64::from_le_bytes(payload_len_bytes);
    let payload_len = usize::try_from(payload_len).map_err(|_| {
        storage_error(format!(
            "payload length for {} does not fit this platform",
            expected_kind.as_str()
        ))
    })?;

    let checksum_offset = len_offset + 8;
    let expected_checksum = &header[checksum_offset..checksum_offset + CHECKSUM_LEN];
    let payload_offset = offset + HEADER_LEN;
    let next_offset = payload_offset.checked_add(payload_len).ok_or_else(|| {
        storage_error(format!(
            "payload length overflow for {}",
            expected_kind.as_str()
        ))
    })?;

    if bytes.len() < next_offset {
        return Err(storage_error(format!(
            "truncated {} payload: expected {} bytes, found {}",
            expected_kind.as_str(),
            payload_len,
            bytes.len().saturating_sub(payload_offset)
        )));
    }

    let payload = &bytes[payload_offset..next_offset];
    let actual_checksum = Sha256::digest(payload);
    if actual_checksum.as_slice() != expected_checksum {
        return Err(storage_error(format!(
            "corrupted {} payload checksum",
            expected_kind.as_str()
        )));
    }

    Ok((
        DecodedFrame {
            kind: actual_kind,
            codec,
            payload: payload.to_vec(),
        },
        next_offset,
    ))
}

pub fn write_messagepack_file<T: Serialize>(
    path: impl AsRef<Path>,
    kind: FileKind,
    value: &T,
) -> Result<()> {
    let payload = rmp_serde::to_vec_named(value)?;
    let bytes = encode_frame(kind, CodecId::MessagePack, &payload)?;
    atomic_write_bytes(path, &bytes)
}

pub fn read_messagepack_file<T: DeserializeOwned>(
    path: impl AsRef<Path>,
    kind: FileKind,
) -> Result<T> {
    let bytes = fs::read(path)?;
    let frame = decode_frame(&bytes, kind)?;
    if frame.codec != CodecId::MessagePack {
        return Err(storage_error(format!(
            "wrong codec for {}: expected {}, found {}",
            kind.as_str(),
            CodecId::MessagePack.as_str(),
            frame.codec.as_str()
        )));
    }
    Ok(rmp_serde::from_slice(&frame.payload)?)
}

pub fn atomic_write_bytes(path: impl AsRef<Path>, bytes: &[u8]) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let temp_path = unique_temp_path(path);
    {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
    }

    replace_with_temp(&temp_path, path)?;
    sync_parent_dir(path)?;
    Ok(())
}

fn unique_temp_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("mge-file");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    parent.join(format!(".{file_name}.tmp-{}-{nanos}", std::process::id()))
}

#[cfg(windows)]
fn replace_with_temp(temp_path: &Path, final_path: &Path) -> Result<()> {
    if final_path.exists() {
        fs::remove_file(final_path)?;
    }
    fs::rename(temp_path, final_path)?;
    Ok(())
}

#[cfg(not(windows))]
fn replace_with_temp(temp_path: &Path, final_path: &Path) -> Result<()> {
    fs::rename(temp_path, final_path)?;
    Ok(())
}

#[cfg(windows)]
fn sync_parent_dir(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(not(windows))]
fn sync_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        let dir = File::open(parent)?;
        dir.sync_all()?;
    }
    Ok(())
}

fn storage_error(message: impl Into<String>) -> MgeError {
    MgeError::StorageFormat(message.into())
}
