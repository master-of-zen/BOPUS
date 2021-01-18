
extern crate execute;
use clap::{App, Arg};
use std::fs;
use std::process::Command;
use execute::Execute;

fn main() {
    let _matches = App::new("My Super Program")
                            .version("0.1")
                            .author("Zen <true.grenight@gmail.com>>")
                            .about("Opus bitrate optimizer")
                            .arg(Arg::with_name("INPUT")
                                .short("i")
                                .long("input")
                                .value_name("INPUT")
                                .help("Sets the input file to use")
                                .required(true)
                                .takes_value(true))
                            .arg(Arg::with_name("TARGET")
                                .short("t")
                                .long("target")
                                .help("Sets value of quality to target")
                                .takes_value(true))
                            .get_matches();

    fs::create_dir_all("temp").ok().expect("Can't create a temp folder");
    let input_file: &str = _matches.value_of("INPUT").unwrap();
    let target_quality = _matches.value_of("TARGET").unwrap_or("4.3");

    println!("Using input file {}", input_file);
    println!("Using target quality {}", target_quality);


    // Making wav
    let mut cmd = Command::new("ffmpeg");
    cmd.args(&["-i", input_file, "-ar", "48000", "temp/ref.wav"]);
    println!("{:?}", cmd);
    if let Some(exit_code) = cmd.execute().unwrap() {
        if exit_code == 0{println!("Converted to wav");}
        else
        {
            eprintln!("Failed");
            println!("{:?}", cmd.output().unwrap())
        }
    }

    else {eprintln!("Interupted")}


}

