use cxx::UniquePtr;
use zim_sys::binding::ffi;

pub struct Blob {
    ptr: UniquePtr<ffi::Blob>,
}

// [SAFETY]
// Blob is readonly and the data is valid as long as we keep a reference to ffi::Blob
// (which is validated by `UniquePtr` and rust borrow rules).
// libzim itself is threadsafe, so it is valid to call libzim method from different threads.
unsafe impl Sync for Blob {}
unsafe impl Send for Blob {}

impl Blob {
    pub(crate) fn from_ptr(ptr: UniquePtr<ffi::Blob>) -> Result<Blob, ()> {
        match ptr.is_null() {
            true => Err(()),
            false => Ok(Blob { ptr }),
        }
    }

    fn inner_ref(&self) -> &ffi::Blob {
        self.ptr.as_ref().unwrap()
    }

    pub fn data(&self) -> &[u8] {
        let data_ptr = ffi::blob_data(self.inner_ref()) as *const u8;
        let data_size = ffi::blob_size(self.inner_ref()) as usize;
        let data_ptr = if data_ptr.is_null() {
            // libzim may return an empty blob if something goes wrong (invalid request, size too big, ...)
            // In this case, size is also 0.
            if data_size == 0 {
                std::ptr::NonNull::<u8>::dangling().as_ptr()
            } else {
                panic!("Invalid blob data.")
            }
        } else {
            data_ptr
        };
        // [SAFETY]
        // - data_ptr is not null
        // - data_ptr points to actual data except if data_size is 0.
        // - data_ptr is by definition correctly aligned on u8
        // - slice has a lifetime of the blob
        //   guaranteed to exist as long as the blob exists.
        unsafe { std::slice::from_raw_parts(data_ptr, data_size) }
    }

    pub fn size(&self) -> u64 {
        ffi::blob_size(self.inner_ref())
    }
}

impl AsRef<[u8]> for Blob {
    fn as_ref(&self) -> &[u8] {
        self.data()
    }
}
