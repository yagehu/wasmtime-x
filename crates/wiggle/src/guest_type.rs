use crate::{GuestError, GuestMemory, GuestPtr, Width};
use std::cell::UnsafeCell;
use std::sync::atomic::{
    AtomicI8, AtomicI16, AtomicI32, AtomicI64, AtomicU8, AtomicU16, AtomicU32, AtomicU64, Ordering,
};
use std::{mem, ops};

/// A trait for types which are used to report errors. Each type used in the
/// first result position of an interface function is used, by convention, to
/// indicate whether the function was successful and subsequent results are valid,
/// or whether an error occurred. This trait allows wiggle to return the correct
/// value when the interface function's idiomatic Rust method returns
/// `Ok(<rest of return values>)`.
pub trait GuestErrorType {
    fn success() -> Self;
}

/// A trait for types that are intended to be pointees in `GuestPtr<T>`.
///
/// This trait abstracts how to read/write information from the guest memory, as
/// well as how to offset elements in an array of guest memory. This layer of
/// abstraction allows the guest representation of a type to be different from
/// the host representation of a type, if necessary. It also allows for
/// validation when reading/writing.
pub trait GuestType<W>: Sized
where
    W: Width,
{
    /// Returns the size, in bytes, of this type in the guest memory.
    fn guest_size() -> W;

    /// Returns the required alignment of this type, in bytes, for both guest
    /// and host memory.
    fn guest_align() -> usize;

    /// Reads this value from the provided `ptr`.
    ///
    /// Must internally perform any safety checks necessary and is allowed to
    /// fail if the bytes pointed to are also invalid.
    ///
    /// Typically if you're implementing this by hand you'll want to delegate to
    /// other safe implementations of this trait (e.g. for primitive types like
    /// `u32`) rather than writing lots of raw code yourself.
    fn read(mem: &GuestMemory, ptr: GuestPtr<Self, W>) -> Result<Self, GuestError<W>>;

    /// Writes a value to `ptr` after verifying that `ptr` is indeed valid to
    /// store `val`.
    ///
    /// Similar to `read`, you'll probably want to implement this in terms of
    /// other primitives.
    fn write(mem: &mut GuestMemory, ptr: GuestPtr<Self, W>, val: Self)
    -> Result<(), GuestError<W>>;
}

/// A trait for `GuestType`s that have the same representation in guest memory
/// as in Rust. These types can be used with the `GuestPtr::as_slice` method to
/// view as a slice.
///
/// Unsafe trait because a correct `GuestTypeTransparent` implementation ensures
/// that the `GuestPtr::as_slice` methods are safe, notably that the
/// representation on the host matches the guest and all bit patterns are
/// valid. This trait should only ever be implemented by
/// wiggle_generate-produced code.
pub unsafe trait GuestTypeTransparent<W>: GuestType<W>
where
    W: Width,
{
}

macro_rules! integer_primitives {
    ($([$ty:ident, $ty_atomic:ident],)*) => ($(
        impl<W: Width> GuestType<W> for $ty
        where
            W: std::ops::Add<Output = W>,
        {
            #[inline]
            fn guest_size() -> W { mem::size_of::<Self>().try_into().unwrap() }

            #[inline]
            fn guest_align() -> usize { mem::align_of::<Self>() }

            #[inline]
            fn read(mem: &GuestMemory, ptr: GuestPtr<Self, W>) -> Result<Self, GuestError<W>> {
                // Use `validate_size_align` to validate offset and alignment
                // internally. The `host_ptr` type will be `&UnsafeCell<Self>`
                // indicating that the memory is valid, and next safety checks
                // are required to access it.
                let offset = ptr.offset();
                let host_ptr = mem.validate_size_align::<Self, W>(offset, W::try_from(1)?)?;

                // If the accessed memory is shared, we need to load the bytes
                // with the correct memory consistency. We could check if the
                // memory is shared each time, but we expect little performance
                // difference between an additional branch and a relaxed memory
                // access and thus always do the relaxed access here.
                let host_ptr: &$ty_atomic = unsafe {
                    let host_ptr: &UnsafeCell<Self> = &host_ptr[0];
                    &*((host_ptr as *const UnsafeCell<Self>).cast::<$ty_atomic>())
                };
                let val = host_ptr.load(Ordering::Relaxed);

                // And as a final operation convert from the little-endian wasm
                // value to a native-endian value for the host.
                Ok($ty::from_le(val))
            }

            #[inline]
            fn write(mem: &mut GuestMemory, ptr: GuestPtr<Self, W>, val: Self) -> Result<(), GuestError<W>> {
                // See `read` above for various checks here.
                let val = val.to_le();
                let offset = ptr.offset();
                let host_ptr = mem.validate_size_align::<Self, W>(offset, W::try_from(1)?)?;
                let host_ptr = &host_ptr[0];
                let atomic_value_ref: &$ty_atomic =
                    unsafe { &*(host_ptr.get().cast::<$ty_atomic>()) };
                atomic_value_ref.store(val, Ordering::Relaxed);
                Ok(())
            }
        }

        unsafe impl<W: Width> GuestTypeTransparent<W> for $ty
        where
            W: std::ops::Add<Output = W>,
        {}

    )*)
}

