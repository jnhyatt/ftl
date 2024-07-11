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
