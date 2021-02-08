use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Transform score for easier score comprehension and usage
/// Scaled 4.0 - 4.75 range to 0.0 - 5.0
pub fn transform_score(score: f32) -> f32 {
    const SCALE_VALUE: f32 = 5.0 / (4.75 - 4.0);

    if score < 4.1 {
        1.0f32
    } else {
        (score - 4.1) * SCALE_VALUE
    }
}

pub fn get_audio_time(input: &Path) -> Duration {
    const MILLIS_PER_MINUTE: u64 = 60_000;
    const MILLIS_PER_HOUR: u64 = MILLIS_PER_MINUTE * 60;

    // FIXME: Don't allow to segment be less that 5 sec
    let mut cmd = Command::new("ffprobe");
    cmd.arg("-i");
    cmd.arg(input);

    cmd.stderr(Stdio::piped());

    let output = String::from_utf8(cmd.output().unwrap().stderr).unwrap();
    debug!("{:?}", output);

    const START: &str = "Duration: ";
    const END: &str = ", start";

    let s = &output[output.find(START).unwrap() + START.len()..output.find(END).unwrap()];

    let mut iter = s.split(':');
    // TODO clean up the error handling here
    let hours: u64 = iter.next().unwrap().parse().unwrap();
    let minutes: u64 = iter.next().unwrap().parse().unwrap();
    let millis: u64 = (1000.0 * iter.next().unwrap().parse::<f32>().unwrap()) as u64;

    Duration::from_millis(millis + minutes * MILLIS_PER_MINUTE + hours * MILLIS_PER_HOUR)
}
