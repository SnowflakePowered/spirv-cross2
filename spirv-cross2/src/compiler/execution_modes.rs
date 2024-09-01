use crate::compiler::Compiler;
use crate::error::ToContextError;
use crate::{error, spirv};
use spirv_cross_sys as sys;
use std::slice;
use spirv_cross_sys::{ConstantId};
use crate::handle::Handle;

#[derive(Debug)]
pub enum ExecutionModeArguments {
    None,
    Unit(u32),
    LocalSize { x: u32, y: u32, z: u32 },
    LocalSizeId { x: Handle<ConstantId>, y: Handle<ConstantId>, z: Handle<ConstantId> },

}

impl ExecutionModeArguments {
    fn expand(self) -> [u32; 3] {
        match self {
            ExecutionModeArguments::None => [0, 0, 0],
            ExecutionModeArguments::Unit(a) => [a, 0, 0],
            ExecutionModeArguments::LocalSize { x, y, z } => [x, y, z],
            ExecutionModeArguments::LocalSizeId { x, y, z} => [
                x.id(), y.id(), z.id()
            ]
        }
    }
}

impl<'a, T> Compiler<'a, T> {
    /// Set or unset execution modes and arguments.
    ///
    /// If arguments is `None`, unsets the execution mode. To set an execution mode that does not
    /// take arguments, pass `Some(ExecutionModeArguments::None)`.
    pub fn set_execution_mode(
        &mut self,
        mode: spirv::ExecutionMode,
        arguments: Option<ExecutionModeArguments>,
    ) {
        unsafe {
            let Some(arguments) = arguments else {
                return sys::spvc_compiler_unset_execution_mode(self.ptr.as_ptr(), mode);
            };

            let [x, y, z] = arguments.expand();

            sys::spvc_compiler_set_execution_mode_with_arguments(self.ptr.as_ptr(), mode, x, y, z);
        }
    }

    pub fn execution_modes(&self) -> error::Result<&'a [spirv::ExecutionMode]> {
        unsafe {
            let mut size = 0;
            let mut modes = std::ptr::null();

            sys::spvc_compiler_get_execution_modes(self.ptr.as_ptr(), &mut modes, &mut size)
                .ok(self)?;

            Ok(slice::from_raw_parts(modes, size))
        }
    }


    /// Get arguments used by the execution mode.
    ///
    /// If the execution mode is unused, returns `None`.
    ///
    /// LocalSizeId query returns an ID. If LocalSizeId execution mode is not used, it returns None.
    /// LocalSize always returns a literal. If execution mode is LocalSizeId, the literal (spec constant or not) is still returned.
    pub fn execution_mode_arguments(&self, mode: spirv::ExecutionMode) -> error::Result<Option<ExecutionModeArguments>> {
        Ok(match mode {
            spirv::ExecutionMode::LocalSize => unsafe {
                let x = sys::spvc_compiler_get_execution_mode_argument_by_index(
                    self.ptr.as_ptr(),
                    mode,
                    0,
                );
                let y = sys::spvc_compiler_get_execution_mode_argument_by_index(
                    self.ptr.as_ptr(),
                    mode,
                    1,
                );
                let z = sys::spvc_compiler_get_execution_mode_argument_by_index(
                    self.ptr.as_ptr(),
                    mode,
                    2,
                );

                if x * y * z == 0 {
                    None
                } else {
                    Some(ExecutionModeArguments::LocalSize { x, y, z })
                }
            },
            spirv::ExecutionMode::LocalSizeId => unsafe {
                let x = sys::spvc_compiler_get_execution_mode_argument_by_index(
                    self.ptr.as_ptr(),
                    mode,
                    0,
                );
                let y = sys::spvc_compiler_get_execution_mode_argument_by_index(
                    self.ptr.as_ptr(),
                    mode,
                    1,
                );
                let z = sys::spvc_compiler_get_execution_mode_argument_by_index(
                    self.ptr.as_ptr(),
                    mode,
                    2,
                );

                if x * y * z == 0 {
                    // If one is zero, then all are zero.
                    None
                } else {
                    Some(ExecutionModeArguments::LocalSizeId {
                        x: self.create_handle(ConstantId::from(x)),
                        y: self.create_handle(ConstantId::from(y)),
                        z: self.create_handle(ConstantId::from(z))
                    })
                }
            }
            spirv::ExecutionMode::Invocations
            | spirv::ExecutionMode::OutputVertices
            | spirv::ExecutionMode::OutputPrimitivesEXT => unsafe {
                if !self.execution_modes()?.contains(&mode) {
                    return Ok(None);
                };

                let x = sys::spvc_compiler_get_execution_mode_argument_by_index(
                    self.ptr.as_ptr(),
                    mode,
                    0,
                );
                Some(ExecutionModeArguments::Unit(x))
            },
            _ => {
                if !self.execution_modes()?.contains(&mode) {
                    return Ok(None);
                };

                Some(ExecutionModeArguments::None)
            },
        })
    }
}

#[cfg(test)]
mod test {
    use crate::compiler::Compiler;
    use crate::error::SpirvCrossError;
    use crate::{spirv, targets, Module, SpirvCross};

    static BASIC_SPV: &[u8] = include_bytes!("../../basic.spv");

    #[test]
    pub fn execution_modes() -> Result<(), SpirvCrossError> {
        let mut spv = SpirvCross::new()?;
        let words = Module::from_words(bytemuck::cast_slice(BASIC_SPV));

        let compiler: Compiler<targets::None> = spv.create_compiler(words)?;
        let resources = compiler.shader_resources()?.all_resources()?;

        // println!("{:#?}", resources);

        let ty = compiler.execution_modes()?;
        assert_eq!([spirv::ExecutionMode::OriginUpperLeft], ty);

        let args = compiler.work_group_size_specialization_constants();
        eprintln!("{:?}", args);

        // match ty.inner {
        //     TypeInner::Struct(ty) => {
        //         compiler.get_type(ty.members[0].id)?;
        //     }
        //     TypeInner::Vector { .. } => {}
        //     _ => {}
        // }
        Ok(())
    }
}
