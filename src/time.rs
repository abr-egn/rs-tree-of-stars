use std::time::Duration;

#[derive(Default)]
pub struct UpdateDelta(pub Duration);

#[derive(Default)]
pub struct DrawDelta(pub Duration);

fn to_seconds(d: &Duration) -> f32 {
    d.as_secs() as f32 + d.subsec_nanos() as f32 *1e-9
}

impl UpdateDelta {
    pub fn seconds(&self) -> f32 {
        to_seconds(&self.0)
    }
}

impl DrawDelta {
    pub fn seconds(&self) -> f32 {
        to_seconds(&self.0)
    }
}