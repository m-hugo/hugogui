//! This example is horrible. Please make a better one soon.
#![allow(unused_variables)]

use crate::wl_surface::WlSurface;
use smithay_client_toolkit::seat::touch::TouchHandler;
use std::process::Command;
use wayland_client::protocol::wl_touch::WlTouch;

use std::convert::TryInto;

use smithay_client_toolkit::{
	compositor::{CompositorHandler, CompositorState},
	delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer, delegate_registry, delegate_seat, delegate_shm,
	delegate_touch,
	output::{OutputHandler, OutputState},
	registry::{ProvidesRegistryState, RegistryState},
	seat::{
		keyboard::{KeyEvent, KeyboardHandler, Modifiers},
		pointer::{AxisKind, PointerHandler, PointerScroll},
		Capability, SeatHandler, SeatState,
	},
	shell::layer::{Anchor, Layer, LayerHandler, LayerState, LayerSurface, LayerSurfaceConfigure},
	shm::{pool::raw::RawPool, ShmHandler, ShmState},
};
use wayland_client::{
	protocol::{wl_buffer, wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
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

		exit: false,
		first_configure: true,
		pool: None,
		width: 256,
		height: 256,
		buffer: None,
		layer: None,
		keyboard: None,
		keyboard_focus: false,
		pointer: None,
		pointer_focus: false,
	};

	event_queue.blocking_dispatch(&mut simple_layer).unwrap();
	// event_queue.blocking_dispatch(&mut simple_layer).unwrap();

	let pool = simple_layer
		.shm_state
		.new_raw_pool(simple_layer.width as usize * simple_layer.height as usize * 4, &mut conn.handle(), &qh, ())
		.expect("Failed to create pool");
	simple_layer.pool = Some(pool);

	let surface = simple_layer.compositor_state.create_surface(&mut conn.handle(), &qh).unwrap();

	let layer = LayerSurface::builder()
		.size((0, 30))
		.anchor(Anchor::TOP | Anchor::LEFT | Anchor::RIGHT)
		.namespace("sample_layer")
		.exclusive_zone(30)
		.map(&mut conn.handle(), &qh, &mut simple_layer.layer_state, surface, Layer::Top)
		.expect("layer surface creation");

	simple_layer.layer = Some(layer);

	// We don't draw immediately, the configure will notify us when to first draw.

	loop {
		event_queue.blocking_dispatch(&mut simple_layer).unwrap();

		if simple_layer.exit {
			println!("exiting example");
			break;
		}
	}
}

struct SimpleLayer {
	registry_state: RegistryState,
	seat_state: SeatState,
	output_state: OutputState,
	compositor_state: CompositorState,
	shm_state: ShmState,
	layer_state: LayerState,

	exit: bool,
	first_configure: bool,
	pool: Option<RawPool>,
	width: u32,
	height: u32,
	buffer: Option<wl_buffer::WlBuffer>,
	layer: Option<LayerSurface>,
	keyboard: Option<wl_keyboard::WlKeyboard>,
	keyboard_focus: bool,
	pointer: Option<wl_pointer::WlPointer>,
	pointer_focus: bool,
}

impl CompositorHandler for SimpleLayer {
	fn compositor_state(&mut self) -> &mut CompositorState {
		&mut self.compositor_state
	}

	fn scale_factor_changed(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _surface: &wl_surface::WlSurface, _new_factor: i32) {
		// Not needed for this example.
	}

	fn frame(&mut self, conn: &mut ConnectionHandle, qh: &QueueHandle<Self>, _surface: &wl_surface::WlSurface, _time: u32) {
		self.draw(conn, qh);
	}
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

	fn closed(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
		self.exit = true;
	}

	fn configure(
		&mut self,
		conn: &mut ConnectionHandle,
		qh: &QueueHandle<Self>,
		_layer: &LayerSurface,
		configure: LayerSurfaceConfigure,
		_serial: u32,
	) {
		dbg!("Configure");
		if configure.new_size.0 == 0 || configure.new_size.1 == 0 {
			self.width = 256;
			self.height = 256;
		} else {
			self.width = configure.new_size.0;
			self.height = configure.new_size.1;
		}

		// Initiate the first draw.
		if self.first_configure {
			self.first_configure = false;
			self.draw(conn, qh);
		}
	}
}

impl SeatHandler for SimpleLayer {
	fn seat_state(&mut self) -> &mut SeatState {
		&mut self.seat_state
	}

