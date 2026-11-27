//! Minimal device filesystem semantics for the initial /dev nodes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DevFsError { NotFound, ReadOnly, InvalidPath }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceNode { Null, Zero, Console }

pub struct DevFs;
impl DevFs {
    pub const fn new() -> Self { Self }
    pub fn lookup(path: &str) -> Result<DeviceNode, DevFsError> {
        match path {
            "/dev/null" | "dev/null" => Ok(DeviceNode::Null),
            "/dev/zero" | "dev/zero" => Ok(DeviceNode::Zero),
            "/dev/console" | "dev/console" => Ok(DeviceNode::Console),
            _ => Err(DevFsError::NotFound),
        }
    }
    /// Read from a device. Console input is supplied by the caller so this
    /// module does not hard-code a UART or keyboard policy.
    pub fn read(
        &self,
        path: &str,
        output: &mut [u8],
        mut console_read: impl FnMut(&mut [u8]) -> usize,
    ) -> Result<usize, DevFsError> {
        match Self::lookup(path)? {
            DeviceNode::Null => Ok(0),
            DeviceNode::Zero => { output.fill(0); Ok(output.len()) }
            DeviceNode::Console => Ok(console_read(output).min(output.len())),
        }
    }
    /// Write to a device. Null/zero discard writes; console delegates output.
    pub fn write(
        &self,
        path: &str,
        input: &[u8],
        mut console_write: impl FnMut(&[u8]) -> usize,
    ) -> Result<usize, DevFsError> {
        match Self::lookup(path)? {
            DeviceNode::Null | DeviceNode::Zero => Ok(input.len()),
            DeviceNode::Console => Ok(console_write(input).min(input.len())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn null_is_eof_and_discards_writes() {
        let fs=DevFs::new();let mut out=[9;8];
        assert_eq!(fs.read("/dev/null",&mut out, |_| 8),Ok(0));
        assert_eq!(fs.write("/dev/null",b"ignored", |_| 0),Ok(7));
    }
    #[test] fn zero_fills_reads_and_discards_writes() {
        let fs=DevFs::new();let mut out=[9;4];
        assert_eq!(fs.read("/dev/zero",&mut out, |_| 0),Ok(4));
        assert_eq!(out,[0;4]);
        assert_eq!(fs.write("/dev/zero",b"data", |_| 0),Ok(4));
    }
    #[test] fn console_delegates_to_kernel_callbacks() {
        let fs=DevFs::new();let mut out=[0;3];
        assert_eq!(fs.read("/dev/console",&mut out, |b| {b[..2].copy_from_slice(b"ok");2}),Ok(2));
        assert_eq!(&out[..2],b"ok");
        assert_eq!(fs.write("/dev/console",b"hello", |b| b.len()),Ok(5));
    }
    #[test] fn unknown_device_is_not_found() {
        assert_eq!(DevFs::lookup("/dev/unknown"),Err(DevFsError::NotFound));
    }
}
