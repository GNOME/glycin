use crate::Cicp;

impl Cicp {
    #[inline]
    pub fn color_primaries(&self) -> u8 {
        self.inner.color_primaries
    }

    #[inline]
    pub fn transfer_characteristics(&self) -> u8 {
        self.inner.transfer_characteristics
    }

    #[inline]
    pub fn matrix_coefficients(&self) -> u8 {
        self.inner.matrix_coefficients
    }

    #[inline]
    pub fn video_full_range_flag(&self) -> u8 {
        self.inner.video_full_range_flag
    }
}
