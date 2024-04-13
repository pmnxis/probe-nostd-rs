use crate::Core;
use crate::CoreType;

pub(crate) const ARM_V6M: [Core<'static>; 1] = [Core::const_default(CoreType::Armv6m)];
pub(crate) const ARM_V7A: [Core<'static>; 1] = [Core::const_default(CoreType::Armv7a)];
pub(crate) const ARM_V7M: [Core<'static>; 1] = [Core::const_default(CoreType::Armv7m)];
pub(crate) const ARM_V7EM: [Core<'static>; 1] = [Core::const_default(CoreType::Armv7em)];
pub(crate) const ARM_V8A: [Core<'static>; 1] = [Core::const_default(CoreType::Armv8a)];
pub(crate) const ARM_V8M: [Core<'static>; 1] = [Core::const_default(CoreType::Armv8m)];
pub(crate) const RISCV: [Core<'static>; 1] = [Core::const_default(CoreType::Riscv)];
pub(crate) const XTENSA: [Core<'static>; 1] = [Core::const_default(CoreType::Xtensa)];
