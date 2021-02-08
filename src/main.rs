use std::cmp;
use std::ffi::OsStr;
use std::fs::{self, DirEntry, File};
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::*;

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use regex::Regex;
use structopt::StructOpt;

use lazy_static::lazy_static;

mod util;

use crate::util::{get_audio_time, transform_score};

#[macro_use]
extern crate log;

use simplelog::{ConfigBuilder, LevelFilter, LevelPadding, TermLogger, TerminalMode};

const SUPPORTED_AUDIO_FORMAT_EXTENSIONS: &[&str] =
    &["flac", "wav", "opus", "ogg", "m4a", "aac", "mp3"];
/// Opus bitrate optimizer
#[derive(StructOpt, Debug)]
#[structopt(author)]
struct Args {
    /// Input file to use
    #[structopt(parse(from_os_str), short, long)]
    input: PathBuf,

    /// Value of quality to target
    #[structopt(short, long = "target", default_value = "4.0")]
    target_quality: f32,

    /// Number of jobs to run
    #[structopt(short, long)]
    jobs: Option<usize>,

    /// Log level (possible values: OFF, ERROR, WARN, INFO, DEBUG, TRACE)
    #[structopt(short, long = "log", default_value = "INFO")]
    log_level: LevelFilter,

    /// Model to use for visqol calculations. If not specified, the default model is used
    #[structopt(parse(from_os_str), short, long)]
    model: Option<PathBuf>,

    /// Keep temporary folder
    #[structopt(long)]
    keep: bool,

    /// Output file
    #[structopt(name = "OUTPUT", parse(from_os_str))]
    output: PathBuf,
}

fn is_program_in_path(program: &str) -> bool {
    which::which(program).is_ok()
}

