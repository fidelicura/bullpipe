pub type Cell<T> = static_cell::StaticCell<T>;

pub type Channel<T, const SIZE: usize> = embassy_sync::channel::Channel<Mutex, T, SIZE>;

pub type Lock<T> = embassy_sync::rwlock::RwLock<Mutex, T>;

pub type Mutex = embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

/// # DESCRIPTION
///
/// TODO.
#[macro_export]
macro_rules! make_leaked {
    ($type:ty, $value:expr) => {{
        use crate::sync::Cell;

        static CELL: Cell<$type> = Cell::new();
        CELL.uninit().write($value)
    }};
}

/// # DESCRIPTION
///
/// TODO.
#[macro_export]
macro_rules! make_shared {
    ($type:ty, $value:expr) => {{
        use crate::sync::Cell;
        use crate::sync::Lock;

        static CELL: Cell<Lock<$type>> = Cell::new();
        let lock = Lock::new($value);
        CELL.uninit().write(lock)
    }};
}
