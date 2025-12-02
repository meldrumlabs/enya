//! Main database implementation

use crate::DatabaseBuilder;
use crate::MetricName;
use crate::SeriesId;
use crate::TagSet;
use crate::Timestamp;
use crate::Value;
use crate::query::filter::parse_filter_query;
use crate::series_key::SeriesKey;
use crate::smap::SeriesMapping;
use crate::storage::Storage;
use crate::tag_index::TagIndex;
use crate::tag_sets::{OwnedTagSets, TagSets};
use crate::time::timestamp;
use byteorder::{BigEndian, ReadBytesExt};
use bytes::Bytes;
use slatedb::Db;
use std::io::Cursor;
use std::marker::PhantomData;
use std::ops::Bound;
use std::sync::Arc;
use tokio::sync::RwLock;

/// One minute in nanoseconds
pub const MINUTE_IN_NS: u128 = 60_000_000_000;

/// A stream item representing a single data point
#[derive(Debug)]
pub struct StreamItem {
    /// The series ID this data point belongs to
    pub series_id: SeriesId,
    /// The timestamp of this data point
    pub ts: Timestamp,
    /// The value of this data point
    pub value: Value,
}

/// A series stream containing tags and data points
pub struct SeriesStream {
    /// The tags for this series
    pub tags: OwnedTagSets,
    /// The data point reader
    pub reader: Box<dyn Iterator<Item = crate::Result<StreamItem>> + Send>,
}

/// Inner database state
struct DatabaseInner {
    /// The underlying storage
    storage: Storage,

    /// Series mapping: series key -> series ID
    smap: SeriesMapping,

    /// Inverted index of tag permutations
    tag_index: TagIndex,

    /// Maps series ID to its tags
    tag_sets: TagSets,

    /// Lock for series creation to prevent races
    series_lock: RwLock<()>,
}

/// An embeddable time series database backed by object storage
#[derive(Clone)]
pub struct Database(Arc<DatabaseInner>);

impl Database {
    /// Creates a new database builder.
    #[must_use]
    pub fn builder() -> DatabaseBuilder {
        DatabaseBuilder::new()
    }

    pub(crate) fn from_db(db: Arc<Db>) -> Self {
        log::info!("Initializing database components");

        let storage = Storage::new(db);

        let smap = SeriesMapping::new(storage.clone());
        let tag_index = TagIndex::new(storage.clone());
        let tag_sets = TagSets::new(storage.clone());

        Self(Arc::new(DatabaseInner {
            storage,
            smap,
            tag_index,
            tag_sets,
            series_lock: RwLock::new(()),
        }))
    }

    fn format_data_point_key(series_id: SeriesId, ts: Timestamp) -> Bytes {
        Storage::data_key(series_id, ts)
    }

    /// Prepare a query, returning streams for each matching series
    ///
    /// # Errors
    ///
    /// Returns error if an I/O error occurred.
    pub async fn prepare_query(
        &self,
        series_ids: &[SeriesId],
        (min, max): (Bound<Timestamp>, Bound<Timestamp>),
    ) -> crate::Result<Vec<SeriesStream>> {
        let mut streams = Vec::with_capacity(series_ids.len());

        for &series_id in series_ids {
            let tags = self.0.tag_sets.get(series_id).await?;

            // Build range bounds for this series
            let (start_key, end_key) = Self::build_range_keys(series_id, min, max);

            // Create iterator over data points
            let iter = self.0.storage.db().scan(start_key..end_key).await?;

            // Convert to StreamItem iterator
            let reader = DataPointIterator::new(iter, series_id);

            streams.push(SeriesStream {
                tags,
                reader: Box::new(reader),
            });
        }

        Ok(streams)
    }