fn main() -> Result<()> {
    let args = Args::from_args();

    TermLogger::init(
        args.log_level,
        ConfigBuilder::new()
            .set_level_padding(LevelPadding::Left)
            .set_time_level(LevelFilter::Off)
            .build(),
        TerminalMode::Mixed,
    )?;

    // check if executables exist after getting CLI args
    if !is_program_in_path("ffmpeg") {
        error!("FFmpeg is not installed or in PATH, required for encoding audio");
        return Ok(());
    }

    // uncomment this if get_audio_time function is used
    // if !is_program_in_path("ffprobe") {
    //     error!("ffprobe is not installed or in PATH, required for getting metadata from audio files");
    //     return Ok(());
    // }

    if !is_program_in_path("visqol") {
        error!("visqol is not installed or in PATH, required for perceptual quality metrics");
        return Ok(());
    }

    if !args.input.exists() {
        error!("The file {:?} does not exist", args.input);
        return Ok(());
    }

    match args.input.extension() {
        Some(ext) => match ext.to_str() {
            Some(ext) => {
                if !SUPPORTED_AUDIO_FORMAT_EXTENSIONS.contains(&ext) {
                    error!("Unsupported file (unknown extension '{}')", ext);
                    info!(
                        "Supported file extensions: {:?}",
                        SUPPORTED_AUDIO_FORMAT_EXTENSIONS
                    );
                    return Ok(());
                }
            }
            None => {
                error!("Unsupported file (extension {:?} is invalid UTF-8)", ext);
                return Ok(());
            }
        },
        _ => {
            error!("Unsupported file (no file extension)");
            return Ok(());
        }
    }

    // Create all required temp dirs
    create_all_dirs()?;

    let model: &Path = match args.model {
        Some(ref model) => model.as_path(),
        None => create_model()?,
    };

    info!("Input file: {:?}", args.input);
    info!("Target quality: {:.2}", args.target_quality);

    let it = segment(&args.input)?;

    let jobs = cmp::min(args.jobs.unwrap_or_else(|| num_cpus::get() / 2), it.len());

    info!("Segments: {}", it.len());
    info!("Running {} jobs", jobs);

    rayon::ThreadPoolBuilder::new()
        .num_threads(jobs as usize)
        .build_global()?;

    it.par_iter()
        .map(|x| optimize(x, args.target_quality, model))
        .collect::<Result<_, _>>()?;

    concatenate(&args.output)?;

    if !args.keep {
        fs::remove_dir_all("temp")?;
    }

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

fn create_all_dirs() -> Result<()> {
    // creating temp dir/removing old ones if they exist
    remove_and_create!("temp");
    remove_and_create!("temp/segments");
    remove_and_create!("temp/probes");
    remove_and_create!("temp/conc");

    Ok(())
}

fn create_model<'a>() -> Result<&'a Path> {
    // Writes included model to temp folder
    const MODEL: &[u8] = include_bytes!("models/visqol_model.txt");
    let model_file = Path::new("temp/model.txt");
    let mut file = File::create(model_file)?;
    file.write_all(MODEL)?;
    Ok(model_file)
}

fn concatenate(output: &Path) -> Result<()> {
    // FIXME: Fix audio silence on concat
    info!("Concatenating");
    let conc_file = Path::new("temp/concat.txt");
    let conc_folder = Path::new("temp/conc");
    let fl = fs::read_dir(conc_folder)?;

    let mut t: Vec<DirEntry> = fl.into_iter().collect::<Result<Vec<_>, _>>()?;
    t.sort_by_key(|k| k.path());

    // save some allocations by giving initial buffer with a capacity close to what it would actually be
    let mut txt: Vec<u8> = Vec::with_capacity(t.len() * (b"file 'conc/'\n".len() + 8));
    for p in t {
        // FIXME if the filename contains the ' character, does ffmpeg properly handle it?
        txt.extend_from_slice(b"file 'conc/");
        txt.extend_from_slice({
            #[cfg(unix)]
            {
                use std::os::unix::prelude::OsStrExt;
                &p.file_name().as_bytes()
            }
            #[cfg(not(unix))]
            {
                &p.file_name()
                    .into_string()
                    .expect("non UTF-8 filenames are not supported on non-Unix platforms")
                    .as_bytes()
            }
        });
        txt.extend_from_slice(b"'\n");
    }

    let mut file = File::create(conc_file)?;
    file.write_all(txt.as_slice())?;

    let mut cmd = Command::new("ffmpeg");
    cmd.args(&["-y", "-safe", "0", "-f", "concat", "-i"]);
    cmd.arg(conc_file);
    cmd.args(&["-c", "copy"]);
    cmd.arg(output);

    cmd.output()?;

    Ok(())
}

fn optimize(file: &DirEntry, target_quality: f32, model: &Path) -> Result<()> {
    const TOLERANCE: f32 = 0.2;

    // get metric score
    let mut bitrate: u32 = 96000;
    let mut count: usize = 0;
    let mut score: f32 = 0.0;
    let path = file.path();
    let stem: &OsStr = path.file_stem().unwrap_or_else(|| path.as_os_str());
    let mut bitrates: Vec<(u32, f32)> = vec![];
    // bitrate | score

    // Search loop
    loop {
        count += 1;

        if count > 8 {
            info!(
                "# {:?} Exceed {} probes, Found B: {}, Score {:.2}",
                stem, count, bitrate, score
            );
            break;
        }

        let pf = file.path();
        score = transform_score(make_probe(&pf, bitrate, model)?);
        bitrates.push((bitrate, score));

        let dif: f32 = (score - target_quality).abs();

        if dif < TOLERANCE {
            break;
        }

        bitrate = ((target_quality / score) * (bitrate as f32)) as u32;

        if bitrate > 512000 {
            bitrate = 512000;
        } else if bitrate < 500 {
            bitrate = 500;
        }
    }
    info!("# {:?} Found B: {}, Score {:.2}", stem, bitrate, score);

    let mut cmd = Command::new("ffmpeg");
    cmd.args(&["-y", "-i"]);
    cmd.arg(&path);
    cmd.args(&["-c:a", "libopus", "-b:a", &format!("{}", bitrate)]);
    cmd.arg(
        &[
            OsStr::new("temp"),
            OsStr::new("conc"),
            Path::new(stem).with_extension("opus").as_os_str(),
        ]
        .iter()
        .collect::<PathBuf>(),
    );

    cmd.output()?;

    Ok(())
}

fn segment(input: &Path) -> Result<Vec<DirEntry>, std::io::Error> {
    // FIXME: Don't allow to segment be less that 5 sec
    let segments = Path::new("temp/segments");
    let mut cmd = Command::new("ffmpeg");
    cmd.args(&["-y", "-i"]);
    cmd.arg(input);
    cmd.args(&[
        "-ar",
        "48000",
        "-f",
        "segment",
        "-segment_time",
        "12",
        "temp/segments/%05d.wav",
    ]);

    cmd.output()?;

    fs::read_dir(&segments)?.collect()
}

// lazily compile regex, avoid recompiling the same thing multiple times throughout the program
lazy_static! {
    static ref RE: Regex = Regex::new(r"([0-9]*\.[0-9]*)").unwrap();
}

fn make_probe(file: &Path, bitrate: u32, model: &Path) -> Result<f32> {
    let probe_name = file
        .file_stem()
        .unwrap_or_else(|| file.as_os_str())
        .to_str()
        // ffmpeg files should not be invalid UTF-8
        .ok_or(anyhow!("`probe_name` has a non UTF-8 filename"))?;

    // Audio to opus
    let mut cmd = Command::new("ffmpeg");
    cmd.args(&["-y", "-i"]);
    cmd.arg(file);
    cmd.args(&[
        "-c:a",
        "libopus",
        "-b:a",
        &format!("{}", bitrate),
        &format!("temp/probes/{}_{}.opus", probe_name, bitrate),
    ]);
    cmd.output()?;

    // Audio to wav
    let mut cmd = Command::new("ffmpeg");
    cmd.args(&["-y", "-i"]);
    cmd.arg(&format!("temp/probes/{}_{}.opus", probe_name, bitrate));
    cmd.args(&["-ar", "48000"]);
    cmd.arg(&format!("temp/probes/{}_{}.wav", probe_name, bitrate));
    cmd.output().expect("opus encoding failed");

    // calculating score
    let mut cmd = Command::new("visqol");

    cmd.arg("--similarity_to_quality_model");
    cmd.arg(model);
    cmd.arg("--reference_file");
    cmd.arg(file);
    cmd.args(&[
        "--degraded_file",
        &format!("temp/probes/{}_{}.wav", probe_name, bitrate),
    ]);

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let output = cmd.output()?;
    debug!("{:?}", output);

    let score_str = String::from_utf8(output.stdout)?;

    let caps = RE
        .captures(&score_str)
        .ok_or(anyhow!("regex capture failed"))?;
    let score: &str = caps.get(1).map_or("", |m| m.as_str());

    Ok(score.parse::<f32>()?)
}
