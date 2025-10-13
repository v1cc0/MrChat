use intx::{I24, U24};

use super::resample::{SampleFrom, SampleInto};

pub fn interleave<T>(samples: Vec<Vec<T>>) -> Vec<T>
where
    T: Copy + PartialEq,
{
    if samples.is_empty() {
        return vec![];
    }

    let length = samples.len();
    let mut result = vec![];

    for i in 0..(samples.len() * samples[0].len()) {
        result.push(samples[i % length][i / length]);
    }

    result
}

// Code is dead on non-Linux platforms only
#[allow(dead_code)]
pub trait Packed {
    fn pack(&self) -> Vec<u8>;
}

macro_rules! impl_packed {
    ($t:ty) => {
        impl Packed for [$t] {
            fn pack(&self) -> Vec<u8> {
                self.iter()
                    .flat_map(|&x| x.to_ne_bytes().to_vec())
                    .collect()
            }
        }
    };
}

impl_packed!(u16);
impl_packed!(U24);
impl_packed!(u32);
impl_packed!(i16);
impl_packed!(I24);
impl_packed!(i32);
impl_packed!(i8);
impl_packed!(f32);
impl_packed!(f64);

// special cases
impl Packed for [u8] {
    fn pack(&self) -> Vec<u8> {
        self.to_vec()
    }
}

pub trait Scale: Sized {
    fn scale(self, factor: f64) -> Self;
}

impl<T> Scale for Vec<Vec<T>>
where
    T: SampleInto<f64> + SampleFrom<f64> + Copy,
{
    fn scale(self, factor: f64) -> Vec<Vec<T>> {
        self.iter()
            .map(|v| {
                v.iter()
                    // anything over 1.0 or under -1.0 will be clamped since it's out of bounds
                    .map(|v| T::sample_from(f64::clamp(v.sample_into() * factor, -1.0, 1.0)))
                    .collect()
            })
            .collect()
    }
}

impl Scale for Vec<Vec<f64>> {
    fn scale(self, factor: f64) -> Vec<Vec<f64>> {
        self.iter()
            .map(|v| {
                v.iter()
                    // anything over 1.0 or under -1.0 will be clamped since it's out of bounds
                    .map(|v| f64::clamp(v * factor, -1.0, 1.0))
                    .collect()
            })
            .collect()
    }
}
