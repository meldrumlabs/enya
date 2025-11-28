use core::{fmt, marker::PhantomData};

use sketches_ddsketch::{Config, DDSketch};
use uwheel::aggregator::{Aggregator, PartialAggregateType};

const DEFAULT_ALPHA: f64 = 0.01;
const DEFAULT_MAX_NUM_BINS: u32 = 2_048;
const DEFAULT_MIN_VALUE: f64 = 1.0e-9;

fn default_config() -> Config {
    Config::new(DEFAULT_ALPHA, DEFAULT_MAX_NUM_BINS, DEFAULT_MIN_VALUE)
}

/// Partial aggregate backed by a DDSketch.
#[derive(Clone, Default)]
pub struct DDSketchPartial {
    inner: Option<DDSketch>,
}

impl fmt::Debug for DDSketchPartial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (count, min, max) = self
            .inner
            .as_ref()
            .map(|sketch| (sketch.count(), sketch.min(), sketch.max()))
            .unwrap_or((0, None, None));

        f.debug_struct("DDSketchPartial")
            .field("count", &count)
            .field("min", &min.unwrap_or(f64::NAN))
            .field("max", &max.unwrap_or(f64::NAN))
            .finish()
    }
}

impl DDSketchPartial {
    /// Construct a partial aggregate from a DDSketch.
    pub fn from_sketch(sketch: DDSketch) -> Self {
        Self {
            inner: Some(sketch),
        }
    }

    /// Returns the wrapped DDSketch, instantiating an empty sketch when no data was observed.
    pub fn into_sketch(self) -> DDSketch {
        self.inner.unwrap_or_else(DDSketchAggregator::empty_sketch)
    }

    /// Returns a clone of the wrapped DDSketch without consuming the partial.
    pub fn to_sketch(&self) -> DDSketch {
        self.inner
            .clone()
            .unwrap_or_else(DDSketchAggregator::empty_sketch)
    }

    /// Returns a reference to the inner DDSketch if one exists.
    pub fn as_ref(&self) -> Option<&DDSketch> {
        self.inner.as_ref()
    }

    /// Returns the number of samples represented by this partial.
    pub fn count(&self) -> usize {
        self.inner.as_ref().map_or(0, DDSketch::count)
    }

    fn merge(self, other: Self) -> Self {
        match (self.inner, other.inner) {
            (Some(mut left), Some(right)) => {
                left.merge(&right)
                    .expect("DDSketch aggregators share the same config");
                Self::from_sketch(left)
            }
            (Some(left), None) => Self::from_sketch(left),
            (None, Some(right)) => Self::from_sketch(right),
            (None, None) => Self::default(),
        }
    }
}

impl PartialAggregateType for DDSketchPartial {}

impl From<DDSketchPartial> for DDSketch {
    fn from(value: DDSketchPartial) -> Self {
        value.into_sketch()
    }
}

impl From<&DDSketchPartial> for DDSketch {
    fn from(value: &DDSketchPartial) -> Self {
        value.to_sketch()
    }
}

/// Final aggregate for a DDSketch-based µWheel aggregation.
#[derive(Clone)]
pub struct DDSketchAggregate {
    sketch: DDSketch,
    marker: PhantomData<DDSketchAggregator>,
}

impl fmt::Debug for DDSketchAggregate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DDSketchAggregate")
            .field("count", &self.sketch.count())
            .field("min", &self.sketch.min())
            .field("max", &self.sketch.max())
            .finish_non_exhaustive()
    }
}

impl DDSketchAggregate {
    /// Returns the owned DDSketch.
    pub fn into_sketch(self) -> DDSketch {
        self.sketch
    }

    /// Borrows the wrapped DDSketch.
    pub fn as_sketch(&self) -> &DDSketch {
        &self.sketch
    }
}

impl From<DDSketchAggregate> for DDSketch {
    fn from(value: DDSketchAggregate) -> Self {
        value.into_sketch()
    }
}