macro_rules! float_primitives {
    ($([$ty:ident, $ty_unsigned:ident, $ty_atomic:ident],)*) => ($(
        impl<W> GuestType<W> for $ty
        where
            W: Width + std::ops::Add<Output = W>,
        {
            #[inline]
            fn guest_size() -> W { mem::size_of::<Self>().try_into().unwrap() }

            #[inline]
            fn guest_align() -> usize { mem::align_of::<Self>() }

            #[inline]
            fn read(mem: &GuestMemory, ptr: GuestPtr<Self, W>) -> Result<Self, GuestError<W>> {
                <$ty_unsigned as GuestType<W>>::read(mem, ptr.cast()).map($ty::from_bits)
            }

            #[inline]
            fn write(mem:&mut GuestMemory, ptr: GuestPtr<Self, W>, val: Self) -> Result<(), GuestError<W>> {
                <$ty_unsigned as GuestType<W>>::write(mem, ptr.cast(), val.to_bits())
            }
        }

        unsafe impl<W> GuestTypeTransparent<W> for $ty
        where
            W: Width + std::ops::Add<Output = W>,
        {}

    )*)
}

integer_primitives! {
    // signed
    [i8, AtomicI8], [i16, AtomicI16], [i32, AtomicI32], [i64, AtomicI64],
    // unsigned
    [u8, AtomicU8], [u16, AtomicU16], [u32, AtomicU32], [u64, AtomicU64],
}

float_primitives! {
    [f32, u32, AtomicU32], [f64, u64, AtomicU64],
}

// Support pointers-to-pointers where pointers are always 32-bits in wasm land
impl<T, W> GuestType<W> for GuestPtr<T, W>
where
    W: Width + GuestType<W>,
{
    #[inline]
    fn guest_size() -> W {
        W::try_from(mem::size_of::<W>()).unwrap()
    }

    #[inline]
    fn guest_align() -> usize {
        mem::align_of::<W>()
    }

    fn read(mem: &GuestMemory, ptr: GuestPtr<Self, W>) -> Result<Self, GuestError<W>> {
        let offset = W::read(mem, ptr.cast())?;
        Ok(GuestPtr::new(offset))
    }

    fn write(
        mem: &mut GuestMemory,
        ptr: GuestPtr<Self, W>,
        val: Self,
    ) -> Result<(), GuestError<W>> {
        W::write(mem, ptr.cast(), val.offset())
    }
}

// Support pointers-to-arrays where pointers are always 32-bits in wasm land
impl<T, W> GuestType<W> for GuestPtr<[T], W>
where
    T: GuestType<W>,
    W: Width + GuestType<W> + ops::Mul<Output = W>,
{
    #[inline]
    fn guest_size() -> W {
        W::guest_size() * W::try_from(2).unwrap()
    }

    #[inline]
    fn guest_align() -> usize {
        W::guest_align()
    }

    fn read(mem: &GuestMemory, ptr: GuestPtr<Self, W>) -> Result<Self, GuestError<W>> {
        let ptr = ptr.cast::<W>();
        let offset = W::read(mem, ptr)?;
        let len = W::read(mem, ptr.add(W::try_from(1)?)?)?;
        Ok(GuestPtr::new(offset).as_array(len))
    }

    fn write(
        mem: &mut GuestMemory,
        ptr: GuestPtr<Self, W>,
        val: Self,
    ) -> Result<(), GuestError<W>> {
        let (offset, len) = val.offset();
        let ptr = ptr.cast::<W>();
        W::write(mem, ptr, offset)?;
        W::write(mem, ptr.add(W::try_from(1)?)?, len)?;
        Ok(())
    }
}
