pub mod raw;
pub mod simple;

use std::io;

use nix::errno::Errno;
use wayland_client::backend::InvalidId;

/// An error that may occur when creating a pool.
#[derive(Debug, thiserror::Error)]
pub enum CreatePoolError {
	/// The wl_shm global is not bound.
	#[error("wl_shm global is not bound")]
	MissingShmGlobal,

	/// Could not create the underlying wayland object for the pool.
	#[error(transparent)]
	Protocol(#[from] InvalidId),

	/// Error while allocating the shared memory.
	#[error(transparent)]
	Create(#[from] io::Error),
}

impl From<Errno> for CreatePoolError {
	fn from(errno: Errno) -> Self {
		Into::<io::Error>::into(errno).into()
	}
}
