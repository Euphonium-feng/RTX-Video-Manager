use std::io::{self, Read, Write};
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_BROKEN_PIPE, ERROR_NO_DATA, ERROR_PIPE_NOT_CONNECTED, HANDLE,
};
use windows_sys::Win32::Storage::FileSystem::{ReadFile, WriteFile};

pub struct PipeHandle(HANDLE);

impl PipeHandle {
    pub fn new(handle: HANDLE) -> Self {
        Self(handle)
    }

    pub fn raw(&self) -> HANDLE {
        self.0
    }

}

impl Drop for PipeHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

pub struct PipeReader(HANDLE);

impl PipeReader {
    pub fn new(handle: &PipeHandle) -> Self {
        Self(handle.raw())
    }
}

impl Read for PipeReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut count = 0u32;
        let size = buffer.len().min(u32::MAX as usize) as u32;
        let ok = unsafe {
            ReadFile(
                self.0,
                buffer.as_mut_ptr(),
                size,
                &mut count,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            let error = unsafe { GetLastError() };
            if matches!(
                error,
                ERROR_BROKEN_PIPE | ERROR_NO_DATA | ERROR_PIPE_NOT_CONNECTED
            ) {
                return Ok(0);
            }
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        Ok(count as usize)
    }
}

pub struct PipeWriter(HANDLE);

impl PipeWriter {
    pub fn new(handle: &PipeHandle) -> Self {
        Self(handle.raw())
    }
}

impl Write for PipeWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let mut count = 0u32;
        let size = buffer.len().min(u32::MAX as usize) as u32;
        let ok = unsafe {
            WriteFile(
                self.0,
                buffer.as_ptr(),
                size,
                &mut count,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(count as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_INVALID_HANDLE};
    use windows_sys::Win32::System::Pipes::PeekNamedPipe;

    #[test]
    fn closed_handle_read_is_not_treated_as_eof() {
        let handle = PipeHandle::new(std::ptr::null_mut());
        let mut reader = PipeReader::new(&handle);
        let error = reader.read(&mut [0u8; 4]).unwrap_err();
        assert_ne!(error.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn broken_pipe_is_the_only_end_of_stream_policy() {
        let actual = pipe_error_is_eof(ERROR_BROKEN_PIPE);
        assert!(actual);
        assert!(pipe_error_is_eof(ERROR_NO_DATA));
        assert!(pipe_error_is_eof(ERROR_PIPE_NOT_CONNECTED));
        assert!(!pipe_error_is_eof(ERROR_ACCESS_DENIED));
        assert!(!pipe_error_is_eof(ERROR_INVALID_HANDLE));
    }

    #[test]
    fn empty_write_is_a_no_op() {
        let handle = PipeHandle::new(std::ptr::null_mut());
        let mut writer = PipeWriter::new(&handle);
        assert_eq!(writer.write(&[]).unwrap(), 0);
    }

    #[test]
    fn peek_of_a_null_handle_is_an_error() {
        let handle = PipeHandle::new(std::ptr::null_mut());
        let mut available = 0u32;
        let ok = unsafe {
            PeekNamedPipe(
                handle.raw(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut available,
                std::ptr::null_mut(),
            )
        };
        assert_eq!(ok, 0);
    }
}

fn pipe_error_is_eof(error: u32) -> bool {
    matches!(
        error,
        ERROR_BROKEN_PIPE | ERROR_NO_DATA | ERROR_PIPE_NOT_CONNECTED
    )
}
