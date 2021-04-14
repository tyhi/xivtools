use log;
use std::ffi::{c_void, CStr};
use std::fmt;
use std::mem;
use thiserror::Error;

use bindings::Windows::Win32::Debug::{GetLastError, ReadProcessMemory};
use bindings::Windows::Win32::ProcessStatus::{
    K32EnumProcessModulesEx, K32EnumProcesses, K32GetModuleBaseNameA, K32GetModuleInformation,
    MODULEINFO,
};
use bindings::Windows::Win32::SystemServices::{
    OpenProcess, FALSE, HANDLE, PROCESS_ACCESS_RIGHTS, PSTR,
};

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("Couldn't enumerate processes: {0}")]
    ProcessEnumeration(u32),
    #[error("Couldn't enumerate modules for handle {0:x}: {1}")]
    ModuleEnumeration(u32, u32),
    #[error("Failed to get module name for handle {0:x}: {1}")]
    ModuleName(u32, u32),
    #[error("Failed to get module information for '{0}': {1}")]
    ModuleInformation(String, u32),
    #[error("Process '{0}' not found")]
    NotFound(String),
}

#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Couldn't read memory at {0:x}: {1} (read: {2})")]
    Read(u64, u32, usize),
    #[error("Incorrect read size (expected: {0}, actual: {1})")]
    IncorrectSize(usize, usize),
    #[error("Unable to find signature")]
    NotFound,
}

// TODO: Consider making 'modules' a ref-counted type for shallow copies.
#[derive(Clone, Default, Debug)]
pub struct ProcessModule {
    pub name: String,
    pub base: u64,
    pub size: usize,
}

#[derive(Clone, Debug)]
pub struct Process {
    pub name: String,
    pub handle: HANDLE,
    pub modules: Vec<ProcessModule>,
}

#[derive(Copy, Debug, Clone, PartialEq)]
pub enum SignatureType {
    // The address is the address of signature location plus the provided offset.
    Absolute { offset: i64 },
    // The address is the address of the signature location plus a u32 value read at offset
    Relative32 { offset: i64 },
}

impl Default for SignatureType {
    fn default() -> Self {
        Self::Absolute { offset: 0 }
    }
}
#[derive(Default, Debug)]
pub struct Signature<'a> {
    pub bytes: &'a [&'a str],
    pub sigtype: SignatureType,
}

impl Process {
    pub fn new(exe_name: &str) -> Result<Self, ProcessError> {
        let mut processes = [0; 1024];
        let mut needed = 0;

        unsafe {
            if K32EnumProcesses(processes.as_mut_ptr(), processes.len() as u32, &mut needed)
                == FALSE
            {
                return Err(ProcessError::ProcessEnumeration(GetLastError().0));
            }

            for &process in processes
                .iter()
                .take(needed as usize / mem::size_of::<u32>())
            {
                let handle = OpenProcess(
                    PROCESS_ACCESS_RIGHTS::PROCESS_VM_READ
                        | PROCESS_ACCESS_RIGHTS::PROCESS_QUERY_INFORMATION,
                    FALSE,
                    process,
                );

                if handle.is_null() {
                    continue;
                }

                let mut name_buf = [0; 260];
                if K32GetModuleBaseNameA(
                    handle,
                    0,
                    PSTR(name_buf.as_mut_ptr()),
                    name_buf.len() as u32,
                ) == 0
                {
                    continue;
                }

                let name_str = CStr::from_ptr(name_buf.as_ptr().cast::<i8>())
                    .to_string_lossy()
                    .to_string();
                if name_str == exe_name {
                    let modules = Self::get_process_modules(handle)?;
                    return Ok(Self {
                        name: name_str,
                        handle,
                        modules,
                    });
                }
            }
        }

        Err(ProcessError::NotFound(exe_name.to_string()))
    }

