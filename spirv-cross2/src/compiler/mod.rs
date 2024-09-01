use crate::error::{ContextRooted, Result, ToContextError};
use crate::handle::Handle;
use crate::targets::CompilableTarget;
use crate::{spirv, ContextRoot};
use spirv_cross_sys as sys;
use spirv_cross_sys::{spvc_compiler_s, spvc_context_s, VariableId};
use std::marker::PhantomData;
use std::ptr::NonNull;

pub mod buffers;
pub mod combined_image_samplers;
pub mod constants;
pub mod decorations;
pub mod entry_points;
pub mod execution_modes;
pub mod hlsl;
pub mod msl;
pub mod names;
pub mod resources;
pub mod types;

pub struct Compiler<'a, T> {
    pub(crate) ptr: NonNull<spvc_compiler_s>,
    ctx: ContextRoot<'a>,
    _pd: PhantomData<T>,
}

impl<T> Compiler<'_, T> {
    /// Create a new compiler instance.
    ///
    /// The pointer to the `spvc_compiler_s` must have the same lifetime as the context root.
    pub(super) unsafe fn new_from_raw(
        ptr: NonNull<spvc_compiler_s>,
        ctx: ContextRoot,
    ) -> Compiler<T> {
        Compiler {
            ptr,
            ctx,
            _pd: PhantomData,
        }
    }
}

impl<T> ContextRooted for &Compiler<'_, T> {
    #[inline(always)]
    fn context(&self) -> NonNull<spvc_context_s> {
        self.ctx.ptr()
    }
}

impl<T> ContextRooted for &mut Compiler<'_, T> {
    #[inline(always)]
    fn context(&self) -> NonNull<spvc_context_s> {
        self.ctx.ptr()
    }
}

/// Holds on to the pointer for a compiler instance,
/// but type erased.
///
/// This is used so that child resources of a compiler track the
/// lifetime of a compiler, or create handles attached with the
/// compiler instance, without needing to refer to the typed
/// output of a compiler.
///
/// The only thing a [`PhantomCompiler`] is able to do is create handles or
/// refer to the root context. It's lifetime should be the same as the lifetime
/// of the compiler.
#[derive(Copy, Clone)]
pub(crate) struct PhantomCompiler<'a> {
    pub(crate) ptr: NonNull<spvc_compiler_s>,
    ctx: NonNull<spvc_context_s>,
    _pd: PhantomData<&'a ()>,
}

impl ContextRooted for PhantomCompiler<'_> {
    fn context(&self) -> NonNull<spvc_context_s> {
        self.ctx
    }
}

impl<'a, T> Compiler<'a, T> {
    /// Create a type erased phantom for lifetime tracking purposes.
    ///
    /// This function is unsafe because a [`PhantomCompiler`] can be used to
    /// **safely** create handles originating from the compiler.
    pub(crate) unsafe fn phantom(&self) -> PhantomCompiler<'a> {
        PhantomCompiler {
            ptr: self.ptr,
            ctx: self.context(),
            _pd: PhantomData,
        }
    }
}

/// Cross-compilation related methods.
impl<T: CompilableTarget> Compiler<'_, T> {
    pub fn add_header_line(&mut self, line: &str) -> Result<()> {
        unsafe {
            sys::spvc_compiler_add_header_line(self.ptr.as_ptr(), line.as_ptr().cast()).ok(self)
        }
    }

    pub fn flatten_buffer_block(&mut self, block: VariableId) -> Result<()> {
        unsafe { sys::spvc_compiler_flatten_buffer_block(self.ptr.as_ptr(), block).ok(self) }
    }

    pub fn require_extension(&mut self, ext: &str) -> Result<()> {
        unsafe {
            sys::spvc_compiler_require_extension(self.ptr.as_ptr(), ext.as_ptr().cast()).ok(self)
        }
    }

    pub fn mask_stage_output_by_location(&mut self, location: u32, component: u32) -> Result<()> {
        unsafe {
            sys::spvc_compiler_mask_stage_output_by_location(self.ptr.as_ptr(), location, component)
                .ok(&*self)
        }
    }

    pub fn mask_stage_output_by_builtin(&mut self, builtin: spirv::BuiltIn) -> Result<()> {
        unsafe {
            sys::spvc_compiler_mask_stage_output_by_builtin(self.ptr.as_ptr(), builtin).ok(&*self)
        }
    }

    pub fn variable_is_depth_or_compare(&self, variable: Handle<VariableId>) -> Result<bool> {
        let id = self.yield_id(variable)?;
        unsafe {
            Ok(sys::spvc_compiler_variable_is_depth_or_compare(
                self.ptr.as_ptr(),
                id,
            ))
        }
    }
}

#[cfg(test)]
mod test {
    use crate::compiler::Compiler;
    use crate::error::SpirvCrossError;
    use crate::targets;
    use crate::{Module, SpirvCross};

    const BASIC_SPV: &[u8] = include_bytes!("../../basic.spv");

    #[test]
    pub fn create_compiler() -> Result<(), SpirvCrossError> {
        let mut spv = SpirvCross::new()?;
        let words = Module::from_words(bytemuck::cast_slice(BASIC_SPV));

        let compiler: Compiler<targets::None> = spv.create_compiler(words)?;
        Ok(())
    }

    #[test]
    pub fn reflect_interface_vars() -> Result<(), SpirvCrossError> {
        let mut spv = SpirvCross::new()?;
        let words = Module::from_words(bytemuck::cast_slice(BASIC_SPV));

        let mut compiler: Compiler<targets::None> = spv.create_compiler(words)?;
        let vars = compiler.active_interface_variables()?;
        assert_eq!(
            &[13, 9],
            &vars
                .to_handles()
                .into_iter()
                .map(|h| h.id())
                .collect::<Vec<_>>()
                .as_slice()
        );

        compiler.set_enabled_interface_variables(vars)?;
        Ok(())
    }
}
