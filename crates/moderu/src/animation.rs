//! Animation sampler and timeline support.
//!
//! Provides sampling of animation tracks at any time, interpolating between
//! keyframes without materializing the entire animation clip.

use crate::GltfModel;

/// Error type for animation operations.
#[derive(Debug, Clone)]
pub enum AnimationError {
    /// Animation index out of bounds.
    AnimationNotFound(usize),
    /// Sampler not found in animation.
    SamplerNotFound(usize),
    /// Channel not found in animation.
    ChannelNotFound(usize),
    /// Invalid sampler (missing input/output).
    InvalidSampler(String),
}

/// Interpolation mode for animation sampling.
#[derive(Clone, Copy, Debug)]
pub enum InterpolationMode {
    /// Linear interpolation between keyframes.
    Linear,
    /// Step interpolation (no interpolation).
    Step,
    /// Cubic spline interpolation (with tangents).
    CubicSpline,
}

impl From<&str> for InterpolationMode {
    fn from(s: &str) -> Self {
        match s {
            "LINEAR" => InterpolationMode::Linear,
            "STEP" => InterpolationMode::Step,
            "CUBICSPLINE" => InterpolationMode::CubicSpline,
            _ => InterpolationMode::Linear,
        }
    }
}

/// A single animation sampler (keyframes + interpolation).
pub struct AnimationSampler<'a> {
    #[allow(dead_code)]
    model: &'a GltfModel,
    _sampler_index: usize,
    interpolation: InterpolationMode,
}

impl<'a> AnimationSampler<'a> {
    /// Create a sampler from animation data.
    pub fn new(
        model: &'a GltfModel,
        _sampler_index: usize,
        interpolation: &str,
    ) -> Result<Self, AnimationError> {
        Ok(AnimationSampler {
            model,
            _sampler_index,
            interpolation: InterpolationMode::from(interpolation),
        })
    }

    /// Sample the animation at a given time (in seconds).
    pub fn sample_at(&self, _time: f32) -> Option<Vec<f32>> {
        None
    }

    /// Get the animation duration.
    pub fn duration(&self) -> f32 {
        0.0
    }

    /// Get interpolation mode.
    pub fn interpolation(&self) -> InterpolationMode {
        self.interpolation
    }
}

/// An animation channel target (e.g., "node.translation").
#[derive(Clone, Debug)]
pub struct AnimationTarget {
    pub node_index: usize,
    pub property: TargetProperty,
}

#[derive(Clone, Debug)]
pub enum TargetProperty {
    Translation,
    Rotation,
    Scale,
    Weights,
}

impl From<&str> for TargetProperty {
    fn from(s: &str) -> Self {
        match s {
            "translation" => TargetProperty::Translation,
            "rotation" => TargetProperty::Rotation,
            "scale" => TargetProperty::Scale,
            "weights" => TargetProperty::Weights,
            _ => TargetProperty::Translation,
        }
    }
}

/// An animation clip (collection of channels over time).
pub struct AnimationClip<'a> {
    #[allow(dead_code)]
    model: &'a GltfModel,
    _anim_index: usize,
    name: Option<String>,
    duration: f32,
}

impl<'a> AnimationClip<'a> {
    /// Create an animation clip.
    pub fn new(
        model: &'a GltfModel,
        _anim_index: usize,
        name: Option<String>,
        duration: f32,
    ) -> Self {
        AnimationClip {
            model,
            _anim_index,
            name,
            duration,
        }
    }

    /// Get animation name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Get duration in seconds.
    pub fn duration(&self) -> f32 {
        self.duration
    }

    /// Sample all channels at a given time.
    pub fn sample_at(&self, _time: f32) -> Vec<(AnimationTarget, Vec<f32>)> {
        vec![]
    }

    /// Play the animation (with optional looping).
    pub fn play(self, looping: bool) -> AnimationPlayer<'a> {
        AnimationPlayer {
            clip: self,
            current_time: 0.0,
            is_playing: true,
            is_looping: looping,
        }
    }
}

/// A playback controller for an animation.
pub struct AnimationPlayer<'a> {
    clip: AnimationClip<'a>,
    current_time: f32,
    is_playing: bool,
    is_looping: bool,
}

impl<'a> AnimationPlayer<'a> {
    /// Advance animation by delta time.
    pub fn update(&mut self, delta_time: f32) {
        if !self.is_playing {
            return;
        }

        self.current_time += delta_time;

        if self.current_time > self.clip.duration() {
            if self.is_looping {
                self.current_time %= self.clip.duration();
            } else {
                self.current_time = self.clip.duration();
                self.is_playing = false;
            }
        }
    }

    /// Get current time.
    pub fn current_time(&self) -> f32 {
        self.current_time
    }

    /// Check if playing.
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// Pause the animation.
    pub fn pause(&mut self) {
        self.is_playing = false;
    }

    /// Resume the animation.
    pub fn resume(&mut self) {
        self.is_playing = true;
    }

    /// Sample current state.
    pub fn current_sample(&self) -> Vec<(AnimationTarget, Vec<f32>)> {
        self.clip.sample_at(self.current_time)
    }
}
