use itertools::Itertools;
use shared::windows::{process, system, thread};
use std::{collections::BTreeMap, ffi::c_void, io};
use thiserror::Error;
use tracing::{instrument, trace};

pub(crate) struct SavedState {
    thread_contexts: BTreeMap<u32, thread::Context>,
    memory_allocations: Vec<MemoryAllocation>,
    memory: BTreeMap<*mut c_void, Vec<u8>>,
}

impl SavedState {
    #[instrument(name = "save_state")]
    pub(crate) fn new(process: &process::Process) -> Result<Self, NewError> {
        for thread_id in process.iter_thread_ids()? {
            thread::Thread::from_id(thread_id)?.increment_suspend_count()?;
        }

        let mut thread_contexts = BTreeMap::new();
        for thread_id in process.iter_thread_ids()? {
            let thread = thread::Thread::from_id(thread_id)?;
            thread_contexts.insert(thread_id, thread.get_context()?);
        }

        let memory_allocations = Self::get_all_memory_allocations(process)?;

        let mut memory = BTreeMap::new();
        for memory_region in memory_allocations.iter().flat_map(|a| &a.regions) {
            if memory_region.is_committed() && !memory_region.permissions().is_guard {
                memory.insert(
                    memory_region.address(),
                    process.read_to_vec(memory_region.address().cast(), memory_region.size())?,
                );
            }
        }

        for thread_id in process.iter_thread_ids()? {
            thread::Thread::from_id(thread_id)?.decrement_suspend_count()?;
        }

        Ok(Self {
            thread_contexts,
            memory_allocations,
            memory,
        })
    }

    #[instrument(name = "load_state", skip(self))]
    pub(crate) fn load(&self, process: &process::Process) -> Result<(), LoadError> {
        for thread_id in process.iter_thread_ids()? {
            thread::Thread::from_id(thread_id)?.increment_suspend_count()?;
        }

        for (&thread_id, thread_context) in &self.thread_contexts {
            thread::Thread::from_id(thread_id)?.set_context(thread_context)?;
        }

        // apply saved memory allocations
        let mut current_memory_allocations = Self::get_all_memory_allocations(process)?
            .into_iter()
            .peekable();
        let mut saved_memory_allocations = self.memory_allocations.iter().peekable();
        loop {
            let current_memory_allocation = current_memory_allocations.peek();
            let saved_memory_allocation = saved_memory_allocations.peek();

            let allocate_memory = |allocation: &MemoryAllocation| -> Result<(), LoadError> {
                trace!(
                    "allocating memory at {:p} with size {:#x}",
                    allocation.address,
                    allocation.size,
                );
                process.allocate_memory_at(
                    allocation.address,
                    allocation.size,
                    process::MemoryPermissions {
                        rwe: process::MemoryPermissionsRwe::None,
                        is_guard: false,
                    },
                )?;
                Ok(())
            };
            let free_memory = |allocation: &MemoryAllocation| -> Result<(), LoadError> {
                trace!("freeing memory at {:p}", allocation.address);
                process.free_memory(allocation.address)?;
                Ok(())
            };

            match (current_memory_allocation, saved_memory_allocation) {
                (Some(current_memory_allocation), Some(saved_memory_allocation))
                    if current_memory_allocation.address == saved_memory_allocation.address
                        && current_memory_allocation.size == saved_memory_allocation.size =>
                {
                    // memory allocations match
                    current_memory_allocations.next();
                    saved_memory_allocations.next();
                }
                (Some(current_memory_allocation), Some(saved_memory_allocation))
                    if current_memory_allocation.address
                        < unsafe {
                            saved_memory_allocation
                                .address
                                .byte_add(saved_memory_allocation.size)
                        } =>
                {
                    // current memory allocation overlaps or precedes saved memory allocation
                    free_memory(current_memory_allocation)?;
                    current_memory_allocations.next();
                }
                (Some(current_memory_allocation), None) => {
                    // current memory allocation follows all saved memory allocations
                    free_memory(current_memory_allocation)?;
                    current_memory_allocations.next();
                }
                (_, Some(saved_memory_allocation)) => {
                    // saved memory allocation isn't a current memory allocation but it can be
                    // created
                    allocate_memory(saved_memory_allocation)?;
                    saved_memory_allocations.next();
                }
                (None, None) => break,
            }
        }

        // apply saved region permissions
        for saved_memory_region in self.memory_allocations.iter().flat_map(|a| &a.regions) {
            let process::MemoryRegion::Reserved(current_memory_region) =
                process.get_memory_region(saved_memory_region.address())?
            else {
                continue;
            };
            if current_memory_region.address() != saved_memory_region.address()
                || current_memory_region.size() != saved_memory_region.size()
                || current_memory_region.permissions() != saved_memory_region.permissions()
            {
                // slightly inefficient but simple approach: if there is any mismatch, set
                // permissions
                process.set_memory_permissions(
                    saved_memory_region.address(),
                    saved_memory_region.size(),
                    saved_memory_region.permissions(),
                )?;
            }
        }

        // write memory
        for (&address, bytes) in &self.memory {
            let set_permissions_result = process.set_memory_permissions(
                address,
                bytes.len(),
                process::MemoryPermissions {
                    rwe: process::MemoryPermissionsRwe::ReadWrite,
                    is_guard: false,
                },
            );
            if let Ok(original_permissions) = set_permissions_result {
                trace!("writing to {address:p}");
                process.write(address.cast(), bytes)?;
                process.set_memory_permissions(address, bytes.len(), original_permissions)?;
            } else {
                trace!("skipping write to {address:p}");
            }
        }

        for thread_id in process.iter_thread_ids()? {
            thread::Thread::from_id(thread_id)?.decrement_suspend_count()?;
        }

        Ok(())
    }

