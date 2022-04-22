#![allow(unused_imports)]
#![allow(dead_code)]
use smithay_client_toolkit::{
	compositor::{CompositorHandler, CompositorState},
	delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer, delegate_registry, delegate_seat, delegate_shm,
	delegate_touch, delegate_xdg_shell, delegate_xdg_window,
	output::{OutputHandler, OutputState},
	registry::{ProvidesRegistryState, RegistryState},
	seat::{
		keyboard::{KeyEvent, KeyboardHandler, Modifiers},
		pointer::{AxisKind, PointerHandler, PointerScroll},
		touch, Capability, SeatHandler, SeatState,
	},
	shell::layer,
	shell::layer::{Anchor, LayerHandler, LayerState, LayerSurface, LayerSurfaceBuilder, LayerSurfaceConfigure},
	shell::xdg::{
		window,
		window::{WindowConfigure, WindowHandler, XdgWindowState},
		XdgShellHandler, XdgShellState,
	},
	shm::{pool::raw::RawPool, ShmHandler, ShmState},
};
use wayland_client::{
	protocol::{wl_buffer, wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface, wl_touch},
	Connection, ConnectionHandle, Dispatch, QueueHandle,
};

pub trait HClient {
	fn width(&self) -> u32;
	fn height(&self) -> u32;
	fn draw(&mut self, buffer: &mut [u8], surface: &wl_surface::WlSurface, conn: &mut ConnectionHandle<'_>);
	fn needs_drawing(&self) -> bool;
	fn set_needs_drawing(&mut self, nd: bool);
	#[cfg(feature = "Window")]
	fn window(&self) -> window::WindowBuilder;
	#[cfg(feature = "Window")]
	fn change_size(&mut self, proposed_size: Option<(u32, u32)>);
	#[cfg(feature = "Clickable")]
	fn short_click(&mut self, position: (usize, usize));
	#[cfg(feature = "Clickable")]
	fn long_click(&mut self, position: (usize, usize));
	#[cfg(feature = "Keyboard")]
	fn press(&mut self, key: &str);
	#[cfg(feature = "Scrollable")]
	fn scroll(&mut self, amount: f64);
	#[cfg(feature = "Motion")]
	fn motion(&self, startpoint: &mut Option<(f64, f64)>, position: (f64, f64));
	#[cfg(feature = "Layer")]
	fn layer(&mut self) -> LayerSurfaceBuilder;
	#[cfg(feature = "Layer")]
	fn change_size(&mut self, proposed_size: (u32, u32));
}

pub struct Hlib {
	seat_state: SeatState,
	keyboard: Option<wl_keyboard::WlKeyboard>,
	keyboard_focus: bool,
	pointer: Option<wl_pointer::WlPointer>,
	pointer_focus: bool,
	pointer_position: (usize, usize),
	presstime: u32,
	touchscreen: Option<wl_touch::WlTouch>,
	startpoint: Option<(f64, f64)>,
	touch_position: (f64, f64),

	pool: Option<RawPool>,
	#[cfg(feature = "Window")]
	window: Option<window::Window>,
	buffer: Option<wl_buffer::WlBuffer>,
	exit: bool,
	first_configure: bool,

	registry_state: RegistryState,
	output_state: OutputState,
	compositor_state: CompositorState,
	shm_state: ShmState,
	#[cfg(feature = "Window")]
	xdg_shell_state: XdgShellState<Self>,
	#[cfg(feature = "Window")]
	xdg_window_state: XdgWindowState,
	#[cfg(feature = "Layer")]
	window: Option<LayerSurface>,
	#[cfg(feature = "Layer")]
	layer_state: LayerState,

