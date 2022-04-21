#![allow(unused_must_use)]

use std::process::Command;
use smithay_client_toolkit::shell::xdg::window;

use smithay_client_toolkit::{
	shell::layer::{Anchor, LayerHandler, LayerState, LayerSurface, LayerSurfaceConfigure, LayerSurfaceBuilder},
};

mod hugo_client_toolkit;
use crate::hugo_client_toolkit::{Motion, Hlib, Layer};

fn main() {
	env_logger::init();

	let mut simple_layer = HClient {
		width: 256,
		height: 256,
		needs_drawing: true,
	};

	Hlib::run(simple_layer);
}


impl Layer for HClient {
	fn layer(&mut self) -> LayerSurfaceBuilder {
		LayerSurface::builder()
		.size((0, 20))
		.margin(0, 20, 0, 20)
		.anchor(Anchor::RIGHT | Anchor::LEFT | Anchor::BOTTOM)
		.namespace("bottom edge gestures")
		.exclusive_zone(-1)
	}
	fn change_size(&mut self, proposed_size: (u32, u32)) {
		if proposed_size.0 == 0 || proposed_size.1 == 0 {
			self.width = 20;
			self.height = 256;
		} else {
			self.width = proposed_size.0;
			self.height = proposed_size.1;
		}
	}
}

impl Motion for HClient {
	fn motion(&self, startpoint: &mut Option<(f64,f64)>, position: (f64,f64)){
		if let Some(sp) = startpoint {
			let dify = position.1 - sp.1;
			let difx = position.0 - sp.0;
			if dify.abs() < 10. {
				if difx <= -20. {
					Command::new("sh").args(["gestures.sh", "BottomEdgeSlideLeft", &(-difx - 19.9).to_string()]).status();
					*startpoint = Some(position);
					println!("BottomEdgeSlideLeft");
					return;
				}
				if difx >= 20. {
					Command::new("sh").args(["gestures.sh", "BottomEdgeSlideRight", &(difx - 19.9).to_string()]).status();
					*startpoint = Some(position);
					println!("BottomEdgeSlideRight");
					return;
				}
			}

			if difx.abs() < 40. {
				if dify >= -80. {
					*startpoint = None;
					if position.0 < (self.width / 3).into() {
						Command::new("sh").args(["gestures.sh", "BottomEdgePullLeft"]).status();
						println!("BottomEdgePullLeft");
					} else if position.0 < (2 * self.width / 3).into() {
						Command::new("sh").args(["gestures.sh", "BottomEdgePullMid"]).status();
						println!("BottomEdgePullMid");
					} else {
						Command::new("sh").args(["gestures.sh", "BottomEdgePullRight"]).status();
						println!("BottomEdgePullRight");
					}
				}
			}
		}
	}
}

pub struct HClient {
	width: u32,
	height: u32,
	needs_drawing: bool,
}

impl HClient {
	fn draw(
		&mut self,
		buffer: &mut [u8],
		surface: &wayland_client::protocol::wl_surface::WlSurface,
		conn: &mut wayland_client::ConnectionHandle<'_>,
	) {
		println!("Non-Drawing");
	}
}
