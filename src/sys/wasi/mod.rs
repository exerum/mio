//! # Notes
//!
//! The current implementation is somewhat limited. The `Waker` is not
//! implemented, as at the time of writing there is no way to support to wake-up
//! a thread from calling `poll_oneoff`.
//!
//! Furthermore the (re/de)register functions also don't work while concurrently
//! polling as both registering and polling requires a lock on the
//! `subscriptions`.
//!
//! Finally `Selector::try_clone`, required by `Registry::try_clone`, doesn't
//! work. However this could be implemented by use of an `Arc`.
//!
//! In summary, this only (barely) works using a single thread.

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering, AtomicU32};
use std::time::Duration;
use wasmer_wasi_experimental_network::abi::*;
use wasmer_wasi_experimental_network::types::*;
use std::sync::{Arc, Mutex};
use std::collections::BTreeMap;
use crate::{Interest, Token};

#[link(wasm_import_module = "wasi_experimental_network_unstable")]
extern "C" {
    fn wasi_eventfd() -> wasi::Fd;
    fn wasi_eventfd_write1(fd: wasi::Fd) -> i32;
    fn wasi_eventfd_read(fd: wasi::Fd) -> i32;
    pub fn wasi_socket_ttl(fd: wasi::Fd, ttl_ptr: *mut u32) -> u16;
    pub fn wasi_socket_set_ttl(fd: wasi::Fd, ttl: u32) -> u16;
    pub fn wasi_socket_nodelay(fd: wasi::Fd, nodelay: *mut u32) -> u16;
    pub fn wasi_socket_set_nodelay(fd: wasi::Fd, nodelay: u32) -> u16;
    pub fn wasi_socket_flush(fd: wasi::Fd) -> u16;
    pub fn wasi_socket_take_error(fd: wasi::Fd, socket_error: *mut u32) -> u16;
}

cfg_net! {
    mod net;
    pub(crate) mod stream;

    pub(crate) use net::{tcp, udp};
}

static mut POLLER: AtomicU32 = AtomicU32::new(0);

/// Unique id for use as `SelectorId`.
#[cfg(debug_assertions)]
static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

pub(crate) struct Subscription {
    pub(crate) readable: bool,
    pub(crate) writable: bool,
    pub(crate) fd: wasi::Fd
}

#[derive(Clone)]
pub(crate) struct Selector {
    #[cfg(debug_assertions)]
    id: usize,
    subscriptions: Arc<Mutex<BTreeMap<Token, Subscription>>>,
    // #[cfg(debug_assertions)]
    // has_waker: AtomicBool,
}

impl Selector {
    pub(crate) fn new() -> io::Result<Selector> {
        unsafe {
            if *POLLER.get_mut() == 0 {
                let error = poller_create(POLLER.get_mut());
                if error != __WASI_ESUCCESS {
                    return Err(io::Error::from_raw_os_error(error as i32))
                }
            };
        }
        Ok(Selector {
            #[cfg(debug_assertions)]
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            subscriptions: Arc::new(Mutex::new(BTreeMap::new())),
            // #[cfg(debug_assertions)]
            // has_waker: AtomicBool::new(false),
        })
    }

    #[cfg(debug_assertions)]
    pub(crate) fn id(&self) -> usize {
        self.id
    }

    pub(crate) fn try_clone(&self) -> io::Result<Selector> {
        Ok(self.clone())
    }

