use std::cmp;
use std::fs::{self, DirEntry, File};
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use regex::Regex;
use structopt::StructOpt;

/// Opus bitrate optimizer
#[derive(StructOpt, Debug)]
#[structopt(author)]
struct Args {
    /// Input file to use
    #[structopt(short, long)]
    input: PathBuf,

    /// Value of quality to target
    #[structopt(short, long = "target", default_value = "4.0")]
    target_quality: f32,

    /// Number of jobs to run
    #[structopt(short, long)]
    jobs: Option<usize>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::from_args();

    // check if executables exist after getting CLI args
    if !is_program_in_path("ffmpeg") {
        println!("FFmpeg is not installed or in PATH, required for encoding audio");
        return Ok(());
    }

    if !is_program_in_path("visqol") {
        println!("visqol is not installed or in PATH, required for perceptual quality metrics");
        return Ok(());
    }

    // Create all required temp dirs
    create_all_dirs()?;

    println!(":: Using input file {:?}", args.input);
    println!(":: Using target quality {}", args.target_quality);

    // making wav and segmenting
    let wav_segments = segment(&args.input);
    let mut it = vec![];

    for seg in wav_segments {
        it.push(seg?)
    }

    let jobs = cmp::min(args.jobs.unwrap_or_else(|| num_cpus::get() / 2), it.len());

    println!(":: Segments {}", it.len());
    println!(":: Running {} jobs", jobs);

    rayon::ThreadPoolBuilder::new()
        .num_threads(jobs as usize)
        .build_global()?;

    // it.par_iter().for_each(|x| optimize(x, args.target_quality));
    it.iter().for_each(|x| optimize(x, args.target_quality));

    concatenate(&args.input)?;

    Ok(())
}

macro_rules! remove_and_create {
    ($file:expr) => {{
        let file = Path::new($file);
        if file.exists() {
            fs::remove_dir_all(file)?;
        }
        fs::create_dir_all(file)?;
    }};
}

fn create_all_dirs() -> anyhow::Result<()> {
    // creating temp dir/removing old ones if they exist
    remove_and_create!("temp");
    remove_and_create!("temp/segments");
    remove_and_create!("temp/probes");
    remove_and_create!("temp/conc");

    Ok(())
}

fn concatenate(output: &Path) -> anyhow::Result<()> {
    println!(":: Concatenating");
    let conc_file = Path::new("temp/concat.txt");
    let conc_folder = Path::new("temp/conc");
    let fl = fs::read_dir(conc_folder)?;

    let mut txt = String::new();
    let mut t = Vec::new();

    for seg in fl {
        t.push(seg?);
    }
    t.sort_by_key(|k| k.path());

    for p in t {
        let st = format!("file 'conc/{}' \n", p.file_name().to_str().unwrap());
        txt.push_str(&st);
    }
    let pt = Path::new(output).with_extension("opus");

    let out = pt.to_string_lossy();

    let mut file = File::create(conc_file)?;
    file.write_all(txt.as_bytes())?;

    let mut cmd = Command::new("ffmpeg");
    cmd.args(&[
        "-y",
        "-safe",
        "0",
        "-f",
        "concat",
        "-i",
        // cannot fail, conc_file has a valid UTF-8 filename
        conc_file.to_str().unwrap(),
        "-c",
        "copy",
        &out,
    ]);

    cmd.output()?;

    Ok(())
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
        score = transform_score(score);
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
        &format!("{}K", bitrate),
        &format!("temp/conc/{}.opus", stem),
    ]);
    cmd.output().unwrap();
}

fn segment(input: &Path) -> Vec<Result<DirEntry, std::io::Error>> {
    let segments = Path::new("temp/segments");
    let mut cmd = Command::new("ffmpeg");
    cmd.args(&[
        "-y",
        "-i",
        // TODO check if filename can be written as a UTF-8 string, or escape
        // the chars when passing to ffmpeg when it isn't
        input.to_str().expect("Filename is not valid UTF-8"),
        "-ar",
        "48000",
        "-f",
        "segment",
        "-segment_time",
        "12",
        "temp/segments/%05d.wav",
    ]);
    cmd.output().unwrap();

    fs::read_dir(&segments).unwrap().collect()
}

/// Transform score for easier score comprehension and usage
/// Scaled 4.0 - 4.75 range to 0.0 - 5.0
fn transform_score(score: f32) -> f32 {
    const SCALE_VALUE: f32 = 5.0 / (4.75 - 4.0);

    if score < 4.1 {
        1.0f32
    } else {
        (score - 4.1) * SCALE_VALUE
    }
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
        &format!("{}K", bitrate),
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
    let score_str = String::from_utf8(output.stdout).unwrap();

    let caps = dbg!(re.captures(dbg!(&score_str))).unwrap();
    let score: &str = caps.get(1).map_or("", |m| m.as_str());

    score.parse().unwrap()
}

fn is_program_in_path(program: &str) -> bool {
    which::which(program).is_ok()
}
