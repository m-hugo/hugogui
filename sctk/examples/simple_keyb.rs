#![allow(unused_variables)]

mod predictor;
use predictor::SimpleWordPredictor;
use std::path::Path;

mod virtual_keyboard;
// all hail rustc use virtual_keyboard::{delegate_vk, VkState, VkHandler};
use crate::virtual_keyboard::{VkHandler, VkState};

use wayland_protocols::misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use wayland_protocols::misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;

use crate::wl_surface::WlSurface;
use smithay_client_toolkit::seat::touch::TouchHandler;

use wayland_client::protocol::wl_touch::WlTouch;

use rusttype::{point, Font, Scale}; //PositionedGlyph
use std::convert::TryInto;

use smithay_client_toolkit::{
	compositor::{CompositorHandler, CompositorState},
	delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer, delegate_registry, delegate_seat, delegate_shm,
	delegate_touch,
	output::{OutputHandler, OutputState},
	registry::{ProvidesRegistryState, RegistryState},
	seat::{
		keyboard::{KeyEvent, KeyboardHandler, Modifiers},
		pointer::{PointerHandler, PointerScroll},
		Capability, SeatHandler, SeatState,
	},
	shell::layer::{Anchor, Layer, LayerHandler, LayerState, LayerSurface, LayerSurfaceConfigure},
	shm::{pool::raw::RawPool, ShmHandler, ShmState},
};
use wayland_client::{
	delegate_dispatch,
	protocol::{wl_buffer, wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface, wl_touch},
	Connection, ConnectionHandle, Dispatch, QueueHandle,
};

mod keymap;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::os::unix::io::IntoRawFd;
use std::time::Instant;
use tempfile::tempfile;
/*
macro_rules! delegate_vk {
	($ty: ty) => {
		delegate_dispatch!($ty: [
			ZwpVirtualKeyboardManagerV1
			//ZwpVirtualKeyboardV1
		] => VkState);
	};
}

impl DelegateDispatchBase<ZwpVirtualKeyboardManagerV1> for VkState {
	type UserData = LayerSurfaceData;
}
*/

fn main() {
	env_logger::init();

	let conn = Connection::connect_to_env().unwrap();

	let display = conn.handle().display();

	let mut event_queue = conn.new_event_queue();
	let qh = event_queue.handle();

	let registry = display.get_registry(&mut conn.handle(), &qh, ()).unwrap();

	let predic = SimpleWordPredictor::from_file(Path::new("./examples/training_data.csv"));
	println!("{}", predic.predict("keyb")[0].word.strip_prefix("keyb").unwrap());

	let mut simple_layer = SimpleLayer {
		registry_state: RegistryState::new(registry),
		seat_state: SeatState::new(),
		output_state: OutputState::new(),
		compositor_state: CompositorState::new(),
		shm_state: ShmState::new(),
		layer_state: LayerState::new(),
		vk_state: VkState::new(),
		vk: None,

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
		touchscreen: None,
		frames: vec![],
		wordtyped: String::new(),
		predic,
		font: include_bytes!("../WenQuanYiMicroHei.ttf").to_vec(),
		pointer_position: (0.0, 0.0),
		time: Instant::now(),
		needs_drawing: true,
	};
	event_queue.blocking_dispatch(&mut simple_layer).unwrap();
	// event_queue.blocking_dispatch(&mut simple_layer).unwrap();

	simple_layer.vk = Some(
		simple_layer
			.vk_state
			.0
			.as_ref()
			.unwrap()
			.create_virtual_keyboard(&mut conn.handle(), &simple_layer.seat_state.seats().next().unwrap(), &qh, ())
			.unwrap(),
	);

	let src = keymap::KEYMAP;
	let keymap_size = keymap::KEYMAP.len();
	let keymap_size_u32: u32 = keymap_size.try_into().unwrap(); // Convert it from usize to u32, panics if it is not possible
	let keymap_size_u64: u64 = keymap_size.try_into().unwrap(); // Convert it from usize to u64, panics if it is not possible
	let mut keymap_file = tempfile().expect("Unable to create tempfile");
	// Allocate space in the file first
	keymap_file.seek(SeekFrom::Start(keymap_size_u64)).unwrap();
	keymap_file.write_all(&[0]).unwrap();
	keymap_file.seek(SeekFrom::Start(0)).unwrap();
	let mut data = unsafe { memmap2::MmapOptions::new().map_mut(&keymap_file).expect("Could not access data from memory mapped file") };
	data[..src.len()].copy_from_slice(src.as_bytes());
	let keymap_raw_fd = keymap_file.into_raw_fd();

	simple_layer.vk.as_ref().unwrap().keymap(&mut conn.handle(), 1, keymap_raw_fd, keymap_size_u32);

	let pool = simple_layer
		.shm_state
		.new_raw_pool(simple_layer.width as usize * simple_layer.height as usize * 4, &mut conn.handle(), &qh, ())
		.expect("Failed to create pool");
	simple_layer.pool = Some(pool);

	let surface = simple_layer.compositor_state.create_surface(&mut conn.handle(), &qh).unwrap();

	let layer = LayerSurface::builder()
		.size((0, 220))
		.anchor(Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT)
		.namespace("sample_layer")
		.exclusive_zone(220)
		.map(&mut conn.handle(), &qh, &mut simple_layer.layer_state, surface, Layer::Overlay)
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
	vk_state: VkState,
	vk: Option<ZwpVirtualKeyboardV1>,

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
	touchscreen: Option<wl_touch::WlTouch>,
	frames: Vec<Frame>,
	wordtyped: String,
	predic: SimpleWordPredictor,
	font: Vec<u8>,
	pointer_position: (f64, f64),
	time: Instant,
	needs_drawing: bool,
}
impl VkHandler for SimpleLayer {
	fn vk_state(&mut self) -> &mut VkState {
		&mut self.vk_state
	}
}