    fn build_range_keys(
        series_id: SeriesId,
        min: Bound<Timestamp>,
        max: Bound<Timestamp>,
    ) -> (Bytes, Bytes) {
        use Bound::{Excluded, Included, Unbounded};

        // Start key (most recent = smallest inverted timestamp)
        let start_key = match max {
            Unbounded => Storage::data_key(series_id, Timestamp::MAX),
            Included(ts) => Storage::data_key(series_id, ts),
            Excluded(ts) => Storage::data_key(series_id, ts.saturating_sub(1)),
        };

        // End key (oldest = largest inverted timestamp)
        let end_key = match min {
            Unbounded => Storage::data_key(series_id, 0),
            Included(ts) => {
                // Need to include this timestamp, so go one past
                let mut key = Storage::data_key(series_id, ts).to_vec();
                // Increment last byte to make it exclusive
                if let Some(last) = key.last_mut() {
                    *last = last.saturating_add(1);
                }
                Bytes::from(key)
            }
            Excluded(ts) => Storage::data_key(series_id, ts),
        };

        (start_key, end_key)
    }

    pub(crate) async fn start_query(
        &self,
        metric: &str,
        filter_expr: &str,
        (min, max): (Bound<Timestamp>, Bound<Timestamp>),
    ) -> crate::Result<Vec<SeriesStream>> {
        let Ok(filter) = parse_filter_query(filter_expr) else {
            return Err(crate::Error::InvalidQuery);
        };

        let series_ids = filter
            .evaluate(&self.0.smap, &self.0.tag_index, metric)
            .await?;

        if series_ids.is_empty() {
            log::debug!("Query {filter_expr:?} did not match any series");
            return Ok(vec![]);
        }

        log::trace!(
            "Querying metric {metric}{{{filter}}} [{min:?}..{max:?}] in series {series_ids:?}"
        );

        self.prepare_query(&series_ids, (min, max)).await
    }

    /// Returns an aggregation builder for computing averages.
    #[must_use]
    pub fn avg<'a>(
        &'a self,
        metric: MetricName<'a>,
        group_by: &'a str,
    ) -> crate::agg::Builder<'a, crate::agg::Average> {
        crate::agg::Builder {
            phantom: PhantomData,
            database: self,
            metric_name: metric.as_str(),
            filter_expr: "*",
            bucket_width: MINUTE_IN_NS,
            group_by,
            max_ts: None,
            min_ts: None,
        }
    }

    /// Returns an aggregation builder for computing sums.
    #[must_use]
    pub fn sum<'a>(
        &'a self,
        metric: MetricName<'a>,
        group_by: &'a str,
    ) -> crate::agg::Builder<'a, crate::agg::Sum> {
        crate::agg::Builder {
            phantom: PhantomData,
            database: self,
            metric_name: metric.as_str(),
            filter_expr: "*",
            bucket_width: MINUTE_IN_NS,
            group_by,
            max_ts: None,
            min_ts: None,
        }
    }

    /// Returns an aggregation builder for computing minimums.
    #[must_use]
    pub fn min<'a>(
        &'a self,
        metric: MetricName<'a>,
        group_by: &'a str,
    ) -> crate::agg::Builder<'a, crate::agg::Min> {
        crate::agg::Builder {
            phantom: PhantomData,
            database: self,
            metric_name: metric.as_str(),
            filter_expr: "*",
            bucket_width: MINUTE_IN_NS,
            group_by,
            max_ts: None,
            min_ts: None,
        }
    }

    /// Returns an aggregation builder for computing maximums.
    #[must_use]
    pub fn max<'a>(
        &'a self,
        metric: MetricName<'a>,
        group_by: &'a str,
    ) -> crate::agg::Builder<'a, crate::agg::Max> {
        crate::agg::Builder {
            phantom: PhantomData,
            database: self,
            metric_name: metric.as_str(),
            filter_expr: "*",
            bucket_width: MINUTE_IN_NS,
            group_by,
            max_ts: None,
            min_ts: None,
        }
    }

    /// Returns an aggregation builder for counting data points.
    #[must_use]
    pub fn count<'a>(
        &'a self,
        metric: MetricName<'a>,
        group_by: &'a str,
    ) -> crate::agg::Builder<'a, crate::agg::Count> {
        crate::agg::Builder {
            phantom: PhantomData,
            database: self,
            metric_name: metric.as_str(),
            filter_expr: "*",
            bucket_width: MINUTE_IN_NS,
            group_by,
            max_ts: None,
            min_ts: None,
        }
    }

    /// Write a data point to the database.
    ///
    /// # Errors
    ///
    /// Returns error if an I/O error occurred.
    pub async fn write(
        &self,
        metric: MetricName<'_>,
        value: Value,
        tags: &TagSet<'_>,
    ) -> crate::Result<()> {
        self.write_at(metric, timestamp(), value, tags).await
    }

    /// Write a data point at a specific timestamp.
    #[doc(hidden)]
    pub async fn write_at(
        &self,
        metric: MetricName<'_>,
        ts: Timestamp,
        value: Value,
        tags: &TagSet<'_>,
    ) -> crate::Result<()> {
        let series_key = SeriesKey::format(metric, tags);
        let series_id = self.0.smap.get(&series_key).await?;

        let series_id = if let Some(series_id) = series_id {
            // Series already exists (happy path)
            series_id
        } else {
            // Create new series
            self.initialize_new_series(&series_key, metric, tags)
                .await?
        };

        let data_point_key = Self::format_data_point_key(series_id, ts);
        self.0
            .storage
            .put(data_point_key, Bytes::from(value.to_be_bytes().to_vec()))
            .await?;

        Ok(())
    }

    async fn initialize_new_series(
        &self,
        series_key: &str,
        metric: MetricName<'_>,
        tags: &TagSet<'_>,
    ) -> crate::Result<SeriesId> {
        // Acquire write lock to prevent race conditions
        let _lock = self.0.series_lock.write().await;

        // Double-check if series was created while waiting for lock
        if let Some(series_id) = self.0.smap.get(series_key).await? {
            return Ok(series_id);
        }

        // Get next series ID
        let series_id = self.0.smap.next_series_id().await?;

        log::trace!("Creating series {series_id} for key {series_key:?}");

        // Index the series
        self.0.tag_index.index(metric, tags, series_id).await?;

        // Store tag set
        let mut serialized_tags = SeriesKey::allocate_string_for_tags(tags, 0);
        SeriesKey::join_tags(&mut serialized_tags, tags);
        self.0.tag_sets.insert(series_id, &serialized_tags).await?;

        // Store series mapping
        self.0.smap.insert(series_key, series_id).await?;

        Ok(series_id)
    }

    /// Close the database.
    ///
    /// This flushes any pending writes to object storage.
    ///
    /// # Errors
    ///
    /// Returns error if an I/O error occurred.
    pub async fn close(&self) -> crate::Result<()> {
        self.0.storage.close().await
    }
}