    fn get_process_modules(hnd: HANDLE) -> Result<Vec<ProcessModule>, ProcessError> {
        let mut result: Vec<ProcessModule> = vec![];
        unsafe {
            let mut modules: Vec<isize> = Vec::with_capacity(1024);
            let mut needed = 0;
            if K32EnumProcessModulesEx(
                hnd,
                modules.as_mut_ptr(),
                (mem::size_of::<isize>() * modules.len()) as u32,
                &mut needed,
                2_u32,
            ) == FALSE
            {
                return Err(ProcessError::ModuleEnumeration(
                    hnd.0 as u32,
                    GetLastError().0,
                ));
            }

            let mut buf = [0; 260];
            for &module in modules
                .iter()
                .take(needed as usize / mem::size_of::<isize>())
            {
                if K32EnumProcessModulesEx(
                    hnd,
                    module as *mut isize,
                    *buf.as_mut_ptr().cast::<u32>(),
                    buf.len() as *mut u32,
                    2_u32,
                ) == FALSE
                {
                    return Err(ProcessError::ModuleName(hnd.0 as u32, GetLastError().0));
                }

                let name = CStr::from_ptr(buf.as_ptr().cast())
                    .to_string_lossy()
                    .clone()
                    .to_string();

                let mut module_info = MODULEINFO::default();
                if K32GetModuleInformation(
                    hnd,
                    module,
                    &mut module_info,
                    mem::size_of::<MODULEINFO>() as u32,
                ) == FALSE
                {
                    return Err(ProcessError::ModuleInformation(name, GetLastError().0));
                }

                result.push(ProcessModule {
                    name: name.clone(),
                    base: module_info.lpBaseOfDll as u64,
                    size: module_info.SizeOfImage as usize,
                });
            }
        }
        Ok(result)
    }

    pub fn read(
        &self,
        addr: u64,
        buf: *mut u8,
        sz: usize,
        read: &mut usize,
    ) -> Result<(), MemoryError> {
        unsafe {
            if ReadProcessMemory(
                self.handle,
                addr as *mut c_void,
                buf.cast::<std::ffi::c_void>(),
                sz,
                read,
            ) == FALSE
            {
                return Err(MemoryError::Read(addr as u64, GetLastError().0, *read));
            }
        }
        Ok(())
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct UnknownField<const N: usize> {
    data: [u8; N],
}

impl<const N: usize> fmt::Debug for UnknownField<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.data.iter() {
            write!(f, " {:02x}", byte)?;
        }

        Ok(())
    }
}

impl<const N: usize> PartialEq for UnknownField<N> {
    fn eq(&self, other: &Self) -> bool {
        for pair in self.data.iter().zip(other.data.iter()) {
            if pair.0 != pair.1 {
                return false;
            }
        }

        true
    }
}

impl<const N: usize> Default for UnknownField<N> {
    fn default() -> Self {
        Self { data: [0; N] }
    }
}

impl<const N: usize> Eq for UnknownField<N> {}

use std::ops::Deref;

impl<T> Deref for RemoteStruct<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.t
    }
}

#[repr(C, packed)]
pub struct RemoteStruct<T> {
    t: T,
    module: usize, // if we need to use other modules someday
    address: u64,
    process: Process,
}

impl<T: std::default::Default> RemoteStruct<T> {
    #[must_use]
    pub fn new(process: Process, address: u64) -> Self {
        log::debug!(
            "Creating new RemoteStruct @ {:#x}",
            address + process.modules[0].base
        );
        Self {
            t: T::default(),
            address,
            module: 0,
            process,
        }
    }

    pub fn read(&mut self) -> Result<(), MemoryError> {
        let t_size = mem::size_of::<T>();
        unsafe {
            let mut read = 0;
            let read_addr = self.process.modules[self.module].base + self.address;
            match ReadProcessMemory(
                self.process.handle,
                read_addr as *mut c_void,
                &mut self.t as *mut _ as *mut c_void,
                t_size,
                &mut read,
            ) {
                FALSE => Err(MemoryError::Read(read_addr, GetLastError().0, read)),
                _ => {
                    if read == t_size {
                        Ok(())
                    } else {
                        Err(MemoryError::IncorrectSize(t_size, read))
                    }
                }
            }
        }
    }
}
