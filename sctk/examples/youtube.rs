//🎬️ 🎧️ 📋️ 📺️ 💾️
//cargo run --example youtube_with_toolkit --features "Clickable Keyboard Scrollable Window"

use rusttype::{point, Font, Scale};
use rusty_pipe::youtube_extractor::search_extractor::*;
use serde_json;
use std::collections::VecDeque;
use std::convert::TryInto;
use std::iter::once;

use std::fs::File;
use std::io::BufReader;

use std::process::Command;

use smithay_client_toolkit::shell::xdg::window;

mod hugo_client_toolkit;
use crate::hugo_client_toolkit::{Clickable, Hlib, Keyboard, Scrollable, Window};

pub struct HClient {
	width: u32,
	height: u32,
	numiter: Vec<String>,
	curry: usize,
	frames: VecDeque<Frame>,
	oldframes: VecDeque<Frame>,
	history: Vec<Frame>,
	searchbox: Frame,
	needquery: bool,
	needs_drawing: bool,
	mpv_running: bool,
	font: Vec<u8>,
	highlight: Frame,
}

fn main() {
	env_logger::init();

	let mut history = Vec::new();

	if let Ok(file) = File::open("history.json") {
		let br = BufReader::new(file);

		for (n, (pd, url)) in serde_json::from_reader::<BufReader<File>, Vec<(Vec<u8>, String)>>(br).unwrap().into_iter().enumerate() {
			history.push(Frame {
				pixel_data: pd,
				x: 0,
				y: (n + 1) * 40,
				height: 40,
				width: 600,
				num: n + 1,
				url: Some(url),
				hidden: false,
				txt: None,
			});
		}
	}

	let frames = VecDeque::new();
	let font = include_bytes!("../WenQuanYiMicroHei.ttf").to_vec();

	let simple_window = HClient {
		numiter: vec![],
		width: 600,
		height: 380,
		curry: 0,
		frames,
		oldframes: VecDeque::new(),
		history,
		searchbox: Frame {
			pixel_data: vec![0_u8; 600 * 40],
			x: 0,
			y: 0,
			height: 40,
			width: 600,
			num: 0,
			url: None,
			hidden: false,
			txt: Some(String::new()),
		},
		highlight: Frame { pixel_data: vec![0_u8; 600 * 40], x: 0, y: 0, height: 40, width: 0, num: 0, url: None, hidden: false, txt: None },
		needquery: false,
		needs_drawing: true,
		mpv_running: false,
		font,
	};

	Hlib::run(simple_window);
}

impl Window for HClient {
	fn window(&mut self) -> window::WindowBuilder {
		window::Window::builder()
			.title("A wayland window")
			// GitHub does not let projects use the `org.github` domain but the `io.github` domain is fine.
			.app_id("io.github.m-hugo.youtube")
			.min_size((256, 256))
	}
	fn change_size(&mut self, _proposed_size: Option<(u32, u32)>) {
		self.width = 600;
		self.height = 380;
	}
}
impl Scrollable for HClient {
	fn scroll(&mut self, i: f64) {
		if i > 0.0 {
			self.curry += i as usize;
		} else {
			self.curry = self.curry.saturating_sub(-i as usize);
		}
		self.needs_drawing = true;
	}
}
impl Clickable for HClient {
	fn short_click(&mut self, position: (usize, usize)) {
		let len = self.frames.len();
		println!("touchshort");
		for i in 0..len {
			if !self.frames[i].hidden
				&& self.frames[i].x < position.0
				&& self.frames[i].y < position.1 + self.curry
				&& self.frames[i].x + self.frames[i].width > position.0
				&& self.frames[i].y + self.frames[i].height > position.1 + self.curry
			{
				println!("{}", self.frames[i].num);
				if self.mpv_running == false {
					if let Some(u) = &self.frames[i].url {
						println!("{}", u);
						println!("{}", i);
						Command::new("mpv").args(["--no-video", "--input-ipc-server=/tmp/mpvsocket", &u]).spawn();
						self.mpv_running = true;
						//Command::new("mpv").args([&u]).spawn();
					}
				}
			}
		}
	}
	fn long_click(&mut self, position: (usize, usize)) {
		//let len = self.frames.len();
		println!("touchlong");
		for f in once(&self.searchbox).chain(&self.frames) {
			if !f.hidden
				&& f.x < position.0
				&& f.y < position.1 + self.curry
				&& f.x + f.width > position.0
				&& f.y + f.height > position.1 + self.curry
			{
				println!("YOLO, {}", f.num);
				if let Some(txt) = &f.txt {
					let (xmin, xmax) = self.xof(txt, position.0 - f.x);
					self.highlight.x = xmin;
					self.highlight.width = xmax;
					self.highlight.num = f.num;
					println!("xmin: {}, xmax: {}", xmin, xmax);
					for x in xmin..xmax {
						for y in f.y..f.height {
							self.highlight.pixel_data[x as usize + y as usize * 600] =
								if f.pixel_data[x as usize + y as usize * 600] < 50 { 255 } else { 0 };
						}
					}
				}
				//Command::new("mpv").args([&u]).spawn();
			}
		}
		for (n, f) in once(&mut self.searchbox).chain(&mut self.frames).enumerate() {
			if n == self.highlight.num {
				f.pixel_data = self.highlight.pixel_data.clone();
			}
		}
		self.needs_drawing = true;
	}
}

