use std::convert::TryInto;

use rusttype::{point, Font, Scale};

use std::collections::VecDeque;
use std::ops::RangeFrom;

use smithay_client_toolkit::shell::xdg::window;

mod hugo_client_toolkit2;
use crate::hugo_client_toolkit2::{HClient, Hlib};

struct Frame {
	pixel_data: Vec<u8>,
	x: usize,
	y: usize,
	height: usize,
	width: usize,
	num: usize,
}

fn main() {
	env_logger::init();

	let mut numiter = 0..;
	let mut frames = VecDeque::new();
	let n = numiter.next();
	let mut frame = Frame { pixel_data: vec![0_u8; 200 * 40], x: 0, y: 40 * n.unwrap(), height: 40, width: 200, num: n.unwrap() };
	let font = include_bytes!("../WenQuanYiMicroHei.ttf").to_vec();
	let fonty = Font::try_from_bytes(&font).expect("error constructing a Font from bytes");
	let scale = Scale { x: 18.0 * 1.4, y: 18.0 };
	let v_metrics = fonty.v_metrics(scale);
	let offset = point(0.0, v_metrics.ascent);
	for g in fonty.layout(&n.unwrap().to_string(), scale, offset) {
		if let Some(bb) = g.pixel_bounding_box() {
			g.draw(|x, y, v| {
				let x = (x + bb.min.x as u32) as usize;
				let y = (y + (bb.min.y + 20) as u32) as usize;
				frame.pixel_data[x as usize + y as usize * 200] = (v * 255.0) as u8;
			});
		}
	}
	frames.push_back(frame);
	while frames.back().unwrap().y + 40 <= 380 {
		let n = numiter.next();
		let mut frame = Frame { pixel_data: vec![0_u8; 200 * 40], x: 0, y: 40 * n.unwrap(), height: 40, width: 200, num: n.unwrap() };
		let scale = Scale { x: 18.0 * 1.4, y: 18.0 };
		let v_metrics = fonty.v_metrics(scale);
		let offset = point(0.0, v_metrics.ascent);
		for g in fonty.layout(&n.unwrap().to_string(), scale, offset) {
			if let Some(bb) = g.pixel_bounding_box() {
				g.draw(|x, y, v| {
					let x = (x + bb.min.x as u32) as usize;
					let y = (y + (bb.min.y + 20) as u32) as usize;
					frame.pixel_data[x as usize + y as usize * 200] = (v * 255.0) as u8;
				});
			}
		}
		frames.push_back(frame);
	}

	let simple_window = MyClient { numiter, width: 200, height: 380, curry: 0, frames, oldframes: VecDeque::new(), font, needs_drawing: true };

	Hlib::run(simple_window);
}

struct MyClient {
	width: u32,
	height: u32,
	numiter: RangeFrom<usize>,
	curry: usize,
	frames: VecDeque<Frame>,
	oldframes: VecDeque<Frame>,
	font: Vec<u8>,
	needs_drawing: bool,
}

impl HClient for MyClient {
	fn width(&self) -> u32 {
		self.width
	}
	fn height(&self) -> u32 {
		self.height
	}
	fn needs_drawing(&self) -> bool {
		self.needs_drawing
	}
	fn set_needs_drawing(&mut self, nd: bool) {
		self.needs_drawing = nd;
	}
	fn short_click(&mut self, position: (usize, usize)) {
		let len = self.frames.len();
		for i in 0..len {
			if self.frames[i].x < position.0
				&& self.frames[i].y < position.1 + self.curry
				&& self.frames[i].x + self.frames[i].width > position.0
				&& self.frames[i].y + self.frames[i].height > position.1 + self.curry
			{
				println!("{}", self.frames[i].num)
			}
		}
	}
	fn long_click(&mut self, position: (usize, usize)) {
		self.short_click(position);
	}

	fn window(&self) -> window::WindowBuilder {
		window::Window::builder()
			.title("A wayland window")
			// GitHub does not let projects use the `org.github` domain but the `io.github` domain is fine.
			.app_id("io.github.m-hugo.simple_scroll")
			.min_size((256, 256))
	}
	fn change_size(&mut self, _proposed_size: Option<(u32, u32)>) {
		self.width = 200;
		self.height = 380;
	}

	fn scroll(&mut self, i: f64) {
		if i > 0.0 {
			self.curry += i as usize;
		} else {
			self.curry = self.curry.saturating_sub(-i as usize);
		}
		//self.needs_drawing = true;
	}

	fn draw(
		&mut self,
		buffer: &mut [u8],
		surface: &wayland_client::protocol::wl_surface::WlSurface,
		conn: &mut wayland_client::ConnectionHandle<'_>,
	) {
		self.curry += 1;
		if self.frames.back().unwrap().y + 40 < self.curry + 380 {
			let n = self.numiter.next();
			let mut frame = Frame { pixel_data: vec![0_u8; 200 * 40], x: 0, y: 40 * n.unwrap(), height: 40, width: 200, num: n.unwrap() };
			let font = Font::try_from_bytes(&self.font).expect("error constructing a Font from bytes");
			let scale = Scale { x: 18.0 * 1.4, y: 18.0 };
			let v_metrics = font.v_metrics(scale);
			let offset = point(0.0, v_metrics.ascent);
			for g in font.layout(&n.unwrap().to_string(), scale, offset) {
				if let Some(bb) = g.pixel_bounding_box() {
					g.draw(|x, y, v| {
						let x = (x + bb.min.x as u32) as usize;
						let y = (y + (bb.min.y + 20) as u32) as usize;
						frame.pixel_data[x as usize + y as usize * 200] = (v * 255.0) as u8;
					});
				}
			}
			self.frames.push_back(frame);
			self.oldframes.push_back(self.frames.pop_front().unwrap());
		} else if self.frames.front().unwrap().y > self.curry {
			self.frames.push_front(self.oldframes.pop_back().unwrap());
		}

		let mut bcs = buffer.chunks_exact_mut(4);

		for f in &self.frames {
			for (index, pd) in f.pixel_data.iter().enumerate() {
				if index / 200 + f.y > self.curry + 8 {
					if let Some(c) = bcs.next() {
						//.get_mut(index+f.y){
						let array: &mut [u8; 4] = c.try_into().unwrap();
						*array = [*pd, *pd, *pd, 255];
					}
				}
			}
		}
		surface.damage(conn, 0, 0, self.width as i32, self.height as i32);
		self.needs_drawing = true;
	}
}
