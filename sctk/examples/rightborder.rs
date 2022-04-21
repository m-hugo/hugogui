#![allow(unused_must_use)]

use smithay_client_toolkit::{
	compositor::{CompositorHandler, CompositorState},
	delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_seat, delegate_shm, delegate_touch,
	output::{OutputHandler, OutputState},
	registry::{ProvidesRegistryState, RegistryState},
	seat::{touch, Capability, SeatHandler, SeatState},
	shell::layer::{Anchor, Layer, LayerHandler, LayerState, LayerSurface, LayerSurfaceConfigure},
	shm::{pool::raw::RawPool, ShmHandler, ShmState},
};
use std::process::Command;
use wayland_client::{
	protocol::{wl_buffer, wl_output, wl_seat, wl_shm, wl_surface, wl_touch},
	Connection, ConnectionHandle, Dispatch, QueueHandle,
};

fn main() {
	env_logger::init();

	let conn = Connection::connect_to_env().unwrap();

	let display = conn.handle().display();

	let mut event_queue = conn.new_event_queue();
	let qh = event_queue.handle();

	let registry = display.get_registry(&mut conn.handle(), &qh, ()).unwrap();

	let mut simple_layer = SimpleLayer {
		registry_state: RegistryState::new(registry),
		seat_state: SeatState::new(),
		output_state: OutputState::new(),
		compositor_state: CompositorState::new(),
		shm_state: ShmState::new(),
		layer_state: LayerState::new(),

		first_configure: true,
		pool: None,
		width: 256,
		height: 256,
		buffer: None,
		layer: None,
		touchscreen: None,
		startpoint: None,
	};

	event_queue.blocking_dispatch(&mut simple_layer).unwrap();

	let pool = simple_layer
		.shm_state
		.new_raw_pool(simple_layer.width as usize * simple_layer.height as usize * 4, &mut conn.handle(), &qh, ())
		.expect("Failed to create pool");
	simple_layer.pool = Some(pool);

	let surface = simple_layer.compositor_state.create_surface(&mut conn.handle(), &qh).unwrap();

	let layer = LayerSurface::builder()
		.size((20, 0))
		.margin(20, 0, 20, 0)
		.anchor(Anchor::TOP | Anchor::RIGHT | Anchor::BOTTOM)
		.namespace("right edge gestures")
		.exclusive_zone(-1)
		.map(&mut conn.handle(), &qh, &mut simple_layer.layer_state, surface, Layer::Overlay)
		.expect("layer surface creation");

	simple_layer.layer = Some(layer);

	loop {
		event_queue.blocking_dispatch(&mut simple_layer).unwrap();
	}
}

struct SimpleLayer {
	registry_state: RegistryState,
	seat_state: SeatState,
	output_state: OutputState,
	compositor_state: CompositorState,
	shm_state: ShmState,
	layer_state: LayerState,

	first_configure: bool,
	pool: Option<RawPool>,
	width: u32,
	height: u32,
	buffer: Option<wl_buffer::WlBuffer>,
	layer: Option<LayerSurface>,
	touchscreen: Option<wl_touch::WlTouch>,
	startpoint: Option<(f64, f64)>,
}

impl CompositorHandler for SimpleLayer {
	fn compositor_state(&mut self) -> &mut CompositorState {
		&mut self.compositor_state
	}

	fn scale_factor_changed(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _surface: &wl_surface::WlSurface, _new_factor: i32) {}

	fn frame(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _surface: &wl_surface::WlSurface, _time: u32) {}
}

impl OutputHandler for SimpleLayer {
	fn output_state(&mut self) -> &mut OutputState {
		&mut self.output_state
	}

	fn new_output(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _output: wl_output::WlOutput) {}

	fn update_output(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _output: wl_output::WlOutput) {}

	fn output_destroyed(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _output: wl_output::WlOutput) {}
}

impl LayerHandler for SimpleLayer {
	fn layer_state(&mut self) -> &mut LayerState {
		&mut self.layer_state
	}

