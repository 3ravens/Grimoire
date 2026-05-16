# zim-rs
Work-in-progress safe rust library for ``libzim``. Depends on my other crate ``zim-sys`` which provides low-level bindings to the C++ library.

## Using the library
Make sure to also clone the bindings in my other repo ``zim-sys`` so that you have both of these folders in the same directory.
Once both the crates are in the directory, a dependency can be added using a ``path = `` specification in the Cargo.toml dependencies list.

## TODO List
- Cleanup Result types
- More complete iterators for Searches and Suggestions rather than only basic IntoIterator
- Add more tests