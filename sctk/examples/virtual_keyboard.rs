use smithay_client_toolkit::registry::ProvidesRegistryState;
use smithay_client_toolkit::registry::RegistryHandler;
use wayland_client::DelegateDispatchBase;
use wayland_client::{ConnectionHandle, DelegateDispatch, Dispatch, QueueHandle};
use wayland_protocols::misc::zwp_virtual_keyboard_v1::client::{
	zwp_virtual_keyboard_manager_v1,
	zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
	zwp_virtual_keyboard_v1::{Event, ZwpVirtualKeyboardV1},
};

pub struct VkState(pub Option<ZwpVirtualKeyboardManagerV1>);

impl VkState {
	pub fn new() -> VkState {
		VkState(None)
	}
}

pub trait VkHandler: Sized {
	fn vk_state(&mut self) -> &mut VkState;
}

impl<D> RegistryHandler<D> for VkState
where
	D: Dispatch<ZwpVirtualKeyboardManagerV1, UserData = ()> + VkHandler + ProvidesRegistryState + 'static,
{
	fn new_global(data: &mut D, conn: &mut ConnectionHandle, qh: &QueueHandle<D>, name: u32, interface: &str, version: u32) {
		if interface == "zwp_virtual_keyboard_manager_v1" {
			if data.vk_state().0.is_some() {
				return;
			}

			data.vk_state().0 = Some(
				data.registry()
					.bind_once::<ZwpVirtualKeyboardManagerV1, _, _>(conn, qh, name, u32::min(version, 4), ())
					.expect("failed to bind vk shell"),
			);
		}
	}

	fn remove_global(_: &mut D, _: &mut ConnectionHandle, _: &QueueHandle<D>, _: u32) {
		// Unlikely to ever occur and the surfaces become inert if this happens.
	}
}

#[macro_export]
macro_rules! delegate_vk {
    ($ty: ty) => {
        delegate_dispatch!($ty: [
            ZwpVirtualKeyboardManagerV1,
            ZwpVirtualKeyboardV1
        ] => VkState);
    };
}

//delegate_dispatch!(SimpleLayer:[ZwpVirtualKeyboardV1, ZwpVirtualKeyboardManagerV1] => VkState);
impl DelegateDispatchBase<ZwpVirtualKeyboardV1> for VkState {
	type UserData = ();
}
impl DelegateDispatchBase<ZwpVirtualKeyboardManagerV1> for VkState {
	type UserData = ();
}
impl<D> DelegateDispatch<ZwpVirtualKeyboardManagerV1, D> for VkState
where
	D: Dispatch<ZwpVirtualKeyboardManagerV1, UserData = ()> + 'static,
{
	fn event(
		_: &mut D,
		_: &ZwpVirtualKeyboardManagerV1,
		_: zwp_virtual_keyboard_manager_v1::Event,
		_: &Self::UserData,
		_: &mut ConnectionHandle,
		_: &QueueHandle<D>,
	) {
		unreachable!("zwlr_layer_shell_v1 has no events")
	}
}
impl<D> DelegateDispatch<ZwpVirtualKeyboardV1, D> for VkState
where
	D: Dispatch<ZwpVirtualKeyboardV1, UserData = ()> + 'static,
{
	fn event(_: &mut D, _: &ZwpVirtualKeyboardV1, _: Event, _: &Self::UserData, _: &mut ConnectionHandle, _: &QueueHandle<D>) {
		unreachable!("zwlr_layer_shell_v1 has no events")
	}
}
