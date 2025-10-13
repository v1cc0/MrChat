#![allow(dead_code)]

use intx::{I24, U24};

use crate::devices::format::SampleFormat;

#[derive(Clone)]
pub enum Samples {
    Float64(Vec<Vec<f64>>),
    Float32(Vec<Vec<f32>>),
    Signed32(Vec<Vec<i32>>),
    Unsigned32(Vec<Vec<u32>>),
    Signed24(Vec<Vec<I24>>),
    Unsigned24(Vec<Vec<U24>>),
    Signed16(Vec<Vec<i16>>),
    Unsigned16(Vec<Vec<u16>>),
    Signed8(Vec<Vec<i8>>),
    Unsigned8(Vec<Vec<u8>>),
    Dsd(Vec<Vec<bool>>),
}

impl Samples {
    pub fn is_format(&self, format: SampleFormat) -> bool {
        match self {
            Samples::Float64(_) => format == SampleFormat::Float64,
            Samples::Float32(_) => format == SampleFormat::Float32,
            Samples::Signed32(_) => format == SampleFormat::Signed32,
            Samples::Unsigned32(_) => format == SampleFormat::Unsigned32,
            Samples::Signed24(_) => format == SampleFormat::Signed24,
            Samples::Unsigned24(_) => format == SampleFormat::Unsigned24,
            Samples::Signed16(_) => format == SampleFormat::Signed16,
            Samples::Unsigned16(_) => format == SampleFormat::Unsigned16,
            Samples::Signed8(_) => format == SampleFormat::Signed8,
            Samples::Unsigned8(_) => format == SampleFormat::Unsigned8,
            Samples::Dsd(_) => format == SampleFormat::Dsd,
        }
    }
}

pub trait Mute {
    fn muted() -> Self;
}

macro_rules! mute_impl {
    ($t:ty, $val:expr) => {
        impl Mute for $t {
            fn muted() -> Self {
                $val
            }
        }
    };
}

mute_impl!(f64, 0.0);
mute_impl!(f32, 0.0);
mute_impl!(u32, 2147483647);
mute_impl!(U24, U24::try_from(8388607).unwrap());
mute_impl!(u16, 32767);
mute_impl!(u8, 127);
mute_impl!(i32, 0);
mute_impl!(I24, I24::from(0_u8));
mute_impl!(i16, 0);
mute_impl!(i8, 0);

pub trait UnwrapSample<T> {
    fn unwrap(self) -> T;
}

macro_rules! unwrap_impl {
    ($t:ty, $m:path) => {
        impl UnwrapSample<Vec<Vec<$t>>> for Samples {
            fn unwrap(self) -> Vec<Vec<$t>> {
                match self {
                    $m(v) => v,
                    _ => panic!("invalid sample format during unwrap"),
                }
            }
        }
    };
}

unwrap_impl!(f64, Samples::Float64);
unwrap_impl!(f32, Samples::Float32);
unwrap_impl!(u32, Samples::Unsigned32);
unwrap_impl!(U24, Samples::Unsigned24);
unwrap_impl!(u16, Samples::Unsigned16);
unwrap_impl!(u8, Samples::Unsigned8);
unwrap_impl!(i32, Samples::Signed32);
unwrap_impl!(I24, Samples::Signed24);
unwrap_impl!(i16, Samples::Signed16);
unwrap_impl!(i8, Samples::Signed8);
unwrap_impl!(bool, Samples::Dsd);

pub trait GetInnerSamples: Sized {
    fn inner(samples: Samples) -> Vec<Vec<Self>>;
}

macro_rules! inner_impl {
    ($t:ty) => {
        impl GetInnerSamples for $t {
            fn inner(samples: Samples) -> Vec<Vec<Self>> {
                samples.unwrap()
            }
        }
    };
    ($t:ty, $($tail:tt)*) => {
        impl GetInnerSamples for $t {
            fn inner(samples: Samples) -> Vec<Vec<Self>> {
                samples.unwrap()
            }
        }

        inner_impl!($($tail)*);
    };
}

