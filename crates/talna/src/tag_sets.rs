use crate::SeriesId;
use fjall::{CompressionType, PartitionCreateOptions, TxKeyspace, TxPartition, WriteTransaction};
use std::io;

const PARTITION_NAME: &str = "_talna#v1#tags";

pub type OwnedTagSets = crate::HashMap<String, String>;

/// Maps Series IDs to their tags
pub struct TagSets {
    partition: TxPartition,
}

impl TagSets {
    pub fn new(keyspace: &TxKeyspace) -> crate::Result<Self> {
        let opts = PartitionCreateOptions::default()
            .block_size(4_096)
            .compression(CompressionType::Lz4)
            .max_memtable_size(8_000_000);

        let partition = keyspace.open_partition(PARTITION_NAME, opts)?;

        Ok(Self { partition })
    }

    pub fn insert(&self, tx: &mut WriteTransaction, series_id: SeriesId, tags: &str) {
        //  log::trace!("Storing tag set {series_id:?} => {tags:?}");
        tx.insert(&self.partition, series_id.to_be_bytes(), tags);
    }

    pub fn get(&self, series_id: SeriesId) -> crate::Result<OwnedTagSets> {
        Ok(self
            .partition
            .get(series_id.to_be_bytes())?
            .filter(|x| !x.is_empty())
            .map(|bytes| {
                let reader = std::str::from_utf8(&bytes).map_err(|err| {
                    crate::Error::Io(io::Error::new(io::ErrorKind::InvalidData, err))
                })?;
                parse_key_value_pairs(reader)
            })
            .transpose()?
            .unwrap_or_default())
    }
}

fn parse_key_value_pairs(input: &str) -> crate::Result<OwnedTagSets> {
    input
        .split(';')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let mut split = pair.splitn(2, ':');

            if let (Some(key), Some(value)) = (split.next(), split.next()) {
                Ok((key.to_string(), value.to_string()))
            } else {
                Err(crate::Error::Io(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid serialized tag",
                )))
            }
        })
        .collect()
}