	hclient: Box<dyn HClient>,
}

impl Hlib {
	#[cfg(feature = "Window")]
	pub fn run(hclient: impl HClient + 'static) {
		let conn = Connection::connect_to_env().unwrap();
		let display = conn.handle().display();
		let mut event_queue = conn.new_event_queue();
		let qh = event_queue.handle();
		let registry = display.get_registry(&mut conn.handle(), &qh, ()).unwrap();

		let mut hlib = Hlib {
			registry_state: RegistryState::new(registry),
			output_state: OutputState::new(),
			compositor_state: CompositorState::new(),
			shm_state: ShmState::new(),
			xdg_shell_state: XdgShellState::new(),
			xdg_window_state: XdgWindowState::new(),
			seat_state: SeatState::new(),
			keyboard: None,
			keyboard_focus: false,
			pointer: None,
			pointer_focus: false,
			pointer_position: (0, 0),
			presstime: 0,
			touchscreen: None,
			startpoint: None,
			touch_position: (0.0, 0.0),
			exit: false,
			first_configure: true,
			pool: None,
			buffer: None,
			window: None,
			hclient: Box::new(hclient),
		};

		event_queue.blocking_dispatch(&mut hlib).unwrap();
		event_queue.blocking_dispatch(&mut hlib).unwrap();

		let pool = hlib
			.shm_state
			.new_raw_pool(hlib.hclient.width() as usize * hlib.hclient.height() as usize * 4, &mut conn.handle(), &qh, ())
			.expect("Failed to create pool");
		hlib.pool = Some(pool);

		let surface = hlib.compositor_state.create_surface(&mut conn.handle(), &qh).unwrap();

		let window =
			hlib.hclient.window().map(&mut conn.handle(), &qh, &hlib.xdg_shell_state, &mut hlib.xdg_window_state, surface).expect("window creation");

		hlib.window = Some(window);

		loop {
			event_queue.blocking_dispatch(&mut hlib).unwrap();

			if hlib.exit {
				println!("exiting example");
				break;
			}
		}
	}
	#[cfg(feature = "Layer")]
	pub fn run(hclient: impl HClient) {
		let conn = Connection::connect_to_env().unwrap();
		let display = conn.handle().display();
		let mut event_queue = conn.new_event_queue();
		let qh = event_queue.handle();
		let registry = display.get_registry(&mut conn.handle(), &qh, ()).unwrap();

		let mut hlib = Hlib {
			registry_state: RegistryState::new(registry),
			output_state: OutputState::new(),
			compositor_state: CompositorState::new(),
			shm_state: ShmState::new(),
			window: None,
			layer_state: LayerState::new(),
			seat_state: SeatState::new(),
			keyboard: None,
			keyboard_focus: false,
			pointer: None,
			pointer_focus: false,
			pointer_position: (0, 0),
			presstime: 0,
			touchscreen: None,
			startpoint: None,
			touch_position: (0.0, 0.0),
			exit: false,
			first_configure: true,
			pool: None,
			buffer: None,
			hclient: Box::new(hclient),
		};

		event_queue.blocking_dispatch(&mut hlib).unwrap();
		event_queue.blocking_dispatch(&mut hlib).unwrap();

		let pool = hlib
			.shm_state
			.new_raw_pool(hlib.hclient.width() as usize * hlib.hclient.height() as usize * 4, &mut conn.handle(), &qh, ())
			.expect("Failed to create pool");
		hlib.pool = Some(pool);

		let surface = hlib.compositor_state.create_surface(&mut conn.handle(), &qh).unwrap();

		let layer =
			hlib.hclient.layer().map(&mut conn.handle(), &qh, &mut hlib.layer_state, surface, layer::Layer::Overlay).expect("layer surface creation");

		hlib.window = Some(layer);

		loop {
			event_queue.blocking_dispatch(&mut hlib).unwrap();

			if hlib.exit {
				println!("exiting example");
				break;
			}
		}
	}
	fn libdraw(&mut self, conn: &mut ConnectionHandle, qh: &QueueHandle<Hlib>) {
		if let Some(window) = self.window.as_ref() {
			if self.hclient.needs_drawing() {
				self.hclient.set_needs_drawing(false);
				// Ensure the pool is big enough to hold the new buffer.
				self.pool.as_mut().unwrap().resize((self.hclient.width() * self.hclient.height() * 4) as usize, conn).expect("resize pool");

				// Destroy the old buffer.
				// FIXME: Integrate this into the pool logic.
				if let Some(buffer) = self.buffer.take() {
					buffer.destroy(conn);
				}

				let offset = 0;
				let stride = self.hclient.width() as i32 * 4;
				let pool = self.pool.as_mut().unwrap();

				let wl_buffer = pool
					.create_buffer(offset, self.hclient.width() as i32, self.hclient.height() as i32, stride, wl_shm::Format::Argb8888, (), conn, qh)
					.expect("create buffer");

				// TODO: Upgrade to a better pool type
				let len = self.hclient.height() as usize * stride as usize; // length of a row
				let buffer = &mut pool.mmap()[offset as usize..][..len];

				self.hclient.draw(buffer, window.wl_surface(), conn);

				self.buffer = Some(wl_buffer);
				assert!(self.buffer.is_some(), "No buffer?");
				window.wl_surface().attach(conn, self.buffer.as_ref(), 0, 0);
			}
			window.wl_surface().frame(conn, qh, window.wl_surface().clone()).expect("create callback");
			window.wl_surface().commit(conn);
		}
	}
}

impl SeatHandler for Hlib {
	fn seat_state(&mut self) -> &mut SeatState {
		&mut self.seat_state
	}

