//! Safe userspace memory access helpers.
//!
//! Do NOT directly dereference user pointers in S-mode (even with SUM enabled):
//! if the userspace page is unmapped / PROT_NONE / COW-protected, it can trigger
//! an S-mode page fault and panic the kernel. These helpers translate user VAs
//! via the current address space page table and then copy through the kernel's
//! direct-mapped window.
//!
//! Later stages (demand paging / COW) will hook into these helpers to "fault in"
//! missing pages instead of returning EFAULT.

use core::mem::MaybeUninit;

use uapi::errno::EFAULT;

use crate::arch::constant::{USER_BASE, USER_TOP};
use crate::arch::mm::paddr_to_vaddr;
use crate::config::PAGE_SIZE;
use crate::kernel::current_memory_space;
use mm::page_table::PageTableInner as _;
use mm::address::{PageNum as _, UsizeConvert as _};

#[inline]
fn range_ok(user_addr: usize, len: usize) -> bool {
    if len == 0 {
        return true;
    }
    if user_addr < USER_BASE || user_addr > USER_TOP {
        return false;
    }
    let end = match user_addr.checked_add(len - 1) {
        Some(v) => v,
        None => return false,
    };
    end <= USER_TOP
}

pub fn copy_from_user(dst: &mut [u8], src_user: usize) -> Result<(), isize> {
    if dst.is_empty() {
        return Ok(());
    }
    if !range_ok(src_user, dst.len()) {
        return Err(-(EFAULT as isize));
    }

    let ms = current_memory_space();
    let mut space = ms.lock();
    let mut read = 0usize;
    while read < dst.len() {
        let cur_va = src_user.checked_add(read).ok_or(-(EFAULT as isize))?;

        let paddr = translate_or_fault_in(&mut space, cur_va, crate::mm::UserAccessType::Read)?;
        let paddr_usize = paddr.as_usize();
        let page_base = paddr_usize & !(PAGE_SIZE - 1);
        let page_off = paddr_usize & (PAGE_SIZE - 1);

        let take = core::cmp::min(dst.len() - read, PAGE_SIZE - page_off);
        let src = (paddr_to_vaddr(page_base) + page_off) as *const u8;
        unsafe {
            core::ptr::copy_nonoverlapping(src, dst[read..].as_mut_ptr(), take);
        }
        read += take;
    }

    Ok(())
}

pub fn copy_to_user(dst_user: usize, src: &[u8]) -> Result<(), isize> {
    if src.is_empty() {
        return Ok(());
    }
    if !range_ok(dst_user, src.len()) {
        return Err(-(EFAULT as isize));
    }

    let ms = current_memory_space();
    let mut space = ms.lock();
    let mut written = 0usize;
    while written < src.len() {
        let cur_va = dst_user.checked_add(written).ok_or(-(EFAULT as isize))?;

        let paddr = translate_or_fault_in(&mut space, cur_va, crate::mm::UserAccessType::Write)?;
        let paddr_usize = paddr.as_usize();
        let page_base = paddr_usize & !(PAGE_SIZE - 1);
        let page_off = paddr_usize & (PAGE_SIZE - 1);

        let take = core::cmp::min(src.len() - written, PAGE_SIZE - page_off);
        let dst = (paddr_to_vaddr(page_base) + page_off) as *mut u8;
        unsafe {
            core::ptr::copy_nonoverlapping(src[written..].as_ptr(), dst, take);
        }
        written += take;
    }

    Ok(())
}

pub fn read_user<T: Copy>(user_ptr: *const T) -> Result<T, isize> {
    if user_ptr.is_null() {
        return Err(-(EFAULT as isize));
    }
    let len = core::mem::size_of::<T>();
    let addr = user_ptr as usize;
    if !range_ok(addr, len) {
        return Err(-(EFAULT as isize));
    }

    let mut out = MaybeUninit::<T>::uninit();
    // SAFETY: We are writing into a stack local value, not dereferencing userspace.
    let buf = unsafe { core::slice::from_raw_parts_mut(out.as_mut_ptr() as *mut u8, len) };
    copy_from_user(buf, addr)?;
    Ok(unsafe { out.assume_init() })
}

pub fn write_user<T>(user_ptr: *mut T, value: T) -> Result<(), isize> {
    if user_ptr.is_null() {
        return Err(-(EFAULT as isize));
    }
    let len = core::mem::size_of::<T>();
    let addr = user_ptr as usize;
    if !range_ok(addr, len) {
        return Err(-(EFAULT as isize));
    }

    // SAFETY: We only take bytes from a stack local value.
    let src = unsafe { core::slice::from_raw_parts((&value as *const T) as *const u8, len) };
    copy_to_user(addr, src)
}

fn translate_or_fault_in(
    space: &mut crate::mm::MemorySpace,
    vaddr: usize,
    access: crate::mm::UserAccessType,
) -> Result<mm::address::Paddr, isize> {
    use mm::address::{Vaddr, Vpn};

    let va = Vaddr::from_usize(vaddr);
    let vpn = Vpn::from_addr_floor(va);

    // If the page is present but write-protected (COW), translate() would still succeed.
    // Use walk() to check PTE flags and resolve COW on-demand.
    if let Ok((_ppn, _size, flags)) = space.page_table().walk(vpn) {
        if access == crate::mm::UserAccessType::Write
            && !flags.contains(mm::page_table::UniversalPTEFlag::WRITEABLE)
        {
            #[cfg(feature = "cow")]
            {
                space.handle_user_page_fault(vaddr, access).map_err(|_| -(EFAULT as isize))?;
            }
            #[cfg(not(feature = "cow"))]
            {
                return Err(-(EFAULT as isize));
            }
        }
        if let Some(paddr) = space.page_table().translate(va) {
            return Ok(paddr);
        }
    }

    #[cfg(any(feature = "demand_paging", feature = "cow"))]
    {
        space.handle_user_page_fault(vaddr, access).map_err(|_| -(EFAULT as isize))?;
        if let Some(paddr) = space.page_table().translate(va) {
            return Ok(paddr);
        }
    }

    Err(-(EFAULT as isize))
}
