#![crate_name = "jpeg_diff_rs"]

use image::io::Reader as ImageReader;
use image::{DynamicImage, RgbImage};
use std::io::{Error, ErrorKind};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
#[cfg(feature = "benchmarking")]
use std::time::SystemTime;
#[cfg(any(feature = "simd512", feature = "simd128"))]
use packed_simd::*;

#[cfg(not(feature = "bitDiv"))]
const DIV : i32 = 10000;
#[cfg(feature = "bitDiv")]
const DIV : i32 = 0b1111111111111;
#[cfg(feature = "bitDiv")]
const BIT_DIV : i32 = 13;

const RED : i32 = (0.2126 * DIV as f32) as i32;
const GREEN : i32 = (0.7152 * DIV as f32) as i32;
const BLUE : i32 = (0.0722 * DIV as f32) as i32;

#[cfg(feature = "simd512")]
const MULT_LUMINANZ: i32x16 = i32x16::new(RED, GREEN, BLUE, RED, GREEN, BLUE, RED, GREEN, BLUE, RED, GREEN, BLUE, RED, GREEN, BLUE, 0);

#[cfg(feature = "simd128")]
const MULT_LUMINANZ: i32x4 = i32x4::new(RED, GREEN, BLUE, 0);

#[cfg(feature = "simd512")]
const I32_ZERO: i32x8 = i32x8::splat(0);

struct Tripple<'a> {
    count: usize,
    data: &'a [u8],
    len: usize
}

impl<'a> Tripple<'a> {
     fn new(d: &'a [u8]) -> Tripple<'a> {
        Tripple { count: 0, data: d, len: d.len() }
     }
}

#[cfg(any(feature = "simd512", feature = "simd128"))]
macro_rules! TrippleIterator {
    ( $element_type:ty, $vector_type:ty, $extract_type:ty, $step:expr, $size:expr ) => {
        impl<'a> Iterator for Tripple<'a> {
            type Item = $vector_type;
            fn next(&mut self) -> Option<Self::Item> {
                let start = self.count;
                let end = start + $step;
                self.count += $size;
                if end <= self.len {
                    let slice = &self.data[start..end];
                    let result = <$extract_type>::from_slice_unaligned(slice);
                    Some(result.into())
                } else if start < self.len {
                    let mut v: $vector_type = <$vector_type>::splat(0);
                    for i in 0..(self.len - start) {
                        v = v.replace(i, self.data[start + i] as $element_type);
                    }
                    Some(v)
                } else {
                    None
                }
            }
        }
    };
}

#[cfg(feature = "simd128")]
TrippleIterator!{i32, i32x4, u8x4, 4, 3 * 1}
#[cfg(feature = "simd512")]
TrippleIterator!{i32, i32x16, u8x16, 16, 3 * 5}

#[cfg(not(any(feature = "simd512", feature = "simd128")))]
impl<'a> Iterator for Tripple<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<Self::Item> {
        let start = self.count;
        self.count += 3;
        if self.count < self.len {
            Some(&self.data[start..self.count])
        } else {
            None
        }
    }
}

pub fn run(files: Vec<&str>) -> Result<f32, Error> {
    let reference = files[0];
    let to_compare = &files[1..];

    #[cfg(feature = "benchmarking")]
    let now = SystemTime::now();

    let image_reference = load_rgb8(reference)?;
    let image_size = (image_reference.width() * image_reference.height()) as f32;

    #[cfg(feature = "parallel")]
    let iter = to_compare.par_iter();

    #[cfg(not(feature = "parallel"))]
    let iter = to_compare.iter();

    let max = iter.map(
        |file| {
            let image2 = load_rgb8(file).unwrap();
            calculate_diff(&image_reference, &image2).unwrap()
        }
    ).max();

    #[cfg(feature = "benchmarking")]
    match now.elapsed() {
        Ok(elapsed) => {
            println!("{:?}", elapsed);
        }
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }

   match max {
       Some(m) => Ok(m as f32 / image_size),
       _ => Err(Error::new(ErrorKind::Other, "No calculation possible"))
   }
}

