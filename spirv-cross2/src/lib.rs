#![cfg_attr(docsrs, feature(doc_cfg, doc_cfg_hide))]
#![forbid(missing_docs)]

//! Safe and sound Rust bindings to [SPIRV-Cross](https://github.com/KhronosGroup/SPIRV-Cross).
//!
//! All backends exposed by the SPIRV-Cross C API are fully supported, including
//!
//! * [GLSL](targets::Glsl)
//! * [HLSL](targets::Hlsl)
//! * [MSL](targets::Msl)
//! * [JSON](targets::Json)
//! * [C++](targets::Cpp)
//! * [Reflection Only](targets::None)
//!
//! The API provided is roughly similar to the SPIRV-Cross [`Compiler`](https://github.com/KhronosGroup/SPIRV-Cross/blob/main/spirv_cross.hpp) C++ API,
//! with some inspiration from [naga](https://docs.rs/naga/latest/naga/index.html). A best effort has been
//! made to ensure that these bindings are sound, and that mutations occur strictly within Rust's
//! borrow rules.
//!
//! ## Context
//! The entry point to the library is [`SpirvCrossContext`], which owns all foreign allocations.
//! Hence, structs wrapping SPIRV-Cross objects have a lifetime parameter that refers to the
//! lifetime of the context.
//!
//! [`Compiler`] instances can share a context, in which case the context must outlive all associated
//! objects, or it can take ownership of a context and have a `'static` lifetime, in which case the
//! context becomes internally ref-counted and will be dropped when the last child resource is dropped.
//!
//! ## Strings
//! Methods on [`Compiler`] return and accept [`ContextStr`] instead of a normal string type. A
//! [`ContextStr`] may or may not be owned by the context, or may come from Rust. Rust string types
//! can be coerced automatically to [`ContextStr`] as an input, and [`ContextStr`] can easily be copied
//! to a Rust string type.
//!
//! If a returned [`ContextStr`] is owned by the context and is immutable,
//! it will share the lifetime of the context. Some functions return _short lived_ strings which
//! are owned by the compiler instance, rather than the context.
//!
//! The underlying string data could possibly be modified by `set_` functions,
//! thus they only have a lifetime corresponding to the lifetime of the immutable borrow of the [`Compiler`]
//! that produced them. References to these short-lived strings can not be alive before calling a
//! mutating function.
//!
//! Strings will automatically allocate as needed when passed to FFI. Rust [`String`] and [`&str`](str)
//! may allocate to create a nul-terminated string. Strings coming from FFI will not reallocate,
//! and the pointer will be passed directly back. Rust [`&CStr`](std::ffi::CStr) will not reallocate.
//!
//! If you are just passing in a string constant using a [C-string literal](https://doc.rust-lang.org/edition-guide/rust-2021/c-string-literals.html)
//! will be the most efficient. Otherwise it is always better to work with Rust [`String`] and [`&str`](str),
//! if you are dynamically building up a string. In particular, [`String`] will not reallocate if
//! there is enough capacity to append a nul byte before being passed to FFI.
//!
//! ## Handles
//! All reflected SPIR-V IDs are returned as [`Handle<T>`](handle::Handle), where the `u32` ID part can
//! be retrieved with [`Handle::id`](handle::Handle::id). Handles are tagged with the pointer of the
//! compiler instance they came from, and are required to ensure safety such that reflection queries
//! aren't made between different SPIR-V modules.
//!
//! Any function that takes or returns SPIR-V handles in the SPIRV-Cross API has been wrapped to accept
//! [`Handle<T>`](handle::Handle) in this crate.
//!
//! Handles can be unsafely forged with [`Compiler::create_handle`], but there are very few if any
//! situations where this would be needed.
//!
//! ## Usage
//! Here is an example of using the API to do some reflection and compile to GLSL.
//!
//! Note the `'static` lifetime of the artifact, as the context is owned by the compiler.
//!
//! ```
//! use spirv_cross2::compile::{CompilableTarget, CompiledArtifact};
//! use spirv_cross2::{Module, SpirvCrossContext, SpirvCrossError};
//! use spirv_cross2::compile::glsl::GlslVersion;
//! use spirv_cross2::reflect::{DecorationValue, ResourceType};
//! use spirv_cross2::spirv;
//! use spirv_cross2::targets::Glsl;
//! use spirv_cross2::sync::UnsendContext;
//!
//! fn compile_spirv(words: &[u32]) -> Result<CompiledArtifact<'static, Glsl>, SpirvCrossError> {
//!     let module = Module::from_words(words);
//!     let context = SpirvCrossContext::new()?;
//!
//!     let mut compiler = context.into_compiler::<Glsl, UnsendContext>(module)?;
//!
//!     let resources = compiler.shader_resources()?;
//!
//!     for resource in resources.resources_for_type(ResourceType::SampledImage)? {
//!         let Some(DecorationValue::Literal(set)) =
//!                 compiler.decoration(resource.id, spirv::Decoration::DescriptorSet)? else {
//!             continue;
//!         };
//!         let Some(DecorationValue::Literal(binding)) =
//!             compiler.decoration(resource.id, spirv::Decoration::Binding)? else {
//!             continue;
//!         };
//!
//!         println!("Image {} at set = {}, binding = {}", resource.name, set, binding);
//!
//!         // Modify the decoration to prepare it for GLSL.
//!         compiler.set_decoration(resource.id, spirv::Decoration::DescriptorSet,
//!                 DecorationValue::unset())?;
//!
//!         // Some arbitrary remapping if we want.
//!         compiler.set_decoration(resource.id, spirv::Decoration::Binding,
//!             Some(set * 16 + binding))?;
//!     }
//!
//!     let mut options = Glsl::options();
//!     options.version = GlslVersion::Glsl300Es;
//!
//!     compiler.compile(&options)
//! }
//! ```
use spirv_cross_sys as sys;
use spirv_cross_sys::{spvc_compiler_s, spvc_context_s, SpvId};
use std::borrow::Borrow;