	fn new_seat(&mut self, _: &mut ConnectionHandle, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

	fn new_capability(&mut self, conn: &mut ConnectionHandle, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, capability: Capability) {
		if capability == Capability::Keyboard && self.keyboard.is_none() {
			println!("Set keyboard capability");
			let keyboard = self.seat_state.get_keyboard(conn, qh, &seat, None).expect("Failed to create keyboard");
			self.keyboard = Some(keyboard);
		}

		if capability == Capability::Pointer && self.pointer.is_none() {
			println!("Set pointer capability");
			let pointer = self.seat_state.get_pointer(conn, qh, &seat).expect("Failed to create pointer");
			self.pointer = Some(pointer);
		}
	}

	fn remove_capability(&mut self, conn: &mut ConnectionHandle, _: &QueueHandle<Self>, _: wl_seat::WlSeat, capability: Capability) {
		if capability == Capability::Keyboard && self.keyboard.is_some() {
			println!("Unset keyboard capability");
			self.keyboard.take().unwrap().release(conn);
		}

		if capability == Capability::Pointer && self.pointer.is_some() {
			println!("Unset pointer capability");
			self.pointer.take().unwrap().release(conn);
		}
	}

	fn remove_seat(&mut self, _: &mut ConnectionHandle, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for SimpleLayer {
	fn enter(
		&mut self,
		_: &mut ConnectionHandle,
		_: &QueueHandle<Self>,
		_: &wl_keyboard::WlKeyboard,
		surface: &wl_surface::WlSurface,
		_: u32,
		_: &[u32],
		keysyms: &[u32],
	) {
		if self.layer.as_ref().map(LayerSurface::wl_surface) == Some(surface) {
			println!("Keyboard focus on window with pressed syms: {:?}", keysyms);
			self.keyboard_focus = true;
		}
	}

	fn leave(&mut self, _: &mut ConnectionHandle, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, surface: &wl_surface::WlSurface, _: u32) {
		if self.layer.as_ref().map(LayerSurface::wl_surface) == Some(surface) {
			println!("Release keyboard focus on window");
			self.keyboard_focus = false;
		}
	}

	fn press_key(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, event: KeyEvent) {
		println!("Key press: {:?}", event);
		self.exit = true;
	}

	fn release_key(&mut self, _: &mut ConnectionHandle, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, event: KeyEvent) {
		println!("Key release: {:?}", event);
	}

	fn update_modifiers(&mut self, _: &mut ConnectionHandle, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _serial: u32, modifiers: Modifiers) {
		println!("Update modifiers: {:?}", modifiers);
	}
}
impl TouchHandler for SimpleLayer {
	///
	/// Indicates a new touch point has appeared on the surface, starting a touch sequence. The ID
	/// associated with this event identifies this touch point for devices with multi-touch and
	/// will be referenced in future events.
	///
	/// The associated touch ID ceases to be valid after the touch up event with the associated ID
	/// and may be reused for other touch points after that.
	///
	/// Coordinates are surface-local.
	#[allow(clippy::too_many_arguments)]
	fn down(
		&mut self,
		conn: &mut ConnectionHandle,
		qh: &QueueHandle<Self>,
		touch: &WlTouch,
		serial: u32,
		time: u32,
		surface: WlSurface,
		id: i32,
		position: (f64, f64),
	) {
	}

	/// End of touch sequence.
	fn up(&mut self, conn: &mut ConnectionHandle, qh: &QueueHandle<Self>, touch: &WlTouch, serial: u32, time: u32, id: i32) {}

	/// Touch point motion.
	///
	/// Coordinates are surface-local.
	fn motion(&mut self, conn: &mut ConnectionHandle, qh: &QueueHandle<Self>, touch: &WlTouch, time: u32, id: i32, position: (f64, f64)) {}

	/// Touch point shape change.
	///
	/// The shape of a touch point is approximated by an ellipse through the major and minor axis
	/// length. Major always represents the larger of the two axis and is orthogonal to minor.
	///
	/// The dimensions are specified in surface-local coordinates and the locations reported by
	/// other events always report the center of the ellipse.
	fn shape(&mut self, conn: &mut ConnectionHandle, qh: &QueueHandle<Self>, touch: &WlTouch, id: i32, major: f64, minor: f64) {}

	/// Touch point shape orientation.
	///
	/// The orientation describes the clockwise angle of a touch point's major axis to the positive
	/// surface y-axis and is normalized to the -180° to +180° range.
	fn orientation(&mut self, conn: &mut ConnectionHandle, qh: &QueueHandle<Self>, touch: &WlTouch, id: i32, orientation: f64) {}

	/// Cancel active touch sequence.
	///
	/// This indicates that the compositor has cancelled the active touch sequence, for example due
	/// to detection of a touch gesture.
	fn cancel(&mut self, conn: &mut ConnectionHandle, qh: &QueueHandle<Self>, touch: &WlTouch) {}
}
impl PointerHandler for SimpleLayer {
	fn pointer_focus(
		&mut self,
		_conn: &mut ConnectionHandle,
		_qh: &QueueHandle<Self>,
		_pointer: &wl_pointer::WlPointer,
		surface: &wl_surface::WlSurface,
		entered: (f64, f64),
	) {
		if self.layer.as_ref().map(LayerSurface::wl_surface) == Some(surface) {
			println!("Pointer focus on layer, entering at {:?}", entered);
			self.pointer_focus = true;
		}
	}

	fn pointer_release_focus(
		&mut self,
		_conn: &mut ConnectionHandle,
		_qh: &QueueHandle<Self>,
		_pointer: &wl_pointer::WlPointer,
		surface: &wl_surface::WlSurface,
	) {
		if self.layer.as_ref().map(LayerSurface::wl_surface) == Some(surface) {
			println!("Release pointer focus on layer");
			self.pointer_focus = false;
		}
	}

	fn pointer_motion(
		&mut self,
		_conn: &mut ConnectionHandle,
		_qh: &QueueHandle<Self>,
		_pointer: &wl_pointer::WlPointer,
		time: u32,
		position: (f64, f64),
	) {
		if self.pointer_focus {
			//println!("Pointer motion: {:?} @ {}", position, time);
		}
	}

	fn pointer_press_button(
		&mut self,
		_conn: &mut ConnectionHandle,
		_qh: &QueueHandle<Self>,
		_pointer: &wl_pointer::WlPointer,
		time: u32,
		button: u32,
	) {
		if self.pointer_focus {
			println!("Pointer press button: {:?} @ {}", button, time);
		}
	}

	fn pointer_release_button(
		&mut self,
		_conn: &mut ConnectionHandle,
		_qh: &QueueHandle<Self>,
		_pointer: &wl_pointer::WlPointer,
		time: u32,
		button: u32,
	) {
		if self.pointer_focus {
			println!("Pointer release button: {:?} @ {}", button, time);
		}
	}

	fn pointer_axis(&mut self, _: &mut ConnectionHandle, _: &QueueHandle<Self>, _: &wl_pointer::WlPointer, time: u32, scroll: PointerScroll) {
		if self.pointer_focus {
			//println!("Pointer scroll: @ {}", time);

			if let Some(vertical) = scroll.axis(wl_pointer::Axis::VerticalScroll) {
				//println!("\nV: {:?}", vertical);
				if let AxisKind::Absolute(i) = vertical {
					if i < -0.3 {
						Command::new("light").args(["-A", &(-i / 3.0).to_string()]).status();
					}
					if i > 0.3 {
						Command::new("light").args(["-U", &(i / 3.0).to_string()]).status();
					}
				}

				if let Some(horizontal) = scroll.axis(wl_pointer::Axis::HorizontalScroll) {
					println!("\nH: {:?}", horizontal);
				}
			}
		}
	}
}

impl ShmHandler for SimpleLayer {
	fn shm_state(&mut self) -> &mut ShmState {
		&mut self.shm_state
	}
}

impl SimpleLayer {
	pub fn draw(&mut self, conn: &mut ConnectionHandle, qh: &QueueHandle<Self>) {
		if let Some(window) = self.layer.as_ref() {
			//window.wl_surface().frame(conn, qh, window.wl_surface().clone());
			// Ensure the pool is big enough to hold the new buffer.
			self.pool.as_mut().unwrap().resize((self.width * self.height * 4) as usize, conn).expect("resize pool");

			// Destroy the old buffer.
			// FIXME: Integrate this into the pool logic.
			if let Some(buffer) = self.buffer.take() {
				buffer.destroy(conn);
			}

			let offset = 0;
			let stride = self.width as i32 * 4;
			let pool = self.pool.as_mut().unwrap();

			let wl_buffer = pool
				.create_buffer(offset, self.width as i32, self.height as i32, stride, wl_shm::Format::Argb8888, (), conn, qh)
				.expect("create buffer");

			// TODO: Upgrade to a better pool type
			let len = self.height as usize * stride as usize; // length of a row
			let buffer = &mut pool.mmap()[offset as usize..][..len];

			// Draw to the window:
			{
				let width = self.width as usize;
				let height = self.height as usize;

				use rusttype::{point, Font, Scale};
				//let colour = (150, 0, 0);
				let font_data = include_bytes!("../WenQuanYiMicroHei.ttf");
				let font = Font::try_from_bytes(font_data as &[u8]).expect("error constructing a Font from bytes");

				// 2x scale in x direction to counter the aspect ratio of monospace characters.
				let scale = Scale { x: (height - 2) as f32 * 1.5, y: (height - 2) as f32 };

				// The origin of a line of text is at the baseline (roughly where
				// non-descending letters sit). We don't want to clip the text, so we shift
				// it down with an offset when laying it out. v_metrics.ascent is the
				// distance between the baseline and the highest edge of any glyph in
				// the font. That's enough to guarantee that there's no clipping.
				let v_metrics = font.v_metrics(scale);
				let offset = point(0.0, v_metrics.ascent);

				// Glyphs to draw for "RustType". Feel free to try other strings.
				let glyphs: Vec<_> = font.layout("Bar", scale, offset).collect();

				// Rasterise directly into ASCII art.
				let mut pixel_data = vec![0; width * height];
				//let mapping = b"@%#x+=:-. "; // The approximation of greyscale
				//let mapping_scale = (mapping.len() - 1) as f32;

				for g in glyphs {
					if let Some(bb) = g.pixel_bounding_box() {
						g.draw(|x, y, v| {
							// v should be in the range 0.0 to 1.0
							//let i = (v * mapping_scale + 0.5) as usize;
							// so something's wrong if you get $ in the output.
							//let c = mapping.get(i).cloned().unwrap_or(b'$');
							let x = x + bb.min.x as u32;
							let y = y + bb.min.y as u32;
							// There's still a possibility that the glyph clips the boundaries of the bitmap
							let x = x as usize;
							let y = y as usize;
							pixel_data[(x + y * width)] = (v * 255.0) as u8;
						})
					}
				}

				buffer.chunks_exact_mut(4).enumerate().for_each(|(index, chunk)| {
					//let x = (index % width as usize) as u32;
					//let y = (index / width as usize) as u32;

					let a = 255;
					let r: u8 = pixel_data[index]; //u32::min(((width - x) * 0xFF) / width, ((height - y) * 0xFF) / height);
					let g: u8 = pixel_data[index]; //u32::min((x * 0xFF) / width, ((height - y) * 0xFF) / height);
					let b: u8 = pixel_data[index]; //pixel_data[index].into();//u32::min(((width - x) * 0xFF) / width, (y * 0xFF) / height);
							   //let color:u32 = (a << 24) + (r << 16) + (g << 8) + b;

					let array: &mut [u8; 4] = chunk.try_into().unwrap();
					*array = [b, g, r, a]; //color.to_le_bytes();
				});
			}

			self.buffer = Some(wl_buffer);

			// Request our next frame
			window.wl_surface().frame(conn, qh, window.wl_surface().clone()).expect("create callback");

			assert!(self.buffer.is_some(), "No buffer?");
			// Attach and commit to present.
			window.wl_surface().attach(conn, self.buffer.as_ref(), 0, 0);
			window.wl_surface().commit(conn);
		}
	}
}

delegate_compositor!(SimpleLayer);
delegate_output!(SimpleLayer);
delegate_shm!(SimpleLayer);

delegate_seat!(SimpleLayer);
delegate_keyboard!(SimpleLayer);
delegate_pointer!(SimpleLayer);
delegate_touch!(SimpleLayer);

delegate_layer!(SimpleLayer);

delegate_registry!(SimpleLayer: [
	CompositorState,
	OutputState,
	ShmState,
	SeatState,
	LayerState,
]);

impl ProvidesRegistryState for SimpleLayer {
	fn registry(&mut self) -> &mut RegistryState {
		&mut self.registry_state
	}
}

// TODO
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
		// todo
	}
}
