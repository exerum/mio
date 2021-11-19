use wasmer_wasi_experimental_network::{types::*, abi::*};
use std::convert::TryInto;
use std::os::wasi::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::io::{self, IoSlice, IoSliceMut};
use std::net::{Shutdown, SocketAddr};
use crate::sys::{wasi_socket_take_error, wasi_socket_ttl, wasi_socket_flush, wasi_socket_nodelay, wasi_socket_set_nodelay, wasi_socket_set_ttl};

#[derive(std::fmt::Debug)]
pub struct TcpStreamWasi {
    pub(crate) raw_fd: std::os::wasi::io::RawFd
}

impl TcpStreamWasi {
    pub fn read(& self, buf: &mut [u8]) -> io::Result<usize> {
        use std::convert::TryInto;
        let mut io_vec = [__wasi_ciovec_t {
            buf: (buf.as_mut_ptr() as usize).try_into().unwrap(),
            buf_len: buf.len().try_into().unwrap(),
        }];
        let mut io_read = 0;
        let error = unsafe {
            socket_recv(
                self.raw_fd as u32,
                io_vec.as_mut_ptr(),
                io_vec.len() as u32,
                0,
                &mut io_read,
            )
        };
        if error != __WASI_ESUCCESS {
            return Err(std::io::Error::from_raw_os_error(error as i32));
        }
        Ok(io_read as usize)
    }

    pub fn read_vectored(& self,  bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        use std::convert::TryInto;
        let mut io_vec: Vec<_> = bufs.iter_mut().map(|s| {
            __wasi_ciovec_t {
                buf: (s.as_mut_ptr() as usize).try_into().unwrap(),
                buf_len: s.len().try_into().unwrap(),
            }
        }).collect();
        let mut io_read = 0;
        let error = unsafe {
            socket_recv(
                self.raw_fd as u32,
                io_vec.as_mut_ptr(),
                io_vec.len() as u32,
                0,
                &mut io_read,
            )
        };
        if error != __WASI_ESUCCESS {
            return Err(std::io::Error::from_raw_os_error(error as i32));
        }
        Ok(io_read as usize)
    }

    pub fn write(& self, buf: &[u8]) -> io::Result<usize> {
        use std::convert::TryInto;
        let io_vec = [__wasi_ciovec_t {
            buf: (buf.as_ptr() as usize).try_into().unwrap(),
            buf_len: buf.len().try_into().unwrap(),
        }];
        let mut io_write = 0;
        let error = unsafe {
            socket_send(
                self.raw_fd as u32,
                io_vec.as_ptr(),
                io_vec.len() as u32,
                0,
                &mut io_write,
            )
        };
        if error != __WASI_ESUCCESS {
            return Err(std::io::Error::from_raw_os_error(error as i32));
        }
        Ok(io_write as usize)
    }

    pub fn write_vectored(& self,  bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        use std::convert::TryInto;
        let mut io_vec: Vec<_> = bufs.iter().map(|s| {
            __wasi_ciovec_t {
                buf: (s.as_ptr() as usize).try_into().unwrap(),
                buf_len: s.len().try_into().unwrap(),
            }
        }).collect();
        let mut io_write = 0;
        let error = unsafe {
            socket_send(
                self.raw_fd as u32,
                io_vec.as_ptr(),
                io_vec.len() as u32,
                0,
                &mut io_write,
            )
        };
        if error != __WASI_ESUCCESS {
            return Err(std::io::Error::from_raw_os_error(error as i32));
        }
        Ok(io_write as usize)
    }

    pub fn ttl(&self) -> io::Result<u32> {
        let mut ttl: u32 = 0;
        let error = unsafe {
            wasi_socket_ttl(self.raw_fd as u32, &mut ttl)
        };
        if error != __WASI_ESUCCESS {
            return Err(std::io::Error::from_raw_os_error(error.try_into().unwrap()));
        }
        Ok(ttl)
    }

    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        let error = unsafe {
            wasi_socket_set_ttl(self.raw_fd as u32, ttl)
        };
        if error != __WASI_ESUCCESS {
            return Err(std::io::Error::from_raw_os_error(error.try_into().unwrap()));
        }
        Ok(())
    }

    pub fn nodelay(&self) -> io::Result<bool> {
        let mut nodelay: u32 = 0;
        let error = unsafe {
            wasi_socket_nodelay(self.raw_fd as u32, &mut nodelay)
        };
        if error != __WASI_ESUCCESS {
            return Err(std::io::Error::from_raw_os_error(error.try_into().unwrap()));
        }
        Ok(nodelay != 0)
    }

    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        let error = unsafe {
            wasi_socket_set_nodelay(self.raw_fd as u32, if nodelay {1} else {0})
        };
        if error != __WASI_ESUCCESS {
            return Err(std::io::Error::from_raw_os_error(error.try_into().unwrap()));
        }
        Ok(())
    }

    pub fn flush(&self) -> io::Result<()> {
        let error = unsafe {
            wasi_socket_flush(self.raw_fd as u32)
        };
        if error != __WASI_ESUCCESS {
            return Err(std::io::Error::from_raw_os_error(error.try_into().unwrap()));
        }
        Ok(())
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        Ok(SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::new(0,0,0,0)), 0))
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::new(0,0,0,0)), 0))
    }

    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        let how = match how {
            Shutdown::Both => SHUT_RDWR,
            Shutdown::Read => SHUT_RD,
            Shutdown::Write => SHUT_WR,
        };
        let error = unsafe {
            socket_shutdown(self.raw_fd as u32, how)
        };
        if error != __WASI_ESUCCESS {
            return Err(std::io::Error::from_raw_os_error(error as i32));
        }
        Ok(())
    }

    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        unimplemented!();
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        let mut socket_error: u32 = __WASI_ESUCCESS as u32;
        let error = unsafe {
            wasi_socket_take_error(self.raw_fd as u32, &mut socket_error)
        };
        if error == __WASI_ESUCCESS as u16 {
            if socket_error == __WASI_ESUCCESS as u32 {
                return Ok(None)
            } else {
                return Ok(Some(io::Error::from_raw_os_error(socket_error as i32)))
            }
        } else {
            return Err(io::Error::from_raw_os_error(error as i32))
        }
    }
}

impl AsRawFd for TcpStreamWasi {
    fn as_raw_fd(&self) -> RawFd {
        self.raw_fd
    }
}

impl FromRawFd for TcpStreamWasi {
    /// Converts a `RawFd` to a `TcpStreamWasi`.
    ///
    /// # Notes
    ///
    /// The caller is responsible for ensuring that the socket is in
    /// non-blocking mode.
    unsafe fn from_raw_fd(fd: RawFd) -> TcpStreamWasi {
        TcpStreamWasi {
            raw_fd: fd
        }
    }
}

impl IntoRawFd for TcpStreamWasi {
    fn into_raw_fd(self) -> RawFd {
        self.raw_fd
    }
}