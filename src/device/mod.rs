// device: the hdc wire protocol, its transports, and the device-facing commands.

pub(crate) mod devtools;
pub(crate) mod hdc_proto;
pub(crate) mod registry;
pub(crate) mod stream;
pub(crate) mod target;
#[cfg(feature = "usb")]
pub(crate) mod usb;