use crate::error::ToContextError;

use crate::sealed::{ContextRooted, Sealed};
use crate::targets::Target;
use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::NonNull;
use std::sync::Arc;
use handle::PhantomCompiler;
use sync::ContextRoot;

/// Compilation of SPIR-V to a textual format.
pub mod compile;

/// Handles to SPIR-V IDs from reflection.
pub mod handle;

/// SPIR-V reflection helpers and types.
pub mod reflect;

/// Compiler output targets.
pub mod targets;

/// Error handling traits and support.
pub(crate) mod error;

/// String helpers
pub(crate) mod string;
pub mod sync;

/// SPIR-V types and definitions.
pub mod spirv {
    pub use spirv::BuiltIn;
    pub use spirv::Capability;
    pub use spirv::Decoration;
    pub use spirv::Dim;
    pub use spirv::ExecutionMode;
    pub use spirv::ExecutionModel;
    pub use spirv::FPRoundingMode;
    pub use spirv::ImageFormat;
    pub use spirv::StorageClass;
}

pub(crate) mod sealed {
    use spirv_cross_sys::spvc_context_s;
    use std::ptr::NonNull;

    pub trait Sealed {}

    pub trait ContextRooted {
        fn context(&self) -> NonNull<spvc_context_s>;
    }
}

pub use crate::error::SpirvCrossError;
pub use crate::string::ContextStr;
use crate::sync::WithContext;

/// The SPIRV-Cross context. All memory allocations originating from
/// this context will have the same lifetime as the context.
#[repr(transparent)]
pub struct SpirvCrossContext(NonNull<spvc_context_s>);

/// A SPIR-V Module represented as SPIR-V words.
pub struct Module<'a>(&'a [SpvId]);

impl<'a> Module<'a> {
    /// Create a new `Module` from SPIR-V words.
    pub fn from_words(words: &'a [u32]) -> Self {
        Module(bytemuck::must_cast_slice(words))
    }
}

impl SpirvCrossContext {
    /// Initialize a new SPIRV-Cross context.
    pub fn new() -> error::Result<Self> {
        unsafe {
            let mut context = std::ptr::null_mut();
            let result = sys::spvc_context_create(&mut context);

            if result != sys::spvc_result::SPVC_SUCCESS {
                return Err(SpirvCrossError::OutOfMemory(String::from("Out of memory")));
            }

            let Some(context) = NonNull::new(context) else {
                return Err(SpirvCrossError::OutOfMemory(String::from("Out of memory")));
            };

            Ok(Self(context))
        }
    }

    /// Create a compiler instance from a SPIR-V module.
    pub fn create_compiler<T: Target, Lock: WithContext>(&self, spirv: Module) -> error::Result<Compiler<T, Lock>> {
        // SAFETY:
        //
        // `SpirvCross::create_compiler` is not mut here, because
        // it only mutates the [allocations](https://github.com/KhronosGroup/SPIRV-Cross/blob/main/spirv_cross_c.cpp#L343)
        // field, which is never observable from Rust.
        //
        // While `allocations` can reallocate being a `SmallVector<std::unique_ptr>`,
        // the actual pointer returned is pinned to `spvc_context` for the lifetime of `Self`.
        // Even if `allocations` reallocates, the pointer returned will always be valid
        // for the lifetime of `spvc_context`.
        unsafe {
            let mut ir = std::ptr::null_mut();
            sys::spvc_context_parse_spirv(
                self.0.as_ptr(),
                spirv.0.as_ptr(),
                spirv.0.len(),
                &mut ir,
            )
            .ok(self)?;

            let mut compiler = std::ptr::null_mut();
            sys::spvc_context_create_compiler(
                self.0.as_ptr(),
                T::BACKEND,
                ir,
                spirv_cross_sys::spvc_capture_mode::TakeOwnership,
                &mut compiler,
            )
            .ok(self)?;

            let Some(compiler) = NonNull::new(compiler) else {
                return Err(SpirvCrossError::OutOfMemory(String::from("Out of memory")));
            };

            Ok(Compiler::new_from_raw(
                compiler,
                ContextRoot::Borrowed(self),
            ))
        }
    }