#[inline(always)]
fn calculate_diff(image1: &RgbImage, image2: &RgbImage) -> Result<i32, Error>
{
    if image1.width() != image2.width() {
        panic!("Different widths")
    }
    if image1.height() != image2.height() {
        panic!("Different heights")
    }

    // using raw container is fast as get_pixel - I need speed
    let diff : i32 = Tripple::new(image1.as_raw()).zip(Tripple::new(image2.as_raw())).map(
        |(rgb1, rgb2)| {
            #[cfg(feature = "simd128")] {
                let mult1 = MULT_LUMINANZ * rgb1;
                let add1 = mult1.wrapping_sum();
                let mult2 = MULT_LUMINANZ * rgb2;
                let add2 = mult2.wrapping_sum();
                let result = (add1 - add2).abs();
                divide_to_original(result)
            }
            #[cfg(feature = "simd512")] {
                let mult1 = MULT_LUMINANZ * rgb1;
                let add1 = add(mult1);
                let mult2 = MULT_LUMINANZ * rgb2;
                let add2 = add(mult2);
                let difference = add1 - add2;
                let abs_mask = difference.lt(I32_ZERO);
                let abs = abs_mask.select(-difference, difference);
                let result = abs.wrapping_sum();
                divide_to_original(result)
            }
            #[cfg(not(any(feature = "simd512", feature = "simd128")))] {
                let lum1 = get_luminance_value(rgb1);
                let lum2 = get_luminance_value(rgb2);
                let result = (lum1 - lum2).abs();
                divide_to_original(result)
            }
        }
    ).sum();
    Ok(diff)
}

#[inline(always)]
fn divide_to_original(value:i32) -> i32 {
    #[cfg(not(feature = "bitDiv"))] {
        value / DIV
    }
    #[cfg(feature = "bitDiv")] {
        value >> BIT_DIV
    }
}

#[cfg(feature = "simd512")]
#[inline(always)]
fn add(vec : i32x16) -> i32x8 {
    unsafe { // i32x16 have 16 lanes = access 0..=14 is save
        let x1 = i32x8::new(vec.extract_unchecked(0), vec.extract_unchecked(3), vec.extract_unchecked(6), vec.extract_unchecked(9), vec.extract_unchecked(12), 0, 0, 0);
        let x2 = i32x8::new(vec.extract_unchecked(1), vec.extract_unchecked(4), vec.extract_unchecked(7), vec.extract_unchecked(10), vec.extract_unchecked(13), 0, 0, 0);
        let x3 = i32x8::new(vec.extract_unchecked(2), vec.extract_unchecked(5), vec.extract_unchecked(8), vec.extract_unchecked(11), vec.extract_unchecked(14), 0, 0, 0);
        x1 + x2 + x3
    }
}

#[cfg(not(any(feature = "simd512", feature = "simd128")))]
#[inline(always)]
fn get_luminance_value(pix: &[u8]) -> i32 {
    // https://de.wikipedia.org/wiki/Luminanz
    // Y' = 0,RED R' + 0,GREEN G' + 0,0BLUE B'(Rec. 709)
    (pix[0] as i32) * RED + (pix[1] as i32) * GREEN + (pix[2] as i32) * BLUE
}

#[inline(always)]
fn load_rgb8(file: &str) -> Result<RgbImage, Error> {
    let img = ImageReader::open(file)?.decode();
    match img {
        Ok(i) => {
            match i {
                DynamicImage::ImageRgb8(i) => Ok(i), // should be RGB8
                _ => {
                    //println!("No rgb8 {}. Need to convert.", file);
                    Ok(i.to_rgb8())
                }
            }
        }, 
        Err(error) => panic!("Problem opening the file: {:?}", error)
    }
}
