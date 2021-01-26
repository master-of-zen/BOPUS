#![feature(type_ascription)]
extern crate execute;
extern crate regex;
extern crate num_cpus;
extern crate threadpool;
use std::env;
use clap::{App, Arg};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use execute::Execute;
use regex::Regex;
use std::fs::DirEntry;
use std::path::PathBuf;
use std::cmp;
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;





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
        .arg(Arg::with_name("JOBS")
            .short("j")
            .long("jobs")
            .help("set amount of jobs to run")
            .takes_value(true))
        .get_matches();


    // Create all required temp dirs
    create_all_dir();

    let input_file: &str = _matches.value_of("INPUT").unwrap();
    let target_quality: f32 = _matches.value_of("TARGET").unwrap_or("4").parse().unwrap();

    let cpu_cores: i32 = (num_cpus::get() / 2) as i32;
    let mut jobs: i32 = _matches.value_of("JOBS").unwrap_or(&format!("{}", cpu_cores)).parse().unwrap();

    // printing some stuff
    println!(":: Using input file {}", input_file);
    println!(":: Using target quality {}", target_quality);


    // making wav and segmenting
    let wav_segments = segment(input_file: &str);
    let mut it = vec!();
    for seg in wav_segments{
        it.push(seg.unwrap())
    }
    jobs = cmp::min(jobs, it.len() as i32);

    println!(":: Segments {}", it.len());
    println!(":: Running {} jobs", jobs);


    rayon::ThreadPoolBuilder::new().num_threads(jobs as usize).build_global().unwrap();
    let f1 = it.par_iter();
    f1.for_each(move |x| {optimize( x, target_quality)});

    //let it = wav_segments.iter().par_iter().for_each(move || {optimize(seg.unwrap(), target_quality)});


    //for_each(move || {optimize(seg.unwrap(), target_quality)})

    //for seg in wav_segments{
    //    pool.install(move || {optimize(seg.unwrap(), target_quality)});
    //}



}

fn create_all_dir(){
    // creating temp dir/removing old
    let temp_path = Path::new("temp");
    let segments = Path::new("temp/segments");
    let probes = Path::new("temp/probes");
    let conc = Path::new("temp/conc");

    if temp_path.exists(){
        fs::remove_dir_all(temp_path).ok().expect("Can't remove temp folder");
        fs::create_dir_all("temp").ok().expect("Can't create a temp folder");
    }else{
        fs::create_dir_all("temp").ok().expect("Can't create a temp folder");
    }

    if segments.exists(){
        fs::remove_dir_all(segments).ok().expect("Can't remove segments folder");
        fs::create_dir_all("temp/segments").ok().expect("Can't create segments folder");
    }else{
        fs::create_dir_all("temp/segments").ok().expect("Can't create segments folder");
    }

    if probes.exists(){
        fs::remove_dir_all(probes).ok().expect("Can't remove probes folder");
        fs::create_dir_all(probes).ok().expect("Can't create probes folder");
    }else{
        fs::create_dir_all(probes).ok().expect("Can't create probes folder");
    }

    if conc.exists(){
        fs::remove_dir_all(conc).ok().expect("Can't remove conc folder");
        fs::create_dir_all(conc).ok().expect("Can't create a conc folder");
    }else{
        fs::create_dir_all(conc).ok().expect("Can't create a conc folder");
    }
}


fn concatenate(){

}

fn optimize(file: &DirEntry, target_quality: f32){

    // get metric score
    let mut bitrate: u32 = 96;
    let mut count: u32 = 0;
    let mut score: f32;
    let path = file.path();
    let stem = path.file_stem().unwrap().to_str().unwrap();
    let file_str: &str = path.to_str().unwrap();
    let mut bitrates: Vec<(u32, f32)> = vec![];
    // bitrate | score

    // Search loop
    loop {
        count += 1;

        if count > 4{
            println!(":: Get more than {}, ending comparison", count );
            break
        }
        let pf = file.path();
        score  = make_probe(pf, bitrate);
        score  = trasnform_score(score: f32);
        bitrates.push((bitrate, score));

        println!(":: Segment {} Try: {} Bitrate: {}, Score: {}", stem, count, bitrate, score);

        bitrate = ((target_quality / score) * (bitrate as f32)) as u32;
        println!(":: New bitrate: {}", bitrate)


    }
    println!("Encoding end result with {} bitrate", bitrate );

    let mut cmd = Command::new("ffmpeg");
    cmd.args(&[ "-y", "-i", file_str, "-c:a","libopus", "-b:a", &format!("{}K", &bitrate.to_string()), &format!("temp/conc/{}.opus", stem) ]);
    cmd.output().unwrap();

}


fn segment(input: &str) -> Vec<std::result::Result<DirEntry, std::io::Error>>{

    let segments = Path::new("temp/segments");
    let mut cmd = Command::new("ffmpeg");
    cmd.args(&["-y", "-i", input, "-ar", "48000", "-f", "segment", "-segment_time", "10", "temp/segments/%05d.wav"]);
    cmd.output().unwrap();

    let mut vc = vec!();
    let files = fs::read_dir(&segments).unwrap();

    vc.extend(files);
    return vc;

}


/// Transform score for easier score comprehension and usage
/// Scaled 4.0 - 4.75 range to 0.0 - 5.0
fn trasnform_score(score:f32) -> f32{
    if score < 4.1{
        return 1.0f32;
    }
    let scale_value = 5.0 / (4.75 - 4.0);
    let new_score:f32 = (score - 4.0) * scale_value;
    new_score
}


fn make_probe(fl: PathBuf ,bitrate:u32) -> f32{

    let file_str: &str = fl.to_str().unwrap();
    // Audio to opus
    let probe_name = fl.file_stem().unwrap().to_str().unwrap();

    let mut cmd = Command::new("ffmpeg");
    cmd.args(&["-y", "-i", file_str, "-c:a","libopus", "-b:a", &format!("{}K", &bitrate.to_string()), &format!("temp/probes/{}{}.opus", probe_name, bitrate) ]);
    cmd.output().unwrap();

    // Audio to wav
    let mut cmd = Command::new("ffmpeg");
    cmd.args(&["-y", "-i", &format!("temp/probes/{}{}.opus", probe_name, bitrate), "-ar", "48000", &format!("temp/probes/{}{}.wav", probe_name, bitrate)]);
    cmd.output().expect("can't  convert opus");

    // calculating score
    let mut cmd = Command::new("visqol");
    cmd.args(&["--reference_file", file_str, "--degraded_file", &format!("temp/probes/{}{}.wav", probe_name, bitrate)]);

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let output = cmd.output().unwrap();
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
