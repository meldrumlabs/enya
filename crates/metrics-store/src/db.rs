//! Main database implementation

use crate::DatabaseBuilder;
use crate::MetricName;
use crate::SeriesId;
use crate::TagSet;
use crate::Timestamp;
use crate::Value;
use crate::cache::LocalCache;
use crate::query::filter::parse_filter_query;
use crate::series_key::SeriesKey;
use crate::smap::SeriesMapping;
use crate::storage::{Storage, WriteBatch};
use crate::tag_index::TagIndex;
use crate::tag_sets::{OwnedTagSets, TagSets};
use crate::time::timestamp;
use byteorder::{BigEndian, ReadBytesExt};
use bytes::Bytes;
use futures::Stream;
use slatedb::Db;
use std::io::Cursor;
use std::marker::PhantomData;
use std::ops::Bound;
use std::pin::Pin;
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

/// Type alias for a boxed async stream of stream items
pub type DataPointStream = Pin<Box<dyn Stream<Item = crate::Result<StreamItem>> + Send>>;

/// A series stream containing tags and data points
pub struct SeriesStream {
    /// The tags for this series
    pub tags: OwnedTagSets,
    /// The async data point stream
    pub stream: DataPointStream,
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

    pub(crate) async fn from_db(db: Arc<Db>, cache: &LocalCache) -> crate::Result<Self> {
        log::info!("Initializing database components");

        let storage = Storage::new(db);

        let smap = SeriesMapping::new(storage.clone(), cache.series().clone());
        let tag_index = TagIndex::new(storage.clone());
        let tag_sets = TagSets::new(storage.clone(), cache.tag_sets().clone());

        // Load series ID counter from storage
        smap.load_counter().await?;

        Ok(Self(Arc::new(DatabaseInner {
            storage,
            smap,
            tag_index,
            tag_sets,
            series_lock: RwLock::new(()),
        })))
    }

    fn format_data_point_key(series_id: SeriesId, ts: Timestamp) -> Bytes {
        Storage::data_key(series_id, ts)
    }

    /// Look up the series ID for a given metric name and tag set.
    ///
    /// Returns `None` if no series exists for this combination.
    ///
    /// # Errors
    ///
    /// Returns error if an I/O error occurred.
    pub async fn get_series_id(
        &self,
        metric: MetricName<'_>,
        tags: &TagSet<'_>,
    ) -> crate::Result<Option<SeriesId>> {
        let series_key = SeriesKey::format(metric, tags);
        self.0.smap.get(&series_key).await
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

            // Create async iterator over data points
            let iter = self.0.storage.db().scan(start_key..end_key).await?;

            // Convert to async StreamItem stream
            let stream = create_data_point_stream(iter, series_id);

            streams.push(SeriesStream {
                tags,
                stream: Box::pin(stream),
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
        self.write_at(metric, timestamp(), value, tags).await?;
        Ok(())
    }

    /// Write a data point at a specific timestamp.
    #[doc(hidden)]
    pub async fn write_at(
        &self,
        metric: MetricName<'_>,
        ts: Timestamp,
        value: Value,
        tags: &TagSet<'_>,
    ) -> crate::Result<SeriesId> {
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

        Ok(series_id)
    }

    /// Write multiple data points in a single batch operation.
    ///
    /// This is more efficient than calling `write()` multiple times when you have
    /// many data points to write, as it reduces the number of storage operations.
    ///
    /// All data points in the batch are written atomically.
    ///
    /// # Arguments
    ///
    /// * `points` - Iterator of (metric, value, tags) tuples to write
    ///
    /// # Errors
    ///
    /// Returns error if an I/O error occurred.
    pub async fn write_batch<'a>(
        &self,
        points: impl IntoIterator<Item = (MetricName<'a>, Value, &'a TagSet<'a>)>,
    ) -> crate::Result<()> {
        self.write_batch_at(points.into_iter().map(|(m, v, t)| (m, timestamp(), v, t)))
            .await
    }

    /// Write multiple data points at specific timestamps in a single batch operation.
    ///
    /// This is the timestamped version of `write_batch()`.
    ///
    /// # Arguments
    ///
    /// * `points` - Iterator of (metric, timestamp, value, tags) tuples to write
    ///
    /// # Errors
    ///
    /// Returns error if an I/O error occurred.
    pub async fn write_batch_at<'a>(
        &self,
        points: impl IntoIterator<Item = (MetricName<'a>, Timestamp, Value, &'a TagSet<'a>)>,
    ) -> crate::Result<()> {
        let mut batch = WriteBatch::new();
        let mut new_series_count = 0u64;

        for (metric, ts, value, tags) in points {
            let series_key = SeriesKey::format(metric, tags);
            let series_id = self.0.smap.get(&series_key).await?;

            let series_id = if let Some(series_id) = series_id {
                series_id
            } else {
                // Track that we need to initialize this series
                let id = self.initialize_new_series(&series_key, metric, tags).await?;
                new_series_count += 1;
                id
            };

            let data_point_key = Self::format_data_point_key(series_id, ts);
            batch.put(&data_point_key, value.to_be_bytes());
        }

        // Write all data points atomically
        self.0.storage.write_batch(batch).await?;

        if new_series_count > 0 {
            log::trace!("Batch write created {new_series_count} new series");
        }

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

        // Get next series ID (persists counter for crash safety)
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
    /// This flushes the series ID counter and any pending writes to object storage.
    ///
    /// # Errors
    ///
    /// Returns error if an I/O error occurred.
    pub async fn close(&self) -> crate::Result<()> {
        // Flush series ID counter before closing
        self.0.smap.flush_counter().await?;
        self.0.storage.close().await
    }
}

/// Creates an async stream of data points from a `SlateDB` scan iterator.
///
/// This properly uses async/await instead of blocking the runtime.
fn create_data_point_stream(
    mut iter: slatedb::DbIterator,
    series_id: SeriesId,
) -> impl Stream<Item = crate::Result<StreamItem>> + Send {
    async_stream::try_stream! {
        loop {
            match iter.next().await {
                Ok(Some(kv)) => {
                    // Parse key to extract timestamp
                    let key = kv.key;
                    // Skip prefix (2 bytes) and series_id (8 bytes) to get timestamp
                    if key.len() < 2 + 8 + 16 {
                        Err(crate::Error::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "key too short",
                        )))?;
                    }

                    let Some(ts_bytes) = key.get(2 + 8..2 + 8 + 16) else {
                        Err(crate::Error::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "key too short for timestamp",
                        )))?;
                        continue;
                    };
                    let mut cursor = Cursor::new(ts_bytes);
                    let inverted_ts = cursor.read_u128::<BigEndian>()?;
                    let ts = !inverted_ts;

                    // Parse value
                    let mut cursor = Cursor::new(&kv.value[..]);
                    let value = cursor.read_f64::<BigEndian>()?;

                    yield StreamItem {
                        series_id,
                        ts,
                        value,
                    };
                }
                Ok(None) => {
                    // Iterator exhausted
                    break;
                }
                Err(e) => {
                    Err(e)?;
                }
            }
        }
    }
}