    fn get_all_memory_allocations(
        process: &process::Process,
    ) -> Result<Vec<MemoryAllocation>, process::GetMemoryRegionError> {
        Ok(Self::get_all_reserved_memory_regions(process)?
            .into_iter()
            .chunk_by(process::ReservedMemoryRegion::allocation_address)
            .into_iter()
            .map(|(address, regions)| {
                let regions = regions.collect::<Vec<_>>();
                MemoryAllocation {
                    address,
                    size: regions
                        .iter()
                        .map(process::ReservedMemoryRegion::size)
                        .sum(),
                    regions,
                }
            })
            .collect::<Vec<_>>())
    }

    fn get_all_reserved_memory_regions(
        process: &process::Process,
    ) -> Result<Vec<process::ReservedMemoryRegion>, process::GetMemoryRegionError> {
        let addressable_range = {
            let system_info = system::get_info();
            (system_info.lpMinimumApplicationAddress.cast())
                ..(system_info.lpMaximumApplicationAddress.cast())
        };
        let mut address = addressable_range.start;
        let mut regions = vec![];
        while address < addressable_range.end {
            let region = process.get_memory_region(address)?;

            let new_address = region.address().wrapping_byte_add(region.size());
            if new_address < address {
                break; // overflow
            }
            address = new_address;

            if let process::MemoryRegion::Reserved(region) = region {
                regions.push(region);
            }
        }
        Ok(regions)
    }
}

struct MemoryAllocation {
    address: *mut c_void,
    size: usize,
    regions: Vec<process::ReservedMemoryRegion>,
}

#[derive(Debug, Error)]
#[error("failed to create saved state")]
pub enum NewError {
    ProcessIterThreadIds(#[from] process::IterThreadIdsError),
    ThreadFromId(#[from] thread::FromIdError),
    ThreadGetContext(#[from] thread::GetContextError),
    ThreadChangeSuspendCount(#[from] thread::ChangeSuspendCountError),
    ProcessGetMemoryInfo(#[from] process::GetMemoryRegionError),
    ProcessReadMemory(#[from] process::ReadMemoryError),
    Os(#[from] io::Error),
}

#[derive(Debug, Error)]
#[error("failed to load saved state")]
pub enum LoadError {
    ThreadSetContext(#[from] thread::SetContextError),
    ThreadFromId(#[from] thread::FromIdError),
    ThreadChangeSuspendCount(#[from] thread::ChangeSuspendCountError),
    ProcessIterThreadIds(#[from] process::IterThreadIdsError),
    ProcessGetMemoryInfo(#[from] process::GetMemoryRegionError),
    ProcessWriteMemory(#[from] process::WriteMemoryError),
    ProcessAllocateMemory(#[from] process::AllocateMemoryError),
    ProcessFreeMemory(#[from] process::FreeMemoryError),
    ProcessSetMemoryPermissions(#[from] process::SetMemoryPermissionsError),
    Os(#[from] io::Error),
}
