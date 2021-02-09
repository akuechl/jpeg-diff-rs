#![crate_name = "jpeg_diff_rs"]

use image::io::Reader as ImageReader;
use image::{DynamicImage, RgbImage};
use std::io::{Error, ErrorKind};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
#[cfg(feature = "benchmarking")]
use std::time::SystemTime;

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
    let diff = Tripple::new(image1.as_raw()).zip(Tripple::new(image2.as_raw())).map(
        |(rgb1, rgb2)| {
            let lum1 = get_luminance_value(rgb1);
            let lum2 = get_luminance_value(rgb2);
            (lum1 - lum2).abs()
        }
    ).sum();
    Ok(diff)
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

#[inline(always)]
fn get_luminance_value(pix: &[u8]) -> i32 {
    // https://de.wikipedia.org/wiki/Luminanz
    // Y' = 0,2126 R' + 0,7152 G' + 0,0722 B'(Rec. 709)
    ((pix[0] as i32) * 2126 + (pix[1] as i32) * 7152 + (pix[2] as i32) * 722) / 10000
}