impl Keyboard for HClient {
	fn press(&mut self, l: &str) {
		self.needs_drawing = true;
		if l == "<" {
			for f in &mut self.frames {
				f.hidden ^= true;
			}
			for f in &mut self.history {
				f.hidden ^= true;
			}
			return;
		}
		if l == "\u{1b}" {
			Command::new("sh").arg("-c").arg("echo 'quit' | socat - /tmp/mpvsocket").spawn();
			self.mpv_running = false;
			return;
		}
		if l == " " {
			Command::new("sh").arg("-c").arg("echo 'cycle pause' | socat - /tmp/mpvsocket").spawn();
			return;
		}
		if l == "\r" {
			for f in &mut self.history {
				f.hidden = true;
			}
			self.needquery = true;
			let (iter, urls) = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime").block_on(self.query()).unwrap();
			self.numiter = iter;

			let fonty = Font::try_from_bytes(&self.font).expect("error constructing a Font from bytes");
			let scale = Scale { x: 18.0 * 1.4, y: 18.0 };
			let v_metrics = fonty.v_metrics(scale);
			let offset = point(0.0, v_metrics.ascent);
			for (n, (s, url)) in self.numiter.iter().zip(urls.into_iter()).enumerate() {
				let mut frame = Frame {
					pixel_data: vec![0_u8; 600 * 40],
					x: 0,
					y: 40 * (n + 1),
					height: 40,
					width: 600,
					num: n + 1,
					url: Some(url),
					hidden: false,
					txt: Some(s.to_owned()),
				};

				for g in fonty.layout(&s, scale, offset) {
					if let Some(bb) = g.pixel_bounding_box() {
						g.draw(|x, y, v| {
							let x = (x + (bb.min.x + 2) as u32) as usize;
							let y = (y + (bb.min.y + 20) as u32) as usize;
							frame.pixel_data[x as usize + y as usize * 600] = (v * 255.0) as u8;
						});
					}
				}

				self.frames.push_back(frame);
			}

			return;
		}
		self.searchbox.txt = Some(self.searchbox.txt.to_owned().unwrap() + &l);
		let mut pixels = self.searchbox.pixel_data.clone();
		let fonty = Font::try_from_bytes(&self.font).expect("error constructing a Font from bytes");
		let scale = Scale { x: 18.0 * 1.4, y: 18.0 };
		let v_metrics = fonty.v_metrics(scale);
		let offset = point(0.0, v_metrics.ascent);
		for g in fonty.layout(&self.searchbox.txt.as_ref().unwrap(), scale, offset) {
			if let Some(bb) = g.pixel_bounding_box() {
				g.draw(|x, y, v| {
					let x = (x + bb.min.x as u32) as usize;
					let y = (y + (bb.min.y + 20) as u32) as usize;
					pixels[x as usize + y as usize * 600] = (v * 255.0) as u8;
				});
			}
		}
		self.searchbox.pixel_data = pixels;
	}
}

impl HClient {
	fn xof(&self, txt: &String, pos: usize) -> (usize, usize) {
		let fonty = Font::try_from_bytes(&self.font).expect("error constructing a Font from bytes");
		let scale = Scale { x: 18.0 * 1.4, y: 18.0 };
		let v_metrics = fonty.v_metrics(scale);
		let offset = point(0.0, v_metrics.ascent);
		for g in fonty.layout(&txt, scale, offset) {
			println!("p: {}, pn: {}", g.position().x + g.unpositioned().h_metrics().advance_width, pos);
			if g.position().x + g.unpositioned().h_metrics().advance_width >= pos as f32 {
				return (g.position().x as usize, (g.position().x + g.unpositioned().h_metrics().advance_width) as usize);
			}
		}
		return (0, 0);
	}
}
// mpv --no-video https://www.youtube.com/watch?v=ZdQYFDjEI60

