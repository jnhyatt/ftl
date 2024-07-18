use std::ops::{Add, Div};

use bevy::math::Vec2;

pub trait MoveToward {
    fn move_toward(self, other: Self, max_delta: f32) -> Self;
}

impl MoveToward for f32 {
    fn move_toward(self, other: Self, max_delta: f32) -> Self {
        self + (other - self).clamp(-max_delta, max_delta)
    }
}

impl MoveToward for Vec2 {
    fn move_toward(self, other: Self, max_delta: f32) -> Self {
        if self.distance_squared(other) <= max_delta * max_delta {
            other
        } else {
            self + (other - self).clamp_length_max(max_delta)
        }
    }
}

pub fn round_to_usize(x: f32) -> usize {
    x.round() as usize
}

pub trait IterAvg: Iterator {
    fn average(self) -> Option<Self::Item>
    where
        Self: Sized,
        Self::Item: Add<Output = Self::Item> + Div<f32, Output = Self::Item>,
    {
        self.map(|x| (x, 1))
            .reduce(|x, y| (x.0 + y.0, x.1 + y.1))
            .map(|(sum, count)| sum / count as f32)
    }
}

impl<I: Iterator> IterAvg for I {}
