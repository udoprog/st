//! runestick, a simple stack-based virtual machine.
//!
//! ## Contributing
//!
//! If you want to help out, there's a number of optimization tasks available in
//! [Future Optimizations][future-optimizations].
//!
//! Create an issue about the optimization you want to work on and communicate that
//! you are working on it.
//!
//! ## Features of runestick
//!
//! * [Clean Rust FFI][rust-ffi].
//! * Stack-based C FFI like with Lua (TBD).
//! * Stack frames, allowing for isolation across function calls.
//! * A rust-like reference language called *Rune*.
//!
//! ## Rune Scripts
//!
//! runestick comes with a simple scripting language called *Rune*.
//!
//! You can run example scripts through rune-cli:
//!
//! ```bash
//! cargo run -- ./scripts/hello_world.rn
//! ```
//!
//! If you want to see diagnostics of your unit, you can do:
//!
//! ```bash
//! cargo run -- ./scripts/hello_world.rn --dump-unit --trace
//! ```
//!
//! [rust-ffi]: https://github.com/udoprog/runestick/blob/master/crates/runestick-http/src/lib.rs
//! [future-optimizations]: https://github.com/udoprog/runestick/blob/master/FUTURE_OPTIMIZATIONS.md

#![deny(missing_docs)]

mod any;
mod context;
mod value;
mod vm;
#[macro_use]
mod macros;
mod access;
mod bytes;
mod error;
mod future;
mod hash;
mod inst;
mod item;
mod meta;
pub(crate) mod module;
pub mod packages;
mod panic;
mod reflection;
mod serde;
mod shared;
mod shared_ptr;
mod stack;
pub mod unit;
mod value_type;
mod value_type_info;

pub use self::meta::{Meta, MetaObject, MetaTuple};
pub use self::module::{AsyncFunction, AsyncInstFn, Function, InstFn, Module};
pub use self::value_type::ValueType;
pub use self::value_type_info::ValueTypeInfo;
pub use crate::access::{
    AccessError, BorrowMut, BorrowRef, NotAccessibleMut, NotAccessibleRef, RawBorrowedMut,
    RawBorrowedRef,
};
pub use crate::any::Any;
pub use crate::bytes::Bytes;
pub use crate::context::{Context, ContextError};
pub use crate::context::{
    ADD, ADD_ASSIGN, DIV, DIV_ASSIGN, FMT_DISPLAY, INDEX_GET, INDEX_SET, MUL, MUL_ASSIGN, NEXT,
    SUB, SUB_ASSIGN,
};
pub use crate::error::{Error, Result};
pub use crate::future::Future;
pub use crate::hash::Hash;
pub use crate::inst::{Inst, OptionVariant, PanicReason, ResultVariant, TypeCheck};
pub use crate::item::{Component, Item};
pub use crate::panic::Panic;
pub use crate::reflection::{
    FromValue, ReflectValueType, ToValue, UnsafeFromValue, UnsafeIntoArgs, UnsafeToValue,
};
pub use crate::shared::{OwnedMut, OwnedRef, RawOwnedMut, RawOwnedRef, Shared};
pub use crate::shared_ptr::SharedPtr;
pub use crate::stack::{Stack, StackError};
pub use crate::unit::{CompilationUnit, CompilationUnitError, Span};
pub use crate::value::{
    Integer, Object, RawMut, RawRef, TypedObject, TypedTuple, Value, ValueError, VariantObject,
    VariantTuple, VecTuple,
};
pub use crate::vm::{Task, Vm, VmError};

mod collections {
    pub use hashbrown::HashMap;
    pub use hashbrown::HashSet;
}