struct Frame {
	pressed: bool,
	needs_damage: bool,
	activated: bool,
	x: f64,
	y: f64,
	width: f64,
	height: f64,
	s: String,
}

impl CompositorHandler for SimpleLayer {
	fn compositor_state(&mut self) -> &mut CompositorState {
		&mut self.compositor_state
	}

	fn scale_factor_changed(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _surface: &wl_surface::WlSurface, _new_factor: i32) {}

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

		let width = self.width as f64;
		let height = (self.height - 50) as f64;

		let rows = 5;
		let cols = 11;
		let vectxt: Vec<_> = vec![
			"1 !", "2 @", "3 #", "4 $", "5 %", "6 ^", "7 &", "8 *", "9 (", "0 )", "<--", "Tab", "q Q", "w W", "e E", "r R", "t T", "y Y", "u U",
			"i I", "o O", "p P", "a A", "s S", "d D", "f F", "g G", "h H", "   J", "k K", "l L", "; :", "<-/", "Shift", "z Z", "x X", "c C", "v V",
			"b B", "n N", "m M", ", <", ". >", "/?", "Ctrl", "Alt", "\\ |", "<-", "->", "------", "cp", "o", "p", "f", "cut",
		];
		for numrow in 0..rows {
			for numcol in 0..cols {
				let frame = Frame {
					pressed: false,
					needs_damage: false,
					activated: true,
					x: numcol as f64 * width / cols as f64,
					y: numrow as f64 * height / rows as f64,
					width: width / cols as f64,
					height: height / rows as f64,
					s: vectxt[cols * numrow + numcol].to_string(),
				};
				self.frames.push(frame);
			}
		}
		for n in 0..3 {
			self.frames.push(Frame {
				pressed: false,
				needs_damage: false,
				activated: false,
				x: 20.0 + (width / 3.0 + 20.0) * n as f64,
				y: 190.0,
				height: 20.0,
				width: (width / 3.0) - 20.0 * 4.,
				s: "".to_string(),
			});
		}