/// Iterator over data points from `SlateDB` scan
struct DataPointIterator {
    inner: slatedb::DbIterator,
    series_id: SeriesId,
    exhausted: bool,
}

impl DataPointIterator {
    fn new(inner: slatedb::DbIterator, series_id: SeriesId) -> Self {
        Self {
            inner,
            series_id,
            exhausted: false,
        }
    }
}

impl Iterator for DataPointIterator {
    type Item = crate::Result<StreamItem>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.exhausted {
            return None;
        }

        // Use blocking approach for now - in production would want async stream
        let result = futures::executor::block_on(async { self.inner.next().await });

        match result {
            Ok(Some(kv)) => {
                // Parse key to extract timestamp
                let key = kv.key;
                // Skip prefix (2 bytes) and series_id (8 bytes) to get timestamp
                if key.len() < 2 + 8 + 16 {
                    return Some(Err(crate::Error::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "key too short",
                    ))));
                }

                let Some(ts_bytes) = key.get(2 + 8..2 + 8 + 16) else {
                    return Some(Err(crate::Error::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "key too short for timestamp",
                    ))));
                };
                let mut cursor = Cursor::new(ts_bytes);
                let inverted_ts = match cursor.read_u128::<BigEndian>() {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e.into())),
                };
                let ts = !inverted_ts;

                // Parse value
                let mut cursor = Cursor::new(&kv.value[..]);
                #[cfg(feature = "high_precision")]
                let value = match cursor.read_f64::<BigEndian>() {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e.into())),
                };
                #[cfg(not(feature = "high_precision"))]
                let value = match cursor.read_f32::<BigEndian>() {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e.into())),
                };

                Some(Ok(StreamItem {
                    series_id: self.series_id,
                    ts,
                    value,
                }))
            }
            Ok(None) => {
                self.exhausted = true;
                None
            }
            Err(e) => Some(Err(e.into())),
        }
    }
}
