use jpeg_diff_rs::{run};
extern crate clap;
use clap::{App, Arg};

fn main() {
    let version: Option<&'static str> = option_env!("CARGO_PKG_VERSION");
    let authors: Option<&'static str> = option_env!("CARGO_PKG_AUTHORS");
    let matches = App::new("JPEG Diff Calculator")
        .version(version.unwrap())
        .author(authors.unwrap())
        .about("Calculate a diff between jpeg images")
        .arg(
            Arg::with_name("data")
                .help("the files to calculate")
                .required(true)
                .multiple(true)
        ).get_matches();

        let files = matches.values_of("data").unwrap().collect::<Vec<_>>();

        match run(files) {
            Ok(val) => println!("{}", &val),
            Err(x) => panic!("Error {:?}", x)
        }
}