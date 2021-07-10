use std::io::Read;
use std::process::Command;
use std::process::Stdio;

use clap::App;
use clap::Arg;

use crate::helper::get_video_dims;
use crate::helper::{get_terminal_size, sample_buffer, sample_buffer_color};

pub mod helper;

static HELP_LONG: &'static str = "
plays videos in the terminal via ascii art

asciiplayer depends on ffmpeg and mpv.
This program is licenced under the GNU general public licence version 3, see LICENCE.
This software is developed at https://www.github.com/MetaMuffin/asciiplayer.";

fn main() {
    let args = App::new("asciiplayer")
        .version("0.1.0")
        .long_about(HELP_LONG)
        .author("metamuffin <metamuffin@metamuffin.org>")
        .arg(
            Arg::with_name("video")
                .help("Sets the video file to play back")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("monochrome")
                .short("m")
                .long("color")
                .takes_value(false)
                .help("dont display color"),
        )
        .arg(
            Arg::with_name("silent")
                .short("s")
                .long("silent")
                .help("dont spawn mpv to play sound")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("fps")
                .short("r")
                .long("fps")
                .takes_value(true)
                .help("specify frames per second to render"),
        )
        .arg(
            Arg::with_name("black-background")
                .short("b")
                .long("black-background") 
                .takes_value(false)
                .help("set the terminal background to opaque black - usefull for transparent terminals"),
        )
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )
        .get_matches();

    let filename = args.value_of("video").unwrap();
    let verbosity = args.occurrences_of("verbose");
    let source_dims = get_video_dims(filename);
    let mut target_dims = get_terminal_size(&args);
    if verbosity > 0 {
        target_dims.1 -= 1 // make space for the stats at the bottom
    }

    let fps = String::from(args.value_of("fps").or_else(|| Some("30")).unwrap())
        .parse::<i64>()
        .expect("Invalid fps value");

    if !args.is_present("silent") {
        Command::new("/bin/mpv")
            .arg("--no-video")
            .arg(String::from(filename))
            .arg("--really-quiet")
            .stdout(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .expect("Could not start mpv");
    }

    let mut ffmpeg = Command::new("/bin/ffmpeg")
        .arg("-i")
        .arg(String::from(filename))
        .arg("-filter:v")
        .arg(format!("fps=fps={}", fps).as_str())
        .arg("-f")
        .arg("image2pipe")
        .arg("-pix_fmt")
        .arg("rgb24")
        .arg("-vcodec")
        .arg("rawvideo")
        .arg("-")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Could not spawn ffmepg");

    let mut frame_buffer;
    let stdout = ffmpeg
        .stdout
        .as_mut()
        .expect("Could not open stdout of ffmpeg");

    let scale_factor = (
        (source_dims.0 as f64) / (target_dims.0 as f64),
        (source_dims.1 as f64) / (target_dims.1 as f64),
    );

    let nanos_per_frame = 1000000000 as i64 / fps;

    let do_color = !args.is_present("monochrome");

    if args.is_present("black-background") {
        print!("\x1b[48;2;1;1;1m");
    }

    let video_start = std::time::Instant::now();
    let mut frame = 0;
    loop {
        let loop_start = std::time::Instant::now();

        let mut a = stdout.take((source_dims.0 * source_dims.1 * 3) as u64);
        frame_buffer = vec![];
        let read = a.read_to_end(&mut frame_buffer).unwrap();
        if read == 0 {
            println!("ffmpeg returned no frame. lets just assume thats the end.");
            break;
        }
        let decode_time = loop_start.elapsed();

        let mut frame_string = String::new();

        for y in 0..(target_dims.1) {
            for x in 0..(target_dims.0) {
                let (fx, fy) = (x as f64, y as f64);
                let vals = (
                    sample_buffer(&frame_buffer, &source_dims, &scale_factor, fx, fy),
                    sample_buffer(&frame_buffer, &source_dims, &scale_factor, fx + 0.5, fy),
                    sample_buffer(&frame_buffer, &source_dims, &scale_factor, fx, fy + 0.5),
                    sample_buffer(
                        &frame_buffer,
                        &source_dims,
                        &scale_factor,
                        fx + 0.5,
                        fy + 0.5,
                    ),
                );
                if do_color {
                    let (r, g, b) =
                        sample_buffer_color(&frame_buffer, &source_dims, &scale_factor, fx, fy);
                    frame_string += format!("\x1b[38;2;{};{};{}m", r, g, b).as_str();
                }
                frame_string.push(select_char(&vals));
            }
        }
        let render_time = loop_start.elapsed();

        let sleep_needed = (frame * nanos_per_frame) - video_start.elapsed().as_nanos() as i64;
        if sleep_needed > 0 {
            std::thread::sleep(std::time::Duration::from_nanos(sleep_needed as u64));
        }

        let sleep_time = loop_start.elapsed();

        if verbosity > 0 {
            frame_string += "\x1b[38;2;255;255;255m";
            frame_string += format!(
                " frame: {:#} | all: {:#} decode: {:#} render: {:#} sleep: {:#}   ",
                frame,
                sleep_time.as_micros(),
                decode_time.as_micros(),
                (render_time - decode_time).as_micros(),
                (sleep_time - render_time).as_micros(),
            )
            .as_str();
        }
        println!("{}\x1b[1;1H", frame_string);

        frame += 1;
    }
    println!("Clean exit.")
}

fn select_char(vals: &(u8, u8, u8, u8)) -> char {
    return match vals {
        (0..=127, 0..=127, 128..=255, 128..=255) => '_',
        (128..=255, 128..=255, 0..=127, 0..=127) => '^',

        (0..=127, 128..=255, 0..=127, 128..=255) => '|',
        (128..=255, 0..=127, 128..=255, 0..=127) => '|',

        (0..=127, 192..=255, 128..=255, 128..=255) => '/',
        (128..=255, 128..=255, 128..=255, 0..=127) => '/',
        (128..=255, 0..=127, 128..=255, 128..=255) => '\\',
        (128..=255, 128..=255, 0..=127, 128..=255) => '\\',

        (128..=255, 0..=127, 0..=127, 0..=127) => '\'',
        (0..=127, 0..=127, 0..=127, 128..=255) => '.',
        (0..=127, 128..=255, 0..=127, 0..=127) => '\'',
        (0..=127, 0..=127, 128..=255, 0..=127) => '.',

        (0..=63, 0..=63, 0..=63, 0..=63) => ' ',
        (64..=127, 64..=127, 64..=127, 64..=127) => '-',
        (128..=191, 128..=191, 128..=191, 128..=191) => 'c',
        (192..=255, 192..=255, 192..=255, 192..=255) => '@',

        _ => ' ',
    };
}
