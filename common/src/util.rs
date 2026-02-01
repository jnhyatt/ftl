use bevy::prelude::*;
use std::ops::{Add, Div, RangeInclusive};

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

pub fn inverse_lerp(a: f32, b: f32, x: f32) -> f32 {
    (x - a) / (b - a)
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

#[derive(Debug, Clone, Copy)]
pub struct Aabb {
    bottom_left: Vec2,
    top_right: Vec2,
}

impl Aabb {
    pub fn from_corners(a: Vec2, b: Vec2) -> Self {
        Self {
            bottom_left: a.min(b),
            top_right: a.max(b),
        }
    }

    pub fn scale_about_origin(self, factor: Vec2) -> Self {
        // Use the `from_corners` constructor to ensure negative `factor` values don't invalidate
        // ordering invariants
        Self::from_corners(self.bottom_left * factor, self.top_right * factor)
    }

    pub fn x_range(&self) -> RangeInclusive<f32> {
        self.bottom_left.x..=self.top_right.x
    }

    pub fn y_range(&self) -> RangeInclusive<f32> {
        self.bottom_left.y..=self.top_right.y
    }
}

impl std::ops::Sub<Vec2> for Aabb {
    type Output = Self;

    fn sub(self, rhs: Vec2) -> Self::Output {
        Self {
            bottom_left: self.bottom_left - rhs,
            top_right: self.top_right - rhs,
        }
    }
}

pub fn intersect(
    lhs: RangeInclusive<f32>,
    rhs: RangeInclusive<f32>,
) -> Option<RangeInclusive<f32>> {
    let start = lhs.start().max(*rhs.start());
    let end = lhs.end().min(*rhs.end());
    (start <= end).then_some(start..=end)
}

pub fn init_resource<R: Resource + FromWorld>(mut commands: Commands) {
    commands.init_resource::<R>();
}

pub fn remove_resource<R: Resource>(mut commands: Commands) {
    commands.remove_resource::<R>();
}

pub fn disable_observer(mut e: EntityWorldMut) {
    let Some(observer) = e.take() else {
        return;
    };
    e.insert(DisabledObserver(observer));
}

#[derive(Component)]
pub struct DisabledObserver(pub Observer);

pub fn enable_observer(mut e: EntityWorldMut) {
    let Some(DisabledObserver(observer)) = e.take() else {
        return;
    };
    e.insert(observer);
}