	fn closed(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {}

	fn configure(
		&mut self,
		conn: &mut ConnectionHandle,
		qh: &QueueHandle<Self>,
		_layer: &LayerSurface,
		configure: LayerSurfaceConfigure,
		_serial: u32,
	) {
		println!("Configure");
		if configure.new_size.0 == 0 || configure.new_size.1 == 0 {
			self.width = 20;
			self.height = 256;
		} else {
			self.width = configure.new_size.0;
			self.height = configure.new_size.1;
		}

		if self.first_configure {
			self.first_configure = false;

			if let Some(window) = self.layer.as_ref() {
				self.pool.as_mut().unwrap().resize((self.width * self.height * 4) as usize, conn).expect("resize pool");

				let pool = self
					.pool
					.as_mut()
					.unwrap()
					.create_buffer(0, self.width as i32, self.height as i32, self.width as i32 * 4, wl_shm::Format::Argb8888, (), conn, qh)
					.expect("create buffer");

				self.buffer = Some(pool);

				window.wl_surface().attach(conn, self.buffer.as_ref(), 0, 0);
				window.wl_surface().commit(conn);
			}
		}
	}
}

impl SeatHandler for SimpleLayer {
	fn new_capability(&mut self, conn: &mut ConnectionHandle, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, capability: Capability) {
		if capability == Capability::Touch && self.touchscreen.is_none() {
			println!("Set Touch capability");
			let touchscreen = self.seat_state.get_touch(conn, qh, &seat).expect("Failed to create Touch");
			self.touchscreen = Some(touchscreen);
		}
	}

	fn seat_state(&mut self) -> &mut SeatState {
		&mut self.seat_state
	}
	fn new_seat(&mut self, _: &mut ConnectionHandle, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
	fn remove_capability(&mut self, _conn: &mut ConnectionHandle, _: &QueueHandle<Self>, _: wl_seat::WlSeat, _capability: Capability) {}
	fn remove_seat(&mut self, _: &mut ConnectionHandle, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl touch::TouchHandler for SimpleLayer {
	#[allow(clippy::too_many_arguments)]
	fn down(
		&mut self,
		_conn: &mut ConnectionHandle,
		_qh: &QueueHandle<Self>,
		_touch: &wl_touch::WlTouch,
		_serial: u32,
		_time: u32,
		_surface: wl_surface::WlSurface,
		_id: i32,
		position: (f64, f64),
	) {
		self.startpoint = Some(position);
	}

	fn up(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _touch: &wl_touch::WlTouch, _serial: u32, _time: u32, _id: i32) {
		self.startpoint = None;
	}

	fn motion(
		&mut self,
		_conn: &mut ConnectionHandle,
		_qh: &QueueHandle<Self>,
		_touch: &wl_touch::WlTouch,
		_time: u32,
		_id: i32,
		position: (f64, f64),
	) {
		if let Some(sp) = self.startpoint {
			let dify = position.1 - sp.1;
			let difx = position.0 - sp.0;
			if difx.abs() < 10. {
				if dify <= -20. {
					Command::new("sh").args(["gestures.sh", "RightEdgeSlideUp", &(-dify - 19.9).to_string()]).status();
					self.startpoint = Some(position);
					println!("RightEdgeSlideUp");
					return;
				}
				if dify >= 20. {
					Command::new("sh").args(["gestures.sh", "RightEdgeSlideDown", &(dify - 19.9).to_string()]).status();
					self.startpoint = Some(position);
					println!("RightEdgeSlideDown");
					return;
				}
			}

			if dify.abs() < 40. {
				if difx <= -80. {
					self.startpoint = None;
					if position.1 < (self.height / 3).into() {
						Command::new("sh").args(["gestures.sh", "RightEdgePullUp"]).status();
						println!("RightEdgePullUp");
					} else if position.1 < (2 * self.height / 3).into() {
						Command::new("sh").args(["gestures.sh", "RightEdgePullMid"]).status();
						println!("RightEdgePullMid");
					} else {
						Command::new("sh").args(["gestures.sh", "RightEdgePullDown"]).status();
						println!("RightEdgePullDown");
					}
				}
			}
		}
	}

	fn shape(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _touch: &wl_touch::WlTouch, _id: i32, _major: f64, _minor: f64) {}

	fn orientation(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _touch: &wl_touch::WlTouch, _id: i32, _orientation: f64) {}

	fn cancel(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _touch: &wl_touch::WlTouch) {}
}

delegate_compositor!(SimpleLayer);
delegate_output!(SimpleLayer);
delegate_shm!(SimpleLayer);
delegate_seat!(SimpleLayer);
delegate_touch!(SimpleLayer);
delegate_layer!(SimpleLayer);
delegate_registry!(SimpleLayer: [
	CompositorState,
	OutputState,
	ShmState,
	SeatState,
	LayerState,
]);

impl ShmHandler for SimpleLayer {
	fn shm_state(&mut self) -> &mut ShmState {
		&mut self.shm_state
	}
}

impl ProvidesRegistryState for SimpleLayer {
	fn registry(&mut self) -> &mut RegistryState {
		&mut self.registry_state
	}
}

impl Dispatch<wl_buffer::WlBuffer> for SimpleLayer {
	type UserData = ();

	fn event(
		&mut self,
		_: &wl_buffer::WlBuffer,
		_: wl_buffer::Event,
		_: &Self::UserData,
		_: &mut wayland_client::ConnectionHandle,
		_: &wayland_client::QueueHandle<Self>,
	) {
	}
}