struct DownloaderExample;

#[async_trait]
impl Downloader for DownloaderExample {
	async fn download(&self, url: &str) -> Result<String, ParsingError> {
		println!("query url : {}", url);
		let resp = reqwest::get(url).await.map_err(|er| ParsingError::DownloadError { cause: er.to_string() })?;
		println!("got response ");
		let body = resp.text().await.map_err(|er| ParsingError::DownloadError { cause: er.to_string() })?;
		println!("suceess query");
		Ok(String::from(body))
	}

	async fn download_with_header(&self, url: &str, header: HashMap<String, String>) -> Result<String, ParsingError> {
		let client = reqwest::Client::new();
		let res = client.get(url);
		let mut headers = reqwest::header::HeaderMap::new();
		for header in header {
			headers.insert(reqwest::header::HeaderName::from_str(&header.0).map_err(|e| e.to_string())?, header.1.parse().unwrap());
		}
		let res = res.headers(headers);
		let res = res.send().await.map_err(|er| er.to_string())?;
		let body = res.text().await.map_err(|er| er.to_string())?;
		Ok(String::from(body))
	}

	async fn eval_js(&self, script: &str) -> Result<String, String> {
		use quick_js::Context; //, JsValue};
		let context = Context::new().expect("Cant create js context");
		// println!("decryption code \n{}",decryption_code);
		// println!("signature : {}",encrypted_sig);
		println!("jscode \n{}", script);
		let res = context.eval(script).unwrap_or(quick_js::JsValue::Null);
		// println!("js result : {:?}", result);
		let result = res.into_string().unwrap_or("".to_string());
		print!("JS result: {}", result);
		Ok(result)
	}
}

//use std::io;
use std::str::FromStr;
//use urlencoding::encode;
use async_trait::async_trait;
use failure::Error;
use rusty_pipe::downloader_trait::Downloader;
use rusty_pipe::youtube_extractor::error::ParsingError;
use std::collections::HashMap;
fn truncate(s: &str, max_chars: usize) -> &str {
	match s.char_indices().nth(max_chars) {
		None => s,
		Some((idx, _)) => &s[..idx],
	}
}
impl HClient {
	async fn query(&self) -> Result<(Vec<String>, Vec<String>), Error> {
		let search_extractor = YTSearchExtractor::new(self.searchbox.txt.as_ref().unwrap(), None, DownloaderExample).await?;
		let items = search_extractor.search_results()?;
		let names = items
			.iter()
			.filter_map(|v| {
				if let YTSearchItem::StreamInfoItem(streaminfoitem) = v {
					let mut n = streaminfoitem.get_name().ok();
					n.as_mut().map(|nn| truncate(nn, 40).to_owned())
				} else {
					None
				}
			})
			.collect();
		let url = items
			.iter()
			.filter_map(|v| if let YTSearchItem::StreamInfoItem(streaminfoitem) = v { streaminfoitem.get_url().ok() } else { None })
			.collect();
		Ok((names, url))
	}
}

struct Frame {
	pixel_data: Vec<u8>,
	txt: Option<String>,
	x: usize,
	y: usize,
	height: usize,
	width: usize,
	num: usize,
	url: Option<String>,
	hidden: bool,
}

impl HClient {
	fn draw(
		&mut self,
		buffer: &mut [u8],
		surface: &wayland_client::protocol::wl_surface::WlSurface,
		conn: &mut wayland_client::ConnectionHandle<'_>,
	) {
		let mut bcs = buffer.chunks_exact_mut(4);

		'outer: for f in once(&self.searchbox).chain(&self.history).chain(&self.frames) {
			if !f.hidden {
				for (index, pd) in f.pixel_data.iter().enumerate() {
					if index / self.width as usize + f.y > self.curry + 8 {
						if let Some(c) = bcs.next() {
							//.get_mut(index+f.y){
							let array: &mut [u8; 4] = c.try_into().unwrap();
							*array = [*pd, *pd, *pd, 255];
						} else {
							break 'outer;
						}
					}
				}
			}
		}

		surface.damage(conn, 0, 0, self.width as i32, self.height as i32);
	}
}
