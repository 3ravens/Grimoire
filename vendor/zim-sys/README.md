# zim-sys
Work-in-progress experiment getting libzim bindings in Rust.

## Compilation
Currently only supports unix-based machines which also support libzim.
- Windows support could be achieved by using ``vcpkg`` to find libzim on windows
- Only Ubuntu Linux has been tested
### Installing dependencies
The build process requires ``clang`` and the original library ``libzim`` will be needed.
#### Ubuntu
```bash
sudo apt-get install libzim-dev clang
```

### Building zim-sys
```
cargo build
```
This process uses my custom C++ wrapper in ``zim-bind.cc`` which catches exceptions and converts them to errors which can be accepted by rust.

## Binding TODO list
Most of these are do to incompatible types that need to be converted.
### archive.h
- getIllustrationItem(unsigned int size=48)
- getIllustrationSizes()
- checkIntegrity(IntegrityCheck checkType)
### entry.h
- getRedirectEntryIndex()

### item.h
- getDirectAccessInformation()

### version.h
- getVersions()