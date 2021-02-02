#![crate_name = "jpeg_diff_rs"]

use image::io::Reader as ImageReader;
use image::Rgb;
use std::io::Error;

pub fn validate_files(value: String) -> Result<(), String> {
    match value.parse::<usize>() {
        Ok(_) => Ok(()),
        _ => Err(format!(r#"Value have to be a number, not "{}"."#, &value)),
    }
}

pub fn run(files: Vec<&str>) -> Result<f32, Error> {
    let reference = files[0];
    let to_compare = &files[1..];
    
    let mut max = 0f32;
    for file in to_compare {
        let diff = load_image(reference, file)?;
        let calculated_diff = diff.0 as f32 / diff.1 as f32;
        max = if max > calculated_diff { max } else { calculated_diff };
    }
    Ok(max)
}

pub fn load_image(file1: &str, file2: &str) -> Result<(u32, u32), Error>
{
    let img1 = ImageReader::open(file1)?.decode();
    let image1 = match img1 {
        Ok(i) => i.into_rgb8(),
        Err(error) => panic!("Problem opening the file: {:?}", error),
    };
    let img2 = ImageReader::open(file2)?.decode();
    let image2 = match img2 {
        Ok(i) => i.into_rgb8(),
        Err(error) => panic!("Problem opening the file: {:?}", error),
    };

    if image1.width() != image2.width() {
        panic!("Different widths")
    }
    if image1.height() != image2.height() {
        panic!("Different heights")
    }

    let mut diff = 0u32;
    for x in 0..image1.width() {
        for y in 0..image1.height() {
            let lum1 = get_luminance_value(image1.get_pixel(x, y));
            let lum2 = get_luminance_value(image2.get_pixel(x, y));
            diff += (lum1 - lum2).abs() as u32;
        }
    }

    let result = (diff, image1.width() * image1.height());
    Ok(result)
}

#[inline(always)]
fn get_luminance_value(pix: &Rgb<u8>) -> i32 {
    ((pix[0] as i32) * 299 + (pix[1] as i32) * 587 + (pix[2] as i32) * 114) / 1000
}