impl From<DDSketch> for DDSketchAggregate {
    fn from(sketch: DDSketch) -> Self {
        Self {
            sketch,
            marker: PhantomData,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DDSketchAggregator(PhantomData<()>);

impl DDSketchAggregator {
    #[inline]
    fn empty_sketch() -> DDSketch {
        DDSketch::new(default_config())
    }
}

impl Aggregator for DDSketchAggregator {
    const IDENTITY: Self::PartialAggregate = DDSketchPartial { inner: None };

    type Input = f64;
    type MutablePartialAggregate = DDSketch;
    type PartialAggregate = DDSketchPartial;
    type Aggregate = DDSketchAggregate;

    fn lift(input: Self::Input) -> Self::MutablePartialAggregate {
        let mut sketch = Self::empty_sketch();
        sketch.add(input);
        sketch
    }

    fn combine_mutable(mutable: &mut Self::MutablePartialAggregate, input: Self::Input) {
        mutable.add(input);
    }

    fn freeze(mutable: Self::MutablePartialAggregate) -> Self::PartialAggregate {
        DDSketchPartial::from_sketch(mutable)
    }

    fn combine(a: Self::PartialAggregate, b: Self::PartialAggregate) -> Self::PartialAggregate {
        a.merge(b)
    }

    fn lower(partial: Self::PartialAggregate) -> Self::Aggregate {
        DDSketchAggregate::from(partial.into_sketch())
    }
}

#[cfg(test)]
mod tests {
    use super::DDSketchAggregator;
    use super::*;
    use uwheel::aggregator::Aggregator;

    #[test]
    fn aggregates_values_into_sketch() {
        let mut partial = DDSketchAggregator::lift(5.0);
        DDSketchAggregator::combine_mutable(&mut partial, 10.0);
        let frozen = DDSketchAggregator::freeze(partial);
        assert_eq!(frozen.count(), 2);
        let sketch = frozen.to_sketch();
        assert_eq!(sketch.count(), 2);
        assert_eq!(sketch.min(), Some(5.0));
        assert_eq!(sketch.max(), Some(10.0));
    }

    #[test]
    fn combines_partials_via_merge() {
        let left = DDSketchAggregator::freeze({
            let mut sketch = DDSketchAggregator::lift(1.0);
            DDSketchAggregator::combine_mutable(&mut sketch, 2.0);
            sketch
        });

        let right = DDSketchAggregator::freeze({
            let mut sketch = DDSketchAggregator::lift(4.0);
            DDSketchAggregator::combine_mutable(&mut sketch, 8.0);
            sketch
        });

        let aggregated = DDSketchAggregator::combine(left, right);
        assert_eq!(aggregated.count(), 4);

        let result = DDSketchAggregator::lower(aggregated).into_sketch();
        assert_eq!(result.count(), 4);
        assert_eq!(result.min(), Some(1.0));
        assert_eq!(result.max(), Some(8.0));
        let median = result.quantile(0.5).unwrap().unwrap();
        assert!(
            (1.0..=8.0).contains(&median),
            "median {median} should stay within observed bounds"
        );
    }

    #[test]
    fn test_ddsketch_partial_methods() {
        let partial = DDSketchPartial::default();
        assert!(partial.as_ref().is_none());
        assert_eq!(partial.count(), 0);
        assert_eq!(partial.to_sketch().count(), 0);

        let mut sketch = DDSketchAggregator::lift(5.0);
        DDSketchAggregator::combine_mutable(&mut sketch, 10.0);
        let partial = DDSketchPartial::from_sketch(sketch);
        assert!(partial.as_ref().is_some());
        assert_eq!(partial.count(), 2);
        assert_eq!(partial.to_sketch().count(), 2);
    }

    #[test]
    fn test_ddsketch_aggregate_methods() {
        let mut sketch = DDSketchAggregator::lift(5.0);
        DDSketchAggregator::combine_mutable(&mut sketch, 10.0);
        let aggregate = DDSketchAggregate::from(sketch);
        assert_eq!(aggregate.as_sketch().count(), 2);
        assert_eq!(aggregate.into_sketch().count(), 2);
    }

    #[test]
    fn test_ddsketch_aggregator_methods() {
        let sketch = DDSketchAggregator::empty_sketch();
        assert_eq!(sketch.count(), 0);
        assert_eq!(sketch.min(), None);
        assert_eq!(sketch.max(), None);
    }

    #[test]
    fn test_ddsketch_aggregator_identity() {
        let identity = DDSketchAggregator::IDENTITY;
        assert!(identity.as_ref().is_none());
        assert_eq!(identity.count(), 0);
        assert_eq!(identity.to_sketch().count(), 0);
    }
}
