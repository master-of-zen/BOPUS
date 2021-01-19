#![feature(type_ascription)]
extern crate execute;
extern crate regex;
use std::env;
use clap::{App, Arg};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use execute::Execute;
use regex::Regex;


fn main() {

    // check is executables exist
    if is_program_in_path("ffmpeg"){}
    else {println!("No.");}

    if is_program_in_path("visqol"){}
    else {println!("No.");}

    // arg parsing
    let _matches = App::new("Bopus")
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

    // creating temp dir/removing old
    let temp_path = Path::new("temp");

    if temp_path.exists(){
        fs::remove_dir_all(temp_path).ok().expect("Can't remove temp folder");
        fs::create_dir_all("temp").ok().expect("Can't create a temp folder");
    }else {
        fs::create_dir_all("temp").ok().expect("Can't create a temp folder");
    }

    let input_file: &str = _matches.value_of("INPUT").unwrap();
    let target_quality: f32 = _matches.value_of("TARGET").unwrap_or("4.3").parse().unwrap();

    // printing some stuff
    println!(":: Using input file {}", input_file);
    println!(":: Using target quality {}", target_quality);


    // making wav
    make_wav(input_file: &str);


    // get metric score
    let mut bitrate: u32 = 96;
    let mut count: u32 = 0;

    let mut bitrates: Vec<(u32, f32)> = vec![];
    // bitrate | score


    // Search loop
    loop {
        count += 1;

        if count > 6{
            println!(":: Get more than {}, ending comparison", count );
            break
        }
        let score:f32 = make_probe(bitrate);

        println!(":: Try: {} Bitrate: {}, Score: {}", count, bitrate, score);
        break
    }
}

fn make_probe(bitrate:u32) -> f32{

    // Audio to opus
    let mut cmd = Command::new("ffmpeg");
    cmd.args(&["-y", "-i", "temp/ref.wav", "-c:a","libopus", "-b:a", &format!("{}K", &bitrate.to_string()), &format!("temp/{}.opus", bitrate) ]);
    cmd.execute().unwrap();

    // Audio to wav
    let mut cmd = Command::new("ffmpeg");
    cmd.args(&["-y", "-i", &format!("temp/{}.opus", bitrate), "-ar", "48000", &format!("temp/{}.wav", bitrate)]);
    cmd.output().expect("can't  convert opus");

    // calculating score
    let mut cmd = Command::new("visqol");
    cmd.args(&["--reference_file", "temp/ref.wav", "--degraded_file", &format!["temp/{}.wav", bitrate]]);

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let output = cmd.execute_output().unwrap();
    let re = Regex::new(r"([0-9]*\.[0-9]*)": &str).unwrap();
    let score_str: &str = &String::from_utf8(output.stdout).unwrap();

    let caps= re.captures(&score_str).unwrap();
    let str_score: &str = caps.get(1).map_or("", |m| m.as_str());
    let viqol_score:f32 = str_score.parse::<f32>().unwrap();
    viqol_score
}

fn is_program_in_path(program: &str) -> bool {
    // Check is program in path
    if let Ok(path) = env::var("PATH") {
        for p in path.split(":") {
            let p_str = format!("{}/{}", p, program);
            if fs::metadata(p_str).is_ok() {
                return true;
            }
        }
    }
    false
}

fn make_wav(input: &str){
        // Making wav
        let mut cmd = Command::new("ffmpeg");
        cmd.args(&["-y", "-i", input, "-ar", "48000", "temp/ref.wav"]);
        if let Some(exit_code) = cmd.execute().unwrap() {
            if exit_code == 0{}
            else
            {
                eprintln!("Failed");
                println!("{:?}", cmd.output().unwrap())
            }
        }
        else {eprintln!("Interupted")}
}

fn encode_audio(bitrate:u32){
    // final encode of the audio
    let mut cmd = Command::new("ffmpeg");
    cmd.args(&[ "-y", "-i", "temp/ref.wav", "-c:a","libopus", "-b:a", &format!("{}K", &bitrate.to_string()), &format!("temp/{}", bitrate) ]);

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


