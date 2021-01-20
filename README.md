<p align="center">
  <img src="https://github.com/master-of-zen/BOPUS/blob/master/BOPUS.png?raw=true">
</p>

# 🅱️OPUS
Bitrate Optimization for OPUS.

Bopus search for bitrate of OPUS that will result in desired quality. Quality of audio is asserted by [Visqol](https://github.com/google/visqol).

## Install
1. Clone and install [Visqol](https://github.com/google/visqol).
2. Put models in same directory where is your audio file.
3. Clone and build bopus, execute it in same folder with input file and `models` from visqol folder.
The following instructions for BOPUS can be found below.

## How to compile BOPUS itself(Linux only for now)
1st part (Installing Rust dependencies. You will need to install curl to do this one)

```
1. curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh (this will install the latest stable RustC release, alongside some other important stuff you need. You might be prompted for your password. If the install is successful, the following line will appear: Rust is installed now. Great!)

1.1. rustup update (this is done to update to the latest stable RustC release if you need to).

2. rustc -V (checking which rustc version you have. Provided you followed the last 2 instructions, it'll say "rustc XXXXX version".

3. rustup toolchain install nightly (this will install the nightly rustc build, which is required to actually compile this program).

4. rustup default nightly && rustc -V (the 1st command will set the nightly as default for the compiler, and rustc -V will check what version you have. If it's nightly, you're good to go.
```

2nd part(Compiling BOPUS)
```
1. git clone https://github.com/master-of-zen/BOPUS bopus

2. cd bopus

3. cargo build

4. Copy the bopus binary found in target/release, and copy it alongside the visqol binary that you should have.
In the end, you should have a directory containing the bopus and the visqol binaries, your audio file, and the models folder with the models inside. You can start encoding now.
```


## Usage
```
-i --input      Input file. Any decodable by FFmpeg.
-t --target     Target quality. Range: 1 - 5. Recommended range 3 - 4.5.
```
## Support me
Bitcoin - `1gU9aQ2qqoQPuvop2jqC68JKZh5cyCivG`

