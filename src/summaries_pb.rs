use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Wire types in Protocol Buffers.
const WIRE_VARINT: u8 = 0;
const WIRE_FIXED64: u8 = 1;
const WIRE_LEN_DELIM: u8 = 2;
const WIRE_FIXED32: u8 = 5;

pub type ParsedSummaries = (HashMap<String, Vec<u8>>, Vec<Vec<u8>>);

pub struct SummariesProto;

impl SummariesProto {
    /// Encode an unsigned 64-bit integer as a protobuf varint.
    pub fn encode_varint(mut val: u64, buf: &mut Vec<u8>) {
        while val > 0x7f {
            buf.push(((val & 0x7f) as u8) | 0x80);
            val >>= 7;
        }
        buf.push(val as u8);
    }

    /// Decode a protobuf varint from data at `offset`.
    pub fn decode_varint(data: &[u8], offset: &mut usize) -> Result<u64> {
        let mut val: u64 = 0;
        let mut shift = 0;
        while *offset < data.len() {
            let b = data[*offset];
            *offset += 1;
            val |= ((b & 0x7f) as u64) << shift;
            if (b & 0x80) == 0 {
                return Ok(val);
            }
            shift += 7;
            if shift >= 64 {
                anyhow::bail!("Varint overflow");
            }
        }
        anyhow::bail!("Unexpected EOF reading varint");
    }

    /// Encode a length-delimited field (tag + length + payload).
    pub fn encode_len_delim(tag: u32, data: &[u8], buf: &mut Vec<u8>) {
        let key = ((tag as u64) << 3) | (WIRE_LEN_DELIM as u64);
        Self::encode_varint(key, buf);
        Self::encode_varint(data.len() as u64, buf);
        buf.extend_from_slice(data);
    }

    /// Encode a varint field (tag + varint value).
    pub fn encode_varint_field(tag: u32, val: u64, buf: &mut Vec<u8>) {
        let key = ((tag as u64) << 3) | (WIRE_VARINT as u64);
        Self::encode_varint(key, buf);
        Self::encode_varint(val, buf);
    }

    /// Parse an existing protobuf summaries file into a map of `conversation_id -> raw_summary`,
    /// preserving any other non-summaries fields verbatim.
    pub fn parse(data: &[u8]) -> Result<ParsedSummaries> {
        let mut entries = HashMap::new();
        let mut other_fields = Vec::new();
        let mut offset = 0;

        while offset < data.len() {
            let field_start = offset;
            let key = Self::decode_varint(data, &mut offset)?;
            let tag = (key >> 3) as u32;
            let wire = (key & 0x7) as u8;

            match wire {
                WIRE_VARINT => {
                    let _ = Self::decode_varint(data, &mut offset)?;
                    other_fields.push(data[field_start..offset].to_vec());
                }
                WIRE_FIXED64 => {
                    if offset + 8 > data.len() {
                        anyhow::bail!("Unexpected EOF reading fixed64");
                    }
                    offset += 8;
                    other_fields.push(data[field_start..offset].to_vec());
                }
                WIRE_LEN_DELIM => {
                    let len = Self::decode_varint(data, &mut offset)? as usize;
                    if offset + len > data.len() {
                        anyhow::bail!("Unexpected EOF reading len-delim field");
                    }
                    let sub = &data[offset..offset + len];
                    offset += len;

                    if tag == 1 {
                        // summaries map entry: field 1 = string key, field 2 = bytes value
                        if let Ok((Some(k), Some(v))) = Self::parse_map_entry(sub) {
                            entries.insert(k, v);
                        } else {
                            other_fields.push(data[field_start..offset].to_vec());
                        }
                    } else {
                        other_fields.push(data[field_start..offset].to_vec());
                    }
                }
                WIRE_FIXED32 => {
                    if offset + 4 > data.len() {
                        anyhow::bail!("Unexpected EOF reading fixed32");
                    }
                    offset += 4;
                    other_fields.push(data[field_start..offset].to_vec());
                }
                _ => {
                    anyhow::bail!("Unsupported wire type: {}", wire);
                }
            }
        }

        Ok((entries, other_fields))
    }

    /// Parse an individual protobuf map entry message (Tag 1 = key string, Tag 2 = value bytes).
    fn parse_map_entry(sub: &[u8]) -> Result<(Option<String>, Option<Vec<u8>>)> {
        let mut offset = 0;
        let mut key: Option<String> = None;
        let mut val: Option<Vec<u8>> = None;

        while offset < sub.len() {
            let tag_wire = Self::decode_varint(sub, &mut offset)?;
            let tag = (tag_wire >> 3) as u32;
            let wire = (tag_wire & 0x7) as u8;

            match wire {
                WIRE_VARINT => {
                    let _ = Self::decode_varint(sub, &mut offset)?;
                }
                WIRE_FIXED64 => {
                    if offset + 8 > sub.len() {
                        break;
                    }
                    offset += 8;
                }
                WIRE_LEN_DELIM => {
                    let len = Self::decode_varint(sub, &mut offset)? as usize;
                    if offset + len > sub.len() {
                        break;
                    }
                    let s = &sub[offset..offset + len];
                    offset += len;

                    if tag == 1 {
                        key = Some(String::from_utf8_lossy(s).to_string());
                    } else if tag == 2 {
                        val = Some(s.to_vec());
                    }
                }
                WIRE_FIXED32 => {
                    if offset + 4 > sub.len() {
                        break;
                    }
                    offset += 4;
                }
                _ => break,
            }
        }

        Ok((key, val))
    }

