pub trait MoveToward {
    fn move_toward(self, other: Self, max_delta: Self) -> Self;
}

impl MoveToward for f32 {
    fn move_toward(self, other: Self, max_delta: Self) -> Self {
        self + (other - self).clamp(-max_delta, max_delta)
    }
}

pub fn round_to_usize(x: f32) -> usize {
    x.round() as usize
}
