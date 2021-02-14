use std::io::Read;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;

fn print_help() {
    println!(
        "asciiplayer:
        Usage:
            asciiplayer play <filename>
            asciiplayer render <filename> <rows>,<cols>

        asciiplayer depends on ffmpeg, ffprobe and mpv.
        This is free software, licenced under the GNU GPL Version 3.
        This software is developed at https://www.github.com/MetaMuffin/asciiplayer."
    );
}

fn parse_dims(strbuf: String) -> (usize, usize) {
    let dims = strbuf
        .split(",")
        .filter(|s| !s.is_empty())
        .map(|s| s.trim().parse::<usize>().unwrap())
        .collect::<Vec<usize>>();

    if dims.len() != 2 {
        panic!(format!(
            "Could not parse video dimension: {:?}__{:?}",
            dims, strbuf
        ))
    }
    return (dims[0], dims[1]);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        return print_help();
    }
    if args[1] == "--help" {
        return print_help();
    }
    let mut file_output = false;
    let mut arg_dims = (0, 0);
    match args[1].as_str() {
        "play" => file_output = false,
        "render" => {
            file_output = true;
            arg_dims = parse_dims(String::from(&args[3]));
        }
        _ => return print_help(),
    }
    let vfile = &args[2];

    let mut file = None;
    if file_output {
        file = Some(std::fs::File::create("render").expect("Could not create render file"));
    }

    let ffprobe = Command::new("/bin/ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg("stream=width,height")
        .arg("-of")
        .arg("csv=p=0")
        .arg(String::from(vfile))
        .stdout(Stdio::piped())
        .spawn()
        .expect("Could not spawn ffprobe");

    let mut res_str_buf = String::new();
    ffprobe
        .stdout
        .expect("Could not read stdout of ffprobe")
        .read_to_string(&mut res_str_buf)
        .expect("Could not convert output of ffprobe to a string.");

    let target_dims = match file_output {
        true => arg_dims,
        false => match term_size::dimensions() {
            Some((w, h)) => (w, h - 1),
            None => (80, 24),
        },
    };
    let source_dims = parse_dims(res_str_buf);

    let fps = 30;
    if !file_output {
        Command::new("/bin/mpv")
            .arg("--no-video")
            .arg(String::from(vfile))
            .arg("--really-quiet")
            .stdout(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .expect("Could not start mpv");
    }

    let mut ffmpeg = Command::new("/bin/ffmpeg")
        .arg("-i")
        .arg(String::from(vfile))
        .arg("-filter:v")
        .arg("fps=fps=30")
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

    let mut frame_buf; //Vec::with_capacity(dims.0 * dims.1 * 3);
    let stdout = ffmpeg
        .stdout
        .as_mut()
        .expect("Could not open stdout of ffmpeg");

    let dim_fac = (
        (source_dims.0 as f64) / (target_dims.0 as f64),
        (source_dims.1 as f64) / (target_dims.1 as f64),
    );

    let nanos_per_frame = 1000000000 as i64 / fps;

    let video_start = std::time::Instant::now();
    let mut frame = 0;
    loop {
        let loop_start = std::time::Instant::now();

        let mut a = stdout.take((source_dims.0 * source_dims.1 * 3) as u64);
        frame_buf = vec![];
        let read = a.read_to_end(&mut frame_buf).unwrap();
        if read == 0 {
            println!("ffmpeg returned no frame. lets just assume thats the end.");
            break;
        }
        let decode_time = loop_start.elapsed();

        let mut b = String::new();

        for y in 0..(target_dims.1) {
            for x in 0..(target_dims.0) {
                let (fx, fy) = (x as f64, y as f64);
                let vals = (
                    buf_sample(&frame_buf, &source_dims, &dim_fac, fx, fy),
                    buf_sample(&frame_buf, &source_dims, &dim_fac, fx + 0.5, fy),
                    buf_sample(&frame_buf, &source_dims, &dim_fac, fx, fy + 0.5),
                    buf_sample(&frame_buf, &source_dims, &dim_fac, fx + 0.5, fy + 0.5),
                );
                b.push(sel_char(&vals));
            }
            if !file_output {
                b += "\n"
            };
        }
        let render_time = loop_start.elapsed();

        let sleep_needed = (frame * nanos_per_frame) - video_start.elapsed().as_nanos() as i64;
        if sleep_needed > 0 && !file_output {
            std::thread::sleep(std::time::Duration::from_nanos(sleep_needed as u64));
        }
        frame += 1;

        let sleep_time = loop_start.elapsed();

        let stats = format!(
            "frame: {:#} | all: {:#} decode: {:#} render: {:#} sleep: {:#}   ",
            frame,
            sleep_time.as_micros(),
            decode_time.as_micros(),
            (render_time - decode_time).as_micros(),
            (sleep_time - render_time).as_micros(),
        );
        if !file_output {
            println!("{}{}\x1b[1;1H", b, stats);
        } else if let Some(f) = &mut file {
            f.write_all(b.as_bytes())
                .expect("Could not write to render file.");
            print!("\r{}", stats);
        }
    }
    println!("Clean exit.")
}

fn buf_sample(
    buf: &[u8],
    source_dims: &(usize, usize),
    dim_fac: &(f64, f64),
    x: f64,
    y: f64,
) -> u8 {
    let (sx, sy) = (
        (x * dim_fac.0).floor() as usize,
        (y * dim_fac.1).floor() as usize,
    );
    let buf_index = sy * source_dims.0 + sx;
    if buf_index < (source_dims.0 * source_dims.1) {
        return buf[buf_index * 3];
    } else {
        return 0;
    }
}

fn sel_char(vals: &(u8, u8, u8, u8)) -> char {
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
