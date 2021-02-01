use std::io::Read;
use std::process::Command;
use std::process::Stdio;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 1 {
        panic!("please tell me, what file to play.")
    }
    let vfile = &args[1];

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
    println!("{}", res_str_buf);
    let dims = res_str_buf
        .split(",")
        .filter(|s| !s.is_empty())
        .map(|s| s.trim().parse::<usize>().unwrap())
        .collect::<Vec<usize>>();

    let target_dims = match term_size::dimensions() {
        Some((w, h)) => (w, h - 4),
        None => (80, 24),
    };
    if dims.len() != 2 {
        panic!(format!("Could not parse video dimension of ffprobe: {:?}__{:?}", dims,res_str_buf))
    }
    let source_dims = (dims[0], dims[1]);
    let fps = 30;

    Command::new("/bin/mpv")
        .arg("--no-video")
        .arg(String::from(vfile))
        .stdout(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("Could not start mpv");

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
            println!("ffmpeg returned no frame. lets just declare this as the end.");
            break;
        }
        let decode_time = loop_start.elapsed();

        let mut b = String::new();

        for y in 0..(target_dims.1) {
            for x in 0..(target_dims.0) {
                let (sx, sy) = (
                    ((x as f64) * dim_fac.0).floor() as usize,
                    ((y as f64) * dim_fac.1).floor() as usize,
                );
                let val = frame_buf[(sy * source_dims.0 + sx) * 3];
                match val {
                    0..=64 => b += " ",
                    65..=128 => b += ".",
                    129..=192 => b += "c",
                    _ => b += "@",
                }
            }
            b += "\n";
        }
        let render_time = loop_start.elapsed();

        let sleep_needed = (frame * nanos_per_frame) - video_start.elapsed().as_nanos() as i64;
        if sleep_needed > 0 {
            std::thread::sleep(std::time::Duration::from_nanos(sleep_needed as u64));
        }
        frame += 1;

        let sleep_time = loop_start.elapsed();

        let stats = format!(
            "all: {:#} decode: {:#} render: {:#} sleep: {:#}",
            sleep_time.as_micros(),
            decode_time.as_micros(),
            (render_time - decode_time).as_micros(),
            (sleep_time - render_time).as_micros(),
        );
        println!("{}\n{}", b, stats);
    }
    println!("Clean exit.")
}
