extern crate rscam;
extern crate image;
extern crate exoquant;
extern crate philipshue;

extern crate time;

mod hue;

use image::{Rgb};
use exoquant::{Color, Histogram, SimpleColorSpace, generate_palette};
use philipshue::hue::{Group, GroupType, LightCommand};

use time::PreciseTime;
use std::thread;
use std::time::Duration;

fn main() {
    let width = 640;
    let height = 360;
    let interval = 0;

    println!("Hue bridge IP: ");
    let ip = read_line();
    let bridge = hue::start(ip);

    let groups = bridge.get_all_groups().unwrap();
    println!("\nAvaliable groups: {:?}", groups.values().filter(|g| is_room(g)).map(|g| g.name.clone()).collect::<Vec<_>>());

    println!("Which group should be controlled?");
    let group: &Group;
    loop {
        let group_name = read_line();
        if let Some(selected_group) = groups.values().find(|g| is_room(g) && g.name == group_name.trim()) {
            group = selected_group;
            break;
        } else {
            println!("Invalid group name.");
        }
    }

    let lights = &group.lights;

    println!("Video device ID:");
    let video_id = read_line();

    let mut camera = rscam::new(&format!("/dev/video{}", video_id.trim())).unwrap();
    camera.start(&rscam::Config {
        interval: (1, 15),
        resolution: (width, height),
        format: b"YU12",
        nbuffers: 1,
        ..Default::default()
    }).unwrap();

    let sleep_duration = Duration::from_millis(interval);

    loop {
        // let start = PreciseTime::now();
        let color_data = capture_rgb_buffer(&camera, width as usize, height as usize);
        // println!("Capture time {}", start.to(PreciseTime::now()));
        // let start = PreciseTime::now();
        let palette = generate_palette_from_rgb_buffer(&color_data, lights.len());
        // println!("palette time {}", start.to(PreciseTime::now()));

        // println!("{:?}", palette.iter().map(|c| format!("({}, {}, {})", c.r, c.g, c.b)).collect::<Vec<String>>().join(", "));
        // let start = PreciseTime::now();
        for i in 0..std::cmp::min(palette.len(), lights.len()) {
            let result = bridge.set_light_state(lights[i], &color_to_light_command(&palette[i]));
            if let Err(_) = result {
                println!("Failed to change color of light {}", lights[i])
            }
        }
        // println!("light time {}", start.to(PreciseTime::now()));

        thread::sleep(sleep_duration);
    }
}

fn is_room(group: &Group) -> bool {
    match group.group_type {
        GroupType::Room => true,
        _ => false
    }
}

fn read_line() -> String {
    use std::io;
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input
}

fn capture_rgb_buffer(camera: &rscam::Camera, width: usize, height: usize) -> Vec<u8> {
    let frame = camera.capture().unwrap();

    yuv_to_rgb(&frame as &[u8], width, height)
}

fn yuv_to_rgb(yuv_buffer: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut rgb: Vec<u8> = Vec::new();

    let total_pixels = width * height;

    for y in 0..height {
        for x in 0..width {
            let yi = (yuv_buffer[y * width + x] & 0xff) as f32;

            let xby2 = x / 2;
            let yby2 = y / 2;

            let u = (yuv_buffer[yby2 * width / 2 + xby2 + total_pixels] & 0xff) as f32;
            let v = (yuv_buffer[yby2 * width / 2 + xby2 + total_pixels + total_pixels / 4] & 0xff) as f32;

            let c = yi - 16.0;
            let d = u - 128.0;
            let e = v - 128.0;

            let r = clamp((298.0 * c + 409.0 * e + 128.0) as i64 >> 8, 0, 255);
            let g = clamp((298.0 * c - 100.0 * d - 208.0 * e + 128.0) as i64 >> 8, 0, 255);
            let b = clamp((298.0 * c + 516.0 * d + 128.0) as i64 >> 8, 0, 255);

            rgb.push(r as u8);
            rgb.push(g as u8);
            rgb.push(b as u8);
        }
    }

    rgb
}

fn clamp<T>(a: T, min: T, max: T) -> T where T: PartialOrd {
    if a < min { min } else if a > max { max } else { a }
}

fn save_rgb_buffer(buffer: &[u8], width: u32, height: u32, path: &'static str) -> Result<(), std::io::Error> {
    let pixels: Vec<u8> = buffer.to_vec();
    let image: image::ImageBuffer<Rgb<u8>, Vec<u8>> = image::ImageBuffer::from_vec(width, height, pixels).unwrap();
    image.save(std::path::Path::new(path))
}

fn generate_palette_from_rgb_buffer(buffer: &[u8], num_colors: usize) -> Vec<Color> {
    let max_limit = 240;

    // Histogram
    // let start = PreciseTime::now();

    let mut colors: Vec<Color> = Vec::new();
    for i in 0..(buffer.len() / 3) {
        if i % 3 != 0 {
            continue;
        }

        let j = i * 3;
        let r = buffer[j as usize];
        let g = buffer[j as usize + 1];
        let b = buffer[j as usize + 2];

        if r > max_limit && g > max_limit && b > max_limit {
            continue;
        }

        colors.push(Color::new(r, g, b, 255));
    }
    let histogram: Histogram = colors.iter().cloned().collect();

    // println!("Histogram generation took {} seconds", start.to(PreciseTime::now()));

    // Palatte
    // let start = PreciseTime::now();

    let palette = generate_palette(&histogram, &SimpleColorSpace::default(), &exoquant::optimizer::None, num_colors);
    // let palette = palette.into_iter().filter(|c| !(c.r > max_limit && c.g > max_limit && c.b > max_limit)).collect::<Vec<Color>>();

    // println!("Palette generation took {} seconds", start.to(PreciseTime::now()));

    palette
}

fn color_to_light_command(color: &Color) -> LightCommand {
    let hsl = color_to_hsl(color);
    LightCommand::default().with_hue(hsl.0).with_sat(hsl.1).with_bri(hsl.2)
}

fn color_to_hsl(color: &Color) -> (u16, u8, u8) {
    let r = color.r as f32 / 255.0;
    let g = color.g as f32 / 255.0;
    let b = color.b as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let mut h: f32;
    let mut s: f32;
    let mut l = (max + min) / 2.0;

    if max == min {
        h = 0.0;
        s = 0.0;
    } else {
        let d = max - min;
        s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
        h = if max == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        h /= 6.0;
    }

    h *= std::u16::MAX as f32;
    s *= 255.0;
    l *= 255.0;

    (h as u16, s as u8, l as u8)
}