    pub(crate) fn select(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        events.clear();
        let mut poller_events: Vec<__wasi_poll_event_t> = Vec::with_capacity(128);
        poller_events.resize(128, __wasi_poll_event_t {
            readable: false,
            writable: false,
            token: u32::max_value()
        });
        let mut events_size_out: u32 = 0;
        let error = unsafe {
            poller_wait(*POLLER.get_mut(), poller_events.as_mut_slice().as_mut_ptr(), poller_events.capacity() as u32, &mut events_size_out)
        };

        if error != __WASI_ESUCCESS {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        for i in 0..events_size_out {
            let event = &mut poller_events[i as usize];
            let subs = self.subscriptions.lock().unwrap();
            match subs.get(&Token(event.token as usize)) {
                Some(sub) => {
                    unsafe {
                        poller_modify(*POLLER.get_mut(), sub.fd, __wasi_poll_event_t {
                            readable: sub.readable,
                            writable: sub.writable,
                            token: event.token
                        })
                    };
                },
                _ => {}
            };
            events.push(*event);
        }
        Ok(())
    }

    pub(crate) fn register(
        &self,
        fd: wasi::Fd,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        let event: __wasi_poll_event_t = __wasi_poll_event_t {
            token: token.0 as u32,
            readable: interests.is_readable(),
            writable: interests.is_writable() 
        };
        let error = unsafe {
            poller_add(*POLLER.get_mut(), fd, event)
        };
        if error != __WASI_ESUCCESS {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        let mut subs = self.subscriptions.lock().unwrap();
        subs.insert(token, Subscription {
            fd,
            readable: interests.is_readable(),
            writable: interests.is_writable() 
        });
        Ok(())
    }

    pub(crate) fn reregister(
        &self,
        fd: wasi::Fd,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.deregister(fd)
            .and_then(|()| self.register(fd, token, interests))
    }

    pub(crate) fn deregister(&self, fd: wasi::Fd) -> io::Result<()> {
        use std::ops::Deref;
        let error = unsafe {
            poller_delete(*POLLER.get_mut(), fd)
        };
        let mut subs = self.subscriptions.lock().unwrap();
        let mut token_to_remove: Option<Token> = None;
        // Find the subscription by fd
        for (k, v) in subs.deref() {
            if v.fd == fd {
                token_to_remove = Some(*k);
            }
        }
        if let Some(t) = token_to_remove {
            subs.remove(&t);
        }
        if error != __WASI_ESUCCESS {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        Ok(())
    }

    #[cfg(debug_assertions)]
    pub(crate) fn register_waker(&self) -> bool {
        // self.has_waker.swap(true, Ordering::AcqRel)
        false
    }
}

/// Token used to a add a timeout subscription, also used in removing it again.
// const TIMEOUT_TOKEN: wasi::Userdata = wasi::Userdata::max_value();

/// Returns a `wasi::Subscription` for `timeout`.
// fn timeout_subscription(timeout: Duration) -> wasi::Subscription {
//     // wasi::Subscription {
//     //     userdata: TIMEOUT_TOKEN,
//     //     u: wasi::SubscriptionU {
//     //         tag: wasi::EVENTTYPE_CLOCK,
//     //         u: wasi::SubscriptionUU {
//     //             clock: wasi::SubscriptionClock {
//     //                 id: wasi::CLOCKID_MONOTONIC,
//     //                 // Timestamp is in nanoseconds.
//     //                 timeout: max(wasi::Timestamp::max_value() as u128, timeout.as_nanos())
//     //                     as wasi::Timestamp,
//     //                 // Give the implementation another millisecond to coalesce
//     //                 // events.
//     //                 precision: Duration::from_millis(1).as_nanos() as wasi::Timestamp,
//     //                 // Zero means the `timeout` is considered relative to the
//     //                 // current time.
//     //                 flags: 0,
//     //             },
//     //         },
//     //     },
//     // }
//     unimplemented!()
// }

// fn is_timeout_event(event: &wasi::Event) -> bool {
//     event.r#type == wasi::EVENTTYPE_CLOCK && event.userdata == TIMEOUT_TOKEN
// }

/// Check all events for possible errors, it returns the first error found.
// fn check_errors(events: &[Event]) -> io::Result<()> {
//     for event in events {
//         if event.error != wasi::ERRNO_SUCCESS {
//             return Err(io_err(event.error));
//         }
//     }
//     Ok(())
// }

/// Convert `wasi::Errno` into an `io::Error`.
// fn io_err(errno: wasi::Errno) -> io::Error {
//     // TODO: check if this is valid.
//     io::Error::from_raw_os_error(errno as i32)
// }

pub(crate) type Events = Vec<Event>;

// pub(crate) type Event = wasi::Event;
pub(crate) type Event = __wasi_poll_event_t;

pub(crate) mod event {
    use std::fmt;

    use crate::sys::Event;
    use crate::Token;

    pub(crate) fn token(event: &Event) -> Token {
        Token(event.token as usize)
    }

    pub(crate) fn is_readable(event: &Event) -> bool {
        event.readable
    }

    pub(crate) fn is_writable(event: &Event) -> bool {
        event.writable
    }

    pub(crate) fn is_error(_: &Event) -> bool {
        // Not supported? It could be that `wasi::Event.error` could be used for
        // this, but the docs say `error that occurred while processing the
        // subscription request`, so it's checked in `Select::select` already.
        false
    }

    pub(crate) fn is_read_closed(event: &Event) -> bool {
        // TODO
        false
        // event.r#type == wasi::EVENTTYPE_FD_READ
        //     // Safety: checked the type of the union above.
        //     && (event.fd_readwrite.flags & wasi::EVENTRWFLAGS_FD_READWRITE_HANGUP) != 0
    }

    pub(crate) fn is_write_closed(event: &Event) -> bool {
        false
        // TODO
        // event.r#type == wasi::EVENTTYPE_FD_WRITE
        //     // Safety: checked the type of the union above.
        //     && (event.fd_readwrite.flags & wasi::EVENTRWFLAGS_FD_READWRITE_HANGUP) != 0
    }

    pub(crate) fn is_priority(_: &Event) -> bool {
        // Not supported.
        false
    }

    pub(crate) fn is_aio(_: &Event) -> bool {
        // Not supported.
        false
    }

    pub(crate) fn is_lio(_: &Event) -> bool {
        // Not supported.
        false
    }

    pub(crate) fn debug_details(f: &mut fmt::Formatter<'_>, event: &Event) -> fmt::Result {
        debug_detail!(
            TypeDetails(wasi::Eventtype),
            PartialEq::eq,
            wasi::EVENTTYPE_CLOCK,
            wasi::EVENTTYPE_FD_READ,
            wasi::EVENTTYPE_FD_WRITE,
        );

        #[allow(clippy::trivially_copy_pass_by_ref)]
        fn check_flag(got: &wasi::Eventrwflags, want: &wasi::Eventrwflags) -> bool {
            (got & want) != 0
        }
        debug_detail!(
            EventrwflagsDetails(wasi::Eventrwflags),
            check_flag,
            wasi::EVENTRWFLAGS_FD_READWRITE_HANGUP,
        );

        struct EventFdReadwriteDetails(wasi::EventFdReadwrite);

        impl fmt::Debug for EventFdReadwriteDetails {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct("EventFdReadwrite")
                    .field("nbytes", &self.0.nbytes)
                    .field("flags", &self.0.flags)
                    .finish()
            }
        }

        f.debug_struct("Event")
            .field("token", &event.token)
            .field("readable", &event.readable)
            .field("writable", &event.writable)
            .finish()
    }
}

#[derive(Debug)]
    pub(crate) struct Waker {
        fd: i32,
    }

    impl Waker {
        pub fn new(selector: &Selector, token: Token) -> io::Result<Waker> {
            let fd = unsafe {
                wasi_eventfd()
            };
            // Turn the file descriptor into a file first so we're ensured
            // it's closed when dropped, e.g. when register below fails.
            // TODO: drop fd if register fails
            selector
                .register(fd, token, Interest::READABLE)
                .map(|()| Waker { fd: fd as i32 })
        }

        pub fn wake(&self) -> io::Result<()> {
            let res = unsafe {
                wasi_eventfd_write1(self.fd as u32)
            };
            match res {
                // No errors
                0 => Ok(()),
                // WouldBlock error
                1 => {
                    // Writing only blocks if the counter is going to overflow.
                    // So we'll reset the counter to 0 and wake it again.
                    self.reset()?;
                    self.wake()
                }
                // Another error
                _ => Err(io::Error::from_raw_os_error(res)),
            }
        }

        /// Reset the eventfd object, only need to call this if `wake` fails.
        fn reset(&self) -> io::Result<()> {
            // let mut buf: [u8; 8] = 0u64.to_ne_bytes();
            let res = unsafe {
                wasi_eventfd_read(self.fd as u32)
            };
            match res {
                // No errors
                0 => Ok(()),
                // WouldBlock error
                // If the `Waker` hasn't been awoken yet this will return a
                // `WouldBlock` error which we can safely ignore.
                1 => Ok(()),
                // Another error
                _ => Err(io::Error::from_raw_os_error(res)),
            }
        }
    }

cfg_io_source! {
    pub(crate) struct IoSourceState;

    impl IoSourceState {
        pub(crate) fn new() -> IoSourceState {
            IoSourceState
        }

        pub(crate) fn do_io<T, F, R>(&self, f: F, io: &T) -> io::Result<R>
        where
            F: FnOnce(&T) -> io::Result<R>,
        {
            // We don't hold state, so we can just call the function and
            // return.
            f(io)
        }
    }
}