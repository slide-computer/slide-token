use std::io;
use ic_cdk::api::stable::{stable_grow, stable_read, stable_size, stable_write, StableMemoryError};

/// A writer to the stable memory.
///
/// Will attempt to grow the memory as it writes,
/// and keep offsets and total capacity.
pub struct StableWriter {
    /// The offset of the next write.
    pub offset: usize,

    /// The capacity, in pages.
    pub capacity: u32,
}

impl Default for StableWriter {
    fn default() -> Self {
        let capacity = stable_size();

        Self {
            offset: 0,
            capacity,
        }
    }
}

impl StableWriter {
    /// Attempts to grow the memory by adding new pages.
    pub fn grow(&mut self, added_pages: u32) -> Result<(), StableMemoryError> {
        let old_page_count = stable_grow(added_pages)?;
        self.capacity = old_page_count + added_pages;
        Ok(())
    }

    /// Writes a byte slice to the buffer.
    ///
    /// The only condition where this will
    /// error out is if it cannot grow the memory.
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, StableMemoryError> {
        if self.offset + buf.len() > ((self.capacity as usize) << 16) {
            self.grow((self.offset + buf.len() >> 16) as u32 + 1)?;
        }

        stable_write(self.offset as u32, buf);
        self.offset += buf.len();
        Ok(buf.len())
    }
}

impl io::Write for StableWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.write(buf)
            .map_err(|e| io::Error::new(io::ErrorKind::OutOfMemory, e))
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        // Noop.
        Ok(())
    }
}

/// A reader to the stable memory.
///
/// Keeps an offset and reads off stable memory consecutively.
pub struct StableReader {
    /// The offset of the next read.
    pub offset: usize,
    /// The capacity, in pages.
    pub capacity: u32,
}

impl Default for StableReader {
    fn default() -> Self {
        Self {
            offset: 0,
            capacity: stable_size(),
        }
    }
}

impl StableReader {
    /// Reads data from the stable memory location specified by an offset.
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, StableMemoryError> {
        let cap = (self.capacity as usize) << 16;
        let read_buf = if buf.len() + self.offset > cap {
            if self.offset < cap {
                &mut buf[..cap - self.offset]
            } else {
                return Err(StableMemoryError::OutOfBounds);
            }
        } else {
            buf
        };
        stable_read(self.offset as u32, read_buf);
        self.offset += read_buf.len();
        Ok(read_buf.len())
    }
}

impl io::Read for StableReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        self.read(buf).or(Ok(0)) // Read defines EOF to be success
    }
}

pub fn stable_save<T>(t: T, offset: usize) -> Result<(), candid::Error>
    where
        T: candid::utils::ArgumentEncoder,
{
    let mut writer = StableWriter {
        offset,
        capacity: stable_size(),
    };
    candid::write_args(&mut writer, t)
}

pub fn stable_bytes(offset: usize) -> Vec<u8> {
    let size = (stable_size() as usize) << 16;
    let mut vec = Vec::with_capacity(size - offset);
    unsafe {
        vec.set_len(size - offset);
    }

    stable_read(offset as u32, vec.as_mut_slice());

    vec
}

pub fn stable_restore<T>(offset: usize) -> Result<T, String>
    where
        T: for<'de> candid::utils::ArgumentDecoder<'de>,
{
    let bytes = stable_bytes(offset);

    let mut de =
        candid::de::IDLDeserialize::new(bytes.as_slice()).map_err(|e| format!("{:?}", e))?;
    let res = candid::utils::ArgumentDecoder::decode(&mut de).map_err(|e| format!("{:?}", e))?;
    Ok(res)
}

