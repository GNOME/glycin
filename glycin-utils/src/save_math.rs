use crate::error::DimensionTooLargerError;

pub trait SafeConversion:
    TryInto<usize> + TryInto<i32> + TryInto<u32> + TryInto<i64> + TryInto<u64>
{
    fn try_usize(self) -> Result<usize, DimensionTooLargerError> {
        self.try_into().map_err(|_| DimensionTooLargerError)
    }

    fn try_i32(self) -> Result<i32, DimensionTooLargerError> {
        self.try_into().map_err(|_| DimensionTooLargerError)
    }

    fn try_u32(self) -> Result<u32, DimensionTooLargerError> {
        self.try_into().map_err(|_| DimensionTooLargerError)
    }

    fn try_i64(self) -> Result<i64, DimensionTooLargerError> {
        self.try_into().map_err(|_| DimensionTooLargerError)
    }

    fn try_u64(self) -> Result<u64, DimensionTooLargerError> {
        self.try_into().map_err(|_| DimensionTooLargerError)
    }
}

impl SafeConversion for usize {}
impl SafeConversion for u32 {}
impl SafeConversion for i32 {}
impl SafeConversion for u64 {}

pub trait SafeMath: Sized {
    fn smul(self, rhs: Self) -> Result<Self, DimensionTooLargerError>;
    fn sadd(self, rhs: Self) -> Result<Self, DimensionTooLargerError>;
    fn srem(self, rhs: Self) -> Result<Self, DimensionTooLargerError>;
}

impl SafeMath for usize {
    fn smul(self, rhs: Self) -> Result<Self, DimensionTooLargerError> {
        self.checked_mul(rhs).ok_or(DimensionTooLargerError)
    }

    fn sadd(self, rhs: Self) -> Result<Self, DimensionTooLargerError> {
        self.checked_add(rhs).ok_or(DimensionTooLargerError)
    }

    fn srem(self, rhs: Self) -> Result<Self, DimensionTooLargerError> {
        self.checked_rem(rhs).ok_or(DimensionTooLargerError)
    }
}

impl SafeMath for u32 {
    fn smul(self, rhs: Self) -> Result<Self, DimensionTooLargerError> {
        self.checked_mul(rhs).ok_or(DimensionTooLargerError)
    }

    fn sadd(self, rhs: Self) -> Result<Self, DimensionTooLargerError> {
        self.checked_add(rhs).ok_or(DimensionTooLargerError)
    }

    fn srem(self, rhs: Self) -> Result<Self, DimensionTooLargerError> {
        self.checked_add(rhs).ok_or(DimensionTooLargerError)
    }
}

impl SafeMath for u64 {
    fn smul(self, rhs: Self) -> Result<Self, DimensionTooLargerError> {
        self.checked_mul(rhs).ok_or(DimensionTooLargerError)
    }

    fn sadd(self, rhs: Self) -> Result<Self, DimensionTooLargerError> {
        self.checked_add(rhs).ok_or(DimensionTooLargerError)
    }

    fn srem(self, rhs: Self) -> Result<Self, DimensionTooLargerError> {
        self.checked_add(rhs).ok_or(DimensionTooLargerError)
    }
}
