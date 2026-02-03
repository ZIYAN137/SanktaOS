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

use mm::page_table::PagingError;
use uapi::errno::EFAULT;

use crate::arch::constant::{USER_BASE, USER_TOP};
use crate::kernel::current_memory_space;

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

#[inline]
fn map_paging_err(_err: PagingError) -> isize {
    -(EFAULT as isize)
}

pub fn copy_from_user(dst: &mut [u8], src_user: usize) -> Result<(), isize> {
    if dst.is_empty() {
        return Ok(());
    }
    if !range_ok(src_user, dst.len()) {
        return Err(-(EFAULT as isize));
    }
    current_memory_space()
        .lock()
        .read_bytes_at(src_user, dst)
        .map_err(map_paging_err)
}

pub fn copy_to_user(dst_user: usize, src: &[u8]) -> Result<(), isize> {
    if src.is_empty() {
        return Ok(());
    }
    if !range_ok(dst_user, src.len()) {
        return Err(-(EFAULT as isize));
    }
    current_memory_space()
        .lock()
        .write_bytes_at(dst_user, src)
        .map_err(map_paging_err)
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
    current_memory_space()
        .lock()
        .read_bytes_at(addr, buf)
        .map_err(map_paging_err)?;
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
    current_memory_space()
        .lock()
        .write_bytes_at(addr, src)
        .map_err(map_paging_err)
}
