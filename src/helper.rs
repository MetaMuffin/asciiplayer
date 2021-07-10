use std::{
    io::Read,
    process::{Command, Stdio},
};

use clap::ArgMatches;

pub fn get_video_dims(filename: &str) -> (usize, usize) {
    let ffprobe = Command::new("/bin/ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg("stream=width,height")
        .arg("-of")
        .arg("csv=p=0")
        .arg(String::from(filename))
        .stdout(Stdio::piped())
        .spawn()
        .expect("Could not spawn ffprobe");

    let mut res_str_buf = String::new();
    ffprobe
        .stdout
        .expect("Could not read stdout of ffprobe")
        .read_to_string(&mut res_str_buf)
        .expect("Could not convert output of ffprobe to a string.");
    return parse_dims(res_str_buf);
}

fn parse_dims(strbuf: String) -> (usize, usize) {
    let dims = strbuf
        .split(",")
        .filter(|s| !s.is_empty())
        .map(|s| s.trim().parse::<usize>().unwrap())
        .collect::<Vec<usize>>();

    if dims.len() != 2 {
        panic!("Could not parse video dimension")
    }
    return (dims[0], dims[1]);
}

pub fn get_terminal_size(args: &ArgMatches) -> (usize, usize) {
    match args.value_of("render-dimension") {
        Some(v) => parse_dims(String::from(v)),
        None => match term_size::dimensions() {
            Some((w, h)) => (w, h),
            None => (80, 24),
        },
    }
}

pub fn sample_buffer(
    buf: &[u8],
    source_dims: &(usize, usize),
    scale_factor: &(f64, f64),
    x: f64,
    y: f64,
) -> u8 {
    let (r, g, b) = sample_buffer_color(buf, source_dims, scale_factor, x, y);
    return r / 3 + g / 3 + b / 3;
}

pub fn sample_buffer_color(
    buf: &[u8],
    source_dims: &(usize, usize),
    scale_factor: &(f64, f64),
    x: f64,
    y: f64,
) -> (u8, u8, u8) {
    let (sx, sy) = (
        (x * scale_factor.0).floor() as usize,
        (y * scale_factor.1).floor() as usize,
    );
    let buf_index = sy * source_dims.0 + sx;
    if buf_index < (source_dims.0 * source_dims.1) {
        return (
            buf[buf_index * 3],
            buf[buf_index * 3 + 1],
            buf[buf_index * 3 + 2],
        );
    } else {
        return (0, 0, 0);
    }
}
