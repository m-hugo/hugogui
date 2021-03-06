use wayland_client::{ConnectionHandle, DelegateDispatch, DelegateDispatchBase, Dispatch, QueueHandle};
use wayland_protocols::xdg_shell::client::{xdg_surface, xdg_wm_base};

use crate::registry::{ProvidesRegistryState, RegistryHandler};

use super::{XdgShellHandler, XdgShellState, XdgSurfaceData};

impl<D> RegistryHandler<D> for XdgShellState<D>
where
	D: Dispatch<xdg_wm_base::XdgWmBase, UserData = ()> + XdgShellHandler + ProvidesRegistryState + 'static,
{
	fn new_global(data: &mut D, conn: &mut ConnectionHandle, qh: &QueueHandle<D>, name: u32, interface: &str, _version: u32) {
		if interface == "xdg_wm_base" {
			if data.xdg_shell_state().xdg_wm_base.is_some() {
				return;
			}

			let xdg_wm_base = data.registry().bind_once::<xdg_wm_base::XdgWmBase, _, _>(conn, qh, name, 2, ()).expect("failed to bind global");

			data.xdg_shell_state().xdg_wm_base = Some((name, xdg_wm_base));
		}
	}

	fn remove_global(_: &mut D, _: &mut ConnectionHandle, _: &QueueHandle<D>, _: u32) {
		// Unlikely to ever occur and the surfaces become inert if this happens.
	}
}

/* Delegate trait impls */

impl<D> DelegateDispatchBase<xdg_wm_base::XdgWmBase> for XdgShellState<D> {
	type UserData = ();
}

impl<D> DelegateDispatch<xdg_wm_base::XdgWmBase, D> for XdgShellState<D>
where
	D: Dispatch<xdg_wm_base::XdgWmBase, UserData = Self::UserData> + XdgShellHandler,
{
	fn event(_: &mut D, xdg_wm_base: &xdg_wm_base::XdgWmBase, event: xdg_wm_base::Event, _: &(), conn: &mut ConnectionHandle, _: &QueueHandle<D>) {
		match event {
			xdg_wm_base::Event::Ping { serial } => {
				xdg_wm_base.pong(conn, serial);
			}

			_ => unreachable!(),
		}
	}
}

impl<D: 'static> DelegateDispatchBase<xdg_surface::XdgSurface> for XdgShellState<D> {
	type UserData = XdgSurfaceData<D>;
}

impl<D> DelegateDispatch<xdg_surface::XdgSurface, D> for XdgShellState<D>
where
	D: Dispatch<xdg_surface::XdgSurface, UserData = XdgSurfaceData<D>> + XdgShellHandler + 'static,
{
	fn event(
		data: &mut D,
		surface: &xdg_surface::XdgSurface,
		event: xdg_surface::Event,
		udata: &Self::UserData,
		conn: &mut ConnectionHandle,
		qh: &QueueHandle<D>,
	) {
		match event {
			xdg_surface::Event::Configure { serial } => {
				// Ack the configure
				surface.ack_configure(conn, serial);
				udata.configure_handler.configure(data, conn, qh, surface, serial);
			}

			_ => unreachable!(),
		}
	}
}
