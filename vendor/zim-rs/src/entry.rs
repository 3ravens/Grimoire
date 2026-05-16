use cxx::UniquePtr;
use zim_sys::binding::ffi;

use crate::{cxx::string_from_ptr, item::Item};

pub struct Entry {
    ptr: UniquePtr<ffi::Entry>,
}

// [SAFETY]
// Entry is readonly and the data is valid as long as we keep a reference to ffi::Entry
// (which is validated by `UniquePtr` and rust borrow rules).
// libzim itself is threadsafe, so it is valid to call libzim method from different threads.
unsafe impl Send for Entry {}
unsafe impl Sync for Entry {}

impl Entry {
    pub(crate) fn from_ptr(ptr: UniquePtr<ffi::Entry>) -> Result<Entry, ()> {
        match ptr.is_null() {
            true => Err(()),
            false => Ok(Entry { ptr }),
        }
    }

    fn inner_ref(&self) -> &ffi::Entry {
        self.ptr.as_ref().unwrap()
    }

    pub fn is_redirect(&self) -> bool {
        ffi::entry_isRedirect(self.inner_ref())
    }

    pub fn get_title(&self) -> String {
        string_from_ptr(ffi::entry_getTitle(self.inner_ref()))
            .expect("Entry::get_title should be infallible")
    }

    pub fn get_path(&self) -> String {
        string_from_ptr(ffi::entry_getPath(self.inner_ref()))
            .expect("Entry::get_path should be infallible")
    }

    pub fn get_item(&self, follow: bool) -> Result<Item, ()> {
        Item::from_ptr(ffi::entry_getItem(self.inner_ref(), follow))
    }

    pub fn get_redirect(&self) -> Result<Item, ()> {
        Item::from_ptr(ffi::entry_getRedirect(self.inner_ref()))
    }

    pub fn get_redirect_entry(&self) -> Result<Entry, ()> {
        Entry::from_ptr(ffi::entry_getRedirectEntry(self.inner_ref()))
    }

    pub fn get_index(&self) -> u32 {
        ffi::entry_getIndex(self.inner_ref())
    }
}
