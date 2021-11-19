use std::io;
use wasmer_wasi_experimental_network::types::*;

/// A lot of function are not support on Wasi, this function returns a
/// consistent error when calling those functions.
fn unsupported() -> io::Error {
    io::Error::new(io::ErrorKind::Other, "not supported on wasi")
}

extern "C" {
    fn wasi_new_v4_socket(fd_out: *mut wasi::Fd) -> u16;
    fn wasi_connect_socket(sock: wasi::Fd, addr: &__wasi_socket_address_t) -> wasi::Fd;
}

pub(crate) mod tcp {
    use std::io;
    use std::net::{self, SocketAddr};
    use std::time::Duration;
    use std::os::wasi::io::FromRawFd;
    use wasmer_wasi_experimental_network::types::*;
    use crate::net::TcpKeepalive;

    use super::unsupported;

    pub type TcpSocket = wasi::Fd;

    pub(crate) fn new_v4_socket() -> io::Result<TcpSocket> {
        let mut fd: wasi::Fd = 0;
        let error = unsafe {
            crate::sys::wasi::net::wasi_new_v4_socket(&mut fd)
        };
        if error == __WASI_ESUCCESS {
            Ok(fd)
        } else {
            Err(io::Error::from_raw_os_error(error as i32))
        }
    }

    pub(crate) fn new_v6_socket() -> io::Result<TcpSocket> {
        Err(unsupported())
    }

    pub(crate) fn bind(_: TcpSocket, _: SocketAddr) -> io::Result<()> {
        Err(unsupported())
    }

    pub(crate) fn connect(sock: TcpSocket, addr: SocketAddr) -> io::Result<net::TcpStream> {
        let wasi_addr: __wasi_socket_address_t = match addr {
            SocketAddr::V4(v4) => {
                __wasi_socket_address_t {
                    v4: __wasi_socket_address_in_t {
                        family: AF_INET,
                        address: v4.ip().octets(),
                        port: v4.port()
                    }
                }
            },
            SocketAddr::V6(_) => {
                unimplemented!()
            }
        };
        let error = unsafe {
            crate::sys::wasi::net::wasi_connect_socket(sock, &wasi_addr)
        };
        if error == __WASI_ESUCCESS as u32 {
            Ok( unsafe { net::TcpStream::from_raw_fd(sock as i32) } )
        } else {
            return Err(io::Error::from_raw_os_error(error as i32))
        }
    }

    pub(crate) fn listen(_: TcpSocket, _: u32) -> io::Result<net::TcpListener> {
        Err(unsupported())
    }

    pub(crate) fn close(socket: TcpSocket) {
        let _ = unsafe { wasi::fd_close(socket) };
    }

    pub(crate) fn set_reuseaddr(_: TcpSocket, _: bool) -> io::Result<()> {
        Err(unsupported())
    }

    pub(crate) fn get_reuseaddr(_: TcpSocket) -> io::Result<bool> {
        Err(unsupported())
    }

    pub(crate) fn get_localaddr(_: TcpSocket) -> io::Result<SocketAddr> {
        Err(unsupported())
    }

    pub(crate) fn set_linger(_: TcpSocket, _: Option<Duration>) -> io::Result<()> {
        Err(unsupported())
    }

    pub(crate) fn set_keepalive_params(socket: TcpSocket, keepalive: TcpKeepalive) -> io::Result<()> { Err(unsupported()) }

    // pub(crate) fn set_keepalive_time(socket: TcpSocket, time: Duration) -> io::Result<()> { Err(unsupported()) }

    pub(crate) fn get_keepalive_time(socket: TcpSocket) -> io::Result<Option<Duration>> { Err(unsupported()) }

    pub(crate) fn set_keepalive(socket: TcpSocket, keepalive: bool) -> io::Result<()> { Err(unsupported()) }

    pub(crate) fn get_keepalive(socket: TcpSocket) -> io::Result<bool> { Err(unsupported()) }

    pub(crate) fn get_linger(_: TcpSocket) -> io::Result<Option<Duration>> {
        Err(unsupported())
    }

    pub(crate) fn set_recv_buffer_size(_: TcpSocket, _: u32) -> io::Result<()> {
        Err(unsupported())
    }

    pub(crate) fn get_recv_buffer_size(_: TcpSocket) -> io::Result<u32> {
        Err(unsupported())
    }

    pub(crate) fn set_send_buffer_size(_: TcpSocket, _: u32) -> io::Result<()> {
        Err(unsupported())
    }

    pub(crate) fn get_send_buffer_size(_: TcpSocket) -> io::Result<u32> {
        Err(unsupported())
    }

    pub(crate) fn accept(_: &net::TcpListener) -> io::Result<(net::TcpStream, SocketAddr)> {
        Err(unsupported())
    }
}

pub(crate) mod udp {
    use std::io;
    use std::net::{self, SocketAddr};

    use super::unsupported;

    pub(crate) fn bind(_: SocketAddr) -> io::Result<net::UdpSocket> {
        Err(unsupported())
    }

    pub(crate) fn only_v6(_: &net::UdpSocket) -> io::Result<bool> { Err(unsupported()) }
}