	fn new_seat(&mut self, _: &mut ConnectionHandle, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

	fn new_capability(&mut self, conn: &mut ConnectionHandle, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, capability: Capability) {
		#[cfg(feature = "Keyboard")]
		if capability == Capability::Keyboard && self.keyboard.is_none() {
			println!("Set keyboard capability");
			let keyboard = self.seat_state.get_keyboard(conn, qh, &seat, None).expect("Failed to create keyboard");
			self.keyboard = Some(keyboard);
		}

		#[cfg(any(feature = "Motion", feature = "Clickable"))]
		if capability == Capability::Touch && self.touchscreen.is_none() {
			println!("Set Touch capability");
			let touchscreen = self.seat_state.get_touch(conn, qh, &seat).expect("Failed to create Touch");
			self.touchscreen = Some(touchscreen);
		}

		#[cfg(any(feature = "Clickable", feature = "Scrollable"))]
		if capability == Capability::Pointer && self.pointer.is_none() {
			println!("Set pointer capability");
			let pointer = self.seat_state.get_pointer(conn, qh, &seat).expect("Failed to create pointer");
			self.pointer = Some(pointer);
		}
	}

	fn remove_capability(&mut self, conn: &mut ConnectionHandle, _: &QueueHandle<Self>, _: wl_seat::WlSeat, capability: Capability) {
		#[cfg(feature = "Keyboard")]
		if capability == Capability::Keyboard && self.keyboard.is_some() {
			println!("Unset keyboard capability");
			self.keyboard.take().unwrap().release(conn);
		}

		#[cfg(any(feature = "Clickable", feature = "Scrollable"))]
		if capability == Capability::Pointer && self.pointer.is_some() {
			println!("Unset pointer capability");
			self.pointer.take().unwrap().release(conn);
		}
	}

	fn remove_seat(&mut self, _: &mut ConnectionHandle, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl CompositorHandler for Hlib {
	fn compositor_state(&mut self) -> &mut CompositorState {
		&mut self.compositor_state
	}

	fn scale_factor_changed(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _surface: &wl_surface::WlSurface, _new_factor: i32) {
		// Not needed for this example.
	}

	fn frame(&mut self, conn: &mut ConnectionHandle, qh: &QueueHandle<Self>, _surface: &wl_surface::WlSurface, _time: u32) {
		self.libdraw(conn, qh);
	}
}

impl OutputHandler for Hlib {
	fn output_state(&mut self) -> &mut OutputState {
		&mut self.output_state
	}

	fn new_output(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _output: wl_output::WlOutput) {}

	fn update_output(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _output: wl_output::WlOutput) {}

	fn output_destroyed(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _output: wl_output::WlOutput) {}
}

#[cfg(feature = "Window")]
impl XdgShellHandler for Hlib {
	fn xdg_shell_state(&mut self) -> &mut XdgShellState<Self> {
		&mut self.xdg_shell_state
	}
}

#[cfg(feature = "Window")]
impl WindowHandler for Hlib {
	fn xdg_window_state(&mut self) -> &mut XdgWindowState {
		&mut self.xdg_window_state
	}

	fn request_close(&mut self, _: &mut ConnectionHandle, _: &QueueHandle<Self>, _: &window::Window) {
		self.exit = true;
	}

	fn configure(&mut self, conn: &mut ConnectionHandle, qh: &QueueHandle<Self>, _window: &window::Window, configure: WindowConfigure, _serial: u32) {
		self.hclient.change_size(configure.new_size);
		// Initiate the first draw.
		if self.first_configure {
			self.first_configure = false;
			self.libdraw(conn, qh);
		}
	}
}

#[cfg(feature = "Layer")]
impl LayerHandler for Hlib {
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

		self.hclient.change_size(configure.new_size);

		if self.first_configure {
			self.first_configure = false;
			self.libdraw(conn, qh);
		}
	}
}

impl ShmHandler for Hlib {
	fn shm_state(&mut self) -> &mut ShmState {
		&mut self.shm_state
	}
}

impl ProvidesRegistryState for Hlib {
	fn registry(&mut self) -> &mut RegistryState {
		&mut self.registry_state
	}
}

// TODO
impl Dispatch<wl_buffer::WlBuffer> for Hlib {
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

#[cfg(feature = "Keyboard")]
impl KeyboardHandler for Hlib {
	fn enter(
		&mut self,
		_: &mut ConnectionHandle,
		_: &QueueHandle<Self>,
		_: &wl_keyboard::WlKeyboard,
		_surface: &wl_surface::WlSurface,
		_: u32,
		_: &[u32],
		_keysyms: &[u32],
	) {
		self.keyboard_focus = true;
	}

	fn leave(&mut self, _: &mut ConnectionHandle, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _surface: &wl_surface::WlSurface, _: u32) {
		self.keyboard_focus = false;
	}

	fn press_key(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, event: KeyEvent) {
		println!("Key press: {:?}", event);
		if let Some(l) = event.utf8 {
			self.hclient.press(&l);
		}
	}

	fn release_key(&mut self, _: &mut ConnectionHandle, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, event: KeyEvent) {
		println!("Key release: {:?}", event);
	}

	fn update_modifiers(&mut self, _: &mut ConnectionHandle, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _serial: u32, modifiers: Modifiers) {
		println!("Update modifiers: {:?}", modifiers);
	}
}

#[cfg(any(feature = "Clickable", feature = "Motion"))]
impl touch::TouchHandler for Hlib {
	#[allow(clippy::too_many_arguments)]
	fn down(
		&mut self,
		_conn: &mut ConnectionHandle,
		_qh: &QueueHandle<Self>,
		_touch: &wl_touch::WlTouch,
		_serial: u32,
		time: u32,
		_surface: wl_surface::WlSurface,
		_id: i32,
		position: (f64, f64),
	) {
		self.touch_position = position;
		self.startpoint = Some(position);
		self.presstime = time;
	}

	fn up(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _touch: &wl_touch::WlTouch, _serial: u32, time: u32, _id: i32) {
		#[cfg(feature = "Clickable")]
		if let Some(sp) = self.startpoint {
			if (self.touch_position.0 - sp.0).abs() < 10.0 {
				if time - self.presstime > 200 {
					self.hclient.long_click(self.pointer_position);
				} else {
					self.hclient.short_click(self.pointer_position);
				}
			}
		}
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
		self.touch_position = position;
		#[cfg(feature = "Motion")]
		self.hclient.motion(&mut self.startpoint, position);
	}

	fn shape(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _touch: &wl_touch::WlTouch, _id: i32, _major: f64, _minor: f64) {}

	fn orientation(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _touch: &wl_touch::WlTouch, _id: i32, _orientation: f64) {}

	fn cancel(&mut self, _conn: &mut ConnectionHandle, _qh: &QueueHandle<Self>, _touch: &wl_touch::WlTouch) {}
}

#[cfg(feature = "Clickable")]
impl PointerHandler for Hlib {
	fn pointer_focus(
		&mut self,
		_conn: &mut ConnectionHandle,
		_qh: &QueueHandle<Self>,
		_pointer: &wl_pointer::WlPointer,
		_surface: &wl_surface::WlSurface,
		_entered: (f64, f64),
	) {
		self.pointer_focus = true;
	}

	fn pointer_release_focus(
		&mut self,
		_conn: &mut ConnectionHandle,
		_qh: &QueueHandle<Self>,
		_pointer: &wl_pointer::WlPointer,
		_surface: &wl_surface::WlSurface,
	) {
		self.pointer_focus = false;
	}

	fn pointer_motion(
		&mut self,
		_conn: &mut ConnectionHandle,
		_qh: &QueueHandle<Self>,
		_pointer: &wl_pointer::WlPointer,
		_time: u32,
		position: (f64, f64),
	) {
		if self.pointer_focus {
			self.pointer_position = (position.0 as usize, position.1 as usize);
		}
	}

	fn pointer_press_button(
		&mut self,
		_conn: &mut ConnectionHandle,
		_qh: &QueueHandle<Self>,
		_pointer: &wl_pointer::WlPointer,
		time: u32,
		_button: u32,
	) {
		if self.pointer_focus {
			self.presstime = time;
		}
	}

	fn pointer_release_button(
		&mut self,
		_conn: &mut ConnectionHandle,
		_qh: &QueueHandle<Self>,
		_pointer: &wl_pointer::WlPointer,
		time: u32,
		_button: u32,
	) {
		if self.pointer_focus {
			if time - self.presstime > 200 {
				self.hclient.long_click(self.pointer_position);
			} else {
				self.hclient.short_click(self.pointer_position);
			}
		}
	}

	fn pointer_axis(&mut self, _: &mut ConnectionHandle, _: &QueueHandle<Self>, _: &wl_pointer::WlPointer, _time: u32, scroll: PointerScroll) {
		if self.pointer_focus {
			if let Some(AxisKind::Absolute(i)) = scroll.axis(wl_pointer::Axis::VerticalScroll) {
				#[cfg(feature = "Scrollable")]
				self.hclient.scroll(i);
			}

			if let Some(horizontal) = scroll.axis(wl_pointer::Axis::HorizontalScroll) {
				println!("\nH: {:?}", horizontal);
			}
		}
	}
}

delegate_compositor!(Hlib);
delegate_output!(Hlib);
delegate_shm!(Hlib);

#[cfg(feature = "Window")]
delegate_xdg_shell!(Hlib);
#[cfg(feature = "Window")]
delegate_xdg_window!(Hlib);

#[cfg(feature = "Layer")]
delegate_layer!(Hlib);

delegate_seat!(Hlib);

#[cfg(feature = "Keyboard")]
delegate_keyboard!(Hlib);

#[cfg(any(feature = "Motion", feature = "Clickable"))]
delegate_touch!(Hlib);

#[cfg(any(feature = "Clickable", feature = "Scrollable"))]
delegate_pointer!(Hlib);

#[cfg(feature = "Window")]
delegate_registry!(Hlib: [
	CompositorState,
	OutputState,
	ShmState,
	SeatState,
	XdgShellState<Self>,
	XdgWindowState,
]);

#[cfg(feature = "Layer")]
delegate_registry!(Hlib: [
	CompositorState,
	OutputState,
	ShmState,
	SeatState,
	LayerState,
]);
