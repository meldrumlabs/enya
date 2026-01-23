//! Layout Animation - Smooth transitions when panes split or close.
//!
//! This module provides animated layout transitions for egui_tiles.
//! When a pane is split, the new pane smoothly grows from 0 to its target size.
//! When a pane is closed, it smoothly shrinks before being removed.

use egui_tiles::TileId;
use rustc_hash::FxHashMap;

use crate::util::Instant;

/// Duration for layout animations in seconds.
const ANIMATION_DURATION: f32 = 0.15;

/// Smooth easing function (ease-out cubic).
fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// A single share animation for a tile within a container.
#[derive(Clone)]
pub struct ShareAnimation {
    /// The tile being animated.
    pub tile_id: TileId,
    /// Starting share value.
    pub start_share: f32,
    /// Target share value.
    pub target_share: f32,
    /// When the animation started.
    pub start_time: Instant,
}

impl ShareAnimation {
    /// Create a new share animation.
    pub fn new(tile_id: TileId, start_share: f32, target_share: f32) -> Self {
        Self {
            tile_id,
            start_share,
            target_share,
            start_time: Instant::now(),
        }
    }

    /// Get the current progress (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        let elapsed = self.start_time.elapsed().as_secs_f32();
        (elapsed / ANIMATION_DURATION).min(1.0)
    }

    /// Check if the animation is complete.
    pub fn is_complete(&self) -> bool {
        self.progress() >= 1.0
    }

    /// Get the current interpolated share value.
    pub fn current_share(&self) -> f32 {
        let progress = ease_out_cubic(self.progress());
        self.start_share + (self.target_share - self.start_share) * progress
    }
}

/// Manages layout animations for the workspace.
#[derive(Clone, Default)]
pub struct LayoutAnimator {
    /// Active share animations keyed by container TileId.
    /// Each container can have multiple tiles animating their shares.
    animations: FxHashMap<TileId, Vec<ShareAnimation>>,
}

impl LayoutAnimator {
    /// Create a new layout animator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a split animation for a new tile in a container.
    /// The new tile will grow from a small share to its target share.
    pub fn animate_split(
        &mut self,
        container_id: TileId,
        new_tile_id: TileId,
        other_tile_id: TileId,
        target_share: f32,
    ) {
        // Small initial share for the new tile
        let start_share = 0.05;

        // The new tile grows from small to target
        let new_anim = ShareAnimation::new(new_tile_id, start_share, target_share);

        // The other tile shrinks from large to its target
        let other_start = 2.0 - start_share; // Compensate so total is ~2.0
        let other_anim = ShareAnimation::new(other_tile_id, other_start, target_share);

        self.animations
            .insert(container_id, vec![new_anim, other_anim]);
    }

    /// Update animations and return the current shares for each container.
    /// Returns a map of container_id -> [(tile_id, current_share), ...]
    pub fn update(&mut self) -> FxHashMap<TileId, Vec<(TileId, f32)>> {
        let mut result = FxHashMap::default();

        for (container_id, anims) in &self.animations {
            let shares: Vec<(TileId, f32)> = anims
                .iter()
                .map(|a| (a.tile_id, a.current_share()))
                .collect();
            result.insert(*container_id, shares);
        }

        // Remove completed animations
        self.animations.retain(|_, anims| {
            anims.retain(|a| !a.is_complete());
            !anims.is_empty()
        });

        result
    }

    /// Check if any animations are active.
    pub fn has_active_animations(&self) -> bool {
        !self.animations.is_empty()
    }
}
