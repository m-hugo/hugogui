use std::convert::TryInto;

use rusttype::{point, Font, Scale};

use std::collections::VecDeque;

use lib_poki_launcher::{App, AppsDB, Config, CFG_PATH, DB_PATH};

use once_cell::sync::OnceCell;
use std::sync::Mutex;

use smithay_client_toolkit::shell::xdg::window;

mod hugo_client_toolkit;
use crate::hugo_client_toolkit::{Clickable, Hlib, Keyboard, Scrollable, Window};

const MAX_APPS_SHOWN: usize = 15;

static APPS_DB: OnceCell<Mutex<AppsDB>> = OnceCell::new();

fn with_apps_db<R>(f: impl FnOnce(&mut AppsDB) -> R) -> Option<R> {
	if let Some(apps_db) = APPS_DB.get() {
		if let Ok(mut apps_db) = apps_db.lock() {
			return Some(f(&mut *apps_db));
		}
	}
	None
}

fn main() {
	env_logger::init();

	let config = match Config::load() {
		Ok(config) => config,
		Err(e) => {
			return;
		}
	};

	let apps_db = match AppsDB::init(config) {
		Ok((apps_db, scan_errors)) => apps_db,
		Err(e) => {
			return;
		}
	};

	APPS_DB.set(Mutex::new(apps_db)).expect("AppsDB initialized twice");

	let mut frames = VecDeque::new();
	let font = include_bytes!("../WenQuanYiMicroHei.ttf").to_vec();
	let fonty = Font::try_from_bytes(&font).expect("error constructing a Font from bytes");

	if let Some(list) = with_apps_db(|apps_db| apps_db.get_ranked_list(Some("a"), Some(MAX_APPS_SHOWN))) {
		for (n, mut el) in list.into_iter().enumerate() {
			let iconpath = "/usr/share/icons/hicolor/48x48/apps/".to_string() + &el.icon + ".png";
			println!("{:#?}", el);
			//if let Some(Ok(icon)) = iconli.get(0){
			//println!("{}, {:?}", el.name, iconpath);
			//} else {
			//println!("{}, noicon", el.name);
			//}
			//};

			el.name.truncate(40);

			use image::imageops::grayscale;
			let image = image::open(&iconpath).ok().map(|i| grayscale(&i));

			let mut frame = Frame {
				pixel_data: vec![0_u8; 600 * 50],
				exec: el.exec,
				//i.to_rgba8()),
				x: 50,
				y: 50 * n,
				height: 50,
				width: 600,
				num: n,
			};

			let scale = Scale { x: 18.0 * 1.4, y: 18.0 };
			let v_metrics = fonty.v_metrics(scale);
			let offset = point(0.0, v_metrics.ascent);
			for g in fonty.layout(&el.name, scale, offset) {
				if let Some(bb) = g.pixel_bounding_box() {
					g.draw(|x, y, v| {
						let x = (x + (bb.min.x + 50) as u32) as usize;
						let y = (y + (bb.min.y + 20) as u32) as usize;
						frame.pixel_data[x as usize + y as usize * 600] = (v * 255.0) as u8;
					});
				}
			}
			if let Some(i) = image {
				println!("tt");
				for (x, y, p) in i.enumerate_pixels() {
					frame.pixel_data[x as usize + y as usize * 600] = p.0[0];
				}
			}

			frames.push_back(frame);
		}
	}

	let simple_window = HClient { width: 600, height: 380, curry: 0, frames, needs_drawing: true };

	Hlib::run(simple_window);
}

pub struct HClient {
	width: u32,
	height: u32,
	curry: usize,
	frames: VecDeque<Frame>,
	needs_drawing: bool,
}
use std::process::Command;

impl Window for HClient {
	fn window(&mut self) -> window::WindowBuilder {
		window::Window::builder()
			.title("A wayland window")
			// GitHub does not let projects use the `org.github` domain but the `io.github` domain is fine.
			.app_id("io.github.m-hugo.app_menu")
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
		for i in 0..len {
			if self.frames[i].x < position.0
				&& self.frames[i].y < position.1 + self.curry
				&& self.frames[i].x + self.frames[i].width > position.0
				&& self.frames[i].y + self.frames[i].height > position.1 + self.curry
			{
				println!("{}", self.frames[i].num);
				Command::new("sh").args(self.frames[i].exec.clone()).status();
			}
		}
	}
	fn long_click(&mut self, position: (usize, usize)) {
		self.short_click(position);
	}
}

impl Keyboard for HClient {
	fn press(&mut self, _l: &str) {}
}

//use image::ImageBuffer;
//use image::Luma;

struct Frame {
	pixel_data: Vec<u8>,
	exec: Vec<String>,
	//image:Option<ImageBuffer<Luma<u8>, Vec<u8>>>,//Option<image::RgbaImage>,
	x: usize,
	y: usize,
	height: usize,
	width: usize,
	num: usize,
}

impl HClient {
	pub fn draw(
		&mut self,
		buffer: &mut [u8],
		surface: &wayland_client::protocol::wl_surface::WlSurface,
		conn: &mut wayland_client::ConnectionHandle<'_>,
	) {
		let mut bcs = buffer.chunks_exact_mut(4);

		'outer: for f in &self.frames {
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

		surface.damage(conn, 0, 0, self.width as i32, self.height as i32);
	}
}
