use std::time::Duration;

#[derive(Debug, Default)]
pub struct Limits {
    pub(crate) inner: glycin_utils::Limits,
}

impl Limits {
    pub fn preset_icons() -> Self {
        Self::default()
            .timeout(Duration::from_secs(10))
            .max_dimensions((2048, 2048))
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.inner.timeout = timeout;
        self
    }

    pub fn max_dimensions(mut self, dimensions: (u32, u32)) -> Self {
        self.inner.max_dimensions = dimensions;
        self
    }
}
