extern crate execute;
extern crate num_cpus;
extern crate regex;
extern crate threadpool;
use clap::{App, Arg};
use rayon::prelude::*;
use regex::Regex;
use std::cmp;
use std::env;
use std::fs;
use std::fs::DirEntry;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn main() {
    // check if executables exist
    if !is_program_in_path("ffmpeg") {
        println!("No.");
    }

    if !is_program_in_path("visqol") {
        println!("No.");
    }

    // arg parsing
    let _matches = App::new("Bopus")
        .version("0.1")
        .author("Zen <true.grenight@gmail.com>>")
        .about("Opus bitrate optimizer")
        .arg(
            Arg::with_name("INPUT")
                .short("i")
                .long("input")
                .value_name("INPUT")
                .help("Sets the input file to use")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("TARGET")
                .short("t")
                .long("target")
                .help("Sets value of quality to target")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("JOBS")
                .short("j")
                .long("jobs")
                .help("set amount of jobs to run")
                .takes_value(true),
        )
        .get_matches();

    // Create all required temp dirs
    create_all_dir();

    let input_file: &str = _matches.value_of("INPUT").unwrap();
    let target_quality: f32 = _matches.value_of("TARGET").unwrap_or("4").parse().unwrap();

    let cpu_cores: i32 = (num_cpus::get() / 2) as i32;
    let mut jobs: i32 = _matches
        .value_of("JOBS")
        .unwrap_or(&format!("{}", cpu_cores))
        .parse()
        .unwrap();

    // printing some stuff
    println!(":: Using input file {}", input_file);
    println!(":: Using target quality {}", target_quality);

    // making wav and segmenting
    let wav_segments = segment(input_file);
    let mut it = vec![];
    for seg in wav_segments {
        it.push(seg.unwrap())
    }
    jobs = cmp::min(jobs, it.len() as i32);

    println!(":: Segments {}", it.len());
    println!(":: Running {} jobs", jobs);

    rayon::ThreadPoolBuilder::new()
        .num_threads(jobs as usize)
        .build_global()
        .unwrap();

    it.par_iter().for_each(move |x| optimize(x, target_quality));

    concatenate(input_file);
}

fn create_all_dir() {
    // creating temp dir/removing old
    let temp_path = Path::new("temp");
    let segments = Path::new("temp/segments");
    let probes = Path::new("temp/probes");
    let conc = Path::new("temp/conc");

    if temp_path.exists() {
        fs::remove_dir_all(temp_path).expect("Can't remove temp folder");
        fs::create_dir_all("temp").expect("Can't create a temp folder");
    } else {
        fs::create_dir_all("temp").expect("Can't create a temp folder");
    }

    if segments.exists() {
        fs::remove_dir_all(segments).expect("Can't remove segments folder");
        fs::create_dir_all("temp/segments").expect("Can't create segments folder");
    } else {
        fs::create_dir_all("temp/segments").expect("Can't create segments folder");
    }

    if probes.exists() {
        fs::remove_dir_all(probes).expect("Can't remove probes folder");
        fs::create_dir_all(probes).expect("Can't create probes folder");
    } else {
        fs::create_dir_all(probes).expect("Can't create probes folder");
    }

    if conc.exists() {
        fs::remove_dir_all(conc).expect("Can't remove conc folder");
        fs::create_dir_all(conc).expect("Can't create a conc folder");
    } else {
        fs::create_dir_all(conc).expect("Can't create a conc folder");
    }
}

fn concatenate(output: &str) {
    println!(":: Concatenating");
    let conc_file = Path::new("temp/concat.txt");
    let conc_folder = Path::new("temp/conc");
    let fl = fs::read_dir(conc_folder).unwrap();

    let mut txt = String::new();
    let mut t = Vec::new();

    for seg in fl {
        t.push(seg.unwrap());
    }
    t.sort_by_key(|k| k.path());

    for p in t {
        let st = format!("file 'conc/{}' \n", p.file_name().to_str().unwrap());
        txt.push_str(&st);
    }
    let pt = Path::new(output).with_extension("opus");
    let out: &str = pt.to_str().unwrap();

    let mut file = File::create(conc_file).unwrap();
    file.write_all(txt.as_bytes()).unwrap();

    let mut cmd = Command::new("ffmpeg");
    cmd.args(&[
        "-y",
        "-safe",
        "0",
        "-f",
        "concat",
        "-i",
        conc_file.to_str().unwrap(),
        "-c",
        "copy",
        out,
    ]);
    //println!("{:?}", cmd);
    cmd.output().expect("Failed to concatenate");
}