    /// Serialize map entries and other fields back into protobuf format.
    pub fn serialize(entries: &HashMap<String, Vec<u8>>, other_fields: &[Vec<u8>]) -> Vec<u8> {
        let mut out = Vec::new();

        // Write map entries for tag 1
        for (k, v) in entries {
            let mut sub = Vec::new();
            Self::encode_len_delim(1, k.as_bytes(), &mut sub);
            Self::encode_len_delim(2, v, &mut sub);
            Self::encode_len_delim(1, &sub, &mut out);
        }

        // Write preserved other fields
        for raw in other_fields {
            out.extend_from_slice(raw);
        }

        out
    }

    /// Synthesize a minimal valid `CascadeTrajectorySummary` protobuf blob if none exists.
    pub fn synthesize_summary(
        title: &str,
        conv_id: &str,
        project_id: &str,
        workspace_uri: &str,
    ) -> Vec<u8> {
        let mut summary = Vec::new();

        // Tag 1: summary (string)
        Self::encode_len_delim(1, title.as_bytes(), &mut summary);
        // Tag 2: step_count (uint32 = 1)
        Self::encode_varint_field(2, 1, &mut summary);
        // Tag 4: trajectory_id (string)
        Self::encode_len_delim(4, conv_id.as_bytes(), &mut summary);
        // Tag 5: status (CascadeRunStatus = 1: CASCADE_RUN_STATUS_IDLE)
        Self::encode_varint_field(5, 1, &mut summary);

        // Tag 17: trajectory_metadata (CortexTrajectoryMetadata)
        let mut traj_meta = Vec::new();
        if !workspace_uri.trim().is_empty() {
            // Tag 7 in CortexTrajectoryMetadata: workspace_uris (repeated string)
            Self::encode_len_delim(7, workspace_uri.as_bytes(), &mut traj_meta);
        }
        if !project_id.trim().is_empty() {
            // Tag 18 in CortexTrajectoryMetadata: project_id (string)
            Self::encode_len_delim(18, project_id.as_bytes(), &mut traj_meta);
        }
        Self::encode_len_delim(17, &traj_meta, &mut summary);

        summary
    }

    /// Upsert one or more conversation summaries into a `.pb` file atomically.
    pub fn upsert_to_file(pb_path: &Path, items: &[(String, Vec<u8>)]) -> Result<()> {
        let (mut entries, other_fields) = if pb_path.exists() {
            let data = fs::read(pb_path)
                .with_context(|| format!("Failed to read protobuf file: {}", pb_path.display()))?;
            Self::parse(&data).unwrap_or_else(|_| (HashMap::new(), Vec::new()))
        } else {
            (HashMap::new(), Vec::new())
        };

        for (cid, raw) in items {
            entries.insert(cid.clone(), raw.clone());
        }

        let serialized = Self::serialize(&entries, &other_fields);

        if let Some(parent) = pb_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let temp_path = pb_path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4()));
        fs::write(&temp_path, &serialized)?;
        if fs::rename(&temp_path, pb_path).is_err() {
            fs::copy(&temp_path, pb_path)?;
            let _ = fs::remove_file(&temp_path);
        }

        Ok(())
    }

    /// Synchronize imported conversation summaries into all applicable Antigravity `.pb` files.
    pub fn sync_all_pb_targets(
        paths: &crate::config::AntigravityPaths,
        items: &[(String, Vec<u8>)],
    ) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let targets = vec![
            paths.base_dir.join("agyhub_summaries_proto.pb"),
            paths.base_dir.join("jetbox_summaries_proto.pb"),
            paths
                .base_dir
                .parent()
                .unwrap_or(&paths.base_dir)
                .join("antigravity-cli")
                .join("jetbox_summaries_proto.pb"),
        ];

        let mut updated_at_least_one = false;
        for target in &targets {
            if target.exists() || target == &targets[0] {
                if let Err(e) = Self::upsert_to_file(target, items) {
                    eprintln!("Warning: Failed to update {}: {}", target.display(), e);
                } else {
                    updated_at_least_one = true;
                }
            }
        }

        // If target[0] didn't exist before, it was created by the loop above.
        if !updated_at_least_one {
            let _ = Self::upsert_to_file(&targets[0], items);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_roundtrip() {
        let values = vec![0, 1, 127, 128, 300, 16384, 1000000, u64::MAX];
        for v in values {
            let mut buf = Vec::new();
            SummariesProto::encode_varint(v, &mut buf);
            let mut offset = 0;
            let decoded = SummariesProto::decode_varint(&buf, &mut offset).unwrap();
            assert_eq!(v, decoded);
            assert_eq!(offset, buf.len());
        }
    }

    #[test]
    fn test_len_delim_roundtrip() {
        let mut buf = Vec::new();
        SummariesProto::encode_len_delim(1, b"hello world", &mut buf);
        assert!(!buf.is_empty());
    }

    #[test]
    fn test_summaries_proto_roundtrip() {
        let mut entries = HashMap::new();
        entries.insert("cid-1".to_string(), vec![1, 2, 3, 4]);
        entries.insert("cid-2".to_string(), vec![5, 6, 7, 8, 9]);

        let serialized = SummariesProto::serialize(&entries, &[]);
        let (parsed, other) = SummariesProto::parse(&serialized).unwrap();

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed.get("cid-1").unwrap(), &vec![1, 2, 3, 4]);
        assert_eq!(parsed.get("cid-2").unwrap(), &vec![5, 6, 7, 8, 9]);
        assert!(other.is_empty());
    }

    #[test]
    fn test_synthesize_summary() {
        let raw = SummariesProto::synthesize_summary(
            "Test Session",
            "test-cid",
            "test-pid",
            "file:///path/to/ws",
        );
        assert!(!raw.is_empty());
    }
}
