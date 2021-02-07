#![crate_name = "jpeg_diff_rs"]

use image::io::Reader as ImageReader;
use image::{DynamicImage, RgbImage};
use std::io::Error;
//use std::time::{SystemTime};

pub fn run(files: Vec<&str>) -> Result<f32, Error> {
    let reference = files[0];
    let to_compare = &files[1..];
//   let now = SystemTime::now();

    let image_reference = load_rgb8(reference)?;
    let mut max = 0f32;
    for file in to_compare {
        let image2 = load_rgb8(file)?;
        let diff = calculate_diff(&image_reference, &image2)?;
        let calculated_diff = diff.0 as f32 / diff.1 as f32;
        max = if max > calculated_diff { max } else { calculated_diff };
    }
/*
    match now.elapsed() {
        Ok(elapsed) => {
            println!("{:?}", elapsed);
        }
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }
*/
    Ok(max)
}


fn calculate_diff(image1: &RgbImage, image2: &RgbImage) -> Result<(u32, u32), Error>
{
    if image1.width() != image2.width() {
        panic!("Different widths")
    }
    if image1.height() != image2.height() {
        panic!("Different heights")
    }

    let mut diff = 0u32;
    
    // using raw container is fast as get_pixel - I need speed
    let container1 = image1.as_raw();
    let container2 = image2.as_raw();
    let mut i = 0;
    while i < container1.len() {
        let i3 = i + 3;
        let rgb1 = &container1[i..i3];
        let lum1 = get_luminance_value(rgb1);
        let rgb2 = &container2[i..i3];
        let lum2 = get_luminance_value(rgb2);
        diff += (lum1 - lum2).abs() as u32;
        i = i3;
    }

    let result = (diff, image1.width() * image1.height());
    Ok(result)
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