inner_impl!(f64, f32, u32, U24, u16, u8, i32, I24, i16, i8, bool);

pub enum SampleFromError {
    WrongFormat,
}

impl TryFrom<Samples> for Vec<Vec<f64>> {
    type Error = SampleFromError;

    fn try_from(value: Samples) -> Result<Self, Self::Error> {
        match value {
            Samples::Float64(v) => Ok(v),
            _ => Err(SampleFromError::WrongFormat),
        }
    }
}

impl TryFrom<Samples> for Vec<Vec<f32>> {
    type Error = SampleFromError;

    fn try_from(value: Samples) -> Result<Self, Self::Error> {
        match value {
            Samples::Float32(v) => Ok(v),
            _ => Err(SampleFromError::WrongFormat),
        }
    }
}

impl TryFrom<Samples> for Vec<Vec<u8>> {
    type Error = SampleFromError;

    fn try_from(value: Samples) -> Result<Self, Self::Error> {
        match value {
            Samples::Unsigned8(v) => Ok(v),
            _ => Err(SampleFromError::WrongFormat),
        }
    }
}

impl TryFrom<Samples> for Vec<Vec<u16>> {
    type Error = SampleFromError;

    fn try_from(value: Samples) -> Result<Self, Self::Error> {
        match value {
            Samples::Unsigned16(v) => Ok(v),
            _ => Err(SampleFromError::WrongFormat),
        }
    }
}

impl TryFrom<Samples> for Vec<Vec<U24>> {
    type Error = SampleFromError;

    fn try_from(value: Samples) -> Result<Self, Self::Error> {
        match value {
            Samples::Unsigned24(v) => Ok(v),
            _ => Err(SampleFromError::WrongFormat),
        }
    }
}

impl TryFrom<Samples> for Vec<Vec<u32>> {
    type Error = SampleFromError;

    fn try_from(value: Samples) -> Result<Self, Self::Error> {
        match value {
            Samples::Unsigned32(v) => Ok(v),
            _ => Err(SampleFromError::WrongFormat),
        }
    }
}

impl TryFrom<Samples> for Vec<Vec<i8>> {
    type Error = SampleFromError;

    fn try_from(value: Samples) -> Result<Self, Self::Error> {
        match value {
            Samples::Signed8(v) => Ok(v),
            _ => Err(SampleFromError::WrongFormat),
        }
    }
}

impl TryFrom<Samples> for Vec<Vec<i16>> {
    type Error = SampleFromError;

    fn try_from(value: Samples) -> Result<Self, Self::Error> {
        match value {
            Samples::Signed16(v) => Ok(v),
            _ => Err(SampleFromError::WrongFormat),
        }
    }
}

impl TryFrom<Samples> for Vec<Vec<I24>> {
    type Error = SampleFromError;

    fn try_from(value: Samples) -> Result<Self, Self::Error> {
        match value {
            Samples::Signed24(v) => Ok(v),
            _ => Err(SampleFromError::WrongFormat),
        }
    }
}

impl TryFrom<Samples> for Vec<Vec<i32>> {
    type Error = SampleFromError;

    fn try_from(value: Samples) -> Result<Self, Self::Error> {
        match value {
            Samples::Signed32(v) => Ok(v),
            _ => Err(SampleFromError::WrongFormat),
        }
    }
}

impl TryFrom<Samples> for Vec<Vec<bool>> {
    type Error = SampleFromError;

    fn try_from(_: Samples) -> Result<Self, Self::Error> {
        Err(SampleFromError::WrongFormat)
    }
}

#[derive(Clone)]
pub struct PlaybackFrame {
    pub samples: Samples,
    /// The sample rate of the frame. This must always be the stereo sample rate (eg. if you have a
    /// mono frame, the sample rate should be doubled).
    pub rate: u32, // god forbid someone invents a PCM format that samples faster than 4 billion Hz
}