fn optimize(file: &DirEntry, target_quality: f32) {
    // get metric score
    let mut bitrate: u32 = 96;
    let mut count: u32 = 0;
    let mut score: f32 = 0.0;
    let path = file.path();
    let stem = path.file_stem().unwrap().to_str().unwrap();
    let file_str: &str = path.to_str().unwrap();
    let mut bitrates: Vec<(u32, f32)> = vec![];
    // bitrate | score

    // Search loop
    loop {
        count += 1;

        if count > 8 {
            println!(
                ":: # {} Exceed {} probes, Found B: {}, Score {:.2}",
                stem, count, bitrate, score
            );
            break;
        }
        let pf = file.path();
        score = make_probe(pf, bitrate);
        score = trasnform_score(score);
        bitrates.push((bitrate, score));

        let dif: f32 = (score - target_quality).abs();

        if dif < 0.3 {
            println!(":: # {} Found B: {}, Score {:.2}", stem, bitrate, score);
            break;
        }

        // println!(":: # {} Probe: {} B: {}, Score: {:.2}", stem, count, bitrate, score);

        bitrate = ((target_quality / score) * (bitrate as f32)) as u32;
        // println!(":: New: {}", bitrate)
    }
    println!(":: # {} Found B: {}, Score {:.2}", stem, bitrate, score);

    let mut cmd = Command::new("ffmpeg");
    cmd.args(&[
        "-y",
        "-i",
        file_str,
        "-c:a",
        "libopus",
        "-b:a",
        &format!("{}K", &bitrate.to_string()),
        &format!("temp/conc/{}.opus", stem),
    ]);
    cmd.output().unwrap();
}

fn segment(input: &str) -> Vec<std::result::Result<DirEntry, std::io::Error>> {
    let segments = Path::new("temp/segments");
    let mut cmd = Command::new("ffmpeg");
    cmd.args(&[
        "-y",
        "-i",
        input,
        "-ar",
        "48000",
        "-f",
        "segment",
        "-segment_time",
        "12",
        "temp/segments/%05d.wav",
    ]);
    cmd.output().unwrap();

    let mut vc = vec![];
    let files = fs::read_dir(&segments).unwrap();

    vc.extend(files);
    vc
}

/// Transform score for easier score comprehension and usage
/// Scaled 4.0 - 4.75 range to 0.0 - 5.0
fn trasnform_score(score: f32) -> f32 {
    if score < 4.1 {
        return 1.0f32;
    }
    let scale_value = 5.0 / (4.75 - 4.0);
    let new_score: f32 = (score - 4.1) * scale_value;
    //println!("Score in {}, Score out{}", score, new_score);
    new_score
}

fn make_probe(fl: PathBuf, bitrate: u32) -> f32 {
    let file_str: &str = fl.to_str().unwrap();
    // Audio to opus
    let probe_name = fl.file_stem().unwrap().to_str().unwrap();

    let mut cmd = Command::new("ffmpeg");
    cmd.args(&[
        "-y",
        "-i",
        file_str,
        "-c:a",
        "libopus",
        "-b:a",
        &format!("{}K", &bitrate.to_string()),
        &format!("temp/probes/{}{}.opus", probe_name, bitrate),
    ]);
    cmd.output().unwrap();

    // Audio to wav
    let mut cmd = Command::new("ffmpeg");
    cmd.args(&[
        "-y",
        "-i",
        &format!("temp/probes/{}{}.opus", probe_name, bitrate),
        "-ar",
        "48000",
        &format!("temp/probes/{}{}.wav", probe_name, bitrate),
    ]);
    cmd.output().expect("can't  convert opus");

    // calculating score
    let mut cmd = Command::new("visqol");
    cmd.args(&[
        "--reference_file",
        file_str,
        "--degraded_file",
        &format!("temp/probes/{}{}.wav", probe_name, bitrate),
    ]);

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let output = cmd.output().unwrap();
    let re = Regex::new(r"([0-9]*\.[0-9]*)").unwrap();
    let score_str: &str = &String::from_utf8(output.stdout).unwrap();

    let caps = re.captures(&score_str).unwrap();
    let str_score: &str = caps.get(1).map_or("", |m| m.as_str());
    let viqol_score: f32 = str_score.parse::<f32>().unwrap();
    viqol_score
}

fn is_program_in_path(program: &str) -> bool {
    // Check is program in path
    if let Ok(path) = env::var("PATH") {
        for p in path.split(':') {
            let p_str = format!("{}/{}", p, program);
            if fs::metadata(p_str).is_ok() {
                return true;
            }
        }
    }
    false
}