    /// Create a compiler instance from a SPIR-V module.
    ///
    /// The compiler instance created carries with it a refcounted
    /// pointer to the SPIRV-Cross context, and thus has a `'static`
    /// lifetime.
    pub fn create_compiler_refcounted<T: Target, Lock: WithContext>(
        self: &Arc<Self>,
        spirv: Module,
    ) -> error::Result<Compiler<'static, T, Lock>> {
        unsafe {
            let mut ir = std::ptr::null_mut();
            sys::spvc_context_parse_spirv(
                self.0.as_ptr(),
                spirv.0.as_ptr(),
                spirv.0.len(),
                &mut ir,
            )
            .ok_raw(self.0)?;

            let mut compiler = std::ptr::null_mut();
            sys::spvc_context_create_compiler(
                self.0.as_ptr(),
                T::BACKEND,
                ir,
                spirv_cross_sys::spvc_capture_mode::TakeOwnership,
                &mut compiler,
            )
            .ok_raw(self.0)?;

            let Some(compiler) = NonNull::new(compiler) else {
                return Err(SpirvCrossError::OutOfMemory(String::from("Out of memory")));
            };

            Ok(Compiler::new_from_raw(
                compiler,
                ContextRoot::RefCounted(Arc::clone(self)),
            ))
        }
    }

    /// Create a compiler instance from a SPIR-V module.
    ///
    /// This consumes the instance so the resulting compiler instance is static,
    /// and allocations will be dropped with the compiler.
    ///
    /// This allows for instances to be stored without keeping a reference to the
    /// context separately.
    pub fn into_compiler<T: Target, Lock: WithContext>(self, spirv: Module) -> error::Result<Compiler<'static, T, Lock>> {
        unsafe {
            let mut ir = std::ptr::null_mut();
            sys::spvc_context_parse_spirv(
                self.0.as_ptr(),
                spirv.0.as_ptr(),
                spirv.0.len(),
                &mut ir,
            )
            .ok_raw(self.0)?;

            let mut compiler = std::ptr::null_mut();
            sys::spvc_context_create_compiler(
                self.0.as_ptr(),
                T::BACKEND,
                ir,
                spirv_cross_sys::spvc_capture_mode::TakeOwnership,
                &mut compiler,
            )
            .ok_raw(self.0)?;

            let Some(compiler) = NonNull::new(compiler) else {
                return Err(SpirvCrossError::OutOfMemory(String::from("Out of memory")));
            };

            Ok(Compiler::new_from_raw(
                compiler,
                ContextRoot::RefCounted(Arc::new(self)),
            ))
        }
    }
}

impl Drop for SpirvCrossContext {
    fn drop(&mut self) {
        unsafe { sys::spvc_context_destroy(self.0.as_ptr()) }
    }
}

/// Helper trait to detach objects with lifetimes attached to
/// a compiler or context.
pub trait ToStatic: Sealed {
    /// The static type to return.
    type Static<'a>
    where
        'a: 'static;

    /// Clone the object into an instance with `'static` lifetime.
    fn to_static(&self) -> Self::Static<'static>;
}

#[cfg(test)]
mod test {
    use crate::SpirvCrossContext;

    #[test]
    pub fn init_context_test() {
        SpirvCrossContext::new().unwrap();
    }
}

/// An instance of a SPIRV-Cross compiler.
///
/// Depending on the target, different methods will be
/// available.
///
/// Once compiled into a [`CompiledArtifact`](compile::CompiledArtifact),
/// reflection methods will still remain available, but the instance will be frozen,
/// and no more mutation will be available.
pub struct Compiler<'a, Target, Lock> {
    lock: Lock,
    _pd: PhantomData<Target>,
}

impl<Target, Lock: WithContext> Compiler<'_, Target, Lock> {
    /// Create a new compiler instance.
    ///
    /// The pointer to the `spvc_compiler_s` must have the same lifetime as the context root.
    pub(crate) unsafe fn new_from_raw<T>(
        ptr: NonNull<spvc_compiler_s>,
        ctx: ContextRoot,
    ) -> Compiler<Target, Lock> {
        Compiler {
            lock: Lock::new(ctx, ptr),
            _pd: PhantomData,
        }
    }
}

// SAFETY: SpirvCrossContext is not clone.
//
// While allocations are interior mutability,
// they should be safe one thread at a time.
//
// C++ new and delete operators are thread safe,
// which is what this uses to allocate.s
unsafe impl Send for SpirvCrossContext {}
static_assertions::assert_not_impl_any!(SpirvCrossContext: Clone);
