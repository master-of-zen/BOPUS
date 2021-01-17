
use clap::{Arg, App};

fn get_matches(){
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
    println!("{:?}", _matches);
    println!("Using input file: {}", _matches.value_of("INPUT").unwrap());
                            return;
}

fn main() {
    get_matches();
}

