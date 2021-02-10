#![crate_name = "jpeg_diff_rs"]

use image::io::Reader as ImageReader;
use image::{DynamicImage, RgbImage};
use std::io::{Error, ErrorKind};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
#[cfg(feature = "benchmarking")]
use std::time::SystemTime;
#[cfg(feature = "simd512")]
use packed_simd::*;

#[cfg(not(feature = "bitDiv"))]
static DIV : i32 = 10000;
#[cfg(feature = "bitDiv")]
static DIV : i32 = 0b1111111111111;
#[cfg(feature = "bitDiv")]
static BIT_DIV : i32 = 13;

static RED : i32 = (0.2126 * DIV as f32) as i32;
static GREEN : i32 = (0.7152 * DIV as f32) as i32;
static BLUE : i32 = (0.0722 * DIV as f32) as i32;

#[cfg(feature = "simd512")]
static MULT_LUMINANZ: i32x16 = i32x16::new(RED, GREEN, BLUE, RED, GREEN, BLUE, RED, GREEN, BLUE, RED, GREEN, BLUE, RED, GREEN, BLUE, 0);

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

#[cfg(feature = "simd512")]
impl<'a> Iterator for Tripple<'a> {
    type Item = i32x16;
    fn next(&mut self) -> Option<Self::Item> {
        let start = self.count;
        let end = start + 16;
        self.count += 15;
        if end < self.len {
            Some(u8x16::from_slice_unaligned(&self.data[start..end]).into())
        } else if start < self.len {
            let mut v: i32x16 = i32x16::splat(0);
            for i in 0..(self.len-start) {
                v = v.replace(i, self.data[start + i] as i32);
            }
            Some(v)
        } else {
            None
        }
    }
}

#[cfg(not(feature = "simd512"))]
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
            #[cfg(feature = "simd512")] {
                let mult1 = MULT_LUMINANZ * rgb1;
                let add1 = add(mult1);
                let mult2 = MULT_LUMINANZ * rgb2;
                let add2 = add(mult2);
                let difference = add1 - add2;
                let abs_mask = difference.lt(i32x8::splat(0));
                let abs = abs_mask.select(-difference, difference);
                let result = abs.wrapping_sum();
                #[cfg(not(feature = "bitDiv"))] {
                    result / DIV
                }
                #[cfg(feature = "bitDiv")] {
                    result >> BIT_DIV
                }
            }
            #[cfg(not(feature = "simd512"))] {
                let lum1 = get_luminance_value(rgb1);
                let lum2 = get_luminance_value(rgb2);
                let result = (lum1 - lum2).abs();
                #[cfg(not(feature = "bitDiv"))] {
                    result / DIV
                }
                #[cfg(feature = "bitDiv")] {
                    result >> BIT_DIV
                }
            }

        }
    ).sum();
    Ok(diff)
}

#[cfg(feature = "simd512")]
#[inline(always)]
fn add(mult1 : i32x16) -> i32x8{
    i32x8::new(
        mult1.extract(0) + mult1.extract(1) + mult1.extract(2),
        mult1.extract(3) + mult1.extract(4) + mult1.extract(5),
        mult1.extract(6) + mult1.extract(7) + mult1.extract(8),
        mult1.extract(9) + mult1.extract(10) + mult1.extract(11),
        mult1.extract(12) + mult1.extract(13) + mult1.extract(14),
    0, 0, 0)    
}

#[cfg(not(feature = "simd512"))]
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
