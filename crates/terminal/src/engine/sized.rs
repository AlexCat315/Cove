//! Translation of `Terminal/Sources/CoveUITerminal/Engine/Sized.swift`.
//!
//! Ghostty "sized structs" open with a `size` field holding the caller's byte
//! layout; the library rejects a struct whose size does not match its own.

/// A Ghostty ABI struct whose leading field is the caller's byte size.
///
/// Implementations write into that leading `size` field and nothing else, so
/// the generic [`sized`] helper stays sound without raw-pointer stores.
pub trait SizedAbi {
    /// Stores the ABI size in the struct's leading `size` field.
    fn write_abi_size(&mut self, size: usize);
}

/// Sets a Ghostty "sized struct"'s leading `size` field. Ghostty rejects sized
/// structs whose size does not match the caller's layout.
pub fn sized<T: SizedAbi>(mut value: T) -> T {
    value.write_abi_size(std::mem::size_of_val(&value));
    value
}