		// Initiate the first draw.
		if self.first_configure {
			self.draw(conn, qh);
			self.first_configure = false;
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

		if capability == Capability::Touch && self.touchscreen.is_none() {
			println!("Set pointer capability");
			let touchscreen = self.seat_state.get_touch(conn, qh, &seat).expect("Failed to create pointer");
			self.touchscreen = Some(touchscreen);
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
fn chartokc(c: char) -> u32 {
	match c {
		'q' => 20,
		'w' => 21,
		'e' => 22,
		'r' => 23,
		't' => 24,
		'y' => 25,
		'u' => 26,
		'i' => 27,
		'o' => 28,
		'p' => 29,
		'a' => 30,
		's' => 31,
		'd' => 32,
		'f' => 33,
		'g' => 34,
		'h' => 35,
		'j' => 36,
		'k' => 37,
		'l' => 38,
		'z' => 42,
		'x' => 43,
		'c' => 44,
		'v' => 45,
		'b' => 46,
		'n' => 47,
		'm' => 48,
		_ => 23,
	}
}

fn into_keycodes(s: &str) -> impl IntoIterator<Item = u32> + '_ {
	s.chars().map(chartokc)
}
impl SimpleLayer {
	fn touchframe(&mut self, position: (f64, f64), conn: &mut ConnectionHandle<'_>) {
		let vk = self.vk.as_mut().unwrap();
		let len = self.frames.len();
		for i in 0..len {
			if self.frames[i].activated
				&& self.frames[i].x < position.0
				&& self.frames[i].y < position.1
				&& self.frames[i].x + self.frames[i].width > position.0
				&& self.frames[i].y + self.frames[i].height > position.1
			{
				if i != 33 {
					if i >= len - 3 {
						for k in into_keycodes(&self.frames[i].s.strip_prefix(self.wordtyped.as_str()).unwrap()) {
							//self.predic.predict(&self.wordtyped)[i-100].word.strip_prefix(self.wordtyped).unwrap().into_keycodes(){
							vk.key(conn, get_time(self.time), k - 8, 1);
							vk.key(conn, get_time(self.time), k - 8, 0);
						}
					} else if self.frames[33].pressed {
						vk.key(conn, get_time(self.time), i.try_into().unwrap(), 1);
						println!("HELLO {}", self.frames[i].s);
						vk.key(conn, get_time(self.time), i.try_into().unwrap(), 0);
						self.frames[33].pressed = false;
						//vk.key(conn, get_time(self.time), 33, 0);
						vk.modifiers(conn, 0, 0, 0, 0);
						self.frames[33].needs_damage = true;
					} else if i <= 5 {
						vk.key(conn, get_time(self.time), i.try_into().unwrap(), 1);
						println!("{}", self.frames[i].s.to_lowercase());
						vk.key(conn, get_time(self.time), i.try_into().unwrap(), 0);
					} else {
						vk.key(conn, get_time(self.time), i.try_into().unwrap(), 1);
						println!("{}", self.frames[i].s);
						vk.key(conn, get_time(self.time), i.try_into().unwrap(), 0);
					}
					if i == 49 || i >= len - 3 {
						self.wordtyped.clear();
						self.frames[len - 3].needs_damage = true;
						self.frames[len - 3].s.clear();
						self.frames[len - 2].needs_damage = true;
						self.frames[len - 2].s.clear();
						self.frames[len - 1].needs_damage = true;
						self.frames[len - 1].s.clear();
						self.frames[len - 3].activated = false;
						self.frames[len - 2].activated = false;
						self.frames[len - 1].activated = false;
					} else {
						self.wordtyped.push(self.frames[i].s.chars().nth(0).unwrap());
						if !self.wordtyped.is_empty() {
							//println!("{}", self.predic.predict("keyb")[0].word.strip_prefix("keyb").unwrap());
							let preds = self.predic.predict(&self.wordtyped);
							self.frames[len - 3].s = if preds.len() > 0 {
								self.frames[len - 3].activated = true;
								preds[0].word.clone()
							} else {
								self.frames[len - 3].activated = false;
								"No Prediction".to_string()
							};
							self.frames[len - 3].needs_damage = true;
							self.frames[len - 2].s = if preds.len() > 1 {
								self.frames[len - 2].activated = true;
								preds[1].word.clone()
							} else {
								self.frames[len - 2].activated = false;
								"No Prediction".to_string()
							};
							self.frames[len - 2].needs_damage = true;
							self.frames[len - 1].s = if preds.len() > 2 {
								self.frames[len - 1].activated = true;
								preds[2].word.clone()
							} else {
								self.frames[len - 1].activated = false;
								"No Prediction".to_string()
							};
							self.frames[len - 1].needs_damage = true;
						}
					}
				} else {
					self.frames[i].pressed = true;
					//vk.key(conn, get_time(self.time), 33, 1);
					vk.modifiers(conn, 1, 0, 0, 0);
					self.frames[i].needs_damage = true;
				}
				self.needs_drawing = true;
				println!("trued");
				return;
			}
		}
		//if x==space: self.wordtyped = String::new()
		//else if x==letter: self.wordtyped += x
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
		self.touchframe(position, conn);
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
			self.pointer_position = position;
		}
	}

	fn pointer_press_button(
		&mut self,
		conn: &mut ConnectionHandle,
		_qh: &QueueHandle<Self>,
		_pointer: &wl_pointer::WlPointer,
		time: u32,
		button: u32,
	) {
		if self.pointer_focus {
			self.touchframe(self.pointer_position, conn);
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

	fn pointer_axis(&mut self, _: &mut ConnectionHandle, _: &QueueHandle<Self>, _: &wl_pointer::WlPointer, time: u32, scroll: PointerScroll) {}
}

impl ShmHandler for SimpleLayer {
	fn shm_state(&mut self) -> &mut ShmState {
		&mut self.shm_state
	}
}
fn get_time(base_time: Instant) -> u32 {
	let duration = base_time.elapsed();
	let time = duration.as_millis();
	time.try_into().unwrap()
}
impl SimpleLayer {
	pub fn draw(&mut self, conn: &mut ConnectionHandle, qh: &QueueHandle<Self>) {
		if let Some(window) = self.layer.as_ref() {
			if self.needs_drawing {
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
					let font = Font::try_from_bytes(&self.font).expect("error constructing a Font from bytes");
					let mut pixel_data = vec![0; self.width as usize * self.height as usize];

					let width = self.width as usize;
					for f in &mut self.frames {
						if f.needs_damage || self.first_configure {
							let scale = Scale { x: (f.height * 1.5 - 2.0) as f32, y: f.height as f32 - 2.0 };
							let v_metrics = font.v_metrics(scale);
							let offset = point(0.0, v_metrics.ascent);
							for g in font.layout(&f.s, scale, offset) {
								if let Some(bb) = g.pixel_bounding_box() {
									g.draw(|x, y, v| {
										let x = (x + bb.min.x as u32) as usize;
										let y = (y + bb.min.y as u32) as usize;
										pixel_data[((x + f.x as usize) + (y + f.y as usize) * width)] =
											if f.pressed { v * 255.0 } else { v * 155.0 } as u8;
									})
								}
							}
						}
						if f.needs_damage {
							window.wl_surface().damage(conn, f.x as i32, f.y as i32, f.width as i32, f.height as i32);
							println!("damaged");
							f.needs_damage = false;
						}
					}
					//println!("{}", self.frames[0].pressed);
					//let scale = Scale { x: 20.0, y: 20.0};

					window.wl_surface().damage(conn, 160 as i32, 180 as i32, 200, 200 as i32);

					buffer.chunks_exact_mut(4).enumerate().for_each(|(index, chunk)| {
						let a = 255;
						let r: u8 = pixel_data[index];
						let g: u8 = pixel_data[index];
						let b: u8 = pixel_data[index];

						let array: &mut [u8; 4] = chunk.try_into().unwrap();
						*array = [b, g, r, a];
					});
				}

				self.buffer = Some(wl_buffer);
				println!("buffer replaced");
				//window.wl_surface().damage(conn, 0, 0, 2000, 2000);
				assert!(self.buffer.is_some(), "No buffer?");
				window.wl_surface().attach(conn, self.buffer.as_ref(), 0, 0);
				self.needs_drawing = false;
			}
			window.wl_surface().frame(conn, qh, window.wl_surface().clone()).expect("create callback");
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

delegate_vk!(SimpleLayer);

delegate_registry!(SimpleLayer: [
	CompositorState,
	OutputState,
	ShmState,
	SeatState,
	LayerState,
	VkState,
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
